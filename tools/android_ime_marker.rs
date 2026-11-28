//! Compile-time safe markers for Android text-entry probes.
#[path = "android_ime_digits.rs"]
mod digits;
pub use digits::is_ime_stable_marker;

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
