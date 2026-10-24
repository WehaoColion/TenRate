// v2.23.2.55 - Restore authenticated current snapshot without eager historical payload scans.
// v2.23.2.54 - Index workspace history in Android 35+ save verification.
// v2.23.2.49 - Fast-path zero/single-row verification without defeating bounded completion-order pipeline.
// v2.22.49 - Stream the pinned SQLite statement through reusable verification buffers.
const REPOSITORY: &str = "com/ofairyo/gridtimer/data/TimerRepository.kt";
const DATABASE: &str = "com/ofairyo/gridtimer/data/AppStateDatabase.kt";
const SCREEN: &str = "com/ofairyo/gridtimer/ui/GridTimerScreen.kt";

fn replace(source: &mut String, old: &str, new: &str) -> Result<(), String> {
    if source.matches(old).count() != 1 {
        return Err(format!("startup stream anchor is not unique: {old}"));
    }
    *source = source.replacen(old, new, 1);
    Ok(())
}

pub fn render(path: &str, base: &str) -> Result<String, String> {
    let mut source = base.to_owned();
    match path {
        SCREEN => {
            replace(&mut source,
                "    val initialized by viewModel.isInitialized.collectAsState()\n    if (!initialized)",
                "    val initialized by viewModel.isInitialized.collectAsState()\n    androidx.activity.compose.ReportDrawnWhen { initialized }\n    if (!initialized)")?;
        }
        REPOSITORY => {
            replace(
                &mut source,
                "val result = stateDatabase.withVerifiedSnapshotReads {",
                "val result = stateDatabase.withVerifiedWorkspaceReads(workspaceKey) {",
            )?;
            replace(&mut source,
                "        var verifiedCurrentAccepted = false\n        stateDatabase.forEachSnapshot(workspaceKey) { stored ->",
                "        var verifiedCurrentAccepted = false\n        stateDatabase.forEachSnapshot(\n            workspaceKey,\n            skipVerifiedHistory = { authoritativeStartup && verifiedCurrentAccepted }\n        ) { stored ->")?;
            replace(&mut source,
                "stateDatabase.verifiedReadHasOnlySealedSnapshots()) {",
                "stateDatabase.verifiedReadHasOnlySealedSnapshots() &&\n                stateDatabase.verifiedReadHasFullHistoryReceipt(workspaceKey)) {")?;
            replace(&mut source,
                "            _isInitialized.value = true\n            logDiagnosticEvent(",
                "            _isInitialized.value = true\n            Log.i(\"StartupLatency\", \"ready elapsedMs=${android.os.SystemClock.elapsedRealtime() - initializationStarted}\")\n            logDiagnosticEvent(")?;
        }
        DATABASE => {
            replace(&mut source,
                "    fun forEachSnapshot(workspaceKey: String, action: (StoredStateSnapshot) -> Unit) {",
                "    fun forEachSnapshot(\n        workspaceKey: String,\n        skipVerifiedHistory: (() -> Boolean)? = null,\n        action: (StoredStateSnapshot) -> Unit\n    ) {")?;
            let start = source
                .find("    fun forEachSnapshot(")
                .ok_or("missing snapshot visitor")?;
            let end = source[start..]
                .find("    // Compatibility for bounded test fixtures")
                .ok_or("missing visitor end")?
                + start;
            let old = source[start..end].to_owned();
            let mut new = old.clone();
            // Do not hash every historical payload before even reaching the
            // authenticated current row; the normal history path still does.
            replace(&mut new,
                "        quarantineDigestMismatches(workspaceKey, includeHistory = true)",
                "        val currentOnlyStartup = skipVerifiedHistory != null &&\n            snapshotReadWindow.get()?.hasCurrentOnlyReceipt(workspaceKey, snapshotWriteStamp()) == true\n        quarantineDigestMismatches(workspaceKey, includeHistory = !currentOnlyStartup)")?;
            // The history query may be rewritten as a cursor-bound visitor by
            // earlier generator stages. Anchor to the uniquely named history
            // projection rather than to the query helper implementation.
            let history_columns = "            HISTORY_TABLE,\n            HISTORY_QUERY_COLUMNS,";
            if new.matches(history_columns).count() != 1 {
                return Err("startup history projection is not unique".into());
            }
            let history_offset = new.find(history_columns).unwrap();
            let history_call = new[..history_offset]
                .rfind("        database.")
                .ok_or("startup history cursor call missing")?;
            let visitor = &new[history_call..history_offset];
            if !visitor.starts_with("        database.visitSnapshotRows(")
                && !visitor.starts_with("        database.querySnapshotRows(")
                && !visitor.starts_with("        database.query(")
            {
                return Err("unexpected startup history query helper".into());
            }
            new.insert_str(history_call,
                "        // The current must decode and be accepted as authoritative.\n        // Unacceptable current rows continue through full quarantine and recovery.\n        if (snapshotReadWindow.get()?.canSkipVerifiedHistory(\n                workspaceKey, snapshotWriteStamp(), skipVerifiedHistory?.invoke() == true\n            ) == true) return\n        if (currentOnlyStartup) quarantineDigestMismatches(workspaceKey, includeHistory = true)\n");
            replace(&mut source, &old, &new)?;
            replace(&mut source,
                "    private val snapshotReadWindow = ThreadLocal<SnapshotVerificationWindow>()",
                &format!("{WORKSPACE_READ}\n    private val snapshotReadWindow = ThreadLocal<SnapshotVerificationWindow>()"))?;
            replace(&mut source,
                "internal class SnapshotVerificationWindow {",
                "internal class SnapshotVerificationWindow {\n    private var nativeHistoryReceipt: Pair<String, Any>? = null\n    private var nativeCurrentReceipt: Pair<String, Any>? = null\n\n    fun recordNativeHistory(workspace: String, stamp: Any) {\n        record(workspace, true, stamp)\n        nativeHistoryReceipt = workspace to stamp\n    }\n\n    fun recordNativeCurrent(workspace: String, stamp: Any) {\n        record(workspace, false, stamp)\n        nativeCurrentReceipt = workspace to stamp\n    }\n\n    fun hasFullHistoryReceipt(workspace: String, stamp: Any): Boolean =\n        nativeHistoryReceipt == (workspace to stamp) && covers(workspace, true, stamp)\n\n    fun hasCurrentOnlyReceipt(workspace: String, stamp: Any): Boolean =\n        nativeCurrentReceipt == (workspace to stamp) && covers(workspace, false, stamp)\n\n    fun canSkipVerifiedHistory(workspace: String, stamp: Any, acceptedCurrent: Boolean): Boolean =\n        acceptedCurrent && (hasFullHistoryReceipt(workspace, stamp) ||\n            nativeCurrentReceipt == (workspace to stamp) && covers(workspace, false, stamp))\n")?;
            source.push_str(NATIVE_BRIDGE);
            source.push_str(VERIFICATION_PIPELINE);
        }
        _ => {}
    }
    Ok(source)
}

