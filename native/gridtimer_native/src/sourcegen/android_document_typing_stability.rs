// v3.0.2.65 - Invalidate stale save receipts in the text input event and verify live captured content.
// v3.0.2.38 - Compose component-local hook and production source-generation regression.
// v3.0.2.37 - Current editor state owns composition, exact selection evidence, same-instant timer comparison.
// v3.0.2.13 - Retain exact autosave receipts for the full-entry exit guard.
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

// Apply to the fully assembled editor, after its workspace handoff and restore gates.
pub fn project_after_debounce(path: &str, source: &mut String) -> Result<(), String> {
    if path != SCREEN { return Ok(()); }
    let effect = source.find("        if (hierarchyHandedOff || isSavingDraft || pendingRevisionRestore != null) return@LaunchedEffect")
        .ok_or("missing document autosave effect")?;
    let start = source[effect..].find("        if (editorSession.canLeaveUnmodified(")
        .map(|index| effect + index).ok_or("missing document autosave entry guard")?;
    let end = source[start..].find("    val lastDurableExitDraft = editorSession.lastDurableExitDraft")
        .map(|index| start + index).ok_or("missing document autosave boundary")?;
    source.replace_range(start..end, AUTOSAVE_AND_PREVIEW);
    replace_once(source, "import androidx.compose.ui.semantics.semantics\n",
        "import androidx.compose.ui.semantics.semantics\nimport androidx.compose.ui.semantics.testTag\nimport androidx.compose.ui.semantics.testTagsAsResourceId\n")?;
    replace_once(source, "modifier = Modifier.weight(1f).onSizeChanged { editorViewportHeightPx = it.height },",
        "modifier = Modifier.weight(1f).documentSaveReceipt(saveState).onSizeChanged { editorViewportHeightPx = it.height },")?;
    let helper = "private fun NoteDocument.ensureEditableBlocks(): NoteDocument {";
    replace_once(source, helper, &format!(r#"@OptIn(androidx.compose.ui.ExperimentalComposeUiApi::class)
private fun Modifier.documentSaveReceipt(state: DocumentEditorSaveState): Modifier = semantics {{
    testTagsAsResourceId = true
    testTag = "document_save_${{state.name.lowercase()}}"
}}

{helper}"#))?;
    // Raw Markdown/source text is authoritative. Predictive autocorrection on
    // Enter previously changed the literal heading (StableHeading -> Stable heading).
    // Use the Compose 1.6 KeyboardOptions API from the pinned BOM.
    replace_once(source, "                            value = textFieldState,\n                            onValueChange = onTextChanged,",
        "                            value = textFieldState,\n                            keyboardOptions = androidx.compose.foundation.text.KeyboardOptions(autoCorrect = false),\n                            onValueChange = onTextChanged,")?;
    // The editor owns the latest TextFieldValue; a block's recomposed value is
    // only a projection. Focus loss must never replay that potentially stale
    // projection, especially after splitting a block or finishing an IME edit.
    replace_once(source, "                onFocused = { activeTextBlockId = block.id },",
        r#"                onFocused = { activeTextBlockId = block.id },
                onCompositionCommit = {
                    if (documentTransitionActive && !isSavingDraft) {
                        val latest = textFieldStates[block.id]
                        if (latest?.composition != null) {
                            textFieldStates[block.id] = latest.copy(composition = null)
                            updateTextBlock(block.id, latest.text)
                        }
                    }
                },"#)?;
    // Image insertion and drag reorder legitimately extend the parameter list.
    // Only the stable focus parameter inside this component owns the new hook.
    replace_between(source, "private fun DocumentCanvasBlock(", "private fun DocumentInlineField(",
        "    onFocused: () -> Unit,\n",
        "    onFocused: () -> Unit,\n    onCompositionCommit: () -> Unit,\n")?;
    replace_once(source, "                                    } else if (textFieldState.composition != null) {\n                                        onTextChanged(textFieldState.copy(composition = null))\n                                    }",
        "                                    } else {\n                                        onCompositionCommit()\n                                    }")?;
    // Expose only offsets/length, not contents, via the existing semantics ID
    // system. The field itself remains the authority for accessibility text.
    replace_once(source, "                                .onFocusChanged { focusState ->\n                                    caretVisibility.focused = focusState.isFocused",
        r#"                                .semantics {
                                    testTag = "document_body_cursor_${textFieldState.selection.start}_${textFieldState.selection.end}_${textFieldState.text.length}_${if (textFieldState.composition == null) "committed" else "composing"}"
                                }
                                .onFocusChanged { focusState ->
                                    caretVisibility.focused = focusState.isFocused"#)?;
    replace_once(source, "encrypted = previewNote.encryption != null,", "encrypted = note.encryption != null,")?;
    replace_between(source, "            DocumentCanvasBlock(", "                active = ",
        "note = previewNote,", "note = note,")?;
    replace_once(source, "    fun ensureTextStates(currentBlocks: List<NoteBlock>) {",
        r#"    val reconciledBlocks = remember(editorSession) { java.util.concurrent.atomic.AtomicReference<List<NoteBlock>?>() }
    val lastTextIndex = remember(editorSession) { intArrayOf(-1) }

    fun ensureTextStates(currentBlocks: List<NoteBlock>) {"#)?;
    replace_once(source, "    LaunchedEffect(blocks) {\n        ensureTextStates(blocks)\n    }",
        r#"    LaunchedEffect(blocks) {
        if (reconciledBlocks.get() !== blocks) {
            ensureTextStates(blocks)
            reconciledBlocks.set(blocks)
        }
    }"#)?;
    replace_between(source, "    fun updateTextBlock(", "    fun insertBlockAfterActive(",
        "        val index = blocks.indexOfFirst { block -> block.id == blockId && block.type == NoteBlockType.TEXT }",
        r#"        val cached = lastTextIndex[0]
        val index = if (blocks.getOrNull(cached)?.let { it.id == blockId && it.type == NoteBlockType.TEXT } == true) cached
            else blocks.indexOfFirst { it.id == blockId && it.type == NoteBlockType.TEXT }.also { lastTextIndex[0] = it }"#)?;
    // An IME edit and a previous asynchronous save callback can arrive
    // before the next composition/effect. Invalidate the old receipt in the
    // input event itself, not only when the debounce effect starts.
    replace_between(source, "    fun updateTextBlock(", "    fun insertBlockAfterActive(",
        "        blocks = nextBlocks",
        "        saveRequestGeneration += 1L\n        saveState = DocumentEditorSaveState.SAVING\n        reconciledBlocks.compareAndSet(blocks, nextBlocks)\n        blocks = nextBlocks")?;
    replace_between(source, "    fun recordTitleChange(", "    fun updateBlock(",
        "        titleText = nextTitle",
        "        saveRequestGeneration += 1L\n        saveState = DocumentEditorSaveState.SAVING\n        titleText = nextTitle")?;
    Ok(())
}

const AUTOSAVE_AND_PREVIEW: &str = r#"        val entry = editorSession.initialContentSnapshot
        if (saveRequestGeneration == 0L && editorSession.canLeaveUnmodified(
                current = entry.copy(title = titleText, accentSeed = accentSeed, pinned = pinned,
                    folderId = folderId, markdownEnabled = markdownEnabled, blocks = blocks),
                requiresBlankDeletion = false,
                workspaceMatches = viewModel.currentWorkspaceKey() == editorWorkspaceKey
            )) return@LaunchedEffect
        saveState = DocumentEditorSaveState.SAVING
        saveRequestGeneration += 1L
        val generation = saveRequestGeneration
        delay(520L)
        if (hierarchyHandedOff || isSavingDraft || pendingRevisionRestore != null ||
            generation != saveRequestGeneration || viewModel.currentWorkspaceKey() != editorWorkspaceKey) return@LaunchedEffect
        val base = latestNote
        val document = base.document.copy(markdownEnabled = markdownEnabled, blocks = materializedTextBlocks().ifEmpty { listOf(NoteBlock(type = NoteBlockType.TEXT)) })
        val draft = base.copy(title = titleText, document = document, accentSeed = accentSeed, pinned = pinned)
        // A generation alone cannot prove the live draft still matches the
        // captured entry (e.g. another UI mutation before its effect runs).
        fun capturedContentStillCurrent(): Boolean =
            titleText == draft.title && accentSeed == draft.accentSeed &&
                pinned == draft.pinned && markdownEnabled == document.markdownEnabled &&
                folderId == base.folderId && materializedTextBlocks() == document.blocks
        val nextNote = kotlinx.coroutines.withContext(kotlinx.coroutines.Dispatchers.Default) {
            draft.copy(content = document.toStorageContent())
        }
        if (hierarchyHandedOff || isSavingDraft || pendingRevisionRestore != null ||
            generation != saveRequestGeneration || viewModel.currentWorkspaceKey() != editorWorkspaceKey) return@LaunchedEffect
        viewModel.upsertNoteAutosave(nextNote, expectedWorkspaceKey = editorWorkspaceKey,
            onDurableEpoch = { epoch ->
                if (editorSaveCallbackActive.get() && !isSavingDraft && !hierarchyHandedOff &&
                    pendingRevisionRestore == null && generation == saveRequestGeneration &&
                    capturedContentStillCurrent() &&
                    viewModel.currentWorkspaceKey() == editorWorkspaceKey &&
                    editorSession.exitSaveGate.recordAutosaveReceipt(epoch, viewModel.noteMutationEpoch())) {
                    editorSession.lastDurableExitDraft.value = nextNote
                }
            }
        ) { saved ->
            if (editorSaveCallbackActive.get() && viewModel.currentWorkspaceKey() == editorWorkspaceKey &&
                generation == saveRequestGeneration && capturedContentStillCurrent()) {
                saveState = if (saved) DocumentEditorSaveState.LOCAL_SAVED else DocumentEditorSaveState.FAILED
            }
        }
    }

    val editorAccent = accentFor(accentSeed)
    val previewNote by remember(editorSession) {
        derivedStateOf {
            val base = latestNote
            val document = base.document.copy(markdownEnabled = markdownEnabled, blocks = blocks)
            base.copy(title = titleText, content = document.toStorageContent(), document = document,
                accentSeed = accentSeed, pinned = pinned)
        }
    }
"#;

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
