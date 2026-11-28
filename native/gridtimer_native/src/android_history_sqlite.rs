// v3.0.2.63 - Verify every one of 100,000 migrated history rows after physical reclaim and reopen.
//! Host-side executable SQLite migration contract for Android history chunks.
//! Not an Android schema upgrade: the Android SQLite adapter must implement
//! this transaction and all backup, privacy and recovery readers before use.
use crate::android_history_chunks::{self as chunks, ChunkError, ChunkId, Manifest};
use crate::android_snapshot_codec;
use crate::android_snapshot_verifier::verify_snapshot;
use rusqlite::{params, Connection, OptionalExtension, Transaction};

const MARKER: &str = "!gridtimer-history-chunks-v1";
const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS history_chunk_payload(
    workspace_key TEXT NOT NULL,
    digest BLOB NOT NULL CHECK(length(digest)=32),
    payload BLOB NOT NULL CHECK(length(payload) BETWEEN 1 AND 2048),
    PRIMARY KEY(workspace_key,digest)
) WITHOUT ROWID;
CREATE TABLE IF NOT EXISTS history_chunk_manifest(
    workspace_key TEXT NOT NULL,
    history_id INTEGER NOT NULL,
    raw_bytes INTEGER NOT NULL CHECK(raw_bytes BETWEEN 1 AND 134217728),
    raw_digest BLOB NOT NULL CHECK(length(raw_digest)=32),
    ordered_ids BLOB NOT NULL CHECK(length(ordered_ids)>0 AND length(ordered_ids)%32=0),
    PRIMARY KEY(workspace_key,history_id)
) WITHOUT ROWID;";

#[derive(Debug)]
pub enum MigrationError {
    Sql(rusqlite::Error),
    Integrity,
    Codec(ChunkError),
}
impl From<rusqlite::Error> for MigrationError {
    fn from(value: rusqlite::Error) -> Self { Self::Sql(value) }
}
impl From<ChunkError> for MigrationError {
    fn from(value: ChunkError) -> Self { Self::Codec(value) }
}

pub fn create_contract_schema(db: &Connection) -> Result<(), MigrationError> {
    db.execute_batch(SCHEMA)?;
    Ok(())
}

struct Row {
    owner: String,
    id: i64,
    schema: i32,
    revision: i64,
    count: i64,
    time: i64,
    stored: String,
    digest: String,
    envelope: String,
}

impl Row {
    fn prefix(&self) -> String {
        format!("gridtimer-snapshot-envelope-v1\0snapshot_history\0{}\0{}\0{}\0{}\0{}\0{}\0",
            self.id, self.owner, self.schema, self.revision, self.count, self.time)
    }
    fn validate(&self, raw: &[u8], supported: i32) -> Result<(), MigrationError> {
        if self.envelope.is_empty() || !verify_snapshot(raw, &self.prefix(), &self.digest, &self.envelope, supported) {
            return Err(MigrationError::Integrity);
        }
        Ok(())
    }
}
fn row(db: &Connection, owner: &str, id: i64) -> Result<Row, MigrationError> {
    Ok(db.query_row(
        "SELECT workspace_key,id,schema_version,revision,item_count,created_at_epoch_millis,app_data_json,sha256,envelope_sha256
         FROM snapshot_history WHERE workspace_key=?1 AND id=?2", params![owner,id],
        |r| Ok(Row { owner:r.get(0)?, id:r.get(1)?, schema:r.get(2)?, revision:r.get(3)?,
            count:r.get(4)?, time:r.get(5)?, stored:r.get(6)?, digest:r.get(7)?, envelope:r.get(8)? }))?)
}