const WORKSPACE_READ: &str = r####"    // A current-only receipt must never become a save-preflight receipt.
    fun verifiedReadHasFullHistoryReceipt(workspaceKey: String): Boolean =
        snapshotReadWindow.get()?.hasFullHistoryReceipt(workspaceKey, snapshotWriteStamp()) == true

    // Index-only metadata probe; do not read any history payload on the normal path.
    // Unknown legacy/unsealed or newer-schema history takes the complete scan.
    private fun historyRequiresFullStartupScan(workspaceKey: String): Boolean =
        writableDatabase.rawQuery(
            "SELECT 1 FROM $HISTORY_TABLE WHERE workspace_key = ? AND " +
                "(schema_version IS NULL OR schema_version > ? OR " +
                "sha256 IS NULL OR sha256 = '' OR " +
                "envelope_sha256 IS NULL OR envelope_sha256 = '') LIMIT 1",
            arrayOf(workspaceKey, APP_DATA_SCHEMA_VERSION.toString())
        ).use { it.moveToFirst() }

    fun <T> withVerifiedWorkspaceReads(workspaceKey: String, action: () -> T): T {
        val database = writableDatabase
        // All rows are read on Android's pinned primary connection. The Rust
        // verifier only receives this cursor's bytes, never opens the database.
        // Nested callers retain the existing verification/recovery behavior.
        val freshTransaction = !database.inTransaction()
        return withVerifiedSnapshotReads {
            if (freshTransaction) {
                val before = snapshotWriteStamp()
                val started = android.os.SystemClock.elapsedRealtime()
                var verifiedRows = -1L
                var scope = "full"
                // Only the authenticated current snapshot may activate the fast path.
                // Its decoded/accepted result is checked later in forEachSnapshot.
                val metadataSafe = runCatching { !historyRequiresFullStartupScan(workspaceKey) }
                    .onFailure { android.util.Log.w("StartupLatency", "historyHeader fallback", it) }
                    .getOrDefault(false)
                if (metadataSafe) {
                    val currentRows = runCatching { verifyNativeSnapshotRows(workspaceKey, currentOnly = true) }
                        .onFailure { android.util.Log.w("StartupLatency", "currentSnapshot fallback", it) }
                        .getOrDefault(-1L)
                    val currentStamp = snapshotWriteStamp()
                    if (canUseVerifiedCurrentStartupReceipt(currentRows, metadataSafe, before == currentStamp)) {
                        snapshotReadWindow.get()?.recordNativeCurrent(workspaceKey, currentStamp)
                        verifiedRows = currentRows
                        scope = "current"
                    }
                }
                if (scope == "full") {
                    // No current, malformed/future current, native verifier failure or
                    // unsafe historical header: keep the original complete verification.
                    verifiedRows = runCatching { verifyNativeSnapshotRows(workspaceKey) }
                        .onFailure { android.util.Log.w("StartupLatency", "snapshotStream fallback: ${it.javaClass.simpleName}") }
                        .getOrDefault(-1L)
                    val after = snapshotWriteStamp()
                    if (verifiedRows >= 0 && before == after) {
                        snapshotReadWindow.get()?.recordNativeHistory(workspaceKey, after)
                    }
                }
                android.util.Log.i("StartupLatency",
                    "snapshotStream rows=$verifiedRows elapsedMs=${android.os.SystemClock.elapsedRealtime() - started} scope=$scope")
            }
            action()
        }
    }

    private fun verifyNativeSnapshotRows(workspaceKey: String, currentOnly: Boolean = false): Long {
        if (!NativeSnapshotVerifier.available) return -1L
        val database = writableDatabase
        check(database.inTransaction())
        val tables = if (currentOnly) arrayOf(CURRENT_TABLE) else arrayOf(CURRENT_TABLE, HISTORY_TABLE)
        val expectedRows = tables.sumOf { table ->
            database.rawQuery("SELECT COUNT(*) FROM $table WHERE workspace_key = ?",
                arrayOf(workspaceKey)).use { cursor ->
                check(cursor.moveToFirst())
                cursor.getLong(0)
            }
        }
        val workers = Runtime.getRuntime().availableProcessors().coerceIn(1, 4)
        val nativeNanos = java.util.concurrent.atomic.AtomicLong()
        val buffers = SnapshotBufferPool(workers + 1)
        var payloadBytes = 0L
        val started = android.os.SystemClock.elapsedRealtime()
        val verified = verifySnapshotPipeline(workers, expectedRows) { submit ->
            for (table in tables) {
                if (android.os.Build.VERSION.SDK_INT >= 35) {
                    payloadBytes += visitRawSnapshots(database, table, workspaceKey, buffers, submit, nativeNanos)
                    continue
                }
                // CursorWindow.getBlob(TEXT) includes the internal string terminator.
                // CAST returns the exact UTF-8 payload as BLOB, so SHA-256 continues
                // to cover the persisted JSON bytes without trimming/changing data.
                val columns = (if (table == CURRENT_TABLE) SNAPSHOT_COLUMNS else HISTORY_QUERY_COLUMNS)
                    .map { column -> if (column == "app_data_json")
                        "CAST(app_data_json AS BLOB) AS app_data_json" else column }
                    .toTypedArray()
                visitNativeSnapshotBatches(database, table, columns, workspaceKey) { cursor ->
                    val owner = cursor.getString(cursor.getColumnIndexOrThrow("workspace_key"))
                    val rowId = if (table == CURRENT_TABLE) owner
                        else cursor.getLong(cursor.getColumnIndexOrThrow("id")).toString()
                    val timestamp = if (table == CURRENT_TABLE) "updated_at_epoch_millis"
                        else "created_at_epoch_millis"
                    val prefix = listOf(
                        "gridtimer-snapshot-envelope-v1", table, rowId, owner,
                        cursor.getInt(cursor.getColumnIndexOrThrow("schema_version")).toString(),
                        cursor.getLong(cursor.getColumnIndexOrThrow("revision")).toString(),
                        cursor.getInt(cursor.getColumnIndexOrThrow("item_count")).toString(),
                        cursor.getLong(cursor.getColumnIndexOrThrow(timestamp)).toString()
                    ).joinToString(separator = "\u0000", postfix = "\u0000")
                    check(owner == workspaceKey) { "Snapshot workspace changed during its read" }
                    val raw = cursor.getBlob(cursor.getColumnIndexOrThrow("app_data_json"))
                    val digest = cursor.getString(cursor.getColumnIndexOrThrow("sha256"))
                    val envelope = cursor.getString(cursor.getColumnIndexOrThrow("envelope_sha256"))
                    payloadBytes += raw?.size ?: 0
                    // These immutable values own their bytes; no worker touches
                    // Android's transaction, cursor or mutable receipt window.
                    submit {
                        val began = android.os.Debug.threadCpuTimeNanos()
                        try { NativeSnapshotVerifier.verify(raw, raw?.size ?: 0, prefix, digest, envelope, APP_DATA_SCHEMA_VERSION) }
                        finally { nativeNanos.addAndGet(android.os.Debug.threadCpuTimeNanos() - began) }
                    }
                }
            }
        }
        android.util.Log.i("StartupLatency", "snapshotPipeline workers=$workers bytes=$payloadBytes nativeCpuMs=${nativeNanos.get() / 1_000_000} elapsedMs=${android.os.SystemClock.elapsedRealtime() - started}")
        return verified
    }

    @androidx.annotation.RequiresApi(35)
    private fun visitRawSnapshots(
        database: SQLiteDatabase, table: String, workspaceKey: String,
        buffers: SnapshotBufferPool, submit: (() -> Boolean) -> Unit,
        nativeNanos: java.util.concurrent.atomic.AtomicLong
    ): Long {
        check(database.inTransaction())
        val timestamp = if (table == CURRENT_TABLE) "updated_at_epoch_millis" else "created_at_epoch_millis"
        val identity = if (table == CURRENT_TABLE) "workspace_key" else "id"
        // NOT INDEXED previously scanned other accounts' entire history.
        // Seek this account's indexed history while authenticating every matching row.
        val indexedTable = if (table == HISTORY_TABLE)
            "$HISTORY_TABLE INDEXED BY snapshot_history_workspace_time" else CURRENT_TABLE
        val rowOrder = if (table == HISTORY_TABLE) "created_at_epoch_millis DESC, id DESC" else "workspace_key"
        val sql = "SELECT $identity, workspace_key, schema_version, revision, item_count, $timestamp, " +
            "CAST(app_data_json AS BLOB) AS app_data_json, sha256, envelope_sha256 " +
            "FROM $indexedTable WHERE workspace_key = ? ORDER BY $rowOrder"
        var bytes = 0L
        // This public API uses the same thread's already-pinned transaction.
        // It avoids CursorWindow copies and never opens another SQLite connection.
        database.createRawStatement(sql).use { row ->
            row.bindText(1, workspaceKey)
            while (row.step()) {
                val owner = row.getColumnText(1)
                check(owner == workspaceKey) { "Snapshot workspace changed during its read" }
                val rowId = if (table == CURRENT_TABLE) owner else row.getColumnLong(0).toString()
                val prefix = listOf("gridtimer-snapshot-envelope-v1", table, rowId, owner,
                    row.getColumnInt(2).toString(), row.getColumnLong(3).toString(),
                    row.getColumnInt(4).toString(), row.getColumnLong(5).toString())
                    .joinToString(separator = "\u0000", postfix = "\u0000")
                val digest = row.getColumnText(7)
                val envelope = row.getColumnText(8)
                val length = row.getColumnLength(6)
                bytes += length
                buffers.submit(length,
                    read = { buffer -> row.readColumnBlob(6, buffer, 0, length, 0) },
                    submit = submit,
                    verify = { buffer, size ->
                        val began = android.os.Debug.threadCpuTimeNanos()
                        try { NativeSnapshotVerifier.verify(buffer, size, prefix, digest, envelope, APP_DATA_SCHEMA_VERSION) }
                        finally { nativeNanos.addAndGet(android.os.Debug.threadCpuTimeNanos() - began) }
                    })
            }
        }
        return bytes
    }

    private fun visitNativeSnapshotBatches(
        database: SQLiteDatabase, table: String, columns: Array<String>,
        workspaceKey: String, action: (Cursor) -> Unit
    ) {
        check(database.inTransaction())
        val sizedWindow = android.os.Build.VERSION.SDK_INT >= 28
        val batchSize = if (sizedWindow) 16 else 1
        var afterRow: Long? = null
        do {
            val selection = "workspace_key = ?" + if (afterRow == null) "" else " AND rowid > ?"
            val arguments = if (afterRow == null) arrayOf(workspaceKey)
                else arrayOf(workspaceKey, afterRow.toString())
            var rows = 0
            // The existing workspace/time index would sort the remaining payloads
            // again for every page. Force an advancing primary-key scan instead.
            database.query("$table NOT INDEXED", arrayOf("rowid AS scan_row_id", *columns), selection,
                arguments, null, null, "rowid ASC", batchSize.toString()).use { cursor ->
                if (sizedWindow && cursor is android.database.sqlite.SQLiteCursor) {
                    cursor.window = android.database.CursorWindow("startup-batch", 16L * 1024 * 1024)
                    cursor.setFillWindowForwardOnly(true)
                }
                while (cursor.moveToNext()) {
                    val id = cursor.getLong(cursor.getColumnIndexOrThrow("scan_row_id"))
                    check(afterRow == null || id > afterRow!!) { "Snapshot scan failed to advance" }
                    afterRow = id
                    action(cursor)
                    rows++
                }
            }
        } while (rows == batchSize)
    }
