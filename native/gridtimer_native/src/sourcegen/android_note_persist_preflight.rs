// v2.23.2.54 - Report native preflight separately from schema and mirror checks.
// v2.23.2.35 - Reuse verified history within the current local-save transaction.
const REPOSITORY: &str = "com/ofairyo/gridtimer/data/TimerRepository.kt";
const DATABASE: &str = "com/ofairyo/gridtimer/data/AppStateDatabase.kt";

fn replace(source: &mut String, old: &str, new: &str) -> Result<(), String> {
    if source.matches(old).count() != 1 {
        return Err(format!(
            "note persist preflight anchor is not unique: {old}"
        ));
    }
    *source = source.replacen(old, new, 1);
    Ok(())
}

pub fn render(path: &str, base: &str) -> Result<String, String> {
    let mut source = base.to_owned();
    match path {
        REPOSITORY => {
            let start = source
                .find("    private fun futureSchemaVersionOnAnySurface(")
                .ok_or("missing note save schema preflight")?;
            let end = start
                + source[start..]
                    .find("    private fun migrateLegacyStateOnce(")
                    .ok_or("missing note save schema preflight end")?;
            source.replace_range(start..end, SCHEMA_PREFLIGHT);
        }
        DATABASE => {
            replace(&mut source, "    private val snapshotReadWindow = ThreadLocal<SnapshotVerificationWindow>()",
                &format!("{DATABASE_PREFLIGHT}\n    private val snapshotReadWindow = ThreadLocal<SnapshotVerificationWindow>()"))?;
            source.push_str(POLICY);
        }
        _ => {}
    }
    Ok(source)
}

const DATABASE_PREFLIGHT: &str = r####"    fun <T> withVerifiedSavePreflight(
        workspaceKey: String,
        onNativeVerification: (Long, Long) -> Unit = { _, _ -> },
        action: () -> T
    ): T =
        withSnapshotWriteTransaction {
            // Reuse the existing native payload/envelope/schema verifier on the
            // already pinned primary connection. Saving never starts history
            // compaction, and no receipt survives this scope or its transaction.
            runNoteSavePreflight(
                workspaceKey = workspaceKey,
                windows = snapshotReadWindow,
                transactionActive = { writableDatabase.inTransaction() },
                snapshotStamp = ::snapshotWriteStamp,
                verifyRows = {
                    val began = System.nanoTime()
                    var verifiedRows = -1L
                    try {
                        verifyNativeSnapshotRows(workspaceKey).also { verifiedRows = it }
                    } finally {
                        val elapsedMs = (System.nanoTime() - began).coerceAtLeast(0L) / 1_000_000L
                        if (elapsedMs >= 1_000L) onNativeVerification(elapsedMs, verifiedRows)
                    }
                },
                onVerificationFailure = {
                    android.util.Log.w("NoteSaveLatency", "Save preflight requires full recovery verification", it)
                },
                action = action
            )
        }
"####;

const SCHEMA_PREFLIGHT: &str = r####"    private fun futureSchemaVersionOnAnySurface(
        workspaceKey: String,
        includeUnmigratedLegacy: Boolean = false
    ): Int? = stateDatabase.withVerifiedSavePreflight(
        workspaceKey,
        onNativeVerification = { millis, rows ->
            logDiagnosticEvent(
                category = "notes.local_save.latency",
                message = "phase=preflight_native_verification durationMs=$millis rows=$rows"
            )
        }
    ) {
        val evidence = NoteSavePreflightEvidence(APP_DATA_SCHEMA_VERSION)
        // Native success proves payload, envelope and BOTH schema declarations.
        // Read and accept the current row before skipping verified histories.
        // Missing/corrupt/future current and native failures use the old visitor.
        stateDatabase.forEachSnapshot(
            workspaceKey,
            skipVerifiedHistory = { evidence.acceptedCurrentWithoutFuture }
        ) { stored ->
            evidence.observe(stored.source, rawSchemaVersion(stored.appDataJson), stored.schemaVersion)
        }
        // A current/history receipt never authorizes quarantine or JSON mirrors.
        stateDatabase.forEachQuarantinedCurrentSnapshot(workspaceKey) {
            evidence.observe(it.source, rawSchemaVersion(it.appDataJson), it.schemaVersion)
        }
        val files = workspaceFiles(workspaceKey)
        fun observeFile(file: File) {
            if (file.isFile) evidence.observe("file", rawSchemaVersion(file.readText()))
        }
        observeFile(files.state)
        observeFile(files.backup)
        if (workspaceKey == GUEST_WORKSPACE_KEY && includeUnmigratedLegacy) {
            observeFile(legacyStateFile)
            observeFile(legacyBackupFile)
            observeFile(legacyTempFile)
        }
        evidence.futureVersion
    }

