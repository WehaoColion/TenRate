// v3.0.2.14 - Keep full history recovery out of the local write transaction.
// v3.0.2.6 - Flush save diagnostics after SQLite unlock and resume legacy authentication.
// v3.0.2.5 - Make preflight guards index-seeking and expose outer transaction wait.
// v3.0.2.4 - Use a transaction-pinned authenticated current snapshot for ordinary saves.
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
            replace(&mut source,
                "            is android.database.sqlite.SQLiteFullException -> NoteSaveFailure.STORAGE_FULL",
                "            is LocalSnapshotSchemaException -> NoteSaveFailure.SCHEMA_MISMATCH\n            is LocalSnapshotRecoveryException -> NoteSaveFailure.READ_ONLY_PROTECTION\n            is android.database.sqlite.SQLiteFullException -> NoteSaveFailure.STORAGE_FULL")?;
            // An unchanged, authenticated startup/save stamp can skip preflight
            // entirely. Record this explicitly so the emulator cannot confuse
            // absence of a scan with missing diagnostic data.
            replace(&mut source,
                "                // Preflight and the authoritative write share one primary transaction;",
                REPOSITORY_CACHE_DIAGNOSTIC)?;
            replace(&mut source,
                "val futureSchema = futureSchemaVersionOnAnySurface(workspaceKey)",
                "val futureSchema = futureSchemaVersionOnAnySurface(workspaceKey, diagnostic = ::recordPreflight)")?;
            replace(&mut source,
                "                    check(futureSchema == null) {\n                        \"Refusing to overwrite persisted evidence created by future schema $futureSchema.\"\n                    }",
                "                    if (futureSchema != null) throw LocalSnapshotSchemaException(futureSchema)")?;
            replace(&mut source,
                "                stageFinished(\"snapshot_preflight\")",
                r####"                val preflightMillis = (System.nanoTime() - stageStarted) / 1_000_000L
                if (preflightMillis >= 1_000L) recordPreflight(
                    "notes.local_save.latency", "phase=snapshot_preflight durationMs=$preflightMillis"
                )
                commitStarted = System.nanoTime()"####)?;
            let transaction = source.find("            val committedStamp = stateDatabase.withSnapshotWriteTransaction {")
                .ok_or("missing save transaction for deferred diagnostics")?;
            let transaction_end = transaction + source[transaction..].find("            stageFinished(\"sqlite_commit\")")
                .ok_or("missing end of save transaction for deferred diagnostics")?;
            let transaction_body = source[transaction..transaction_end]
                .strip_prefix("            val committedStamp = stateDatabase.withSnapshotWriteTransaction {\n")
                .and_then(|body| body.strip_suffix("            }\n"))
                .ok_or("unexpected save transaction shape")?;
            let transaction_body: String = transaction_body.lines().map(|line| format!("    {line}\n")).collect();
            source.replace_range(transaction..transaction_end + "            stageFinished(\"sqlite_commit\")\n".len(),
                &format!("{TRACKED_TRANSACTION}{transaction_body}{FLUSH_DIAGNOSTICS}"));
        }
        DATABASE => {
            if source.contains("    fun forEachSnapshot(\n") {
                replace(&mut source,
                    "        skipVerifiedHistory: (() -> Boolean)? = null,\n        action: (StoredStateSnapshot) -> Unit",
                    "        skipVerifiedHistory: (() -> Boolean)? = null,\n        currentOnlyForLocalWrite: Boolean = false,\n        action: (StoredStateSnapshot) -> Unit")?;
                replace(&mut source,
                    "        quarantineDigestMismatches(workspaceKey, includeHistory = !currentOnlyStartup)",
                    "        quarantineDigestMismatches(\n            workspaceKey, includeHistory = !currentOnlyStartup && !currentOnlyForLocalWrite\n        )")?;
                replace(&mut source,
                    "        if (currentOnlyStartup) quarantineDigestMismatches(workspaceKey, includeHistory = true)",
                    "        if (currentOnlyForLocalWrite) return\n        if (currentOnlyStartup) quarantineDigestMismatches(workspaceKey, includeHistory = true)")?;
            }
            // Sourcegen unit tests also render a pre-migration v4 fragment.
            // Full production generation has the v5 history codec already installed.
            if source.contains("const val DATABASE_VERSION = 5") {
            // Migrate existing v5 history storage in place, retaining all payloads.
            replace(&mut source, "const val DATABASE_VERSION = 5", "const val DATABASE_VERSION = 7")?;
            replace(&mut source, "        createHistoryEncodingSchema(database)\n    }\n\n    override fun onUpgrade",
                "        createHistoryEncodingSchema(database)\n        createHistoryGuardIndexes(database)\n    }\n\n    override fun onUpgrade")?;
            replace(&mut source, "        if (oldVersion < 5) createHistoryEncodingSchema(database)",
                "        if (oldVersion < 5) createHistoryEncodingSchema(database)\n        if (oldVersion < 7) {\n            if (oldVersion == 6) database.execSQL(\"DROP INDEX IF EXISTS snapshot_history_envelope_guard\")\n            createHistoryGuardIndexes(database)\n        }")?;
            replace(&mut source, "    private fun createHistoryEncodingSchema(database: SQLiteDatabase) {",
                &format!("{HISTORY_GUARD_INDEXES}\n    private fun createHistoryEncodingSchema(database: SQLiteDatabase) {{"))?;
            // An exported database stamped with v7 must contain the same guards;
            // otherwise restoring it would skip onUpgrade and fail INDEXED BY.
            replace(&mut source,
                "                database.execSQL(\"PRAGMA app_state_backup.user_version=$DATABASE_VERSION\")",
                r####"                database.execSQL(
                    "CREATE INDEX app_state_backup.snapshot_history_schema_guard " +
                        "ON snapshot_history(workspace_key, schema_version)"
                )
                database.execSQL(
                    "CREATE INDEX app_state_backup.snapshot_history_digest_guard " +
                        "ON snapshot_history(workspace_key) WHERE sha256 IS NULL OR sha256 = ''"
                )
                database.execSQL(
                    "CREATE INDEX app_state_backup.snapshot_history_envelope_guard " +
                        "ON snapshot_history(workspace_key) WHERE envelope_sha256 IS NULL OR " +
                        "(envelope_sha256 = '' AND storage_encoding != 1)"
                )
                database.execSQL("PRAGMA app_state_backup.user_version=$DATABASE_VERSION")"####)?;
            // Saves may bypass a legacy row's body, but must NEVER prune
            // unauthenticated history before an explicit recovery/verification.
            replace(&mut source,
                "              AND created_at_epoch_millis < ?\n              AND id NOT IN (",
                "              AND created_at_epoch_millis < ?\n              AND sha256 IS NOT NULL AND sha256 != ''\n              AND envelope_sha256 IS NOT NULL AND envelope_sha256 != ''\n              AND id NOT IN (")?;
            }
            // Startup and save use the same historical metadata safety guard.
            let startup = source.find("    private fun historyRequiresFullStartupScan(")
                .ok_or("missing startup history metadata guard")?;
            let startup_end = startup + source[startup..].find("\n\n    fun <T> withVerifiedWorkspaceReads(")
                .ok_or("missing startup history guard end")?;
            source.replace_range(startup..startup_end,
                "    private fun historyRequiresFullStartupScan(workspaceKey: String): Boolean =\n        historyRequiresFullSaveVerification(workspaceKey) || historyHasUncheckedLegacyRows(workspaceKey)");
            replace(&mut source, "    private val snapshotReadWindow = ThreadLocal<SnapshotVerificationWindow>()",
                &format!("{DATABASE_PREFLIGHT}\n    private val snapshotReadWindow = ThreadLocal<SnapshotVerificationWindow>()"))?;
            source.push_str(POLICY);
        }
        _ => {}
    }
    Ok(source)
}