"####;

const VERIFICATION_PIPELINE: &str = r####"
// Only a single fully authenticated current row, in an unchanged DB transaction,
// can avoid eagerly streaming authenticated history. This is *not* a save receipt.
internal fun canUseVerifiedCurrentStartupReceipt(
    currentRows: Long, historyMetadataSafe: Boolean, unchanged: Boolean
): Boolean = currentRows == 1L && historyMetadataSafe && unchanged


// A buffer stays exclusively owned by its verifier until that call returns.
// Reuse removes one large Java allocation per history row; the explicit length
// excludes bytes left over from a previous, larger snapshot.
internal const val MAX_RETAINED_SNAPSHOT_BYTES = 4 * 1024 * 1024

internal class SnapshotBufferPool(slots: Int) {
    private val free = java.util.concurrent.ArrayBlockingQueue<ByteArray>(slots).apply {
        repeat(slots) { add(ByteArray(0)) }
    }

    private fun recycle(buffer: ByteArray): ByteArray =
        if (buffer.size <= MAX_RETAINED_SNAPSHOT_BYTES) buffer else ByteArray(0)

    fun submit(
        length: Int, read: (ByteArray) -> Int,
        submit: (() -> Boolean) -> Unit, verify: (ByteArray, Int) -> Boolean
    ) {
        require(length > 0)
        var buffer = free.take()
        var handedOff = false
        try {
            if (buffer.size < length) {
                val capacity = minOf(Int.MAX_VALUE.toLong(),
                    maxOf(length.toLong(), buffer.size.toLong() * 2)).toInt()
                buffer = ByteArray(capacity)
            }
            check(read(buffer) == length) { "Snapshot payload was not read completely" }
            val owned = buffer
            submit {
                try { verify(owned, length) }
                finally { free.add(recycle(owned)) }
            }
            handedOff = true
        } finally {
            if (!handedOff) free.add(recycle(buffer))
        }
    }
}

