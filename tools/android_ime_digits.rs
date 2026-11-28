//! Shared ASCII digit predicate; no role-specific probe types.
pub const fn is_ime_stable_marker(value: &str) -> bool {
    let bytes = value.as_bytes();
    if bytes.is_empty() {
        return false;
    }
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] < b'0' || bytes[index] > b'9' {
            return false;
        }
        index += 1;
    }
    true
}

