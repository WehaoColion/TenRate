//! Read-only physical and logical storage accounting for a real Android emulator.
use std::{fs, path::Path};

const APP_ROOT: &str = "/data/user/0/com.ofairyo.gridtimer";

fn quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}

/// Samples are taken outside the timed edit/save operation. Directory sizes
/// are allocated KiB (inclusive, never sum nested rows); SQLite lengths and
/// payload accounting are logical bytes, so the two cannot be conflated.
pub fn capture(
    root: &Path,
    stage: &str,
    database: &str,
    shell: impl Fn(&str) -> Result<String, String>,
) -> Result<(), String> {
    let disk = shell(&format!("du -k -d 3 {}", quote(APP_ROOT)))?;
    if !disk.lines().any(|line| line.split_whitespace().last() == Some(APP_ROOT)) {
        return Err("Storage evidence missing application directory total".into());
    }
    let mut file_sizes = String::new();
    for suffix in ["", "-wal", "-shm"] {
        let file = format!("{database}{suffix}");
        let path = quote(&file);
        let raw = shell(&format!("if [ -f {path} ]; then stat -c %s {path}; else printf '0\\n'; fi"))?;
        let bytes = raw.trim().parse::<u64>().map_err(|_| format!("Invalid file length: {file}: {raw}"))?;
        file_sizes.push_str(&format!("{file}\t{bytes}\n"));
    }
    let query = "PRAGMA query_only=ON; \
        SELECT 'page_size',page_size FROM pragma_page_size; \
        SELECT 'page_count',page_count FROM pragma_page_count; \
        SELECT 'freelist_count',freelist_count FROM pragma_freelist_count; \
        SELECT 'journal_mode',journal_mode FROM pragma_journal_mode; \
        SELECT 'current_rows',count(*) FROM current_snapshot; \
        SELECT 'current_bytes',coalesce(sum(length(CAST(app_data_json AS BLOB))),0) FROM current_snapshot; \
        SELECT 'history_rows',count(*) FROM snapshot_history; \
        SELECT 'history_bytes',coalesce(sum(length(CAST(app_data_json AS BLOB))),0) FROM snapshot_history; \
        SELECT 'packed_rows',count(*) FROM snapshot_history WHERE storage_encoding=1; \
        SELECT 'quarantine_rows',count(*) FROM snapshot_quarantine; \
        SELECT 'quarantine_bytes',coalesce(sum(length(CAST(app_data_json AS BLOB))),0) FROM snapshot_quarantine;";
    let metrics = shell(&format!("sqlite3 -readonly -batch -bail -cmd '.timeout 8000' {} {}", quote(database), quote(query)))?;
    for key in ["page_size", "page_count", "freelist_count", "journal_mode",
        "current_rows", "current_bytes", "history_rows", "history_bytes",
        "packed_rows", "quarantine_rows", "quarantine_bytes"] {
        if !metrics.lines().any(|line| line.starts_with(&format!("{key}|"))) {
            return Err(format!("Missing actual SQLite storage metric: {key}"));
        }
    }
    let report = format!("stage={stage}\n[sqlite_logical]\n{metrics}\n[database_file_bytes]\n{file_sizes}[allocated_kib_inclusive]\n{disk}");
    fs::write(root.join(format!("storage_{stage}.txt")), &report).map_err(|e| e.to_string())?;
    println!("{report}");
    Ok(())
}
