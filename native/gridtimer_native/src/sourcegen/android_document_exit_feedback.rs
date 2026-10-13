//! v2.23.2.28 - Keep the document visible during a durable exit save.
//! No repository, queue ordering, checkpoint or encryption changes.

const PATH: &str = "com/ofairyo/gridtimer/ui/NoteDocumentEditor.kt";

fn replace(source: &mut String, before: &str, after: &str) -> Result<(), String> {
    if source.matches(before).count() != 1 {
        return Err(format!(
            "document exit feedback anchor missing or repeated: {before}"
        ));
    }
    *source = source.replacen(before, after, 1);
    Ok(())
}

pub fn render(path: &str, base: &str) -> Result<String, String> {
    if path != PATH {
        return Ok(base.to_owned());
    }
    let mut source = base.to_owned();
    for import in [
        "androidx.compose.runtime.derivedStateOf",
        "androidx.compose.ui.platform.LocalSoftwareKeyboardController",
        "androidx.compose.ui.semantics.clearAndSetSemantics",
    ] {
        let statement = format!("import {import}\n");
        if !source.contains(&statement) {
            replace(
                &mut source,
                "package com.ofairyo.gridtimer.ui\n",
                &format!("package com.ofairyo.gridtimer.ui\n{statement}"),
            )?;
        }
    }
    replace(&mut source, "    val focusManager = LocalFocusManager.current", "    val focusManager = LocalFocusManager.current\n    val exitKeyboard = LocalSoftwareKeyboardController.current\n    val documentTransitionActive = LocalDocumentTransitionActive.current")?;
    replace(
        &mut source,
        "    fun handleBack() {",
        "    fun handleBack() {\n        if (!documentTransitionActive) return",
    )?;
    replace(&mut source, "        enabled = !revisionDialogVisible && !callPickerVisible && htmlPreview == null", "        enabled = documentTransitionActive && !revisionDialogVisible && !callPickerVisible && htmlPreview == null")?;
    replace(&mut source, "        isSavingDraft = true\n        persistLatestDraft(", "        pendingFocusBlockId = null\n        focusManager.clearFocus(force = true)\n        exitKeyboard?.hide()\n        isSavingDraft = true\n        persistLatestDraft(")?;
    replace(&mut source, "    val markdownPreviewText = documentMarkdownPreviewText(initialDocument, blocks, textFieldStates)", PREVIEW)?;
    replace(&mut source, "        if (documentMarkdownPreviewText(initialDocument, blocks, textFieldStates) == null) return", "        if (!documentMarkdownPreviewAvailable(initialDocument, blocks, textFieldStates)) return")?;
    for (before, after) in [
        (
            "showingMarkdownPreview, markdownPreviewText != null)",
            "showingMarkdownPreview, markdownPreviewAvailable)",
        ),
        (
            "pendingBlockIndex, markdownPreviewText != null, showingMarkdownPreview,",
            "pendingBlockIndex, markdownPreviewAvailable, showingMarkdownPreview,",
        ),
        (
            "        if (markdownPreviewText != null) {",
            "        if (markdownPreviewAvailable) {",
        ),
    ] {
        replace(&mut source, before, after)?;
    }
    replace(&mut source, "    Column(\n        modifier = Modifier\n            .fillMaxSize()\n            .background(flowusEditorBackground())", "    Box(modifier = Modifier.fillMaxSize()) {\n    Column(\n        modifier = Modifier\n            .fillMaxSize()\n            .then(if (isSavingDraft) Modifier.clearAndSetSemantics { } else Modifier)\n            .background(flowusEditorBackground())")?;
    replace(
        &mut source,
        "    if (insertCommandsVisible) {\n        DocumentInsertCommandDialog(",
        SAVE_FEEDBACK,
    )?;
    replace(&mut source, "    if (isSavingDraft || isRenderingLongImage || isRenderingHtml || isTranscribingMedia) {", "    if (isRenderingLongImage || isRenderingHtml || isTranscribingMedia) {")?;
    replace(
        &mut source,
        "                        isSavingDraft -> \"正在保存…\"\n",
        "",
    )?;
    replace(&mut source,
        "    var selectedBlockMenuId by remember { mutableStateOf<String?>(null) }",
        "    var selectedBlockMenuId by remember { mutableStateOf<String?>(null) }\n    LaunchedEffect(documentTransitionActive) {\n        if (!documentTransitionActive) {\n            focusManager.clearFocus(force = true)\n            exitKeyboard?.hide()\n            documentMoreVisible = false\n            shareExportVisible = false\n            documentPropertiesVisible = false\n            insertCommandsVisible = false\n            selectedBlockMenuId = null\n            revisionDialogVisible = false\n            callPickerVisible = false\n            htmlPreview = null\n            encryptDialogVisible = false\n            encryptionSettingsVisible = false\n            changePasswordDialogVisible = false\n            disableEncryptionDialogVisible = false\n        }\n    }")?;
    replace(&mut source, "    fun recordTitleChange(nextTitle: String) {",
        "    fun recordTitleChange(nextTitle: String) {\n        if (!documentTransitionActive || isSavingDraft) return")?;
    replace(&mut source, "    fun recordChange(next: NoteDraftState) {",
        "    fun recordChange(next: NoteDraftState) {\n        if (!documentTransitionActive || isSavingDraft) return")?;
    replace(&mut source, "                onTextChanged = { updated ->\n                    activeTextBlockId = block.id",
        "                onTextChanged = textChange@ { updated ->\n                    if (!documentTransitionActive || isSavingDraft) return@textChange\n                    activeTextBlockId = block.id")?;
    replace(&mut source, "    if (insertCommandsVisible) {\n        DocumentInsertCommandDialog(",
        "    if (documentTransitionActive) {\n    if (insertCommandsVisible) {\n        DocumentInsertCommandDialog(")?;
    replace(
        &mut source,
        "}\n\nprivate fun NoteDocument.ensureEditableBlocks()",
        "    }\n}\n\nprivate fun NoteDocument.ensureEditableBlocks()",
    )?;
    source.push_str(PREVIEW_HELPER);
    Ok(source)
}

