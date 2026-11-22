// v3.0.2.10 - Seal and recover Android history with bounded decoding.
use crate::android_snapshot_verifier::verify_snapshot;
use base64::{engine::general_purpose::STANDARD, Engine};
use flate2::{write::ZlibEncoder, Compression, Decompress, FlushDecompress, Status};
use jni::objects::{JByteArray, JClass, JString, ReleaseMode};
use jni::sys::{jboolean, jstring, JNI_FALSE, JNI_TRUE};
use jni::JNIEnv;
use ring::hmac;
use std::io::Write;

const MARKER: &[u8] = b"!gridtimer-history-v1:";
const DOMAIN: &[u8] = b"gridtimer-history-authenticated-content-v1\0";
const MAX_RAW: usize = 128 * 1024 * 1024;
const MAX_COMPRESSED: usize = MAX_RAW + 1024 * 1024;
const MAX_PREFIX: usize = 16 * 1024;
const HEADER_LEN: usize = 20;
const INFLATE_CHUNK: usize = 32 * 1024;
const TAG_LEN: usize = 32;
const MAX_BINARY: usize = HEADER_LEN + MAX_PREFIX + 128 + MAX_COMPRESSED + TAG_LEN;
const MAX_ENCODED: usize = MAX_BINARY.div_ceil(3) * 4;

pub fn is_packed(raw: &[u8]) -> bool {
    raw.starts_with(MARKER)
}

fn history_prefix(prefix: &str, supported: i32) -> bool {
    prefix.len() <= MAX_PREFIX
        && supported >= 0
        && crate::android_snapshot_verifier::snapshot_table(prefix, supported)
            == Some("snapshot_history")
}

fn valid_digest(value: &str) -> bool {
    value.len() == 64 && value.as_bytes().iter().all(u8::is_ascii_hexdigit)
}

fn tag(bytes: &[u8], key: &[u8]) -> hmac::Tag {
    let mut context = hmac::Context::with_key(&hmac::Key::new(hmac::HMAC_SHA256, key));
    context.update(DOMAIN);
    context.update(bytes);
    context.sign()
}

struct Packed {
    // Domain and decoded payload share one allocation, so authentication can
    // borrow the complete message instead of copying every retained snapshot.
    authenticated_bytes: Vec<u8>,
    raw_len: usize,
    sealed_supported: i32,
    prefix_end: usize,
    digest_end: usize,
    envelope_end: usize,
    payload_end: usize,
}

impl Packed {
    fn parse(encoded: &[u8]) -> Option<Self> {
        let encoded = encoded.strip_prefix(MARKER)?;
        if encoded.len() > MAX_ENCODED {
            return None;
        }
        // Decode whole Base64 quanta covering the header before allocating for the frame.
        const GROUPS: usize = HEADER_LEN.div_ceil(3);
        let mut header = [0; GROUPS * 3];
        STANDARD
            .decode_slice(encoded.get(..GROUPS * 4)?, &mut header)
            .ok()?;
        let raw_len = u32::from_le_bytes(header[0..4].try_into().ok()?) as usize;
        let sealed_supported = i32::from_le_bytes(header[4..8].try_into().ok()?);
        let prefix_len = u32::from_le_bytes(header[8..12].try_into().ok()?) as usize;
        let digest_len = u16::from_le_bytes(header[12..14].try_into().ok()?) as usize;
        let envelope_len = u16::from_le_bytes(header[14..16].try_into().ok()?) as usize;
        let payload_len = u32::from_le_bytes(header[16..20].try_into().ok()?) as usize;
        if raw_len == 0
            || raw_len > MAX_RAW
            || sealed_supported < 0
            || prefix_len == 0
            || prefix_len > MAX_PREFIX
            || digest_len != 64
            || !matches!(envelope_len, 0 | 64)
            || payload_len == 0
            || payload_len > MAX_COMPRESSED
        {
            return None;
        }
        let prefix_end = HEADER_LEN + prefix_len;
        let digest_end = prefix_end + digest_len;
        let envelope_end = digest_end + envelope_len;
        let payload_end = envelope_end + payload_len;
        let frame_len = payload_end + TAG_LEN;
        if encoded.len() != base64::encoded_len(frame_len, true)? {
            return None;
        }
        let mut authenticated_bytes = Vec::new();
        authenticated_bytes
            .try_reserve_exact(DOMAIN.len() + base64::decoded_len_estimate(encoded.len()))
            .ok()?;
        authenticated_bytes.extend_from_slice(DOMAIN);
        STANDARD
            .decode_vec(encoded, &mut authenticated_bytes)
            .ok()?;
        if authenticated_bytes.len() != DOMAIN.len() + frame_len {
            return None;
        }
        let packed = Self {
            authenticated_bytes,
            raw_len,
            sealed_supported,
            prefix_end,
            digest_end,
            envelope_end,
            payload_end,
        };
        if !history_prefix(packed.prefix()?, sealed_supported)
            || !valid_digest(packed.expected()?)
            || !(packed.envelope()?.is_empty() || valid_digest(packed.envelope()?))
        {
            return None;
        }
        Some(packed)
    }

