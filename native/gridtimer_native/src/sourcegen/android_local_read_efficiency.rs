// v2.23.2.47 - Avoid unrelated media IO and unbounded maintenance before ready.
const MEDIA: &str = "com/ofairyo/gridtimer/data/NoteMediaStore.kt";
const REPOSITORY: &str = "com/ofairyo/gridtimer/data/TimerRepository.kt";
const DATABASE: &str = "com/ofairyo/gridtimer/data/AppStateDatabase.kt";

fn replace(source: &mut String, old: &str, new: &str) -> Result<(), String> {
    if source.matches(old).count() != 1 {
        return Err(format!("local read efficiency anchor is not unique: {old}"));
    }
    *source = source.replacen(old, new, 1);
    Ok(())
}

pub fn render(path: &str, base: &str) -> Result<String, String> {
    let mut source = base.to_owned();
    match path {
        MEDIA => {
            let start = source
                .find("    fun resolvedAttachmentHashes(")
                .ok_or("missing hash hydration")?;
            let end = start
                + source[start..]
                    .find("    fun verifiedAttachmentFile(")
                    .ok_or("missing media verifier")?;
            source.replace_range(start..end, MEDIA_HYDRATION);
            source.push_str(MEDIA_POLICY);
        }
        REPOSITORY => {
            replace(
                &mut source,
                "    private val accountBootstrapJob = scope.launch {",
                &format!(
                    "{MAINTENANCE_JOB}\n    private val accountBootstrapJob = scope.launch {{"
                ),
            )?;
            for (name, expression) in [
                ("legacyMigration", "migrateLegacyStateOnce()"),
                ("workspaceRestore", "loadPersistedData() ?: AppData.default()"),
                ("draftRecovery", "recoverJournaledNoteDrafts(activeWorkspaceKey, resolvedMicroBreaks)"),
                ("mediaRecovery", "recoverPendingNoteMediaImports(activeWorkspaceKey, recoveredDrafts.data, now())"),
                ("missingMediaHashes", "hydrateMissingAttachmentHashes(activeWorkspaceKey, recoveredMedia, now())"),
            ] {
                // Restrict instrumentation to initial loading, not account switching.
                let start = source.find("    private suspend fun loadFromDisk()").ok_or("missing disk load")?;
                let end = start + source[start..].find("    private suspend fun updateData(").ok_or("missing disk load end")?;
                let mut load = source[start..end].to_owned();
                replace(&mut load, expression, &format!("measureLocalStartup(\"{name}\") {{ {expression} }}"))?;
                source.replace_range(start..end, &load);
            }
            source.push_str(STARTUP_TIMING);
            source.push_str(MAINTENANCE_POLICY);
        }
        DATABASE => {
            replace(&mut source,
                "        if (freshTransaction) runCatching { compactHistory(workspaceKey) }.onFailure {\n            android.util.Log.w(\"StartupLatency\", \"historyCompact incomplete; retaining original rows\", it)\n        }\n",
                "        // Packing old rows is maintenance, not a prerequisite for reading them.\n")?;
            // Keep canonical packing, verification and readback, but do one row per
            // repository-owned transaction. No background job owns a long DB lock.
            let start = source
                .find("    @Synchronized\n    private fun compactHistory(")
                .ok_or("missing history compaction")?;
            let end = start
                + source[start..]
                    .find("    fun <T> withVerifiedSavePreflight(")
                    .ok_or("missing compaction end")?;
            let mut compact = source[start..end].to_owned();
            replace(
                &mut compact,
                "private fun compactHistory(workspaceKey: String) {",
                "fun compactHistoryBatch(workspaceKey: String, afterId: Long): Long? {",
            )?;
            replace(
                &mut compact,
                "NativeSnapshotVerifier.key.size != 32) return",
                "NativeSnapshotVerifier.key.size != 32) return null",
            )?;
            replace(
                &mut compact,
                "check(!database.inTransaction())",
                "check(database.inTransaction())",
            )?;
            replace(&mut compact, "var after = 0L", "var after = afterId")?;
            replace(&mut compact, "while (true) {", "run {")?;
            replace(&mut compact, "ORDER BY id LIMIT 16", "ORDER BY id LIMIT 1")?;
            replace(
                &mut compact,
                "if (ids.isEmpty()) break",
                "if (ids.isEmpty()) return null",
            )?;
            replace(&mut compact, "        if (changed > 0) android.util.Log.i(\"StartupLatency\", \"historyCompact rows=$changed rawBytes=$beforeBytes packedBytes=$afterBytes elapsedMs=${android.os.SystemClock.elapsedRealtime() - started}\")",
                "        return after")?;
            for line in ["        var changed = 0\n", "        var beforeBytes = 0L\n", "        var afterBytes = 0L\n", "        val started = android.os.SystemClock.elapsedRealtime()\n", "                        changed++\n", "                        beforeBytes += raw.toByteArray(Charsets.UTF_8).size\n", "                        afterBytes += packed.length\n", "            if (changed > 0 && changed % 128 == 0) android.util.Log.i(\"StartupLatency\", \"historyCompact progress=$changed\")\n"] {
                replace(&mut compact, line, "")?;
            }
            source.replace_range(start..end, &compact);
            // Both APIs follow the existing workspace/time/id index, rather than
            // traversing payload pages belonging to other accounts.
            replace(&mut source, "            \"FROM $table NOT INDEXED WHERE workspace_key = ? ORDER BY rowid\"",
                "            \"FROM $table WHERE workspace_key = ? ORDER BY \" +\n            if (table == HISTORY_TABLE) \"created_at_epoch_millis DESC, id DESC\" else \"rowid\"")?;
            let start = source
                .find("    private fun visitNativeSnapshotBatches(")
                .ok_or("missing snapshot batches")?;
            let end = start
                + source[start..]
                    .find("    private fun historyPrefix(")
                    .ok_or("missing batch end")?;
            source.replace_range(start..end, INDEXED_BATCHES);
        }
        _ => {}
    }
    Ok(source)
}