fn read_chunks(db: &Connection, row: &Row, supported: i32) -> Result<Vec<u8>, MigrationError> {
    let (raw_bytes, digest, ids): (i64, Vec<u8>, Vec<u8>) = db.query_row(
        "SELECT raw_bytes,raw_digest,ordered_ids FROM history_chunk_manifest
         WHERE workspace_key=?1 AND history_id=?2 AND length(ordered_ids) <= 16777216",
        params![row.owner,row.id], |r| Ok((r.get(0)?,r.get(1)?,r.get(2)?)))?;
    let raw_bytes = usize::try_from(raw_bytes).map_err(|_| MigrationError::Integrity)?;
    if ids.is_empty() || ids.len()%32 != 0 || raw_bytes == 0 || raw_bytes > chunks::MAX_RAW_BYTES ||
        ids.len()/32 > raw_bytes.div_ceil(256) {
        return Err(MigrationError::Integrity);
    }
    let digest: ChunkId = digest.try_into().map_err(|_| MigrationError::Integrity)?;
    let manifest = Manifest { raw_bytes, raw_digest:digest,
        chunks:ids.chunks_exact(32).map(|part| part.try_into().expect("32 byte chunk id")).collect() };
    let mut expected = [0u8;32];
    if row.digest.len()!=64 || !row.digest.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(MigrationError::Integrity);
    }
    for (i, pair) in row.digest.as_bytes().chunks_exact(2).enumerate() {
        let hex=std::str::from_utf8(pair).map_err(|_| MigrationError::Integrity)?;
        expected[i]=u8::from_str_radix(hex,16).map_err(|_| MigrationError::Integrity)?;
    }
    let mut select = db.prepare("SELECT payload FROM history_chunk_payload WHERE workspace_key=?1 AND digest=?2")?;
    let raw = chunks::decode(&manifest, expected, |id| {
        select.query_row(params![row.owner,id.as_slice()], |r| r.get(0))
            .optional().map_err(|_| ChunkError::Storage)
    })?;
    row.validate(&raw, supported)?;
    Ok(raw)
}

pub fn read_history(db: &Connection, owner: &str, id: i64, supported: i32) -> Result<Vec<u8>, MigrationError> {
    let r = row(db,owner,id)?;
    if r.stored == MARKER {
        return read_chunks(db,&r,supported);
    }
    let raw = if android_snapshot_codec::is_packed(r.stored.as_bytes()) {
        android_snapshot_codec::decode(&r.stored,&[],supported).ok_or(MigrationError::Integrity)?.into_bytes()
    } else { r.stored.as_bytes().to_vec() };
    r.validate(&raw,supported)?;
    Ok(raw)
}

/// Caller owns a transaction. Never replace a legacy row until the manifest,
/// all chunk bytes, the independent row digest and the row envelope round-trip.
/// Failure must roll back this transaction, not commit the staged records.
fn migrate_inside(tx: &Transaction<'_>, owner: &str, id: i64, supported: i32,
    before_publish: impl FnOnce() -> Result<(), MigrationError>) -> Result<(), MigrationError> {
    let r = row(tx,owner,id)?;
    let raw = read_history(tx,owner,id,supported)?;
    if r.stored == MARKER { return Ok(()); }
    let mut insert = tx.prepare("INSERT OR IGNORE INTO history_chunk_payload(workspace_key,digest,payload) VALUES(?1,?2,?3)")?;
    let mut check = tx.prepare("SELECT payload FROM history_chunk_payload WHERE workspace_key=?1 AND digest=?2")?;
    let manifest = chunks::encode(&raw, |digest, bytes| {
        insert.execute(params![owner,digest.as_slice(),bytes]).map_err(|_| ChunkError::Storage)?;
        let existing: Vec<u8> = check.query_row(params![owner,digest.as_slice()],|r|r.get(0)).map_err(|_|ChunkError::Storage)?;
        if existing != bytes { return Err(ChunkError::Digest); }
        Ok(())
    })?;
    let ids: Vec<u8> = manifest.chunks.iter().flat_map(|id|id.iter().copied()).collect();
    let raw_bytes = i64::try_from(manifest.raw_bytes).map_err(|_| MigrationError::Integrity)?;
    tx.execute("INSERT INTO history_chunk_manifest(workspace_key,history_id,raw_bytes,raw_digest,ordered_ids) VALUES(?1,?2,?3,?4,?5)",
        params![owner,id,raw_bytes,manifest.raw_digest.as_slice(),ids])?;
    if read_chunks(tx,&r,supported)? != raw { return Err(MigrationError::Integrity); }
    before_publish()?;
    if tx.execute("UPDATE snapshot_history SET app_data_json=?1 WHERE workspace_key=?2 AND id=?3 AND app_data_json=?4",
        params![MARKER,owner,id,r.stored])? != 1 { return Err(MigrationError::Integrity); }
    Ok(())
}

