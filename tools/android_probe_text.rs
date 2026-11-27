//! Shared, locale-tolerant visible-text predicates for Android UI probes.
pub const COMPOSER_LABELS: &[&str] = &[
    "输入问题",
    "继续追问…",
    "Enter a question",
    "Continue asking",
];

pub fn composer_visible(ui: &str) -> bool {
    ["输入问题", "继续追问", "Enter a question", "Continue asking"]
        .iter()
        .any(|label| ui.contains(label))
}

pub fn image_action_visible(ui: &str) -> bool {
    ["＋ 图片", "+ 图片", "+ Image", "Add image"]
        .iter()
        .any(|label| ui.contains(label))
}