const MEDIA_HYDRATION: &str = r####"    fun resolvedAttachmentHashes(
        context: Context,
        workspaceKey: String,
        attachments: Collection<NoteAttachment>
    ): Map<String, String> = withWorkspaceMediaLock(workspaceKey) {
        resolveMissingMediaHashes(attachments, NoteAttachment::fileName, NoteAttachment::sha256) { attachment ->
            val file = runCatching { attachmentFile(context, workspaceKey, attachment) }
                .getOrNull()?.takeIf(File::isFile)
            file?.let { sha256(it) }
        }
    }

"####;

pub const MEDIA_POLICY: &str = r####"

// Hydration only fills absent metadata. Existing hashes are verified by the
// actual reader/copy/sync paths; rehashing them here cannot repair anything.
internal fun <T> resolveMissingMediaHashes(
    attachments: Collection<T>, fileNameOf: (T) -> String,
    expectedHashOf: (T) -> String, readDigest: (T) -> String?
): Map<String, String> {
    val missing = attachments.asSequence()
        .filter { expectedHashOf(it).isBlank() }
        .map(fileNameOf).filter(String::isNotBlank).toSet()
    if (missing.isEmpty()) return emptyMap()
    val digestPattern = Regex("[0-9a-f]{64}")
    return buildMap {
        attachments.filter { fileNameOf(it) in missing }.groupBy(fileNameOf).forEach { (name, references) ->
            // A historical reference may already carry a digest for this file.
            // Check every declared digest before filling a missing reference.
            val expected = references.map(expectedHashOf).filter(String::isNotBlank)
                .map { it.trim().lowercase(java.util.Locale.ROOT) }.toSet()
            if (expected.size > 1 || expected.any { !digestPattern.matches(it) }) return@forEach
            val digest = readDigest(references.first()) ?: return@forEach
            if (digestPattern.matches(digest) && (expected.isEmpty() || digest in expected)) put(name, digest)
        }
    }
}
"####;

