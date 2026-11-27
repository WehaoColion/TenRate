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

/// Runtime-checked numeric text for paced Android IME automation.
/// Long rows are sent in bounded chunks so the input channel cannot silently
/// outrun a busy editor while still preserving an exact, non-autocorrectable payload.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ImeStableText<'a>(&'a str);

impl<'a> ImeStableText<'a> {
    pub fn new(value: &'a str) -> Result<Self, &'static str> {
        if is_ime_stable_marker(value) {
            Ok(Self(value))
        } else {
            Err("Paced IME text must be non-empty ASCII digits")
        }
    }

    pub fn chunks(&self, chunk_size: usize) -> Result<impl Iterator<Item = &'a str>, &'static str> {
        if chunk_size == 0 {
            return Err("IME text chunk size must be greater than zero");
        }
        Ok(self.0.as_bytes().chunks(chunk_size).map(|chunk| {
            std::str::from_utf8(chunk).expect("validated ASCII digit chunks remain UTF-8")
        }))
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
    use super::{is_ime_stable_marker, ImeStableMarker, ImeStableText};

    #[test]
    fn historical_autocorrectable_document_words_are_rejected() {
        for value in ["StableHeading", "Stable heading", "StableParagraph", "NoJump", "HotSave", "ExitSave"] {
            assert!(!is_ime_stable_marker(value), "autocorrectable marker accepted: {value}");
        }
        assert!(!is_ime_stable_marker(""));
        assert!(is_ime_stable_marker("7391804025006017"));
        assert_eq!(ImeStableMarker::new("7391804025006017").as_str(), "7391804025006017");
    }

    #[test]
    fn paced_chunks_preserve_every_digit_and_order() {
        let chunks = ImeStableText::new("01234567890123456789").unwrap()
            .chunks(10).unwrap().collect::<Vec<_>>();
        assert_eq!(chunks, ["0123456789", "0123456789"]);
    }

    #[test]
    fn paced_text_rejects_non_digits_and_zero_chunk_size() {
        assert!(ImeStableText::new("12a4").is_err());
        assert!(ImeStableText::new("1234").unwrap().chunks(0).is_err());
    }
}
