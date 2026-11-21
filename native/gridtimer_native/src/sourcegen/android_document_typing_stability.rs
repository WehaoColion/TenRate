//! Android document editing: keep keystrokes stationary and avoid phantom heading blocks.
//! This Rust source transform runs after the document drag and caret transforms.

pub const SCREEN: &str = "com/ofairyo/gridtimer/ui/NoteDocumentEditor.kt";

fn replace_once(source: &mut String, before: &str, after: &str) -> Result<(), String> {
    let count = source.matches(before).count();
    if count != 1 {
        return Err(format!("document typing stability: expected one anchor, found {count}: {before}"));
    }
    *source = source.replacen(before, after, 1);
    Ok(())
}

fn replace_between(
    source: &mut String,
    opening: &str,
    closing: &str,
    before: &str,
    after: &str,
) -> Result<(), String> {
    let from = source.find(opening).ok_or_else(|| format!("missing section start: {opening}"))?;
    let to = source[from..].find(closing)
        .map(|offset| from + offset)
        .ok_or_else(|| format!("missing section end: {closing}"))?;
    let mut section = source[from..to].to_owned();
    replace_once(&mut section, before, after)?;
    source.replace_range(from..to, &section);
    Ok(())
}

pub fn render(path: &str, input: &str) -> Result<String, String> {
    if path != SCREEN {
        return Ok(input.to_owned());
    }
    let mut source = input.to_owned();

    // A selection-only or heading transformation must not clear every sibling
    // TextFieldValue, IME composition, cursor and focus through applyDraftState.
    replace_between(
        &mut source,
        "    fun applyActiveTextAction(action: NoteTextAction): Boolean {",
        "    val activeTextFieldValue =",
        "        beginEditing(requestBodyFocus = true)",
        "        beginEditing()",
    )?;
    replace_between(
        &mut source,
        "    fun applyActiveTextAction(action: NoteTextAction): Boolean {",
        "    val activeTextFieldValue =",
        r#"        recordChange(currentDraftState().copy(blocks = nextBlocks))
        textFieldStates[textUpdate.first] = textUpdate.second
        return true"#,
        r#"        val before = currentDraftState()
        if (nextBlocks != before.blocks) {
            pushUndoState(before)
            redoStack.clear()
            blocks = nextBlocks
            resetTextEditBatch()
        }
        textFieldStates[textUpdate.first] = textUpdate.second
        // No new block was created: don't request a second list-level jump.
        pendingFocusBlockId = null
        return true"#,
    )?;

    // Inserting H1/H2 into an empty current block should reuse that block.
    // Keep explicitly written blank lines and nonempty paragraphs untouched.
    replace_once(
        &mut source,
        "    fun insertTranscriptionBlock(result: NoteMediaTranscriptionResult) {",
        r#"    fun insertHeadingAtActive(prefix: String) {
        val id = activeTextBlockId ?: run {
            insertTextBlockAfterActive(prefix)
            return
        }
        val index = blocks.indexOfFirst { it.id == id && it.type == NoteBlockType.TEXT }
        val field = textFieldStates[id]
        val text = field?.text ?: blocks.getOrNull(index)?.text
        if (index < 0 || text?.isEmpty() != true || field?.composition != null) {
            insertTextBlockAfterActive(prefix)
            return
        }
        beginEditing()
        val before = currentDraftState()
        val updated = before.blocks.toMutableList()
        updated[index] = updated[index].copy(text = prefix)
        pushUndoState(before)
        redoStack.clear()
        blocks = updated
        resetTextEditBatch()
        textFieldStates[id] = TextFieldValue(prefix, TextRange(prefix.length))
        pendingFocusBlockId = id
    }

    fun insertTranscriptionBlock(result: NoteMediaTranscriptionResult) {"#,
    )?;
    replace_once(
        &mut source,
        r###"if (!applyActiveTextAction(NoteTextAction.HEADING)) insertTextBlockAfterActive("# ")"###,
        r###"if (!applyActiveTextAction(NoteTextAction.HEADING)) insertHeadingAtActive("# ")"###,
    )?;
    replace_once(
        &mut source,
        r###"onAddHeadingOne = { insertTextBlockAfterActive("# ") }"###,
        r###"onAddHeadingOne = { insertHeadingAtActive("# ") }"###,
    )?;
    replace_once(
        &mut source,
        r###"onAddHeadingTwo = { insertTextBlockAfterActive("## ") }"###,
        r###"onAddHeadingTwo = { insertHeadingAtActive("## ") }"###,
    )?;

    // The drag grip is not a 44dp-high text line. Smaller minimum height and
    // text-local centering let the intrinsic line height determine block size.
    replace_between(
        &mut source,
        "private fun DocumentCanvasBlock(",
        "private fun DocumentInlineField(",
        ".padding(vertical = 4.dp),",
        ".padding(vertical = if (block.type == NoteBlockType.TEXT) 2.dp else 4.dp),",
    )?;
    replace_between(
        &mut source,
        "private fun DocumentCanvasBlock(",
        "private fun DocumentInlineField(",
        ".size(width = 34.dp, height = 44.dp)",
        ".size(width = 34.dp, height = if (block.type == NoteBlockType.TEXT) 30.dp else 44.dp)",
    )?;
    replace_between(
        &mut source,
        "private fun DocumentCanvasBlock(",
        "private fun DocumentInlineField(",
        ".heightIn(min = 52.dp)",
        ".heightIn(min = 36.dp)",
    )?;
    replace_between(
        &mut source,
        "private fun DocumentCanvasBlock(",
        "private fun DocumentInlineField(",
        ".weight(1f)\n                    .padding(end = 8.dp),\n                verticalArrangement = Arrangement.spacedBy(8.dp)",
        ".weight(1f)\n                    .padding(end = 8.dp),\n                verticalArrangement = Arrangement.spacedBy(4.dp)",
    )?;
    // Keep a little rhythm between paragraphs, without an extra blank row.
    replace_once(
        &mut source,
        "contentPadding = PaddingValues(start = 20.dp, top = 4.dp, end = 20.dp, bottom = 12.dp),\n            verticalArrangement = Arrangement.spacedBy(6.dp)",
        "contentPadding = PaddingValues(start = 20.dp, top = 4.dp, end = 20.dp, bottom = 12.dp),\n            verticalArrangement = Arrangement.spacedBy(3.dp)",
    )?;
    Ok(source)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn editor() -> String {
        let initial = super::super::kotlin_sources::SOURCES.iter()
            .find(|item| item.path == SCREEN).expect("document editor").contents;
        let caret = super::super::android_document_caret::render(SCREEN, initial)
            .expect("caret");
        let markdown = super::super::android_document_markdown::render(SCREEN, &caret)
            .expect("markdown");
        let alignment = super::super::android_document_block_alignment::render(SCREEN, &markdown)
            .expect("alignment");
        let images = super::super::android_document_image_insertion::render(SCREEN, &alignment)
            .expect("image insertion");
        let drag = super::super::android_document_drag_reorder::render(SCREEN, &images)
            .expect("drag");
        render(SCREEN, &drag).expect("stable typing transform")
    }

    #[test]
    fn empty_heading_reuses_current_block_without_dropping_composition() {
        let output = editor();
        assert!(output.contains("text?.isEmpty() != true || field?.composition != null"));
        assert!(output.contains("updated[index] = updated[index].copy(text = prefix)"));
        assert!(output.contains(r###"onAddHeadingOne = { insertHeadingAtActive("# ") }"###));
        assert!(output.contains(r###"onAddHeadingTwo = { insertHeadingAtActive("## ") }"###));
    }

    #[test]
    fn formatting_keeps_sibling_ime_state_and_cursor_in_place() {
        let output = editor();
        let start = output.find("    fun applyActiveTextAction(").unwrap();
        let end = output[start..].find("    val activeTextFieldValue =").unwrap() + start;
        let action = &output[start..end];
        assert!(action.contains("pushUndoState(before)"));
        assert!(action.contains("textFieldStates[textUpdate.first] = textUpdate.second"));
        assert!(!action.contains("recordChange(currentDraftState().copy(blocks = nextBlocks))"));
        assert!(!action.contains("beginEditing(requestBodyFocus = true)"));
    }

    #[test]
    fn typing_has_compact_blocks_without_position_animation() {
        let output = editor();
        assert!(output.contains(".heightIn(min = 36.dp)"));
        assert!(output.contains("height = if (block.type == NoteBlockType.TEXT) 30.dp else 44.dp"));
        assert!(!output.contains(".animateItemPlacement()"));
        assert_eq!(render("other.kt", &output).unwrap(), output);
        assert!(render(SCREEN, &output).is_err());
    }
}
