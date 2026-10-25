// v2.23.2.56 - Open Android sticky notes in the editable source, not rendered Markdown.
// Preserve opt-in preview and read-only historical version rendering.
const PATH: &str = "com/ofairyo/gridtimer/ui/SmartisanNoteUi.kt";

const OLD_ENTRY_MODE: &str = r####"    var markdownPreview by rememberSaveable(note.id) {
        mutableStateOf(note.resolvedDocument().markdownEnabled && !initialDraft.richTextEnabled && initialDraft.plainBody.isNotBlank())
    }"####;

const NEW_ENTRY_MODE: &str = r####"    // Opening or reopening a sticky note always starts in editable source.
    // Preview belongs to the current editor session and is explicitly opted into.
    var markdownPreview by remember(note.id) {
        mutableStateOf(false)
    }"####;

pub fn render(path: &str, source: &str) -> Result<String, String> {
    if path != PATH {
        return Ok(source.to_owned());
    }
    let count = source.matches(OLD_ENTRY_MODE).count();
    if count != 1 {
        return Err(format!(
            "sticky-note edit-first entry: expected one old preview initializer, found {count}"
        ));
    }
    Ok(source.replacen(OLD_ENTRY_MODE, NEW_ENTRY_MODE, 1))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn existing_markdown_sticky_notes_open_editable_without_auto_preview() {
        let original = super::super::kotlin_sources::SOURCES
            .iter()
            .find(|source| source.path == PATH)
            .expect("generated sticky-note editor");
        let output = render(original.path, original.contents).unwrap();
        assert!(output.contains(NEW_ENTRY_MODE));
        assert!(!output.contains(OLD_ENTRY_MODE));
        assert!(!output.contains("var markdownPreview by rememberSaveable(note.id)"));
        assert!(output.contains("markdownPreview = !markdownPreview"));
        assert!(output.contains("Text(if (markdownPreview) \"编辑原文\" else \"预览公式与排版\")"));
        assert!(output.contains("} else if (markdownPreview) {"));
        assert!(output.contains("AndroidRenderedMarkdown("));
        assert!(output.contains("BasicTextField("));
        assert!(output.contains("if (activeVersionId == previewNote.latestVersionId)"));
        assert!(output.contains("readOnly = true"));
    }

    #[test]
    fn unrelated_sources_remain_unchanged_and_drift_fails_closed() {
        let unaffected = "document or other generated source";
        assert_eq!(render("other/path.kt", unaffected).unwrap(), unaffected);
        assert!(render(PATH, "missing the original initializer").is_err());
        assert!(render(PATH, &format!("{OLD_ENTRY_MODE}\n{OLD_ENTRY_MODE}")).is_err());
    }
}