const MAINTENANCE_JOB: &str = r####"    private val historyCompactionJob = scope.launch {
        initializationJob.join()
        // Give the first frame and initial user action priority. Every iteration
        // yields, uses tryLock, and packs at most one row. Restart resumes via the
        // persisted encoding queue; no current/history row is discarded.
        delay(2_000L)
        val workspaceKey = activeWorkspaceKey
        var afterId = 0L
        while (true) {
            if (!writeMutex.tryLock()) {
                delay(1_000L)
                continue
            }
            val next = try {
                if (activeWorkspaceKey != workspaceKey || futureSchemaVersion != null ||
                    loadedFromRecoveryCandidate || persistenceWriteBlockedReason != null) break
                withContext(Dispatchers.IO) {
                    val cached = verifiedWritePreflight
                    verifiedWritePreflight = null
                    val files = workspaceFiles(workspaceKey)
                    val result = compactVerifiedHistoryBatch(
                        expected = cached?.takeIf { it.first == workspaceKey }?.second,
                        transaction = { action -> stateDatabase.withSnapshotWriteTransaction(action) },
                        stamp = { PersistPreflightStamp(stateDatabase.snapshotWriteStamp(),
                            persistFileStamp(files.state), persistFileStamp(files.backup)) },
                        mirrorsUnchanged = { before, after -> before.primary == after.primary && before.backup == after.backup },
                        compact = { stateDatabase.compactHistoryBatch(workspaceKey, afterId) }
                    )
                    // Publish after successful commit only. Packing preserves exact
                    // verified content, so it must not force another full scan on tap.
                    result.stamp?.let { verifiedWritePreflight = workspaceKey to it }
                    result.nextRow
                }
            } catch (cancelled: CancellationException) {
                throw cancelled
            } catch (failure: Exception) {
                Log.w("StartupLatency", "Deferred history packing stopped; original rows retained", failure)
                null
            } finally {
                writeMutex.unlock()
            }
            if (next == null) break
            afterId = next
            delay(1_000L)
        }
    }
"####;

pub const MAINTENANCE_POLICY: &str = r####"

internal data class HistoryMaintenanceResult<S>(val nextRow: Long?, val stamp: S?)

internal fun <S : Any> compactVerifiedHistoryBatch(
    expected: S?,
    transaction: (() -> HistoryMaintenanceResult<S>) -> HistoryMaintenanceResult<S>,
    stamp: () -> S,
    mirrorsUnchanged: (S, S) -> Boolean,
    compact: () -> Long?
): HistoryMaintenanceResult<S> = transaction {
    val before = stamp()
    if (expected != before) HistoryMaintenanceResult(null, null)
    else {
        val next = compact()
        val after = stamp()
        HistoryMaintenanceResult(next, after.takeIf { mirrorsUnchanged(before, it) })
    }
}
"####;

const INDEXED_BATCHES: &str = r####"    private fun visitNativeSnapshotBatches(
        database: SQLiteDatabase, table: String, columns: Array<String>,
        workspaceKey: String, action: (Cursor) -> Unit
    ) {
        check(database.inTransaction())
        val history = table == HISTORY_TABLE
        val sizedWindow = android.os.Build.VERSION.SDK_INT >= 28
        val batchSize = if (sizedWindow) 16 else 1
        var afterTime: Long? = null
        var afterId: Long? = null
        do {
            val selection = "workspace_key = ?" + if (afterId == null) "" else
                " AND (created_at_epoch_millis, id) < (?, ?)"
            val arguments = if (afterId == null) arrayOf(workspaceKey) else
                arrayOf(workspaceKey, afterTime.toString(), afterId.toString())
            val scanColumns = if (history) arrayOf("id AS scan_row_id", "created_at_epoch_millis AS scan_time", *columns)
                else columns
            val order = if (history) "created_at_epoch_millis DESC, id DESC" else null
            var rows = 0
            database.query(table, scanColumns, selection, arguments, null, null, order, batchSize.toString()).use { cursor ->
                if (sizedWindow && cursor is android.database.sqlite.SQLiteCursor) {
                    cursor.window = android.database.CursorWindow("startup-batch", 16L * 1024 * 1024)
                    cursor.setFillWindowForwardOnly(true)
                }
                while (cursor.moveToNext()) {
                    if (history) {
                        val id = cursor.getLong(cursor.getColumnIndexOrThrow("scan_row_id"))
                        val time = cursor.getLong(cursor.getColumnIndexOrThrow("scan_time"))
                        check(afterId == null || time < afterTime!! || time == afterTime && id < afterId!!) {
                            "Snapshot scan failed to advance"
                        }
                        afterTime = time
                        afterId = id
                    }
                    action(cursor)
                    rows++
                }
            }
        } while (history && rows == batchSize)
    }