// At most two jobs per worker are pending. Cursor reading stays on the primary
// transaction thread; only immutable snapshot bytes cross to verification workers.
internal fun verifySnapshotPipeline(
    workers: Int, expectedRows: Long, produce: ((() -> Boolean) -> Unit) -> Unit
): Long {
    require(workers in 1..4)
    require(expectedRows >= 0)
    // Fresh workspaces have no history. Avoid starting a thread pool for
    // zero or one authenticated row; all existing fail-closed guards remain.
    if (expectedRows <= 1L) {
        var verified = 0L
        produce { verify ->
            check(verified < expectedRows) { "Snapshot scan visited unexpected rows" }
            check(verify()) { "Snapshot verification rejected a row" }
            verified++
        }
        check(verified == expectedRows) { "Snapshot scan did not cover the complete transaction" }
        return verified
    }
    val executor = java.util.concurrent.Executors.newFixedThreadPool(workers)
    // Completion order matters: one enormous snapshot must not prevent the
    // producer from feeding idle workers after smaller rows finish.
    val completions = java.util.concurrent.ExecutorCompletionService<Boolean>(executor)
    var pending = 0
    var verified = 0L
    fun awaitOne() {
        check(completions.take().get()) { "Snapshot verification rejected a row" }
        pending--
        verified++
    }
    try {
        produce { verify ->
            while (pending >= workers * 2) awaitOne()
            completions.submit(java.util.concurrent.Callable { verify() })
            pending++
        }
        while (pending > 0) awaitOne()
        check(verified == expectedRows) { "Snapshot scan did not cover the complete transaction" }
        return verified
    } finally {
        // A rejected or interrupted scan cannot outlive its transaction. Always
        // join already submitted workers before recovery or the receipt can run.
        executor.shutdown()
        var interrupted = false
        while (!executor.isTerminated) {
            try { executor.awaitTermination(1, java.util.concurrent.TimeUnit.SECONDS) }
            catch (_: InterruptedException) { interrupted = true }
        }
        if (interrupted) Thread.currentThread().interrupt()
    }
}
"####;