    fn bytes(&self) -> &[u8] {
        &self.authenticated_bytes[DOMAIN.len()..]
    }

    fn prefix(&self) -> Option<&str> {
        std::str::from_utf8(&self.bytes()[HEADER_LEN..self.prefix_end]).ok()
    }

    fn expected(&self) -> Option<&str> {
        std::str::from_utf8(&self.bytes()[self.prefix_end..self.digest_end]).ok()
    }

    fn envelope(&self) -> Option<&str> {
        std::str::from_utf8(&self.bytes()[self.digest_end..self.envelope_end]).ok()
    }

    fn authenticated(&self, key: &[u8]) -> bool {
        if key.len() != 32 {
            return false;
        }
        // ring performs the authentication comparison in constant time. The domain
        // is part of the authenticated message, including on the verification side.
        hmac::verify(
            &hmac::Key::new(hmac::HMAC_SHA256, key),
            &self.authenticated_bytes[..DOMAIN.len() + self.payload_end],
            &self.bytes()[self.payload_end..],
        )
        .is_ok()
    }

    fn matches_row(&self, prefix: &str, expected: &str, envelope: &str) -> bool {
        self.prefix() == Some(prefix)
            && self.expected() == Some(expected)
            && self.envelope() == Some(envelope)
    }

    /// Deliver only bytes produced by one complete zlib stream of the declared length.
    fn inflate_chunks(&self, mut accept: impl FnMut(&[u8]) -> Option<()>) -> Option<()> {
        let payload = &self.bytes()[self.envelope_end..self.payload_end];
        let mut decoder = Decompress::new(true);
        let mut chunk = [0; INFLATE_CHUNK];
        loop {
            let input = decoder.total_in() as usize;
            let output = decoder.total_out() as usize;
            // One extra byte distinguishes an overlong stream from its final checksum.
            let room = chunk.len().min(self.raw_len - output + 1);
            let status = decoder
                .decompress(&payload[input..], &mut chunk[..room], FlushDecompress::None)
                .ok()?;
            let consumed = decoder.total_in() as usize;
            let produced = decoder.total_out() as usize;
            if produced > self.raw_len {
                return None;
            }
            if produced > output {
                accept(&chunk[..produced - output])?;
            }
            if status == Status::StreamEnd {
                return (consumed == payload.len() && produced == self.raw_len).then_some(());
            }
            if input == consumed && output == produced {
                return None;
            }
        }
    }