"####;

const STARTUP_TIMING: &str = r####"

private inline fun <T> measureLocalStartup(stage: String, action: () -> T): T {
    val started = android.os.SystemClock.elapsedRealtime()
    try { return action() }
    finally { android.util.Log.i("StartupLatency", "stage=$stage elapsedMs=${android.os.SystemClock.elapsedRealtime() - started}") }
}
"####;

pub const TEST_PATH: &str = "com/ofairyo/gridtimer/data/MissingMediaHashesTest.kt";
pub const TEST_CONTENTS: &str = r####"package com.ofairyo.gridtimer.data

import org.junit.Assert.*
import org.junit.Test

class MissingMediaHashesTest {
    private data class Attachment(val file: String, val hash: String)
    private val digest = "a".repeat(64)

    @Test fun completeMediaDoesNotOpenAnyFilesEvenWithManyHistoricalReferences() {
        val refs = List(10_000) { Attachment("image-${it % 100}.jpg", digest) }
        val resolved = resolveMissingMediaHashes(refs, Attachment::file, Attachment::hash) {
            error("Known attachment must not be read at startup")
        }
        assertTrue(resolved.isEmpty())
    }

    @Test fun onlyMissingFilesAreReadOnceAcrossCurrentAndHistoricalReferences() {
        val reads = mutableListOf<String>()
        val refs = listOf(Attachment("known", digest), Attachment("missing", ""),
            Attachment("missing", digest.uppercase()), Attachment("missing", ""))
        val resolved = resolveMissingMediaHashes(refs, Attachment::file, Attachment::hash) {
            reads += it.file; digest
        }
        assertEquals(listOf("missing"), reads)
        assertEquals(mapOf("missing" to digest), resolved)
    }

    @Test fun changedMissingInvalidOrConflictingFilesCannotAcquireATrustedHash() {
        for (hashes in listOf(listOf("", digest), listOf("", "invalid"), listOf("", digest, "b".repeat(64)))) {
            val refs = hashes.map { Attachment("file", it) }
            assertTrue(resolveMissingMediaHashes(refs, Attachment::file, Attachment::hash) { "c".repeat(64) }.isEmpty())
        }
        val refs = listOf(Attachment("gone", ""), Attachment("", ""))
        assertTrue(resolveMissingMediaHashes(refs, Attachment::file, Attachment::hash) { null }.isEmpty())
    }

    @Test fun unhashedLegacyFilesAreHashedAndIoFailuresRemainFailures() {
        val refs = listOf(Attachment("legacy", ""))
        assertEquals(mapOf("legacy" to digest),
            resolveMissingMediaHashes(refs, Attachment::file, Attachment::hash) { digest })
        assertTrue(runCatching {
            resolveMissingMediaHashes(refs, Attachment::file, Attachment::hash) { throw java.io.IOException("unreadable") }
        }.exceptionOrNull() is java.io.IOException)
    }

    private data class Stamp(val db: Int, val mirror: Int)

    @Test fun maintenanceReusesProofOnlyAfterItsTransactionCommits() {
        var inTransaction = false
        var committed = false
        var current = Stamp(1, 1)
        val result = compactVerifiedHistoryBatch(current, { action ->
            inTransaction = true
            try { action().also { committed = true } } finally { inTransaction = false }
        }, { assertTrue(inTransaction); current }, { a, b -> a.mirror == b.mirror }) {
            assertTrue(inTransaction)
            current = Stamp(2, 1); 8L
        }
        assertTrue(committed)
        assertFalse(inTransaction)
        assertEquals(current, result.stamp)
        assertEquals(8L, result.nextRow)
    }