const NATIVE_BRIDGE: &str = r####"

private object NativeSnapshotVerifier {
    val available: Boolean = runCatching {
        System.loadLibrary("gridtimer_native")
        true
    }.onFailure { android.util.Log.w("StartupLatency", "nativeLoad fallback: ${it.javaClass.simpleName}") }
        .getOrDefault(false)

    fun verify(raw: ByteArray?, length: Int, prefix: String, digest: String?, envelope: String?, schema: Int): Boolean =
        available && raw != null && digest != null && envelope != null &&
            runCatching { nativeVerify(raw, length, prefix, digest, envelope, schema) }
                .onFailure { android.util.Log.w("StartupLatency", "nativeVerify fallback: ${it.javaClass.simpleName}") }
                .getOrDefault(false)

    @JvmStatic private external fun nativeVerify(
        raw: ByteArray, length: Int, prefix: String, digest: String, envelope: String, schema: Int
    ): Boolean
}
"####;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn api35_snapshot_scan_is_indexed_and_visits_every_workspace_row() {
        assert!(WORKSPACE_READ.contains("INDEXED BY snapshot_history_workspace_time"));
        assert!(WORKSPACE_READ.contains("created_at_epoch_millis DESC, id DESC"));
        assert!(!WORKSPACE_READ.contains(
            "FROM $table NOT INDEXED WHERE workspace_key = ? ORDER BY rowid"
        ));
        let database = rusqlite::Connection::open_in_memory().unwrap();
        database.execute_batch(
            "CREATE TABLE snapshot_history (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                workspace_key TEXT NOT NULL,
                created_at_epoch_millis INTEGER NOT NULL
            );
            CREATE INDEX snapshot_history_workspace_time
                ON snapshot_history(workspace_key, created_at_epoch_millis DESC, id DESC);"
        ).unwrap();
        for id in 0..1500 {
            let workspace = if id % 73 == 0 { "target" } else { "unrelated" };
            database.execute(
                "INSERT INTO snapshot_history(workspace_key, created_at_epoch_millis) VALUES (?1, ?2)",
                rusqlite::params![workspace, (id % 7) as i64]
            ).unwrap();
        }
        let old = "SELECT id FROM snapshot_history NOT INDEXED WHERE workspace_key = ?1 ORDER BY rowid";
        let fast = "SELECT id FROM snapshot_history INDEXED BY snapshot_history_workspace_time
            WHERE workspace_key = ?1 ORDER BY created_at_epoch_millis DESC, id DESC";
        let ids = |sql: &str| -> std::collections::BTreeSet<i64> {
            let mut statement = database.prepare(sql).unwrap();
            let mapped = statement.query_map(["target"], |row| row.get::<_, i64>(0)).unwrap();
            mapped.collect::<Result<_, _>>().unwrap()
        };
        let baseline = ids(old);
        assert_eq!(baseline.len(), 21);
        assert_eq!(baseline, ids(fast));

        let mut explain = database.prepare(&format!("EXPLAIN QUERY PLAN {fast}")).unwrap();
        let details: Vec<String> = explain.query_map(["target"], |row| row.get(3))
            .unwrap().collect::<Result<_, _>>().unwrap();
        assert!(details.iter().any(|line| line.contains(
            "USING COVERING INDEX snapshot_history_workspace_time")), "{details:?}");
        assert!(!details.iter().any(|line| line.contains("SCAN snapshot_history")
            || line.contains("USE TEMP B-TREE")), "{details:?}");
    }


    #[test]
    fn bounded_buffers_and_completion_order_are_used_by_generated_android() {
        assert!(WORKSPACE_READ.contains("SnapshotBufferPool(workers + 1)"));
        assert!(VERIFICATION_PIPELINE.contains("ExecutorCompletionService<Boolean>(executor)"));
        assert!(VERIFICATION_PIPELINE.contains("while (pending >= workers * 2) awaitOne()"));
        assert!(VERIFICATION_PIPELINE.contains("while (pending > 0) awaitOne()"));
        assert!(!VERIFICATION_PIPELINE.contains("pending.removeFirst().get()"));
        assert!(VERIFICATION_PIPELINE.contains("free.add(recycle(owned))"));
        assert!(VERIFICATION_PIPELINE.contains("MAX_RETAINED_SNAPSHOT_BYTES"));
    }

    #[test]
    fn receipt_cannot_escape_the_existing_transaction_or_replace_recovery_selection() {
        let base = super::super::android_local_startup::tests::rendered(DATABASE);
        let base = super::super::android_timer_action::render(DATABASE, &base).unwrap();
        let source = render(DATABASE, &base).unwrap();
        assert!(source.contains("val freshTransaction = !database.inTransaction()"));
        assert!(source.contains("if (freshTransaction)"));
        assert!(source.contains("if (verifiedRows >= 0 && before == after)"));
        assert!(source.contains("verifyNativeSnapshotRows(workspaceKey, currentOnly = true)"));
        assert!(source.contains("historyRequiresFullStartupScan(workspaceKey)"));
        assert!(source.contains("nativeCurrentReceipt == (workspace to stamp)"));
        assert!(source.contains("val currentOnlyStartup = skipVerifiedHistory != null"));
        assert!(source.contains("quarantineDigestMismatches(workspaceKey, includeHistory = !currentOnlyStartup)"));
        assert!(source.contains("if (currentOnlyStartup) quarantineDigestMismatches(workspaceKey, includeHistory = true)"));
        assert!(source.contains("nativeHistoryReceipt == (workspace to stamp)"));
        assert!(source.contains("fun verifiedReadHasFullHistoryReceipt("));
        assert!(source.contains("val tables = if (currentOnly) arrayOf(CURRENT_TABLE)"));
        assert!(source.contains("CAST(app_data_json AS BLOB) AS app_data_json"));
        assert!(source.contains("finally {\n                snapshotReadWindow.remove()"));
        let base = super::super::android_local_startup::tests::rendered(REPOSITORY);
        let source = render(REPOSITORY, &base).unwrap();
        assert!(source
            .contains("skipVerifiedHistory = { authoritativeStartup && verifiedCurrentAccepted }"));
        assert!(source.contains(
            "if (candidate.source == \"sqlite-current\") verifiedCurrentAccepted = true"
        ));
        assert!(source.contains(
            "futureCurrentSurface ?: databaseCurrent ?: jsonPrimaryAuthority ?: recovery"
        ));
        // A trusted current can skip restoring historical payloads, but must
        // never inherit the full-history stamp used by save preflight.
        assert!(source.contains("stateDatabase.verifiedReadHasFullHistoryReceipt(workspaceKey)"));
        assert!(source.contains("stateDatabase.verifiedReadHasOnlySealedSnapshots() &&"));
        let database = super::super::android_local_startup::tests::rendered(DATABASE);
        let database = render(DATABASE, &database).unwrap();
        let database = super::super::android_note_persist_preflight::render(DATABASE, &database).unwrap();
        assert!(database.contains("verifyNativeSnapshotRows(workspaceKey).also"));
        assert!(!database.contains("verifyNativeSnapshotRows(workspaceKey, currentOnly = true).also"));
    }
}


