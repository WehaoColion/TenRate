// v3.0.2.15 - Keep authored sticky-note line boundaries and trailing Enter in the editor and version draft.
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
    let mut output = source.replacen(OLD_ENTRY_MODE, NEW_ENTRY_MODE, 1);
    // A search/preview digest intentionally strips formatting and compresses
    // whitespace. It must never serve as the editable source of a sticky note.
    let old = r####"    val fullBody = editableDocument.plainText(preserveStructure = true)
    val trimmedBody = if (title.trim().isBlank() && !editableDocument.richTextEnabled) {
        editableDocument.withoutLeadingMeaningfulTextLine().plainText(preserveStructure = true)
    } else {
        fullBody
    }"####;
    let new = r####"    fun authoredBody(document: NoteDocument): String =
        if (document.richTextEnabled) {
            document.richTextPlainText.replace("\r\n", "\n").replace('\r', '\n')
        } else {
            document.blocks.filter { it.type == NoteBlockType.TEXT }
                .joinToString("\n\n") { it.text.replace("\r\n", "\n").replace('\r', '\n') }
        }
    val fullBody = authoredBody(editableDocument)
    val trimmedBody = if (title.trim().isBlank() && !editableDocument.richTextEnabled) {
        // For legacy notes with implicit titles, remove only the title line
        // and its preceding whitespace, never trim the authored body tail.
        val firstMeaningful = fullBody.split('\n').indexOfFirst { it.isNotBlank() }
        if (firstMeaningful < 0) fullBody
        else fullBody.split('\n').drop(firstMeaningful + 1).joinToString("\n")
    } else {
        fullBody
    }"####;
    if output.matches(old).count() != 1 {
        return Err("sticky note editable body projection drifted".into());
    }
    output = output.replacen(old, new, 1);
    // Keep trailing Enter presses meaningful in the draft, dirty detection,
    // and version snapshots. Normalize only line-ending encodings.
    for (before, after) in [
        (r####"val normalizedBody = plainBody.replace("\r\n", "\n").trimEnd()"####,
         r####"val normalizedBody = plainBody.replace("\r\n", "\n").replace('\r', '\n')"####),
        (r####"currentDraft.plainBody.replace("\r\n", "\n").trimEnd() == normalizedBody"####,
         r####"currentDraft.plainBody.replace("\r\n", "\n").replace('\r', '\n') == normalizedBody"####),
        (r####"baseDraft.plainBody.replace("\r\n", "\n").trimEnd()"####,
         r####"baseDraft.plainBody.replace("\r\n", "\n").replace('\r', '\n')"####),
        (r####"candidateDraft.plainBody.replace("\r\n", "\n").trimEnd()"####,
         r####"candidateDraft.plainBody.replace("\r\n", "\n").replace('\r', '\n')"####),
    ] {
        if output.matches(before).count() != 1 {
            return Err(format!("sticky note exact draft anchor drifted: {before}"));
        }
        output = output.replacen(before, after, 1);
    }
    Ok(output)
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