    fn decode_verified(&self, supported: i32) -> Option<String> {
        if self.sealed_supported > supported || supported < 0 {
            return None;
        }
        let mut raw = Vec::new();
        self.inflate_chunks(|chunk| {
            let needed = raw.len() + chunk.len();
            if needed > raw.capacity() {
                let capacity = needed.max(raw.capacity() * 2).min(self.raw_len);
                raw.try_reserve_exact(capacity - raw.len()).ok()?;
            }
            raw.extend_from_slice(chunk);
            Some(())
        })?;
        if !verify_snapshot(
            &raw,
            self.prefix()?,
            self.expected()?,
            self.envelope()?,
            supported,
        ) {
            return None;
        }
        String::from_utf8(raw).ok()
    }
}

/// Only verified history is packed. Current rows remain plain JSON. A successful
/// seal binds the exact original bytes, all row metadata and schema support floor.
pub fn seal(
    raw: &[u8],
    prefix: &str,
    expected: &str,
    envelope: &str,
    supported: i32,
    key: &[u8],
) -> Option<String> {
    if key.len() != 32
        || raw.is_empty()
        || raw.len() > MAX_RAW
        || !history_prefix(prefix, supported)
        || !valid_digest(expected)
        || !(envelope.is_empty() || valid_digest(envelope))
        || !verify_snapshot(raw, prefix, expected, envelope, supported)
    {
        return None;
    }
    let payload_start = HEADER_LEN + prefix.len() + expected.len() + envelope.len();
    let mut bytes = Vec::with_capacity(payload_start);
    bytes.extend_from_slice(&(raw.len() as u32).to_le_bytes());
    bytes.extend_from_slice(&supported.to_le_bytes());
    bytes.extend_from_slice(&(prefix.len() as u32).to_le_bytes());
    bytes.extend_from_slice(&(expected.len() as u16).to_le_bytes());
    bytes.extend_from_slice(&(envelope.len() as u16).to_le_bytes());
    bytes.extend_from_slice(&0u32.to_le_bytes());
    bytes.extend_from_slice(prefix.as_bytes());
    bytes.extend_from_slice(expected.as_bytes());
    bytes.extend_from_slice(envelope.as_bytes());
    let mut encoder = ZlibEncoder::new(bytes, Compression::fast());
    encoder.write_all(raw).ok()?;
    let mut bytes = encoder.finish().ok()?;
    let payload_len = bytes.len() - payload_start;
    if payload_len > MAX_COMPRESSED {
        return None;
    }
    bytes[16..20].copy_from_slice(&(payload_len as u32).to_le_bytes());
    bytes.extend_from_slice(tag(&bytes, key).as_ref());
    let mut encoded = String::new();
    encoded
        .try_reserve_exact(MARKER.len() + base64::encoded_len(bytes.len(), true)?)
        .ok()?;
    encoded.push_str(std::str::from_utf8(MARKER).ok()?);
    STANDARD.encode_string(bytes, &mut encoded);
    // Check the final stored representation against the already verified original.
    let packed = Packed::parse(encoded.as_bytes())?;
    if packed.sealed_supported != supported
        || packed.raw_len != raw.len()
        || !packed.matches_row(prefix, expected, envelope)
        || !packed.authenticated(key)
    {
        return None;
    }
    let mut remaining = raw;
    packed.inflate_chunks(|chunk| {
        remaining = remaining.strip_prefix(chunk)?;
        Some(())
    })?;
    remaining.is_empty().then_some(encoded)
}

/// Every startup authenticates actual compressed bytes. A missing key merely
/// disables this shortcut; it never strands a snapshot that passes original checks.
pub fn verify(
    packed: &[u8],
    prefix: &str,
    expected: &str,
    envelope: &str,
    supported: i32,
    key: &[u8],
) -> bool {
    let Some(packed) = Packed::parse(packed) else {
        return false;
    };
    if packed.sealed_supported > supported || !packed.matches_row(prefix, expected, envelope) {
        return false;
    }
    packed.authenticated(key) || packed.decode_verified(supported).is_some()
}