/// Exactly one row per transaction; a crash before commit leaves the old
/// representation readable, after commit the checked chunk representation.
pub fn migrate_history(db: &mut Connection, owner: &str, id: i64, supported: i32) -> Result<(), MigrationError> {
    let tx = db.transaction()?;
    migrate_inside(&tx,owner,id,supported,||Ok(()))?;
    tx.commit()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    const SUPPORTED: i32 = 15;
    const KEY: [u8;32] = [7;32];
    fn schema(db: &Connection) {
        db.execute_batch("CREATE TABLE snapshot_history(
            id INTEGER PRIMARY KEY,workspace_key TEXT NOT NULL,schema_version INTEGER NOT NULL,
            revision INTEGER NOT NULL,item_count INTEGER NOT NULL,created_at_epoch_millis INTEGER NOT NULL,
            app_data_json TEXT NOT NULL,sha256 TEXT NOT NULL,envelope_sha256 TEXT NOT NULL);").unwrap();
        create_contract_schema(db).unwrap();
    }
    fn hex(id: ChunkId) -> String { id.iter().map(|b|format!("{b:02x}")).collect() }
    fn raw(round: i64, body: &str) -> Vec<u8> {
        format!(r#"{{"schemaVersion":15,"revision":{round},"notes":[{{"title":"知识","text":"{body}{}"}}]}}"#,round%2).into_bytes()
    }
    fn seed(db: &Connection, owner: &str, id:i64, raw:&[u8]) {
        let prefix = format!("gridtimer-snapshot-envelope-v1\0snapshot_history\0{id}\0{owner}\x0015\0{id}\x001\0{id}\0");
        let digest = hex(chunks::sha256(raw));
        let envelope = hex(chunks::sha256(&[prefix.as_bytes(),raw].concat()));
        let packed = android_snapshot_codec::seal(raw,&prefix,&digest,&envelope,SUPPORTED,&KEY).unwrap();
        db.execute("INSERT INTO snapshot_history VALUES(?1,?2,15,?1,1,?1,?3,?4,?5)",
            params![id,owner,packed,digest,envelope]).unwrap();
    }

    #[test]
    fn failed_migration_keeps_original_bytes_and_account_isolation() {
        let mut db = Connection::open_in_memory().unwrap();
        schema(&db);
        let raw=raw(1,&"正文".repeat(1000));
        seed(&db,"a",1,&raw);
        seed(&db,"b",2,&raw);
        let before=row(&db,"a",1).unwrap().stored;
        {
            let tx=db.transaction().unwrap();
            let failed=migrate_inside(&tx,"a",1,SUPPORTED,||Err(MigrationError::Integrity));
            assert!(failed.is_err());
            // Model a process failure: no commit, SQLite rolls back.
        }
        assert_eq!(row(&db,"a",1).unwrap().stored,before);
        assert_eq!(db.query_row("SELECT count(*) FROM history_chunk_payload",[],|r|r.get::<_,i64>(0)).unwrap(),0);
        assert_eq!(db.query_row("SELECT count(*) FROM history_chunk_manifest",[],|r|r.get::<_,i64>(0)).unwrap(),0);
        migrate_history(&mut db,"a",1,SUPPORTED).unwrap();
        migrate_history(&mut db,"b",2,SUPPORTED).unwrap();
        migrate_history(&mut db,"a",1,SUPPORTED).unwrap(); // idempotent
        assert_eq!(read_history(&db,"a",1,SUPPORTED).unwrap(),raw);
        assert_eq!(read_history(&db,"b",2,SUPPORTED).unwrap(),raw);
        db.execute("DELETE FROM history_chunk_payload WHERE workspace_key='a'",[]).unwrap();
        assert!(read_history(&db,"a",1,SUPPORTED).is_err());
        assert_eq!(read_history(&db,"b",2,SUPPORTED).unwrap(),raw);
        // A changed envelope must fail even with valid deduplicated chunks.
        db.execute("UPDATE snapshot_history SET envelope_sha256='' WHERE id=2",[]).unwrap();
        assert!(read_history(&db,"b",2,SUPPORTED).is_err());
    }

    fn physical_reclaim(rows: i64) {
        let path=std::env::temp_dir().join(format!("tenrate-chunks-{}-{}.db",std::process::id(),
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
        let mut db=Connection::open(&path).unwrap();
        schema(&db);
        let mut seed_state=7u32;
        let body:String=(0..10000).map(|_| {
            seed_state=seed_state.wrapping_mul(1664525).wrapping_add(1013904223);
            char::from_u32(0x4e00+((seed_state>>8)%2000)).unwrap()
        }).collect();
        // Bounded seeding transactions; each subsequent migration remains
        // a separate durable transaction, using the same full 10,000-character body.
        for first in (1..=rows).step_by(1000) {
            let last=(first+999).min(rows);
            let tx=db.transaction().unwrap();
            for i in first..=last { seed(&tx,"guest",i,&raw(i,&body)); }
            tx.commit().unwrap();
            if last % 10000 == 0 || last == rows {
                println!("seeded_history_rows={last}/{rows}");
            }
        }
        db.execute_batch("VACUUM").unwrap();
        let before=std::fs::metadata(&path).unwrap().len();
        for i in 1..=rows {
            migrate_history(&mut db,"guest",i,SUPPORTED).unwrap();
            if i % 10000 == 0 || i == rows {
                println!("migrated_history_rows={i}/{rows}");
            }
        }
        let count:i64=db.query_row("SELECT count(*) FROM snapshot_history",[],|r|r.get(0)).unwrap();
        assert_eq!(count,rows);
        let migrated:i64=db.query_row("SELECT count(*) FROM snapshot_history WHERE workspace_key='guest' AND app_data_json=?1",[MARKER],|r|r.get(0)).unwrap();
        assert_eq!(migrated,rows);
        db.execute_batch("VACUUM").unwrap();
        let after=std::fs::metadata(&path).unwrap().len();
        println!("sqlite_physical_before={before} sqlite_physical_after={after} retained_history={count}");
        assert!(after < before/2,"physical space not reclaimed");
        drop(db);
        let db=Connection::open(&path).unwrap();
        for i in 1..=rows {
            assert_eq!(read_history(&db,"guest",i,SUPPORTED).unwrap(),raw(i,&body),"history id {i}");
            if i % 10000 == 0 || i == rows {
                println!("restored_history_rows={i}/{rows}");
            }
        }
        assert_eq!(db.query_row("SELECT count(*) FROM snapshot_history",[],|r|r.get::<_,i64>(0)).unwrap(),rows);
        assert_eq!(db.query_row("PRAGMA quick_check",[],|r|r.get::<_,String>(0)).unwrap(),"ok");
        drop(db);
        std::fs::remove_file(path).unwrap();
        println!("verified_history_rows={rows}");
    }

    #[test]
    fn physical_sqlite_reclaim_retains_all_2500_verified_history_rows() {
        physical_reclaim(2500);
    }

    // Dedicated required CI job, so the regular release build still runs
    // the fast 2500-row regression without duplicating the extreme workload.
    #[test]
    #[ignore = "mandatory dedicated 100000-row GitHub job"]
    fn physical_sqlite_reclaim_retains_all_100000_verified_history_rows() {
        physical_reclaim(100_000);
    }
}