    @Test fun staleProofChangedMirrorAndCommitFailureCannotPublishMaintenanceProof() {
        for (expected in listOf(null, Stamp(0, 1), Stamp(1, 0))) {
            val result = compactVerifiedHistoryBatch(expected, { it() }, { Stamp(1, 1) }, { _, _ -> true }) {
                error("Unverified workspace must not be compacted")
            }
            assertNull(result.stamp)
            assertNull(result.nextRow)
        }
        var current = Stamp(1, 1)
        val changed = compactVerifiedHistoryBatch(current, { it() }, { current }, { a, b -> a.mirror == b.mirror }) {
            current = Stamp(2, 2); 1L
        }
        assertNull(changed.stamp)
        var published: Stamp? = null
        assertTrue(runCatching {
            val result = compactVerifiedHistoryBatch(current, { action -> action(); error("commit failed") },
                { current }, { _, _ -> true }, { 1L })
            published = result.stamp
        }.isFailure)
        assertNull(published)
    }
}
"####;

#[cfg(test)]
mod tests {
    use super::*;

    fn rendered_database() -> String {
        let base = super::super::android_local_startup::tests::rendered(DATABASE);
        let base = super::super::android_timer_action::render(DATABASE, &base).unwrap();
        let base = super::super::android_startup_stream::render(DATABASE, &base).unwrap();
        let base = super::super::android_history_storage::render(DATABASE, &base).unwrap();
        let base = super::super::android_note_persist_preflight::render(DATABASE, &base).unwrap();
        render(DATABASE, &base).unwrap()
    }

    #[test]
    fn final_database_keeps_full_verification_and_packs_only_one_row_after_startup() {
        let source = rendered_database();
        let startup = source
            .split("fun <T> withVerifiedWorkspaceReads")
            .nth(1)
            .unwrap()
            .split("private fun verifyNativeSnapshotRows")
            .next()
            .unwrap();
        assert!(!startup.contains("compactHistory"));
        assert!(startup.contains("verifyNativeSnapshotRows(workspaceKey)"));
        assert!(startup.contains("verifiedRows >= 0 && before == after"));
        let compact = source
            .split("fun compactHistoryBatch")
            .nth(1)
            .unwrap()
            .split("fun <T> withVerifiedSavePreflight")
            .next()
            .unwrap();
        assert!(compact.contains("check(database.inTransaction())"));
        assert!(compact.contains("ORDER BY id LIMIT 1"));
        assert!(!compact.contains("while (true)"));
        assert!(compact.contains("NativeSnapshotVerifier.seal(raw, prefix, digest, envelope)"));
        assert!(compact.contains("check(roundTrip == packed)"));
    }