const REPOSITORY_CACHE_DIAGNOSTIC: &str = r####"                if (cached?.first == workspaceKey && cached?.second == stamp) {
                    recordPreflight("notes.local_save.preflight", "scope=cached")
                }
                // Preflight and the authoritative write share one primary transaction;"####;

const TRACKED_TRANSACTION: &str = r####"            val preflightDiagnostics = mutableListOf<Pair<String, String>>()
            var commitStarted: Long? = null
            fun recordPreflight(category: String, message: String) {
                preflightDiagnostics.add(category to message)
            }
            val committedStamp = try {
                stateDatabase.withSnapshotWriteTransactionTracked(
                    onMonitorWait = { millis -> recordPreflight(
                        "notes.local_save.latency", "phase=outer_database_monitor_wait durationMs=$millis"
                    ) },
                    onSqliteBeginWait = { millis -> recordPreflight(
                        "notes.local_save.latency", "phase=outer_sqlite_begin_wait durationMs=$millis"
                    ) }
                ) {
                    stageStarted = System.nanoTime()
"####;

const FLUSH_DIAGNOSTICS: &str = r####"                }
            } finally {
                commitStarted?.let { started ->
                    val commitMillis = (System.nanoTime() - started) / 1_000_000L
                    if (commitMillis >= 1_000L) recordPreflight(
                        "notes.local_save.latency", "phase=sqlite_commit durationMs=$commitMillis"
                    )
                }
                preflightDiagnostics.forEach { (category, message) -> logDiagnosticEvent(category, message) }
            }
            stageStarted = System.nanoTime()
"####;

const HISTORY_GUARD_INDEXES: &str = r####"    // Compact probes for unsafe headers and legacy payloads still awaiting validation.
    private fun createHistoryGuardIndexes(database: SQLiteDatabase) {
        database.execSQL(
            "CREATE INDEX IF NOT EXISTS snapshot_history_schema_guard " +
                "ON $HISTORY_TABLE(workspace_key, schema_version)"
        )
        database.execSQL(
            "CREATE INDEX IF NOT EXISTS snapshot_history_digest_guard " +
                "ON $HISTORY_TABLE(workspace_key) WHERE " +
                "sha256 IS NULL OR sha256 = ''"
        )
        database.execSQL(
            "CREATE INDEX IF NOT EXISTS snapshot_history_envelope_guard " +
                "ON $HISTORY_TABLE(workspace_key) WHERE " +
                "envelope_sha256 IS NULL OR (envelope_sha256 = '' AND storage_encoding != 1)"
        )
    }

"####;

const DATABASE_PREFLIGHT: &str = r####"    // Two index seeks instead of repeated per-row reads next to history JSON.
    // Unknown schema/digest triggers full verification; legacy envelopes
    // are preserved unpruned and checked during startup or explicit recovery.
    private fun historyRequiresFullSaveVerification(workspaceKey: String): Boolean {
        val unknownSchema = writableDatabase.rawQuery(
            "SELECT 1 FROM $HISTORY_TABLE INDEXED BY snapshot_history_schema_guard " +
                "WHERE workspace_key = ? AND schema_version IS NULL LIMIT 1",
            arrayOf(workspaceKey)
        ).use { it.moveToFirst() }
        if (unknownSchema) return true
        val futureSchema = writableDatabase.rawQuery(
            "SELECT 1 FROM $HISTORY_TABLE INDEXED BY snapshot_history_schema_guard " +
                "WHERE workspace_key = ? AND schema_version > ? LIMIT 1",
            arrayOf(workspaceKey, APP_DATA_SCHEMA_VERSION.toString())
        ).use { it.moveToFirst() }
        if (futureSchema) return true
        return writableDatabase.rawQuery(
            "SELECT 1 FROM $HISTORY_TABLE INDEXED BY snapshot_history_digest_guard " +
                "WHERE workspace_key = ? AND (sha256 IS NULL OR sha256 = '') LIMIT 1",
            arrayOf(workspaceKey)
        ).use { it.moveToFirst() }
    }

    // A local write cannot replace historical evidence. Keep damaged history for
    // recovery, while refusing a future header even when its digest is unusable.
    fun blockingHistorySchemaForLocalWrite(workspaceKey: String): Int? {
        val database = writableDatabase
        check(database.inTransaction())
        val unknownSchema = database.rawQuery(
            "SELECT 1 FROM $HISTORY_TABLE INDEXED BY snapshot_history_schema_guard " +
                "WHERE workspace_key = ? AND schema_version IS NULL LIMIT 1",
            arrayOf(workspaceKey)
        ).use { it.moveToFirst() }
        if (unknownSchema) throw LocalSnapshotRecoveryException("A history snapshot has an unknown schema.")
        return database.rawQuery(
            "SELECT schema_version FROM $HISTORY_TABLE INDEXED BY snapshot_history_schema_guard " +
                "WHERE workspace_key = ? AND schema_version > ? ORDER BY schema_version DESC LIMIT 1",
            arrayOf(workspaceKey, APP_DATA_SCHEMA_VERSION.toString())
        ).use { cursor ->
            if (cursor.moveToFirst()) cursor.getLong(0).coerceAtMost(Int.MAX_VALUE.toLong()).toInt() else null
        }
    }

    fun hasHistoryForLocalWrite(workspaceKey: String): Boolean = writableDatabase.rawQuery(
        "SELECT 1 FROM $HISTORY_TABLE INDEXED BY snapshot_history_schema_guard WHERE workspace_key = ? LIMIT 1",
        arrayOf(workspaceKey)
    ).use { it.moveToFirst() }

    // The existing codec writes storage_encoding=1 only after validating the
    // original digest and both schema declarations, packing and reading back
    // the payload. Legacy rows retain their empty envelopes and remain recovery
    // evidence. Export resets the packing queue, so imports revalidate them.
    private fun historyHasUncheckedLegacyRows(workspaceKey: String): Boolean =
        writableDatabase.rawQuery(
            "SELECT 1 FROM $HISTORY_TABLE INDEXED BY snapshot_history_envelope_guard " +
                "WHERE workspace_key = ? AND (envelope_sha256 IS NULL OR " +
                "(envelope_sha256 = '' AND storage_encoding != 1)) LIMIT 1",
            arrayOf(workspaceKey)
        ).use { it.moveToFirst() }

    fun <T> withVerifiedSavePreflight(
        workspaceKey: String,
        currentOnlyForLocalWrite: Boolean = false,
        onNativeVerification: (Long, Long) -> Unit = { _, _ -> },
        onTransactionWait: (Long) -> Unit = {},
        onScope: (Boolean) -> Unit = {},
        action: () -> T
    ): T {
        val waitingSince = System.nanoTime()
        return withSnapshotWriteTransaction {
            val waitMs = (System.nanoTime() - waitingSince).coerceAtLeast(0L) / 1_000_000L
            if (waitMs >= 1_000L) onTransactionWait(waitMs)
            var currentOnlyVerified = false
            // Every receipt belongs solely to this pinned transaction.
            runNoteSavePreflight(
                workspaceKey = workspaceKey,
                windows = snapshotReadWindow,
                transactionActive = { writableDatabase.inTransaction() },
                snapshotStamp = ::snapshotWriteStamp,
                verifyRows = {
                    val began = System.nanoTime()
                    var verifiedRows = -1L
                    try {
                        val safeHistoryHeaders = currentOnlyForLocalWrite ||
                            !historyRequiresFullSaveVerification(workspaceKey)
                        val currentRows = if (safeHistoryHeaders)
                            runCatching { verifyNativeSnapshotRows(workspaceKey, currentOnly = true) }
                                .getOrDefault(-1L)
                        else -1L
                        if (canUseCurrentOnlySaveProof(safeHistoryHeaders, currentRows)) {
                            currentOnlyVerified = true
                            verifiedRows = currentRows
                        } else if (!currentOnlyForLocalWrite) {
                            // Unknown schema, absent/corrupt current or native failure:
                            // preserve the original full history/recovery guard.
                            verifyNativeSnapshotRows(workspaceKey).also { verifiedRows = it }
                        }
                        verifiedRows
                    } finally {
                        val elapsedMs = (System.nanoTime() - began).coerceAtLeast(0L) / 1_000_000L
                        if (elapsedMs >= 1_000L) onNativeVerification(elapsedMs, verifiedRows)
                    }
                },
                currentOnlyVerified = { currentOnlyVerified },
                onVerificationFailure = {
                    android.util.Log.w("NoteSaveLatency", "Native verification unavailable; using the snapshot reader", it)
                },
                action = {
                    val currentProof = currentOnlyVerified &&
                        snapshotReadWindow.get()?.hasCurrentOnlyReceipt(
                            workspaceKey, snapshotWriteStamp()
                        ) == true
                    // Diagnostic logging must never turn a committed save into a failure.
                    runCatching { onScope(currentProof) }.onFailure {
                        android.util.Log.w("NoteSaveLatency", "scope diagnostic unavailable", it)
                    }
                    android.util.Log.i(
                        "NoteSaveLatency",
                        "savePreflight scope=" + if (currentProof) "current" else "full"
                    )
                    action()
                }
            )
        }
    }
"####;

const SCHEMA_PREFLIGHT: &str = r####"    private fun futureSchemaVersionOnAnySurface(
        workspaceKey: String,
        includeUnmigratedLegacy: Boolean = false,
        currentOnlyForLocalWrite: Boolean = false,
        diagnostic: (String, String) -> Unit = { _, _ -> }
    ): Int? = stateDatabase.withVerifiedSavePreflight(
        workspaceKey,
        currentOnlyForLocalWrite = currentOnlyForLocalWrite,
        onNativeVerification = { millis, rows ->
            diagnostic(
                "notes.local_save.latency",
                "phase=preflight_native_verification durationMs=$millis rows=$rows"
            )
        },
        onTransactionWait = { millis ->
            diagnostic(
                "notes.local_save.latency",
                "phase=preflight_transaction_wait durationMs=$millis"
            )
        },
        onScope = { current ->
            diagnostic(
                "notes.local_save.preflight",
                if (current) "scope=current" else if (currentOnlyForLocalWrite) "scope=current_fallback" else "scope=full"
            )
        }
    ) {
        val evidence = NoteSavePreflightEvidence(APP_DATA_SCHEMA_VERSION)
        if (currentOnlyForLocalWrite) {
            stateDatabase.blockingHistorySchemaForLocalWrite(workspaceKey)?.let {
                return@withVerifiedSavePreflight it
            }
        }
        // Native success proves payload, envelope and BOTH schema declarations.
        // Read and accept the current row before skipping verified histories.
        // Missing/corrupt/future current and native failures use the old visitor.
        stateDatabase.forEachSnapshot(
            workspaceKey,
            skipVerifiedHistory = { evidence.acceptedCurrentWithoutFuture },
            currentOnlyForLocalWrite = currentOnlyForLocalWrite
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
        if (currentOnlyForLocalWrite && evidence.futureVersion == null && !evidence.acceptedCurrentWithoutFuture) {
            if (stateDatabase.hasHistoryForLocalWrite(workspaceKey)) {
                throw LocalSnapshotRecoveryException("The current snapshot requires recovery.")
            }
        }
        evidence.futureVersion
    }

"####;

// Production policy is deliberately Android-free so its transaction, failure,
// workspace and schema boundaries run as ordinary JVM specifications.
pub const POLICY: &str = r####"

internal class LocalSnapshotSchemaException(version: Int) :
    IllegalStateException("Refusing to overwrite persisted evidence created by future schema $version.")

internal class LocalSnapshotRecoveryException(message: String) : IllegalStateException(message)

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

internal fun canUseCurrentOnlySaveProof(historyMetadataSafe: Boolean, verifiedCurrentRows: Long): Boolean =
    historyMetadataSafe && verifiedCurrentRows == 1L

internal fun <Stamp : Any, Result> runNoteSavePreflight(
    workspaceKey: String,
    windows: ThreadLocal<SnapshotVerificationWindow>,
    transactionActive: () -> Boolean,
    snapshotStamp: () -> Stamp,
    verifyRows: () -> Long,
    onVerificationFailure: (Throwable) -> Unit = {},
    currentOnlyVerified: () -> Boolean = { false },
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
            if (currentOnlyVerified()) window.recordNativeCurrent(workspaceKey, after)
            else window.recordNativeHistory(workspaceKey, after)
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

    @Test fun saveCurrentOnlyDecisionRequiresTrustworthyMetadataAndExactlyOneValidCurrent() {
        assertTrue(canUseCurrentOnlySaveProof(true, 1L))
        for (rows in listOf(-1L, 0L, 2L, 7001L)) {
            assertFalse(canUseCurrentOnlySaveProof(true, rows))
        }
        assertFalse(canUseCurrentOnlySaveProof(false, 1L))
    }

    @Test fun currentOnlySaveReceiptNeverPretendsToHaveValidatedAllHistory() {
        runNoteSavePreflight("guest", windows, { true }, { 7L }, { 1L },
            currentOnlyVerified = { true }) {
            val evidence = NoteSavePreflightEvidence(15)
            evidence.observe("sqlite-current", 15, 15)
            assertTrue(canSkip(evidence))
            assertFalse(windows.get()!!.hasFullHistoryReceipt("guest", 7L))
            assertFalse(canSkip(evidence, workspace = "account:other"))
            assertFalse(canSkip(evidence, stamp = 8L))
        }
        assertNull(windows.get())
    }

    @Test fun currentOnlyProofNeedsUnchangedTransactionalStampAndNativeSuccess() {
        var stamp = 7L
        runNoteSavePreflight("guest", windows, { true }, { stamp },
            { stamp = 8L; 1L }, currentOnlyVerified = { true }) {
            val evidence = NoteSavePreflightEvidence(15)
            evidence.observe("sqlite-current", 15, 15)
            assertFalse(canSkip(evidence, stamp = 8L))
        }
        runNoteSavePreflight("guest", windows, { true }, { 8L },
            { -1L }, currentOnlyVerified = { true }) {
            val evidence = NoteSavePreflightEvidence(15)
            evidence.observe("sqlite-current", 15, 15)
            assertFalse(canSkip(evidence, stamp = 8L))
        }
    }

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
    fn history_anomaly_guard_uses_indexes_and_detects_unsealed_and_future_rows() {
        let db = rusqlite::Connection::open_in_memory().unwrap();
        db.execute_batch(r#"
            CREATE TABLE snapshot_history(
                workspace_key TEXT NOT NULL,
                schema_version INTEGER,
                app_data_json TEXT,
                sha256 TEXT,
                envelope_sha256 TEXT,
                storage_encoding INTEGER NOT NULL DEFAULT 0
            );
            CREATE INDEX snapshot_history_schema_guard
                ON snapshot_history(workspace_key, schema_version);
            CREATE INDEX snapshot_history_digest_guard
                ON snapshot_history(workspace_key)
                WHERE sha256 IS NULL OR sha256 = '';
            CREATE INDEX snapshot_history_envelope_guard
                ON snapshot_history(workspace_key)
                WHERE envelope_sha256 IS NULL OR (envelope_sha256 = '' AND storage_encoding != 1);
        "#).unwrap();
        for i in 0..7200 {
            db.execute("INSERT INTO snapshot_history VALUES (?1, 15, ?2, 'digest', 'envelope', 0)",
                rusqlite::params![if i % 113 == 0 { "other" } else { "guest" }, "note-content"]).unwrap();
        }
        let schema_null = "SELECT 1 FROM snapshot_history INDEXED BY snapshot_history_schema_guard
            WHERE workspace_key = ?1 AND schema_version IS NULL LIMIT 1";
        let schema_future = "SELECT 1 FROM snapshot_history INDEXED BY snapshot_history_schema_guard
            WHERE workspace_key = ?1 AND schema_version > ?2 LIMIT 1";
        let highest_future = "SELECT schema_version FROM snapshot_history INDEXED BY snapshot_history_schema_guard
            WHERE workspace_key = ?1 AND schema_version > ?2 ORDER BY schema_version DESC LIMIT 1";
        let digest = "SELECT 1 FROM snapshot_history INDEXED BY snapshot_history_digest_guard
            WHERE workspace_key = ?1 AND (sha256 IS NULL OR sha256 = '') LIMIT 1";
        let envelope = "SELECT 1 FROM snapshot_history INDEXED BY snapshot_history_envelope_guard
            WHERE workspace_key = ?1 AND (envelope_sha256 IS NULL OR
                (envelope_sha256 = '' AND storage_encoding != 1)) LIMIT 1";
        let unsafe_schema = |workspace: &str| -> bool {
            db.prepare(schema_null).unwrap().exists([workspace]).unwrap() ||
                db.prepare(schema_future).unwrap().exists(rusqlite::params![workspace, 15]).unwrap()
        };
        let unsafe_digest = |workspace: &str| -> bool {
            db.prepare(digest).unwrap().exists([workspace]).unwrap()
        };
        let unsafe_envelope = |workspace: &str| -> bool {
            db.prepare(envelope).unwrap().exists([workspace]).unwrap()
        };
        assert!(!unsafe_schema("guest"));
        assert!(!unsafe_digest("guest"));
        assert!(!unsafe_envelope("guest"));
        let plan = |statement: &str, args: Vec<&dyn rusqlite::ToSql>| -> String {
            let mut prepared = db.prepare(&format!("EXPLAIN QUERY PLAN {statement}")).unwrap();
            let details = prepared.query_map(args.as_slice(), |row| row.get::<_, String>(3))
                .unwrap().collect::<Result<Vec<_>, _>>().unwrap();
            details.join(" | ")
        };
        let version: &dyn rusqlite::ToSql = &15_i64;
        let workspace: &dyn rusqlite::ToSql = &"guest";
        let null_plan = plan(schema_null, vec![workspace]);
        let schema_plan = plan(schema_future, vec![workspace, version]);
        let highest_plan = plan(highest_future, vec![workspace, version]);
        let digest_plan = plan(digest, vec![workspace]);
        let envelope_plan = plan(envelope, vec![workspace]);
        assert!(schema_plan.contains("COVERING INDEX snapshot_history_schema_guard"), "{schema_plan}");
        assert!(schema_plan.contains("schema_version>?"), "{schema_plan}");
        assert!(highest_plan.contains("COVERING INDEX snapshot_history_schema_guard"), "{highest_plan}");
        assert!(!highest_plan.contains("TEMP B-TREE"), "{highest_plan}");
        assert!(null_plan.contains("COVERING INDEX snapshot_history_schema_guard"), "{null_plan}");
        assert!(null_plan.contains("schema_version=?"), "{null_plan}");
        assert!(digest_plan.contains("INDEX snapshot_history_digest_guard"), "{digest_plan}");
        assert!(envelope_plan.contains("INDEX snapshot_history_envelope_guard"), "{envelope_plan}");
        db.execute("INSERT INTO snapshot_history VALUES ('other', 16, 'x', 'digest', 'envelope', 0)", []).unwrap();
        assert!(!unsafe_schema("guest"));
        db.execute("INSERT INTO snapshot_history VALUES ('guest', 16, 'x', 'digest', 'envelope', 0)", []).unwrap();
        assert!(unsafe_schema("guest"));
        db.execute("INSERT INTO snapshot_history VALUES ('guest', NULL, 'x', 'digest', 'envelope', 0)", []).unwrap();
        assert!(unsafe_schema("guest"));
        db.execute("INSERT INTO snapshot_history VALUES ('guest', 15, 'x', 'digest', '', 0)", []).unwrap();
        assert!(unsafe_envelope("guest"));
        db.execute("UPDATE snapshot_history SET storage_encoding = 1 WHERE envelope_sha256 = ''", []).unwrap();
        assert!(!unsafe_envelope("guest"));
        db.execute("UPDATE snapshot_history SET storage_encoding = 0 WHERE envelope_sha256 = ''", []).unwrap();
        assert!(unsafe_envelope("guest"));
        assert!(!unsafe_digest("guest"));
        db.execute("INSERT INTO snapshot_history VALUES ('guest', 15, 'x', '', 'envelope', 0)", []).unwrap();
        assert!(unsafe_digest("guest"));
        db.execute("INSERT INTO snapshot_history VALUES ('guest', 17, 'x', '', '', 0)", []).unwrap();
        db.execute("INSERT INTO snapshot_history VALUES ('other', 18, 'x', '', '', 0)", []).unwrap();
        let guarded_version: i64 = db.query_row(highest_future, rusqlite::params!["guest", 15], |row| row.get(0)).unwrap();
        assert_eq!(guarded_version, 17);
    }


    #[test]
    fn slow_native_verification_records_only_bounded_metrics() {
        assert!(DATABASE_PREFLIGHT.contains("withSnapshotWriteTransaction"));
        assert!(DATABASE_PREFLIGHT.contains("verifyNativeSnapshotRows(workspaceKey)"));
        assert!(DATABASE_PREFLIGHT.contains("verifyNativeSnapshotRows(workspaceKey, currentOnly = true)"));
        assert!(DATABASE_PREFLIGHT.contains("historyRequiresFullSaveVerification(workspaceKey)"));
        assert!(DATABASE_PREFLIGHT.contains("INDEXED BY snapshot_history_schema_guard"));
        assert!(DATABASE_PREFLIGHT.contains("INDEXED BY snapshot_history_digest_guard"));
        assert!(DATABASE_PREFLIGHT.contains("INDEXED BY snapshot_history_envelope_guard"));
        assert!(HISTORY_GUARD_INDEXES.contains("CREATE INDEX IF NOT EXISTS snapshot_history_digest_guard"));
        assert!(DATABASE_PREFLIGHT.contains("envelope_sha256 IS NULL OR "));
        assert!(DATABASE_PREFLIGHT.contains("envelope_sha256 = '' AND storage_encoding != 1"));
        assert!(DATABASE_PREFLIGHT.contains("schema_version IS NULL LIMIT 1"));
        assert!(DATABASE_PREFLIGHT.contains("schema_version > ? LIMIT 1"));
        assert!(DATABASE_PREFLIGHT.contains("sha256 IS NULL OR sha256 = ''"));
        assert!(DATABASE_PREFLIGHT.contains("onTransactionWait(waitMs)"));
        assert!(SCHEMA_PREFLIGHT.contains("notes.local_save.preflight"));
        assert!(REPOSITORY_CACHE_DIAGNOSTIC.contains("scope=cached"));
        assert!(DATABASE_PREFLIGHT.contains("hasCurrentOnlyReceipt"));
        assert!(DATABASE_PREFLIGHT.contains("onScope(currentProof)"));
        assert!(SCHEMA_PREFLIGHT.contains("phase=preflight_transaction_wait"));
        assert!(POLICY.contains("window.recordNativeCurrent(workspaceKey, after)"));
        assert!(POLICY.contains("window.recordNativeHistory(workspaceKey, after)"));
        assert!(DATABASE_PREFLIGHT.contains("finally {"));
        assert!(DATABASE_PREFLIGHT.contains("onNativeVerification(elapsedMs, verifiedRows)"));
        assert!(SCHEMA_PREFLIGHT.contains("phase=preflight_native_verification durationMs=$millis rows=$rows"));
        assert!(!SCHEMA_PREFLIGHT.contains("noteId="));
        assert!(!SCHEMA_PREFLIGHT.contains("workspaceKey=$"));
        assert!(SCHEMA_PREFLIGHT.contains("forEachQuarantinedCurrentSnapshot"));
        assert!(SCHEMA_PREFLIGHT.contains("observeFile(files.backup)"));
    }
}