pub const PIPELINE_TEST_PATH: &str = "com/ofairyo/gridtimer/data/SnapshotPipelineSchedulingTest.kt";
pub const PIPELINE_TEST_CONTENTS: &str = r####"package com.ofairyo.gridtimer.data

import org.junit.Assert.*
import org.junit.Test
import java.util.concurrent.Callable
import java.util.concurrent.CountDownLatch
import java.util.concurrent.Executors
import java.util.concurrent.TimeUnit

class SnapshotPipelineSchedulingTest {
    @Test fun hugeFirstRowDoesNotBlockCompletedSmallRowsFromFeedingWorkers() {
        val slowRow = CountDownLatch(1)
        val fifthStarted = CountDownLatch(1)
        val driver = Executors.newSingleThreadExecutor()
        try {
            val result = driver.submit(Callable {
                verifySnapshotPipeline(2, 5) { submit ->
                    submit { slowRow.await(8, TimeUnit.SECONDS) }
                    repeat(3) { submit { true } }
                    submit { fifthStarted.countDown(); true }
                }
            })
            // FIFO waiting deadlocks the fifth submission behind the first row;
            // completion-order waiting permits the fifth row to run immediately.
            assertTrue("A large row blocked faster verification results",
                fifthStarted.await(5, TimeUnit.SECONDS))
            slowRow.countDown()
            assertEquals(5L, result.get(5, TimeUnit.SECONDS))
        } finally {
            slowRow.countDown()
            driver.shutdownNow()
        }
    }