/// Recovery always validates the decompressed original, even with a valid receipt.
/// This preserves original payload/envelope checks and independently checks codec IO.
pub fn decode(packed: &str, _key: &[u8], supported: i32) -> Option<String> {
    Packed::parse(packed.as_bytes())?.decode_verified(supported)
}

fn verify_range(
    buffer: &[u8],
    length: i32,
    prefix: &str,
    expected: &str,
    envelope: &str,
    supported: i32,
    key: &[u8],
) -> bool {
    usize::try_from(length)
        .ok()
        .and_then(|length| buffer.get(..length))
        .is_some_and(|packed| verify(packed, prefix, expected, envelope, supported, key))
}

fn clear_jni_exception(env: &mut JNIEnv<'_>) {
    if env.exception_check().unwrap_or(false) {
        let _ = env.exception_clear();
    }
}

#[no_mangle]
pub extern "system" fn Java_com_ofairyo_gridtimer_data_NativeSnapshotVerifier_nativeSeal(
    mut env: JNIEnv,
    _class: JClass,
    raw: JString,
    prefix: JString,
    expected: JString,
    envelope: JString,
    supported: i32,
    key: JByteArray,
) -> jstring {
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let raw = env.get_string(&raw).map(String::from).ok()?;
        let prefix = env.get_string(&prefix).map(String::from).ok()?;
        let expected = env.get_string(&expected).map(String::from).ok()?;
        let envelope = env.get_string(&envelope).map(String::from).ok()?;
        let key = env.convert_byte_array(&key).ok()?;
        let packed = seal(
            raw.as_bytes(),
            &prefix,
            &expected,
            &envelope,
            supported,
            &key,
        )?;
        env.new_string(packed).ok().map(JString::into_raw)
    }))
    .ok()
    .flatten()
    .unwrap_or(std::ptr::null_mut());
    clear_jni_exception(&mut env);
    result
}

#[no_mangle]
pub extern "system" fn Java_com_ofairyo_gridtimer_data_NativeSnapshotVerifier_nativeDecode(
    mut env: JNIEnv,
    _class: JClass,
    packed: JString,
    supported: i32,
    key: JByteArray,
) -> jstring {
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let packed = env.get_string(&packed).map(String::from).ok()?;
        let key = env.convert_byte_array(&key).ok()?;
        env.new_string(decode(&packed, &key, supported)?)
            .ok()
            .map(JString::into_raw)
    }))
    .ok()
    .flatten()
    .unwrap_or(std::ptr::null_mut());
    clear_jni_exception(&mut env);
    result
}