"####;

// Production policy is deliberately Android-free so its transaction, failure,
// workspace and schema boundaries run as ordinary JVM specifications.
pub const POLICY: &str = r####"

internal class NoteSavePreflightEvidence(private val supportedVersion: Int) {
    var futureVersion: Int? = null
        private set
    private var acceptedCurrent = false
    val acceptedCurrentWithoutFuture: Boolean get() = acceptedCurrent && futureVersion == null

    fun observe(source: String, rawVersion: Int?, declaredVersion: Int? = null) {
        val effective = maxOf(rawVersion ?: supportedVersion, declaredVersion ?: supportedVersion)
        if (effective > supportedVersion) {
            futureVersion = maxOf(futureVersion ?: effective, effective)
        }
        if (source == "sqlite-current" && effective <= supportedVersion) acceptedCurrent = true
    }
}

internal fun <Stamp : Any, Result> runNoteSavePreflight(
    workspaceKey: String,
    windows: ThreadLocal<SnapshotVerificationWindow>,
    transactionActive: () -> Boolean,
    snapshotStamp: () -> Stamp,
    verifyRows: () -> Long,
    onVerificationFailure: (Throwable) -> Unit = {},
    action: () -> Result
): Result {
    check(transactionActive()) { "Save preflight requires the primary transaction." }
    val previousWindow = windows.get()
    val window = SnapshotVerificationWindow()
    windows.set(window)
    try {
        val before = snapshotStamp()
        val verifiedRows = try {
            verifyRows()
        } catch (failure: Throwable) {
            onVerificationFailure(failure)
            -1L
        }
        val after = snapshotStamp()
        if (verifiedRows >= 0L && before == after) {
            window.recordNativeHistory(workspaceKey, after)
        }
        return action()
    } finally {
        if (previousWindow == null) windows.remove() else windows.set(previousWindow)
    }
}
"####;

pub const TEST_PATH: &str = "com/ofairyo/gridtimer/data/NoteSavePreflightTest.kt";
pub const TEST_CONTENTS: &str = r####"package com.ofairyo.gridtimer.data

import org.junit.Assert.*
import org.junit.Test

class NoteSavePreflightTest {
    private val windows = ThreadLocal<SnapshotVerificationWindow>()

    private fun canSkip(evidence: NoteSavePreflightEvidence, workspace: String = "guest", stamp: Long = 7L): Boolean =
        windows.get()?.canSkipVerifiedHistory(workspace, stamp, evidence.acceptedCurrentWithoutFuture) == true

    @Test fun unchangedNativeProofAcceptsCurrentAndAvoidsHistoryExpansion() {
        var expanded = 0
        runNoteSavePreflight("guest", windows, { true }, { 7L }, { 1001L }) {
            val evidence = NoteSavePreflightEvidence(15)
            evidence.observe("sqlite-current", 15, 15)
            if (!canSkip(evidence)) repeat(1000) { expanded++ }
            assertEquals(0, expanded)
            assertNull(evidence.futureVersion)
        }
        assertNull(windows.get())
    }

    @Test fun failedOrUnavailableNativeVerificationCannotReuseAnOuterReceipt() {
        val outer = SnapshotVerificationWindow().also { it.recordNativeHistory("guest", 7L) }
        windows.set(outer)
        for (failsByThrowing in listOf(false, true)) {
            var fallbackVisited = false
            runNoteSavePreflight("guest", windows, { true }, { 7L }, {
                if (failsByThrowing) error("unreadable native payload") else -1L
            }) {
                val evidence = NoteSavePreflightEvidence(15)
                evidence.observe("sqlite-current", 15, 15)
                if (!canSkip(evidence)) {
                    fallbackVisited = true
                    evidence.observe("sqlite-history:9", 16, 15)
                }
                assertTrue(fallbackVisited)
                assertEquals(16, evidence.futureVersion)
            }
            assertSame(outer, windows.get())
        }
    }