    @Test fun rejectedAndMissingRowsStillPreventAStartupReceipt() {
        assertTrue(runCatching {
            verifySnapshotPipeline(2, 1) { submit -> submit { false } }
        }.isFailure)
        assertTrue(runCatching {
            verifySnapshotPipeline(2, 2) { submit -> submit { true } }
        }.isFailure)
    }

    @Test fun retainedBuffersStayBoundedWithoutTruncatingVerificationInputs() {
        assertEquals(4 * 1024 * 1024, MAX_RETAINED_SNAPSHOT_BYTES)
        val pool = SnapshotBufferPool(1)
        val length = MAX_RETAINED_SNAPSHOT_BYTES + 1
        var accepted = false
        pool.submit(length, read = { bytes ->
            bytes[0] = 42
            bytes[length - 1] = 7
            length
        }, submit = { verify ->
            accepted = verify()
        }, verify = { bytes, count ->
            count == length && bytes[0] == 42.toByte() && bytes[length - 1] == 7.toByte()
        })
        assertTrue(accepted)
    }
}
"####;

pub const TEST_PATH: &str = "com/ofairyo/gridtimer/data/StartupSnapshotPipelineTest.kt";
pub const TEST_CONTENTS: &str = r####"package com.ofairyo.gridtimer.data

