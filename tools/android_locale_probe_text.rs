//! Locale probe predicate for the verified Japanese Knowledge navigation synonym.
pub fn matched_navigation_label<'a>(locale: &str, expected: &'a str, xml: &str) -> Option<&'a str> {
    let contains = |label: &str| {
        xml.contains(&format!("text=\"{label}\"")) ||
            xml.contains(&format!("content-desc=\"{label}\""))
    };
    if contains(expected) {
        Some(expected)
    } else if locale == "ja-JP" && expected == "ナレッジ" && contains("知識") {
        Some("知識")
    } else {
        None
    }
}