const PREVIEW: &str = r####"    // Reading the snapshot map inside derivedStateOf tracks IME text changes.
    // Save-state and transition frames do not reparse the complete Markdown body.
    val markdownPreviewAvailable by remember(initialDocument, blocks, textFieldStates) {
        derivedStateOf { documentMarkdownPreviewAvailable(initialDocument, blocks, textFieldStates) }
    }
    val markdownPreviewText by remember(initialDocument, blocks, textFieldStates, markdownPreview) {
        derivedStateOf {
            if (markdownPreview) documentMarkdownPreviewText(initialDocument, blocks, textFieldStates) else null
        }
    }"####;

const PREVIEW_HELPER: &str = r####"

private fun documentMarkdownPreviewAvailable(
    sourceDocument: NoteDocument,
    draftBlocks: List<NoteBlock>,
    textFieldStates: Map<String, TextFieldValue>
): Boolean = sourceDocument.markdownEnabled && !sourceDocument.richTextEnabled &&
    draftBlocks.all { it.type == NoteBlockType.TEXT } &&
    draftBlocks.any { (textFieldStates[it.id]?.text ?: it.text).isNotBlank() }
"####;

const SAVE_FEEDBACK: &str = r####"    // Block new edits after capturing the exit draft without blanking the page.
    // The delayed indicator avoids flashing for saves completed within one beat.
    if (isSavingDraft) {
        DocumentEditorInputBarrier()
        var showExitProgress by remember { mutableStateOf(false) }
        LaunchedEffect(Unit) { delay(180L); showExitProgress = true }
        if (showExitProgress) {
            Surface(
                modifier = Modifier.align(Alignment.BottomCenter).navigationBarsPadding().padding(16.dp),
                shape = RoundedCornerShape(16.dp),
                color = MaterialTheme.colorScheme.surfaceContainerHigh
            ) {
                Row(
                    modifier = Modifier.padding(horizontal = 16.dp, vertical = 12.dp),
                    verticalAlignment = Alignment.CenterVertically,
                    horizontalArrangement = Arrangement.spacedBy(10.dp)
                ) {
                    CircularProgressIndicator(modifier = Modifier.size(18.dp), strokeWidth = 2.dp)
                    Text("正在保存…", style = MaterialTheme.typography.bodyMedium)
                }
            }
        }
    }
    }

    if (insertCommandsVisible) {
        DocumentInsertCommandDialog("####;