import org.junit.Assert.*
import org.junit.Test

class StartupSnapshotPipelineTest {
    @Test fun currentOnlyReceiptRequiresOneVerifiedRowStableStampAndSafeHistoryHeaders() {
        assertTrue(canUseVerifiedCurrentStartupReceipt(1, true, true))
        for (rows in listOf(-1L, 0L, 2L, 100L)) {
            assertFalse(canUseVerifiedCurrentStartupReceipt(rows, true, true))
        }
        assertFalse(canUseVerifiedCurrentStartupReceipt(1, false, true))
        assertFalse(canUseVerifiedCurrentStartupReceipt(1, true, false))
        assertFalse(canUseVerifiedCurrentStartupReceipt(1, false, false))
    }

    @Test fun emptyWorkspaceDoesNotScheduleAnyVerifier() {
        assertEquals(0L, verifySnapshotPipeline(4, 0) { _ -> })
    }

    @Test fun singleCurrentRowIsVerifiedExactlyOnce() {
        var calls = 0
        assertEquals(1L, verifySnapshotPipeline(4, 1) { submit ->
            submit { calls++; true }
        })
        assertEquals(1, calls)
    }

    @Test fun singleRowFailuresAndCountMismatchRemainFailClosed() {
        assertTrue(runCatching { verifySnapshotPipeline(4, 1) { _ -> } }.isFailure)
        assertTrue(runCatching {
            verifySnapshotPipeline(4, 1) { submit -> submit { false } }
        }.isFailure)
        assertTrue(runCatching {
            verifySnapshotPipeline(4, 0) { submit -> submit { true } }
        }.isFailure)
    }

    @Test fun multiRowVerifierChecksEveryResultAndCount() {
        val calls = java.util.concurrent.atomic.AtomicInteger()
        assertEquals(3L, verifySnapshotPipeline(2, 3) { submit ->
            repeat(3) { submit { calls.incrementAndGet(); true } }
        })
        assertEquals(3, calls.get())
        assertTrue(runCatching {
            verifySnapshotPipeline(2, 2) { submit ->
                submit { true }
                submit { false }
            }
        }.isFailure)
    }
}
"####;