#[no_mangle]
pub extern "system" fn Java_com_ofairyo_gridtimer_data_NativeSnapshotVerifier_nativeVerifyPacked(
    mut env: JNIEnv,
    _class: JClass,
    payload: JByteArray,
    length: i32,
    prefix: JString,
    expected: JString,
    envelope: JString,
    supported: i32,
    key: JByteArray,
) -> jboolean {
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let prefix = env.get_string(&prefix).map(String::from).ok()?;
        let expected = env.get_string(&expected).map(String::from).ok()?;
        let envelope = env.get_string(&envelope).map(String::from).ok()?;
        let key = env.convert_byte_array(&key).ok()?;
        // SAFETY: callers keep this buffer exclusively owned until return. The
        // array guard releases the VM copy before their pool can recycle it.
        let bytes = unsafe { env.get_array_elements(&payload, ReleaseMode::NoCopyBack) }.ok()?;
        let buffer = unsafe { std::slice::from_raw_parts(bytes.as_ptr().cast(), bytes.len()) };
        Some(verify_range(
            buffer, length, &prefix, &expected, &envelope, supported, &key,
        ))
    }))
    .ok()
    .flatten()
    .unwrap_or(false);
    clear_jni_exception(&mut env);
    if result {
        JNI_TRUE
    } else {
        JNI_FALSE
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ring::digest::{digest, SHA256};

    const KEY: [u8; 32] = [7; 32];

    fn hex(bytes: &[u8]) -> String {
        bytes.iter().map(|byte| format!("{byte:02x}")).collect()
    }

    fn metadata(raw: &[u8], schema: i32) -> (String, String, String) {
        let prefix = format!(
            "gridtimer-snapshot-envelope-v1\0snapshot_history\x001\0owner\0{schema}\x007\x002\x0099\0"
        );
        let expected = hex(digest(&SHA256, raw).as_ref());
        let envelope = hex(digest(&SHA256, &[prefix.as_bytes(), raw].concat()).as_ref());
        (prefix, expected, envelope)
    }

    fn encoded(bytes: &[u8]) -> String {
        format!(
            "{}{}",
            std::str::from_utf8(MARKER).unwrap(),
            STANDARD.encode(bytes)
        )
    }

    fn fixture(raw: &[u8]) -> (String, String, String, String) {
        let (prefix, expected, envelope) = metadata(raw, 15);
        let packed = seal(raw, &prefix, &expected, &envelope, 15, &KEY).unwrap();
        (packed, prefix, expected, envelope)
    }

    #[test]
    fn history_written_by_3_0_2_9_recovers_after_key_loss() {
        // Produced by the 3.0.2.9 writer for this row and KEY.
        let stored = concat!(
            "!gridtimer-history-v1:HwAAAA8AAABCAAAAQABAACcAAABncmlkdGltZXItc25hcHNob3Qt",
            "ZW52ZWxvcGUtdjEAc25hcHNob3RfaGlzdG9yeQAxAG93bmVyADE1ADcAMgA5OQAyNmZiODhh",
            "OWFlNWYzNTRhZGU1ZTM5MWMxYzJlY2UwNTk1MzM2MmY4NDEwNDIzMmQ4OTZkYjQwY2RkMzE4",
            "Y2EwMjUyMDcyNjkzZWZhZGYzNmQyYWJjNjFlM2U1YTcwYWRiNWNiYTYwMGQzNWVhZjQ1M2Y4",
            "YzY5YzI5MDdiOWM3Y3gBq1YqTs5IzU0MSy0qzszPU7IyNNVRyssvSS1WsoqOrQUAsQQKvw1b",
            "NS+qGu8j08UN4JxjUziTrcRhBkQch6R6oVaTLCIn"
        );
        let raw = r#"{"schemaVersion":15,"notes":[]}"#;
        let (prefix, digest, envelope) = metadata(raw.as_bytes(), 15);
        assert!(Packed::parse(stored.as_bytes())
            .unwrap()
            .authenticated(&KEY));
        assert!(verify(
            stored.as_bytes(),
            &prefix,
            &digest,
            &envelope,
            15,
            &KEY
        ));
        assert_eq!(decode(stored, &[], 15).as_deref(), Some(raw));
    }

    #[test]
    #[ignore = "Opt-in host microbenchmark; not a device startup latency assertion"]
    fn startup_authentication_copy_benchmark() {
        use std::{hint::black_box, time::Instant};
        let mut state = 7u64;
        let values: Vec<_> = (0..16384)
            .map(|_| {
                state ^= state << 13;
                state ^= state >> 7;
                state ^= state << 17;
                state.to_string()
            })
            .collect();
        let raw = serde_json::json!({"schemaVersion":15,"values":values}).to_string();
        let (encoded, _, _, _) = fixture(raw.as_bytes());
        let packed = Packed::parse(encoded.as_bytes()).unwrap();
        let rounds = 2000;
        let mut legacy = Vec::new();
        let mut contiguous = Vec::new();
        for sample in 0..6 {
            for old in [sample % 2 == 0, sample % 2 != 0] {
                let started = Instant::now();
                for _ in 0..rounds {
                    let ok = if old {
                        let mut message = Vec::with_capacity(DOMAIN.len() + packed.payload_end);
                        message.extend_from_slice(DOMAIN);
                        message.extend_from_slice(&packed.bytes()[..packed.payload_end]);
                        hmac::verify(
                            &hmac::Key::new(hmac::HMAC_SHA256, &KEY),
                            black_box(&message),
                            &packed.bytes()[packed.payload_end..],
                        )
                        .is_ok()
                    } else {
                        black_box(&packed).authenticated(black_box(&KEY))
                    };
                    assert!(black_box(ok));
                }
                let elapsed = started.elapsed().as_micros();
                if old {
                    legacy.push(elapsed);
                } else {
                    contiguous.push(elapsed);
                }
            }
        }
        legacy.sort_unstable();
        contiguous.sort_unstable();
        println!("authentication only: rows={rounds}, payload_bytes={}, legacy_median_us={}, contiguous_median_us={}, eliminated_copy_bytes={}",
            packed.payload_end, legacy[3], contiguous[3], rounds * packed.payload_end);
    }

    #[test]
    fn contiguous_authentication_matches_legacy_domain_message() {
        // Exercise base64 padding and different content lengths. The on-disk
        // representation must stay byte-for-byte compatible with existing APKs.
        for length in [0, 1, 2, 3, 31, 32, 255, 4096, 65536] {
            let raw =
                serde_json::json!({"schemaVersion":15,"text":"甲😀x".repeat(length)}).to_string();
            let (encoded, _, _, _) = fixture(raw.as_bytes());
            let packed = Packed::parse(encoded.as_bytes()).unwrap();
            let legacy_bytes = STANDARD
                .decode(encoded.as_bytes().strip_prefix(MARKER).unwrap())
                .unwrap();
            assert_eq!(packed.bytes(), legacy_bytes);
            let mut legacy_message = DOMAIN.to_vec();
            legacy_message.extend_from_slice(&legacy_bytes[..legacy_bytes.len() - TAG_LEN]);
            assert_eq!(
                &packed.authenticated_bytes[..DOMAIN.len() + packed.payload_end],
                legacy_message,
            );
            assert!(packed.authenticated(&KEY));
            assert!(!packed.authenticated(&[8; 32]));
            let mut changed_domain = packed;
            changed_domain.authenticated_bytes[0] ^= 1;
            assert!(!changed_domain.authenticated(&KEY));
        }
    }

    #[test]
    fn packed_history_roundtrips_unicode_and_recovers_when_key_is_lost() {
        let raw = serde_json::json!({"schemaVersion":15,"notes":[{"content":"你好😀\u{0000}".repeat(4096)}]}).to_string();
        let (packed, prefix, expected, envelope) = fixture(raw.as_bytes());
        assert!(is_packed(packed.as_bytes()));
        assert!(packed.len() < raw.len() / 4);
        for key in [KEY.as_slice(), [].as_slice(), [9; 32].as_slice()] {
            assert!(verify(
                packed.as_bytes(),
                &prefix,
                &expected,
                &envelope,
                15,
                key
            ));
            assert_eq!(decode(&packed, key, 15).as_deref(), Some(raw.as_str()));
        }
        assert!(seal(raw.as_bytes(), &prefix, &expected, &envelope, 15, &[]).is_none());
    }

    #[test]
    fn recovered_history_requires_every_chunk_and_the_exact_original_length() {
        for length in [
            INFLATE_CHUNK - 1,
            INFLATE_CHUNK,
            INFLATE_CHUNK + 1,
            3 * INFLATE_CHUNK,
        ] {
            let mut raw = String::from(r#"{"schemaVersion":15}"#);
            raw.extend(std::iter::repeat_n(' ', length - raw.len()));
            let (encoded_row, _, _, _) = fixture(raw.as_bytes());
            assert_eq!(decode(&encoded_row, &[], 15).as_deref(), Some(raw.as_str()));

            let packed = Packed::parse(encoded_row.as_bytes()).unwrap();
            for declared in [length - 1, length + 1] {
                let mut bytes = packed.bytes().to_vec();
                bytes[..4].copy_from_slice(&(declared as u32).to_le_bytes());
                assert!(decode(&encoded(&bytes), &[], 15).is_none());
            }
        }
    }

    #[test]
    fn changed_row_identity_or_metadata_never_uses_a_content_receipt() {
        let (packed, prefix, expected, envelope) = fixture(br#"{"schemaVersion":15}"#);
        for (old, new) in [
            ("snapshot_history", "current_snapshot"),
            ("\x001\0", "\x003\0"),
            ("owner", "other"),
            ("\x0015\0", "\x0014\0"),
            ("\x007\0", "\x008\0"),
            ("\x002\0", "\x003\0"),
            ("\x0099\0", "\x00100\0"),
        ] {
            assert!(!verify(
                packed.as_bytes(),
                &prefix.replace(old, new),
                &expected,
                &envelope,
                15,
                &KEY
            ));
        }
        assert!(!verify(
            packed.as_bytes(),
            &prefix,
            &"0".repeat(64),
            &envelope,
            15,
            &KEY
        ));
        assert!(!verify(
            packed.as_bytes(),
            &prefix,
            &expected,
            &"0".repeat(64),
            15,
            &KEY
        ));
        let raw = br#"{"schemaVersion":15}"#;
        assert!(seal(
            raw,
            &prefix.replace("snapshot_history", "current_snapshot"),
            &expected,
            &envelope,
            15,
            &KEY
        )
        .is_none());
    }

    #[test]
    fn future_schema_and_support_floor_are_preserved_for_legacy_defaults() {
        for raw in [
            b"{}".as_slice(),
            br#"{"schemaVersion":null}"#,
            br#"{"schemaVersion":15}"#,
        ] {
            let (packed, prefix, expected, envelope) = fixture(raw);
            for key in [KEY.as_slice(), [].as_slice()] {
                assert!(!verify(
                    packed.as_bytes(),
                    &prefix,
                    &expected,
                    &envelope,
                    14,
                    key
                ));
                assert!(decode(&packed, key, 14).is_none());
                assert!(verify(
                    packed.as_bytes(),
                    &prefix,
                    &expected,
                    &envelope,
                    16,
                    key
                ));
            }
        }
        for (raw, declared) in [
            (r#"{"schemaVersion":16}"#, 15),
            (r#"{"schemaVersion":15}"#, 16),
            (r#"{"schemaVersion":15,"schemaVersion":16}"#, 15),
            (r#"{"schemaVersion":9223372036854775807}"#, 15),
            ("malformed", 15),
        ] {
            let (prefix, expected, envelope) = metadata(raw.as_bytes(), declared);
            assert!(seal(raw.as_bytes(), &prefix, &expected, &envelope, 15, &KEY).is_none());
        }
    }

    #[test]
    fn bad_payloads_truncation_and_unbounded_lengths_are_rejected() {
        let (packed, prefix, expected, envelope) = fixture(br#"{"schemaVersion":15,"notes":[]}"#);
        let original = Packed::parse(packed.as_bytes()).unwrap();
        for offset in [
            0,
            4,
            8,
            12,
            14,
            16,
            HEADER_LEN,
            original.prefix_end,
            original.digest_end,
            original.envelope_end,
            original.payload_end - 1,
        ] {
            let mut bytes = original.bytes().to_vec();
            bytes[offset] ^= 1;
            let changed = encoded(&bytes);
            assert!(
                !verify(changed.as_bytes(), &prefix, &expected, &envelope, 15, &KEY),
                "offset {offset}"
            );
            assert!(decode(&changed, &[], 15).is_none(), "offset {offset}");
        }
        for remove in [1, TAG_LEN, TAG_LEN + 1] {
            assert!(decode(
                &encoded(&original.bytes()[..original.bytes().len() - remove]),
                &KEY,
                15
            )
            .is_none());
        }
        let mut bytes = original.bytes().to_vec();
        bytes[..4].copy_from_slice(&((MAX_RAW + 1) as u32).to_le_bytes());
        assert!(decode(&encoded(&bytes), &KEY, 15).is_none());
        bytes.extend_from_slice(b"trailing");
        assert!(decode(&encoded(&bytes), &KEY, 15).is_none());
    }

    #[test]
    fn tampered_authentication_tag_only_falls_back_to_full_original_validation() {
        let (packed, prefix, expected, envelope) = fixture(br#"{"schemaVersion":15}"#);
        let original = Packed::parse(packed.as_bytes()).unwrap();
        let mut bytes = original.bytes().to_vec();
        *bytes.last_mut().unwrap() ^= 1;
        let changed = encoded(&bytes);
        assert!(!Packed::parse(changed.as_bytes())
            .unwrap()
            .authenticated(&KEY));
        assert!(verify(
            changed.as_bytes(),
            &prefix,
            &expected,
            &envelope,
            15,
            &KEY
        ));
        bytes[original.envelope_end] ^= 1;
        let changed = encoded(&bytes);
        assert!(!verify(
            changed.as_bytes(),
            &prefix,
            &expected,
            &envelope,
            15,
            &KEY
        ));
    }

    #[test]
    fn compressed_trailing_stream_and_declared_zipbomb_size_fail_even_without_key() {
        let (packed, prefix, expected, envelope) = fixture(br#"{"schemaVersion":15}"#);
        let original = Packed::parse(packed.as_bytes()).unwrap();
        let mut bytes = original.bytes().to_vec();
        bytes.splice(original.payload_end..original.payload_end, [1, 2, 3]);
        let payload_len = (original.payload_end - original.envelope_end + 3) as u32;
        bytes[16..20].copy_from_slice(&payload_len.to_le_bytes());
        assert!(!verify(
            encoded(&bytes).as_bytes(),
            &prefix,
            &expected,
            &envelope,
            15,
            &[]
        ));
        let mut bytes = original.bytes().to_vec();
        bytes[..4].copy_from_slice(&1u32.to_le_bytes());
        assert!(decode(&encoded(&bytes), &[], 15).is_none());
        // A missing zlib checksum can still yield all original JSON bytes. It
        // remains an incomplete compressed stream and cannot become a receipt.
        let mut bytes = original.bytes().to_vec();
        bytes.remove(original.payload_end - 1);
        let shortened_len = (original.payload_end - original.envelope_end - 1) as u32;
        bytes[16..20].copy_from_slice(&shortened_len.to_le_bytes());
        assert!(decode(&encoded(&bytes), &[], 15).is_none());
    }

    #[test]
    fn reused_buffers_validate_only_the_requested_range() {
        let (packed, prefix, expected, envelope) = fixture(br#"{"schemaVersion":15}"#);
        let buffer = [packed.as_bytes(), b"previous larger payload"].concat();
        assert!(verify_range(
            &buffer,
            packed.len() as i32,
            &prefix,
            &expected,
            &envelope,
            15,
            &KEY
        ));
        for length in [
            -1,
            0,
            packed.len() as i32 - 1,
            buffer.len() as i32,
            buffer.len() as i32 + 1,
        ] {
            assert!(!verify_range(
                &buffer, length, &prefix, &expected, &envelope, 15, &KEY
            ));
        }
    }

    #[test]
    fn legacy_history_without_envelope_still_requires_exact_payload_digest() {
        let raw = br#"{"schemaVersion":15}"#;
        let (prefix, expected, _) = metadata(raw, 15);
        let packed = seal(raw, &prefix, &expected, "", 15, &KEY).unwrap();
        assert!(verify(packed.as_bytes(), &prefix, &expected, "", 15, &KEY));
        assert_eq!(
            decode(&packed, &[], 15).as_deref(),
            std::str::from_utf8(raw).ok()
        );
        assert!(seal(raw, &prefix, &"0".repeat(64), "", 15, &KEY).is_none());
    }
}