    @Test fun changedDatabaseStampRejectsNativeReceipt() {
        var stamp = 7L
        runNoteSavePreflight("guest", windows, { true }, { stamp }, { stamp = 8L; 1L }) {
            val evidence = NoteSavePreflightEvidence(15)
            evidence.observe("sqlite-current", 15, 15)
            assertFalse(canSkip(evidence, stamp = 8L))
        }
    }

    @Test fun missingCorruptOrFutureCurrentCannotSkipHistory() {
        runNoteSavePreflight("guest", windows, { true }, { 7L }, { 100L }) {
            for (source in listOf("sqlite-history:1", "sqlite-current-unverified", "sqlite-quarantine-current:1")) {
                val evidence = NoteSavePreflightEvidence(15)
                evidence.observe(source, 15, 15)
                assertFalse(canSkip(evidence))
            }
            assertFalse(canSkip(NoteSavePreflightEvidence(15)))
            for ((raw, declared) in listOf(16 to 15, 15 to 16)) {
                val evidence = NoteSavePreflightEvidence(15)
                evidence.observe("sqlite-current", raw, declared)
                assertFalse(canSkip(evidence))
                assertEquals(16, evidence.futureVersion)
            }
        }
    }

    @Test fun proofCannotCrossWorkspaceOrLaterDatabaseMutation() {
        runNoteSavePreflight("guest", windows, { true }, { 7L }, { 2L }) {
            val evidence = NoteSavePreflightEvidence(15)
            evidence.observe("sqlite-current", 15, 15)
            assertFalse(canSkip(evidence, workspace = "account:other"))
            assertFalse(canSkip(evidence, stamp = 8L))
        }
    }

    @Test fun futureQuarantineAndMirrorEvidenceStillBlockSaving() {
        for (source in listOf("sqlite-quarantine-current:1", "file", "legacy-file")) {
            runNoteSavePreflight("guest", windows, { true }, { 7L }, { 2L }) {
                val evidence = NoteSavePreflightEvidence(15)
                evidence.observe("sqlite-current", 15, 15)
                assertTrue(canSkip(evidence))
                evidence.observe(source, 16, 15)
                assertEquals(16, evidence.futureVersion)
                assertFalse(evidence.acceptedCurrentWithoutFuture)
                evidence.observe("file", 15, 15)
                assertEquals(16, evidence.futureVersion)
            }
        }
    }

    @Test fun failedActionReleasesItsReceiptAndPreservesOuterReadScope() {
        val outer = SnapshotVerificationWindow()
        windows.set(outer)
        try {
            runNoteSavePreflight("guest", windows, { true }, { 7L }, { 1L }) {
                throw IllegalStateException("mirror read failed")
            }
            fail("The failed read must propagate")
        } catch (_: IllegalStateException) {
            assertSame(outer, windows.get())
            assertFalse(outer.canSkipVerifiedHistory("guest", 7L, true))
        }
    }

    @Test fun noTransactionCannotVerifyOrPublishAReceipt() {
        var verified = false
        try {
            runNoteSavePreflight("guest", windows, { false }, { 7L }, { verified = true; 1L }) { }
            fail("Unpinned verification must fail")
        } catch (_: IllegalStateException) {
            assertFalse(verified)
            assertNull(windows.get())
        }
    }
}
"####;

#[cfg(test)]
mod generated_preflight_diagnostics_tests {
    use super::*;

    #[test]
    fn slow_native_verification_records_only_bounded_metrics() {
        assert!(DATABASE_PREFLIGHT.contains("withSnapshotWriteTransaction"));
        assert!(DATABASE_PREFLIGHT.contains("verifyNativeSnapshotRows(workspaceKey)"));
        assert!(DATABASE_PREFLIGHT.contains("finally {"));
        assert!(DATABASE_PREFLIGHT.contains("onNativeVerification(elapsedMs, verifiedRows)"));
        assert!(SCHEMA_PREFLIGHT.contains("phase=preflight_native_verification durationMs=$millis rows=$rows"));
        assert!(!SCHEMA_PREFLIGHT.contains("noteId="));
        assert!(!SCHEMA_PREFLIGHT.contains("workspaceKey=$"));
        assert!(SCHEMA_PREFLIGHT.contains("forEachQuarantinedCurrentSnapshot"));
        assert!(SCHEMA_PREFLIGHT.contains("observeFile(files.backup)"));
    }
}
