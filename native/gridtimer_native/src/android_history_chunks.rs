// v3.0.2.64 - Bound and authenticate the Android native history chunk write plan.
//! Lossless, bounded content-addressed building blocks for Android recovery history.
//!
//! This is a codec, not a database migration. Its caller must first verify the
//! existing row envelope, scope every chunk key to the workspace, and publish
//! the manifest and referenced chunks in the same transaction. A stored row's
//! independently authenticated digest remains authoritative. Never remove a
//! legacy payload until the new representation has been read back and verified.
use ring::digest::{digest, Context, SHA256};

pub const MAX_RAW_BYTES: usize = 128 * 1024 * 1024;
const MIN_CHUNK: usize = 256;
const MAX_CHUNK: usize = 2048;
const BOUNDARY_MASK: u64 = 511;
const MAX_CHUNKS: usize = MAX_RAW_BYTES.div_ceil(MIN_CHUNK);

pub type ChunkId = [u8; 32];

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Manifest {
    pub raw_bytes: usize,
    pub raw_digest: ChunkId,
    pub chunks: Vec<ChunkId>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ChunkError {
    Length,
    Missing,
    Digest,
    Storage,
}

pub fn sha256(bytes: &[u8]) -> ChunkId {
    let mut out = [0; 32];
    out.copy_from_slice(digest(&SHA256, bytes).as_ref());
    out
}

// A deterministic gear value, independent of process, platform and Rust's
// randomized hash maps. Hash boundaries, not fixed offsets, allow a small
// insertion to resynchronize with an unchanged following region.
fn gear(byte: u8) -> u64 {
    let mut x = u64::from(byte).wrapping_add(0x9e3779b97f4a7c15);
    x = (x ^ (x >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
    x = (x ^ (x >> 27)).wrapping_mul(0x94d049bb133111eb);
    x ^ (x >> 31)
}

/// The sink must compare an already-present chunk byte-for-byte before
/// accepting it, and must not publish partial manifests on failure.
pub fn encode(
    raw: &[u8],
    mut put: impl FnMut(ChunkId, &[u8]) -> Result<(), ChunkError>,
) -> Result<Manifest, ChunkError> {
    if raw.is_empty() || raw.len() > MAX_RAW_BYTES {
        return Err(ChunkError::Length);
    }
    let mut chunks = Vec::new();
    let mut start = 0;
    let mut hash = 0u64;
    for (i, &byte) in raw.iter().enumerate() {
        hash = hash.wrapping_shl(1).wrapping_add(gear(byte));
        let len = i + 1 - start;
        if len == MAX_CHUNK
            || (len >= MIN_CHUNK && hash & BOUNDARY_MASK == 0)
            || i + 1 == raw.len()
        {
            let part = &raw[start..i + 1];
            let id = sha256(part);
            put(id, part)?;
            chunks.push(id);
            start = i + 1;
            hash = 0;
        }
    }
    Ok(Manifest {
        raw_bytes: raw.len(),
        raw_digest: sha256(raw),
        chunks,
    })
}

/// Verify every fetched chunk and the complete original byte sequence. No
/// prefix is returned on failure. The expected digest comes from the existing
/// authenticated history row, not from the manifest being decoded.
pub fn decode(
    manifest: &Manifest,
    expected: ChunkId,
    mut get: impl FnMut(&ChunkId) -> Result<Option<Vec<u8>>, ChunkError>,
) -> Result<Vec<u8>, ChunkError> {
    if manifest.raw_bytes == 0
        || manifest.raw_bytes > MAX_RAW_BYTES
        || manifest.chunks.is_empty()
        || manifest.chunks.len() > MAX_CHUNKS
        || manifest.chunks.len() > manifest.raw_bytes.div_ceil(MIN_CHUNK)
    {
        return Err(ChunkError::Length);
    }
    if manifest.raw_digest != expected {
        return Err(ChunkError::Digest);
    }
    let mut out = Vec::new();
    out.try_reserve_exact(manifest.raw_bytes).map_err(|_| ChunkError::Storage)?;
    let mut full = Context::new(&SHA256);
    for (i, id) in manifest.chunks.iter().enumerate() {
        let bytes = get(id)?.ok_or(ChunkError::Missing)?;
        if bytes.is_empty()
            || bytes.len() > MAX_CHUNK
            || (i + 1 != manifest.chunks.len() && bytes.len() < MIN_CHUNK)
            || bytes.len() > manifest.raw_bytes - out.len()
        {
            return Err(ChunkError::Length);
        }
        if sha256(&bytes) != *id {
            return Err(ChunkError::Digest);
        }
        full.update(&bytes);
        out.extend_from_slice(&bytes);
    }
    if out.len() != manifest.raw_bytes {
        return Err(ChunkError::Length);
    }
    if full.finish().as_ref() != expected.as_slice() {
        return Err(ChunkError::Digest);
    }
    Ok(out)
}

/// Android's storage adapter receives only a bounded, ordered write plan:
/// little-endian raw length, raw SHA-256, count, then (u16 length, SHA-256)
/// for each chunk. Payload bytes remain in the caller's buffer. No record is
/// authorized to migrate unless its original digest AND envelope verify.
pub fn authenticated_chunk_plan(
    raw: &[u8],
    prefix: &str,
    expected: &str,
    envelope: &str,
    supported: i32,
) -> Option<Vec<u8>> {
    if raw.is_empty() || raw.len() > MAX_RAW_BYTES || envelope.len() != 64
        || crate::android_snapshot_verifier::snapshot_table(prefix, supported) != Some("snapshot_history")
        || !crate::android_snapshot_verifier::verify_snapshot(raw, prefix, expected, envelope, supported)
    {
        return None;
    }
    let mut entries = Vec::new();
    let manifest = encode(raw, |id, part| {
        entries.extend_from_slice(&(part.len() as u16).to_le_bytes());
        entries.extend_from_slice(&id);
        Ok(())
    }).ok()?;
    let mut plan = Vec::with_capacity(40 + entries.len());
    plan.extend_from_slice(&(manifest.raw_bytes as u32).to_le_bytes());
    plan.extend_from_slice(&manifest.raw_digest);
    plan.extend_from_slice(&(manifest.chunks.len() as u32).to_le_bytes());
    plan.extend_from_slice(&entries);
    Some(plan)
}

#[no_mangle]
pub extern "system" fn Java_com_ofairyo_gridtimer_data_NativeSnapshotVerifier_nativeHistoryChunkPlan(
    mut env: jni::JNIEnv,
    _class: jni::objects::JClass,
    raw: jni::objects::JByteArray,
    prefix: jni::objects::JString,
    expected: jni::objects::JString,
    envelope: jni::objects::JString,
    supported: i32,
) -> jni::sys::jbyteArray {
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let length = usize::try_from(env.get_array_length(&raw).ok()?).ok()?;
        if length == 0 || length > MAX_RAW_BYTES { return None; }
        let prefix = env.get_string(&prefix).map(String::from).ok()?;
        let expected = env.get_string(&expected).map(String::from).ok()?;
        let envelope = env.get_string(&envelope).map(String::from).ok()?;
        // Caller owns the immutable input for the duration of the JNI call.
        // Release the VM buffer before allocating the returned plan array.
        let plan = {
            let bytes = unsafe { env.get_array_elements(&raw, jni::objects::ReleaseMode::NoCopyBack) }.ok()?;
            let buffer = unsafe { std::slice::from_raw_parts(bytes.as_ptr().cast(), bytes.len()) };
            authenticated_chunk_plan(buffer, &prefix, &expected, &envelope, supported)?
        };
        env.byte_array_from_slice(&plan).ok().map(|array| array.into_raw())
    })).ok().flatten().unwrap_or(std::ptr::null_mut());
    if env.exception_check().unwrap_or(false) { let _ = env.exception_clear(); }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;
    use std::io::Write;
    use flate2::{write::ZlibEncoder, Compression};

    fn save(
        raw: &[u8],
        pool: &mut BTreeMap<ChunkId, Vec<u8>>,
    ) -> Result<Manifest, ChunkError> {
        encode(raw, |id, bytes| {
            if let Some(old) = pool.get(&id) {
                if old != bytes { return Err(ChunkError::Digest); }
            } else {
                pool.insert(id, bytes.to_vec());
            }
            Ok(())
        })
    }

    fn load(manifest: &Manifest, raw: &[u8], pool: &BTreeMap<ChunkId, Vec<u8>>) -> Result<Vec<u8>, ChunkError> {
        decode(manifest, sha256(raw), |id| Ok(pool.get(id).cloned()))
    }

    fn fixture(round: usize, body: &str) -> Vec<u8> {
        // Vary metadata as in a real saved workspace; the large user body
        // stays unchanged except for a final digit. Keep each old revision.
        format!(r#"{{"schemaVersion":15,"revision":{round},"notes":[{{"id":"a","title":"知识","document":{{"blocks":[{{"text":"{body}{}"}}]}},"updatedAt":{round}}}]}}"#, round % 2).into_bytes()
    }

    #[test]
    fn thousands_of_fixed_length_edits_share_payload_without_losing_any_revision() {
        let mut seed = 7u32;
        let body: String = (0..10_000).map(|_| {
            seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
            char::from_u32(0x4e00 + ((seed >> 8) % 2000)).unwrap()
        }).collect();
        let mut pool = BTreeMap::new();
        let mut manifests = Vec::new();
        let mut old_bytes = 0usize;
        for round in 0..2500 {
            let raw = fixture(round, &body);
            let manifest = save(&raw, &mut pool).unwrap();
            let mut old = ZlibEncoder::new(Vec::new(), Compression::fast());
            old.write_all(&raw).unwrap();
            old_bytes += old.finish().unwrap().len();
            manifests.push(manifest);
        }
        // Every row has its own ordered manifest; count its full hash list
        // and length/digest in addition to all unique chunk payloads and keys.
        let unique: usize = pool.values().map(|x| x.len() + 32).sum();
        let references: usize = manifests.iter().map(|m| 40 + m.chunks.len() * 32).sum();
        let new_bytes = unique + references;
        println!("logical_history_bytes_old_zlib={old_bytes} chunk_payload_and_manifest_bytes={new_bytes} unique_chunks={}", pool.len());
        assert!(new_bytes < old_bytes / 2, "no substantial lossless reduction");
        for (round, manifest) in manifests.iter().enumerate() {
            let raw = fixture(round, &body);
            assert_eq!(load(manifest, &raw, &pool).unwrap(), raw);
        }
    }

    #[test]
    fn rejects_missing_corrupt_reordered_and_forged_history() {
        let raw = (0..6000).map(|x| (x % 251) as u8).collect::<Vec<_>>();
        let mut pool = BTreeMap::new();
        let manifest = save(&raw, &mut pool).unwrap();
        assert_eq!(load(&manifest, &raw, &pool).unwrap(), raw);
        let first = manifest.chunks[0];
        let mut wrong = pool.clone();
        wrong.remove(&first);
        assert_eq!(load(&manifest, &raw, &wrong), Err(ChunkError::Missing));
        wrong = pool.clone();
        wrong.get_mut(&first).unwrap()[0] ^= 1;
        assert_eq!(load(&manifest, &raw, &wrong), Err(ChunkError::Digest));
        let mut reordered = manifest.clone();
        let different = reordered.chunks.iter().position(|id| *id != first).unwrap();
        reordered.chunks.swap(0, different);
        assert!(load(&reordered, &raw, &pool).is_err());
        let mut forged = manifest.clone();
        forged.raw_digest = [0; 32];
        assert_eq!(load(&forged, &raw, &pool), Err(ChunkError::Digest));
        let mut oversized = manifest.clone();
        oversized.raw_bytes = MAX_RAW_BYTES + 1;
        assert_eq!(load(&oversized, &raw, &pool), Err(ChunkError::Length));
        assert_eq!(encode(b"", |_, _| Ok(())), Err(ChunkError::Length));
        assert_eq!(encode(&raw, |_, _| Err(ChunkError::Storage)), Err(ChunkError::Storage));
    }

    #[test]
    fn android_write_plan_covers_exact_original_bytes_and_requires_row_envelope() {
        let raw = fixture(7, &"知识".repeat(1500));
        let prefix = "gridtimer-snapshot-envelope-v1\0snapshot_history\x001\0owner\x0015\x007\x001\x0099\0";
        let hex = |value: ChunkId| value.iter().map(|b| format!("{b:02x}")).collect::<String>();
        let expected = hex(sha256(&raw));
        let envelope = hex(sha256(&[prefix.as_bytes(), &raw].concat()));
        let plan = authenticated_chunk_plan(&raw, prefix, &expected, &envelope, 15).unwrap();
        assert_eq!(u32::from_le_bytes(plan[..4].try_into().unwrap()) as usize, raw.len());
        assert_eq!(&plan[4..36], sha256(&raw).as_slice());
        let count = u32::from_le_bytes(plan[36..40].try_into().unwrap()) as usize;
        assert_eq!(plan.len(), 40 + count * 34);
        let mut offset = 0;
        for entry in plan[40..].chunks_exact(34) {
            let size = u16::from_le_bytes(entry[..2].try_into().unwrap()) as usize;
            assert!((1..=MAX_CHUNK).contains(&size));
            assert_eq!(&entry[2..], sha256(&raw[offset..offset + size]).as_slice());
            offset += size;
        }
        assert_eq!(offset, raw.len());
        assert!(authenticated_chunk_plan(&raw, prefix, &expected, "", 15).is_none());
        assert!(authenticated_chunk_plan(&raw, prefix, &expected, &envelope, 14).is_none());
        assert!(authenticated_chunk_plan(&raw, &prefix.replace("owner", "other"), &expected, &envelope, 15).is_none());
        let mut corrupt = raw.clone();
        corrupt[10] ^= 1;
        assert!(authenticated_chunk_plan(&corrupt, prefix, &expected, &envelope, 15).is_none());
    }

    #[test]
    fn insertion_resynchronizes_unchanged_suffix_and_restores_exact_bytes() {
        let mut seed = 29u32;
        let raw = (0..32768).map(|_| {
            seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
            (seed >> 24) as u8
        }).collect::<Vec<_>>();
        let mut pool = BTreeMap::new();
        let a = save(&raw, &mut pool).unwrap();
        let mut changed = raw.clone();
        changed.splice(100..100, b"extra".iter().copied());
        let b = save(&changed, &mut pool).unwrap();
        let shared = b.chunks.iter().filter(|id| a.chunks.contains(id)).count();
        assert!(shared > b.chunks.len() / 2);
        assert_eq!(load(&a, &raw, &pool).unwrap(), raw);
        assert_eq!(load(&b, &changed, &pool).unwrap(), changed);
    }
}
