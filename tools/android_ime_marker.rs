//! Compile-time safe markers for Android text-entry probes.

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ImeStableMarker(&'static str);

impl ImeStableMarker {
    pub const fn new(value: &'static str) -> Self {
        if !is_ime_stable_marker(value) {
            panic!("IME-stable markers must be non-empty ASCII digits; do not use autocorrectable words");
        }
        Self(value)
    }

    pub const fn as_str(&self) -> &'static str {
        self.0
    }
}

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

#[cfg(test)]
mod tests {
    use super::{is_ime_stable_marker, ImeStableMarker};

    #[test]
    fn historical_autocorrectable_document_words_are_rejected() {
        for value in ["StableHeading", "Stable heading", "StableParagraph", "NoJump", "HotSave", "ExitSave"] {
            assert!(!is_ime_stable_marker(value), "autocorrectable marker accepted: {value}");
        }
        assert!(!is_ime_stable_marker(""));
        assert!(is_ime_stable_marker("7391804025006017"));
        assert_eq!(ImeStableMarker::new("7391804025006017").as_str(), "7391804025006017");
    }
}