    #[test]
    fn indexed_pages_keep_every_workspace_row_with_ties_and_out_of_order_ids() {
        use rusqlite::{params, Connection};
        let source = rendered_database();
        // Use the actual generated Kotlin query fragments in this SQLite test.
        let order = source
            .split("val order = if (history) \"")
            .nth(1)
            .unwrap()
            .split('"')
            .next()
            .unwrap();
        let boundary = source
            .split("\" AND (created_at_epoch_millis, id)")
            .nth(1)
            .unwrap()
            .split('"')
            .next()
            .unwrap();
        let boundary = format!(" AND (created_at_epoch_millis, id){boundary}");
        let db = Connection::open_in_memory().unwrap();
        db.execute_batch("CREATE TABLE snapshot_history(id INTEGER PRIMARY KEY, workspace_key TEXT NOT NULL, created_at_epoch_millis INTEGER NOT NULL, app_data_json TEXT NOT NULL); CREATE INDEX snapshot_history_workspace_time ON snapshot_history(workspace_key, created_at_epoch_millis DESC, id DESC);").unwrap();
        for id in 1..=2048i64 {
            let workspace = if id % 3 == 0 { "active" } else { "other" };
            let time = if id % 7 == 0 { -1 } else { (id * 19) % 13 };
            db.execute(
                "INSERT INTO snapshot_history VALUES(?1, ?2, ?3, ?4)",
                params![id, workspace, time, "payload".repeat(32)],
            )
            .unwrap();
        }
        let select = "SELECT created_at_epoch_millis, id, app_data_json FROM snapshot_history WHERE workspace_key = ?1";
        let all_sql = format!("{select} ORDER BY {order}");
        let expected: Vec<(i64, i64, String)> = db
            .prepare(&all_sql)
            .unwrap()
            .query_map(["active"], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
            .unwrap()
            .map(Result::unwrap)
            .collect();
        for limit in [1, 16, 32] {
            let mut found = Vec::new();
            let mut after: Option<(i64, i64)> = None;
            loop {
                let sql = format!(
                    "{select}{} ORDER BY {order} LIMIT {limit}",
                    if after.is_some() {
                        boundary.as_str()
                    } else {
                        ""
                    }
                );
                let bindings: Vec<rusqlite::types::Value> = match after {
                    Some((time, id)) => vec!["active".to_owned().into(), time.into(), id.into()],
                    None => vec!["active".to_owned().into()],
                };
                let plan: Vec<String> = db
                    .prepare(&format!("EXPLAIN QUERY PLAN {sql}"))
                    .unwrap()
                    .query_map(rusqlite::params_from_iter(&bindings), |r| r.get(3))
                    .unwrap()
                    .map(Result::unwrap)
                    .collect();
                assert!(
                    plan.iter().any(|line| line.contains(
                        "SEARCH snapshot_history USING INDEX snapshot_history_workspace_time"
                    )),
                    "{plan:?}"
                );
                assert!(
                    !plan
                        .iter()
                        .any(|line| line.contains("TEMP B-TREE") || line.starts_with("SCAN")),
                    "{plan:?}"
                );
                let rows: Vec<(i64, i64, String)> = db
                    .prepare(&sql)
                    .unwrap()
                    .query_map(rusqlite::params_from_iter(&bindings), |r| {
                        Ok((r.get(0)?, r.get(1)?, r.get(2)?))
                    })
                    .unwrap()
                    .map(Result::unwrap)
                    .collect();
                if rows.is_empty() {
                    break;
                }
                after = rows.last().map(|(time, id, _)| (*time, *id));
                found.extend(rows);
            }
            assert_eq!(found, expected);
        }
        assert_eq!(expected.len(), 682);
    }

    #[test]
    fn media_read_and_workspace_copy_verifiers_are_unchanged() {
        let base = super::super::kotlin_sources::SOURCES
            .iter()
            .find(|s| s.path == MEDIA)
            .unwrap()
            .contents;
        let source = render(MEDIA, base).unwrap();
        assert_eq!(
            base.split("    fun resolvedAttachmentHashes(").next(),
            source.split("    fun resolvedAttachmentHashes(").next()
        );
        assert_eq!(
            base.split("    fun verifiedAttachmentFile(")
                .nth(1)
                .unwrap(),
            source
                .split("    fun verifiedAttachmentFile(")
                .nth(1)
                .unwrap()
                .strip_suffix(MEDIA_POLICY)
                .unwrap()
        );
    }

    #[test]
    fn maintenance_is_bounded_and_does_not_issue_proof_before_commit() {
        assert!(MAINTENANCE_JOB.contains("initializationJob.join()"));
        assert!(MAINTENANCE_JOB.contains("writeMutex.tryLock()"));
        assert!(MAINTENANCE_JOB.contains("cached?.takeIf { it.first == workspaceKey }?.second"));
        assert!(
            MAINTENANCE_JOB
                .find("val result = compactVerifiedHistoryBatch")
                .unwrap()
                < MAINTENANCE_JOB
                    .find("result.stamp?.let { verifiedWritePreflight")
                    .unwrap()
        );
        assert!(INDEXED_BATCHES.contains("(created_at_epoch_millis, id) < (?, ?)"));
        assert!(!INDEXED_BATCHES.contains("NOT INDEXED"));
    }
}
