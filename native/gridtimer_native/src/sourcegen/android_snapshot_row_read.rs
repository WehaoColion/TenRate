// v2.23.2.36 - Read large snapshot payloads outside Android's bounded CursorWindow.
const DATABASE: &str = "com/ofairyo/gridtimer/data/AppStateDatabase.kt";

fn replace(source: &mut String, old: &str, new: &str) -> Result<(), String> {
    if source.matches(old).count() != 1 {
        return Err(format!("snapshot row read anchor is not unique: {old}"));
    }
    *source = source.replacen(old, new, 1);
    Ok(())
}

pub fn render(path: &str, base: &str) -> Result<String, String> {
    if path != DATABASE {
        return Ok(base.to_owned());
    }
    let mut source = base.to_owned();
    // Every normal projection, including privacy and recovery/backup readers,
    // must avoid placing a whole workspace JSON value in a CursorWindow.
    for receiver in ["database", "readableDatabase", "backup"] {
        source = source.replace(
            &format!("{receiver}.query("),
            &format!("{receiver}.querySnapshotRows("),
        );
    }
    replace(
        &mut source,
        "            query(table, columns, rowSelection,",
        "            querySnapshotRows(table, columns, rowSelection,",
    )?;
    replace(
        &mut source,
        r####"        readableDatabase.rawQuery(
            """
            SELECT id, workspace_key, schema_version, revision, item_count, app_data_json,
                observed_sha256 AS sha256,
                source_timestamp_epoch_millis AS created_at_epoch_millis
            FROM $QUARANTINE_TABLE
            WHERE workspace_key = ? AND source_table = ?
            ORDER BY quarantined_at_epoch_millis DESC, id DESC
            """.trimIndent(),
            arrayOf(workspaceKey, CURRENT_TABLE)
        ).use { cursor ->"####,
        r####"        readableDatabase.querySnapshotRows(
            QUARANTINE_TABLE,
            arrayOf("id", "workspace_key", "schema_version", "revision", "item_count", "app_data_json",
                "observed_sha256 AS sha256", "source_timestamp_epoch_millis AS created_at_epoch_millis"),
            "workspace_key = ? AND source_table = ?", arrayOf(workspaceKey, CURRENT_TABLE),
            null, null, "quarantined_at_epoch_millis DESC, id DESC"
        ).use { cursor ->"####,
    )?;
    replace(
        &mut source,
        r####"                        val roundTrip = database.rawQuery("SELECT app_data_json FROM $HISTORY_TABLE WHERE id = ?", arrayOf(id.toString())).use {"####,
        r####"                        val roundTrip = database.querySnapshotRows(HISTORY_TABLE, arrayOf("app_data_json"),
                            "id = ?", arrayOf(id.toString()), null, null, null, "1").use {"####,
    )?;
    let union_start = source
        .find("            backup.rawQuery(\n                \"\"\"\n                SELECT '$CURRENT_TABLE' AS source_table")
        .ok_or("missing backup envelope union")?;
    let union_end = union_start
        + source[union_start..]
            .find("            backup.querySnapshotRows(\n                QUARANTINE_TABLE,")
            .ok_or("missing backup envelope union end")?;
    let mut envelope = source[union_start..union_end].to_owned();
    replace(
        &mut envelope,
        r####"            backup.rawQuery(
                """
                SELECT '$CURRENT_TABLE' AS source_table, workspace_key AS source_row_id,
                    workspace_key, schema_version, revision, item_count, app_data_json,
                    envelope_sha256, updated_at_epoch_millis AS source_timestamp
                FROM $CURRENT_TABLE
                UNION ALL
                SELECT '$HISTORY_TABLE' AS source_table, CAST(id AS TEXT) AS source_row_id,
                    workspace_key, schema_version, revision, item_count, app_data_json,
                    envelope_sha256, created_at_epoch_millis AS source_timestamp
                FROM $HISTORY_TABLE
                """.trimIndent(),
                null
            ).use { cursor ->"####,
        r####"            listOf(CURRENT_TABLE, HISTORY_TABLE).forEach { sourceTable ->
                val sourceRow = if (sourceTable == CURRENT_TABLE) "workspace_key" else "CAST(id AS TEXT)"
                val timestamp = if (sourceTable == CURRENT_TABLE) "updated_at_epoch_millis" else "created_at_epoch_millis"
                backup.querySnapshotRows(sourceTable,
                    arrayOf("'$sourceTable' AS source_table", "$sourceRow AS source_row_id",
                        "workspace_key", "schema_version", "revision", "item_count", "app_data_json",
                        "envelope_sha256", "$timestamp AS source_timestamp"),
                    null, null, null, null, null
                ).use { cursor ->"####,
    )?;
    // Close the per-table iteration after the original envelope verification.
    envelope.push_str("            }\n");
    source.replace_range(union_start..union_end, &envelope);
    source.push_str(ROW_CURSOR);
    source.push_str(CHUNKS);
    Ok(source)
}

const ROW_CURSOR: &str = r####"

// Query only small metadata through CursorWindow. The unchanged TEXT/BLOB value
// is fetched by byte slices on the same pinned connection when a caller needs it.
// This covers current/history, quarantine, privacy rewriting and backup checking.
private fun android.database.sqlite.SQLiteDatabase.querySnapshotRows(
    table: String,
    columns: Array<String>,
    selection: String?,
    selectionArgs: Array<String>?,
    groupBy: String?,
    having: String?,
    orderBy: String?,
    limit: String? = null
): android.database.Cursor {
    val payloadIndices = columns.indices.filter { index ->
        val expression = columns[index].trim()
        expression.equals("app_data_json", ignoreCase = true) ||
            expression.equals("CAST(app_data_json AS BLOB) AS app_data_json", ignoreCase = true)
    }
    if (payloadIndices.isEmpty()) return query(table, columns, selection, selectionArgs,
        groupBy, having, orderBy, limit)
    require(payloadIndices.size == 1 && groupBy == null && having == null) {
        "Snapshot payload queries must preserve individual rows."
    }
    val payloadIndex = payloadIndices.single()
    val blob = columns[payloadIndex].trim().startsWith("CAST(", ignoreCase = true)
    val metadataColumns = columns.copyOf().also { it[payloadIndex] = "NULL AS app_data_json" }
    // Read-only backup files are private completed copies and have no writer.
    // Active databases pin the metadata row and all its byte slices until close.
    val ownsTransaction = !isReadOnly && !inTransaction()
    if (ownsTransaction) beginTransactionNonExclusive()
    try {
        val metadata = query(table, arrayOf(*metadataColumns, "rowid AS snapshot_payload_row_id"),
            selection, selectionArgs, null, null, orderBy, limit)
        return SnapshotPayloadCursor(this, table, metadata, columns.size, payloadIndex, blob, ownsTransaction)
    } catch (failure: Throwable) {
        if (ownsTransaction) endTransaction()
        throw failure
    }
}

private class SnapshotPayloadCursor(
    private val database: android.database.sqlite.SQLiteDatabase,
    private val table: String,
    metadata: android.database.Cursor,
    private val visibleColumns: Int,
    private val payloadIndex: Int,
    private val blob: Boolean,
    private val ownsTransaction: Boolean
) : android.database.CursorWrapper(metadata) {
    private var loadedPosition = Int.MIN_VALUE
    private var payloadLength: Long? = null
    private var payloadRowId = 0L
    private var payloadBytes: ByteArray? = null
    private var payloadText: String? = null
    private var closedHere = false

    private fun inspectPayload() {
        if (loadedPosition == position) return
        check(position >= 0 && position < count) { "Snapshot cursor is not on a row." }
        payloadRowId = super.getLong(visibleColumns)
        payloadLength = database.rawQuery(
            "SELECT length(CAST(app_data_json AS BLOB)) FROM $table WHERE rowid = ?",
            arrayOf(payloadRowId.toString())
        ).use { cursor ->
            check(cursor.moveToFirst()) { "Snapshot row disappeared during its pinned read." }
            if (cursor.isNull(0)) null else cursor.getLong(0)
        }
        payloadBytes = null
        payloadText = null
        loadedPosition = position
    }

    private fun bytes(): ByteArray? {
        inspectPayload()
        val size = payloadLength ?: return null
        payloadBytes?.let { return it }
        return readSnapshotPayloadChunks(size) { offset, count ->
            database.rawQuery(
                "SELECT substr(CAST(app_data_json AS BLOB), ?, ?) FROM $table WHERE rowid = ?",
                arrayOf((offset + 1L).toString(), count.toString(), payloadRowId.toString())
            ).use { cursor ->
                check(cursor.moveToFirst() && !cursor.isNull(0)) {
                    "Snapshot payload disappeared during its pinned read."
                }
                cursor.getBlob(0)
            }
        }.also {
            payloadBytes = it
            payloadText = null
        }
    }

    override fun getString(columnIndex: Int): String? {
        if (columnIndex != payloadIndex) return super.getString(columnIndex)
        inspectPayload()
        payloadText?.let { return it }
        return bytes()?.toString(Charsets.UTF_8)?.also {
            payloadText = it
            payloadBytes = null
        }
    }

    override fun getBlob(columnIndex: Int): ByteArray? =
        if (columnIndex == payloadIndex) bytes() else super.getBlob(columnIndex)

    override fun isNull(columnIndex: Int): Boolean {
        if (columnIndex != payloadIndex) return super.isNull(columnIndex)
        inspectPayload()
        return payloadLength == null
    }

    override fun getType(columnIndex: Int): Int {
        if (columnIndex != payloadIndex) return super.getType(columnIndex)
        return if (isNull(columnIndex)) android.database.Cursor.FIELD_TYPE_NULL
            else if (blob) android.database.Cursor.FIELD_TYPE_BLOB else android.database.Cursor.FIELD_TYPE_STRING
    }

    override fun getColumnCount(): Int = visibleColumns
    override fun getColumnNames(): Array<String> = super.getColumnNames().copyOfRange(0, visibleColumns)
    override fun getColumnIndex(columnName: String): Int =
        super.getColumnIndex(columnName).takeIf { it in 0 until visibleColumns } ?: -1
    override fun getColumnIndexOrThrow(columnName: String): Int = getColumnIndex(columnName).also {
        require(it >= 0) { "Unknown snapshot column $columnName" }
    }

    override fun copyStringToBuffer(columnIndex: Int, buffer: android.database.CharArrayBuffer) {
        if (columnIndex != payloadIndex) {
            super.copyStringToBuffer(columnIndex, buffer)
            return
        }
        val text = getString(columnIndex).orEmpty()
        if (buffer.data == null || buffer.data.size < text.length) buffer.data = CharArray(text.length)
        text.toCharArray(buffer.data, 0, 0, text.length)
        buffer.sizeCopied = text.length
    }

    override fun close() {
        if (closedHere) return
        closedHere = true
        payloadBytes = null
        payloadText = null
        try {
            super.close()
            if (ownsTransaction) database.setTransactionSuccessful()
        } finally {
            if (ownsTransaction) database.endTransaction()
        }
    }
}
"####;

// Android-free production domain contract: byte offsets must never become
// character offsets or silently accept partial/oversized/missing chunks.
pub const CHUNKS: &str = r####"

internal const val SNAPSHOT_PAYLOAD_CHUNK_BYTES = 128 * 1024

internal fun readSnapshotPayloadChunks(
    byteCount: Long,
    readChunk: (offset: Int, count: Int) -> ByteArray
): ByteArray {
    require(byteCount in 0..Int.MAX_VALUE.toLong()) { "Snapshot payload cannot be represented in memory." }
    val result = ByteArray(byteCount.toInt())
    var offset = 0
    while (offset < result.size) {
        val expected = minOf(SNAPSHOT_PAYLOAD_CHUNK_BYTES, result.size - offset)
        val chunk = readChunk(offset, expected)
        check(chunk.size == expected) { "Snapshot payload chunk did not match its declared byte length." }
        chunk.copyInto(result, destinationOffset = offset)
        offset += expected
    }
    return result
}
"####;

pub const TEST_PATH: &str = "com/ofairyo/gridtimer/data/SnapshotPayloadChunksTest.kt";
pub const TEST_CONTENTS: &str = r####"package com.ofairyo.gridtimer.data

import org.junit.Assert.*
import org.junit.Test

class SnapshotPayloadChunksTest {
    @Test fun largeUnicodeWorkspaceRoundTripsAcrossByteBoundaries() {
        val text = "{\"notes\":[\"" + "中😀文é".repeat(300_000) + "\"]}"
        val bytes = text.toByteArray(Charsets.UTF_8)
        assertTrue(bytes.size > 2 * 1024 * 1024)
        var next = 0
        var calls = 0
        val actual = readSnapshotPayloadChunks(bytes.size.toLong()) { offset, count ->
            assertEquals(next, offset)
            assertTrue(count in 1..SNAPSHOT_PAYLOAD_CHUNK_BYTES)
            next += count
            calls++
            bytes.copyOfRange(offset, offset + count)
        }
        assertTrue(calls > 16)
        assertEquals(bytes.size, next)
        assertArrayEquals(bytes, actual)
        assertEquals(text, actual.toString(Charsets.UTF_8))
    }

    @Test fun emptyAndExactBoundariesDoNotRequestExtraBytes() {
        for (size in listOf(0, 1, SNAPSHOT_PAYLOAD_CHUNK_BYTES - 1,
                SNAPSHOT_PAYLOAD_CHUNK_BYTES, SNAPSHOT_PAYLOAD_CHUNK_BYTES + 1)) {
            val bytes = ByteArray(size) { (it % 251).toByte() }
            var received = 0
            val result = readSnapshotPayloadChunks(size.toLong()) { offset, count ->
                assertEquals(received, offset)
                received += count
                bytes.copyOfRange(offset, offset + count)
            }
            assertArrayEquals(bytes, result)
            assertEquals(size, received)
        }
    }

    @Test fun shortOversizedAndMissingChunksCannotProduceASuccessfulPayload() {
        for (adjustment in listOf(-1, 1, -64)) {
            try {
                readSnapshotPayloadChunks(64L) { _, count -> ByteArray(count + adjustment) }
                fail("An incomplete or oversized payload must fail")
            } catch (_: IllegalStateException) { }
        }
        try {
            readSnapshotPayloadChunks(1L) { _, _ -> error("row disappeared") }
            fail("A missing row must fail")
        } catch (_: IllegalStateException) { }
    }

    @Test fun invalidLengthsFailBeforeReadingAndEarlierRowsCannotLeak() {
        for (size in listOf(-1L, Int.MAX_VALUE.toLong() + 1)) {
            var read = false
            try {
                readSnapshotPayloadChunks(size) { _, _ -> read = true; ByteArray(0) }
                fail("Unrepresentable length must fail")
            } catch (_: IllegalArgumentException) { assertFalse(read) }
        }
        var firstRead = false
        var secondRead = false
        val first = readSnapshotPayloadChunks(3L) { _, _ ->
            assertFalse(firstRead); firstRead = true; byteArrayOf(1, 2, 3)
        }
        val second = readSnapshotPayloadChunks(1L) { _, _ ->
            assertFalse(secondRead); secondRead = true; byteArrayOf(9)
        }
        assertArrayEquals(byteArrayOf(1, 2, 3), first)
        assertArrayEquals(byteArrayOf(9), second)
    }
}
"####;

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::{params, Connection};

    fn production_sql(prefix: &str) -> String {
        let start = ROW_CURSOR.find(prefix).expect("production payload SQL");
        let sql = &ROW_CURSOR[start..];
        let end = sql.find('"').expect("SQL string end");
        sql[..end].replace("$table", "snapshots")
    }

    #[test]
    fn sqlite_blob_slices_preserve_large_unicode_payload_and_row_identity() {
        let database = Connection::open_in_memory().unwrap();
        database
            .execute_batch(
                "CREATE TABLE snapshots (workspace_key TEXT UNIQUE, app_data_json TEXT);",
            )
            .unwrap();
        let raw = format!("{{\"notes\":[\"{}\"]}}", "中😀文é".repeat(300_000));
        assert!(raw.len() > 2 * 1024 * 1024);
        database
            .execute(
                "INSERT INTO snapshots (workspace_key, app_data_json) VALUES (?, ?)",
                params!["current", raw],
            )
            .unwrap();
        database
            .execute(
                "INSERT INTO snapshots (workspace_key, app_data_json) VALUES (?, ?)",
                params!["other", "must not be returned"],
            )
            .unwrap();
        let row_id: i64 = database
            .query_row(
                "SELECT rowid FROM snapshots WHERE workspace_key = ?",
                ["current"],
                |row| row.get(0),
            )
            .unwrap();
        let length_sql = production_sql("SELECT length(CAST(app_data_json AS BLOB))");
        let chunk_sql = production_sql("SELECT substr(CAST(app_data_json AS BLOB), ?, ?)");
        let byte_count: i64 = database
            .query_row(&length_sql, [row_id], |row| row.get(0))
            .unwrap();
        let byte_count = usize::try_from(byte_count).unwrap();
        assert_eq!(byte_count, raw.len());
        let mut result = Vec::with_capacity(byte_count);
        while result.len() < byte_count {
            let expected = (128 * 1024).min(byte_count - result.len());
            let chunk: Vec<u8> = database
                .query_row(
                    &chunk_sql,
                    params![result.len() as i64 + 1, expected as i64, row_id],
                    |row| row.get(0),
                )
                .unwrap();
            assert_eq!(chunk.len(), expected);
            result.extend_from_slice(&chunk);
        }
        assert_eq!(result, raw.as_bytes());
        let after: String = database
            .query_row(
                "SELECT app_data_json FROM snapshots WHERE rowid = ?",
                [row_id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(after, raw);
    }

    #[test]
    fn sqlite_payload_lengths_distinguish_null_empty_and_missing_rows() {
        let database = Connection::open_in_memory().unwrap();
        database
            .execute_batch(
                "CREATE TABLE snapshots (app_data_json TEXT);\n\
                 INSERT INTO snapshots VALUES (NULL), ('');",
            )
            .unwrap();
        let length_sql = production_sql("SELECT length(CAST(app_data_json AS BLOB))");
        let null: Option<i64> = database
            .query_row(&length_sql, [1], |row| row.get(0))
            .unwrap();
        let empty: Option<i64> = database
            .query_row(&length_sql, [2], |row| row.get(0))
            .unwrap();
        assert_eq!(null, None);
        assert_eq!(empty, Some(0));
        assert!(matches!(
            database.query_row(&length_sql, [3], |row| row.get::<_, Option<i64>>(0)),
            Err(rusqlite::Error::QueryReturnedNoRows)
        ));
    }
}
