//! Android document block grip drag reordering.
//! All UI mutations are emitted by this Rust module; no checked-in Kotlin output is edited.
//! Preview order is ephemeral. Commit one undoable document mutation on pointer release.
pub const SCREEN: &str = "com/ofairyo/gridtimer/ui/NoteDocumentEditor.kt";
const CANVAS_START: &str = "private fun DocumentCanvasBlock(";
const CANVAS_END: &str = "private fun DocumentInlineField(";

fn replace_once(source: &mut String, old: &str, new: &str) -> Result<(), String> {
    let count = source.matches(old).count();
    if count != 1 {
        return Err(format!("block drag generator anchor count {count}: {}", old.lines().next().unwrap_or("")));
    }
    *source = source.replacen(old, new, 1);
    Ok(())
}

pub fn render(path: &str, input: &str) -> Result<String, String> {
    if path != SCREEN {
        return Ok(input.to_owned());
    }
    let mut source = input.to_owned();
    for import in [
        "androidx.compose.foundation.gestures.detectDragGestures",
        "androidx.compose.foundation.gestures.scrollBy",
        "androidx.compose.ui.hapticfeedback.HapticFeedbackType",
        "androidx.compose.ui.input.pointer.pointerInput",
        "androidx.compose.ui.draw.zIndex",
        "androidx.compose.ui.graphics.graphicsLayer",
        "androidx.compose.ui.platform.LocalDensity",
        "androidx.compose.ui.platform.LocalHapticFeedback",
        "androidx.compose.ui.platform.LocalSoftwareKeyboardController",
    ] {
        let line = format!("import {import}\n");
        if !source.contains(&line) {
            replace_once(
                &mut source,
                "package com.ofairyo.gridtimer.ui\n",
                &format!("package com.ofairyo.gridtimer.ui\n{line}"),
            )?;
        }
    }
    replace_once(
        &mut source,
        "@OptIn(ExperimentalLayoutApi::class)\n@Composable\ninternal fun NoteEditorContent(",
        "@OptIn(ExperimentalFoundationApi::class, ExperimentalLayoutApi::class)\n@Composable\ninternal fun NoteEditorContent(",
    )?;
    replace_once(
        &mut source,
        "    var editorViewportHeightPx by remember(note.id, workspaceKey) { mutableStateOf(0) }",
        DRAG_STATE,
    )?;
    replace_once(
        &mut source,
        "        } else items(blocks, key = NoteBlock::id) { block ->\n            DocumentCanvasBlock(",
        DRAG_LIST_START,
    )?;
    replace_once(
        &mut source,
        "                onOpenActions = { selectedBlockMenuId = block.id },",
        DRAG_CALLBACKS,
    )?;
    replace_once(
        &mut source,
        "                onPreviewAttachment = { block.attachmentId?.let(onRequestPreviewAttachment) }\n            )\n        }",
        DRAG_LIST_END,
    )?;
    let start = source.find(CANVAS_START).ok_or("missing canvas block start")?;
    let end = source[start..].find(CANVAS_END).map(|offset| start + offset)
        .ok_or("missing canvas block end")?;
    if start >= end { return Err("invalid canvas block boundaries".into()); }
    let mut canvas = source[start..end].to_owned();
    replace_once(
        &mut canvas,
        "    onOpenActions: () -> Unit,\n    onInsertAfter: () -> Unit,\n    onTextChanged: (TextFieldValue) -> Unit,",
        "    onOpenActions: () -> Unit,\n    onInsertAfter: () -> Unit,\n    onDragStart: (Float) -> Unit,\n    onDragBy: (Float) -> Unit,\n    onDragEnd: () -> Unit,\n    onDragCancel: () -> Unit,\n    dragging: Boolean,\n    dragOffsetY: Float,\n    onTextChanged: (TextFieldValue) -> Unit,",
    )?;
    replace_once(
        &mut canvas,
        "    Surface(\n        modifier = Modifier.fillMaxWidth(),\n        shape = RoundedCornerShape(12.dp),\n        color = containerColor,\n        shadowElevation = 0.dp",
        "    Surface(\n        modifier = Modifier.fillMaxWidth().graphicsLayer {\n            translationY = if (dragging) dragOffsetY else 0f\n            scaleX = if (dragging) 1.012f else 1f\n            scaleY = if (dragging) 1.012f else 1f\n        },\n        shape = RoundedCornerShape(12.dp),\n        color = if (dragging) MaterialTheme.colorScheme.surface else containerColor,\n        shadowElevation = if (dragging) 9.dp else 0.dp",
    )?;
    replace_once(
        &mut canvas,
        "    val imageAttachment = remember(note.attachments, block.attachmentId) {",
        "    val currentDragStart by rememberUpdatedState(onDragStart)\n    val currentDragBy by rememberUpdatedState(onDragBy)\n    val currentDragEnd by rememberUpdatedState(onDragEnd)\n    val currentDragCancel by rememberUpdatedState(onDragCancel)\n    val imageAttachment = remember(note.attachments, block.attachmentId) {",
    )?;
    replace_once(
        &mut canvas,
        "                    .combinedClickable(onClick = onFocused, onLongClick = onOpenActions),",
        DRAG_HANDLE,
    )?;
    source.replace_range(start..end, &canvas);
    Ok(source)
}

const DRAG_STATE: &str = r####"    var editorViewportHeightPx by remember(note.id, workspaceKey) { mutableStateOf(0) }
    val dragDensity = LocalDensity.current
    val dragKeyboard = LocalSoftwareKeyboardController.current
    val dragHaptics = LocalHapticFeedback.current
    val gripTopPx = with(dragDensity) { 6.dp.toPx() }
    val dragEdgePx = with(dragDensity) { 68.dp.toPx() }
    var draggedBlockId by remember(note.id, workspaceKey) { mutableStateOf<String?>(null) }
    var dragFingerY by remember(note.id, workspaceKey) { mutableStateOf(0f) }
    var dragTouchLocalY by remember(note.id, workspaceKey) { mutableStateOf(0f) }
    var dragDropIndex by remember(note.id, workspaceKey) { mutableStateOf(0) }
    var dragPreviewIds by remember(note.id, workspaceKey) { mutableStateOf<List<String>?>(null) }

    // Compute an insertion position among the unchanged block identities.
    // LazyList viewport offsets are also used after the keyboard resizes.
    fun locateDragDestination(dragId: String, y: Float): Int {
        val remaining = blocks.map(NoteBlock::id).filterNot { it == dragId }
        if (remaining.isEmpty()) return 0
        val visible = editorListState.layoutInfo.visibleItemsInfo.mapNotNull { item ->
            val id = item.key as? String ?: return@mapNotNull null
            if (id !in remaining) return@mapNotNull null
            id to (item.offset + item.size / 2f)
        }.sortedBy { it.second }
        if (visible.isEmpty()) return dragDropIndex.coerceIn(0, remaining.size)
        val next = visible.firstOrNull { (_, centerY) -> y < centerY }?.first
        return if (next != null) remaining.indexOf(next)
            else (remaining.indexOf(visible.last().first) + 1).coerceIn(0, remaining.size)
    }

    fun updateDragPreview(dragId: String) {
        if (draggedBlockId != dragId) return
        val nextIndex = locateDragDestination(dragId, dragFingerY)
        if (nextIndex == dragDropIndex) return
        dragDropIndex = nextIndex
        val remaining = blocks.map(NoteBlock::id).filterNot { it == dragId }.toMutableList()
        remaining.add(nextIndex, dragId)
        dragPreviewIds = remaining
    }

    fun startBlockDrag(id: String, localY: Float) {
        if (draggedBlockId != null || blocks.size < 2) return
        val position = editorListState.layoutInfo.visibleItemsInfo
            .firstOrNull { it.key == id } ?: return
        focusManager.clearFocus(force = true)
        dragKeyboard?.hide()
        selectedBlockMenuId = null
        draggedBlockId = id
        dragTouchLocalY = localY
        dragFingerY = position.offset + gripTopPx + localY
        dragDropIndex = blocks.indexOfFirst { it.id == id }.coerceAtLeast(0)
        dragPreviewIds = blocks.map(NoteBlock::id)
        dragHaptics.performHapticFeedback(HapticFeedbackType.LongPress)
    }

    fun dragBlockBy(id: String, deltaY: Float) {
        if (draggedBlockId != id) return
        dragFingerY += deltaY
        updateDragPreview(id)
    }

    fun finishBlockDrag(id: String, commit: Boolean) {
        if (draggedBlockId != id) return
        val destination = dragDropIndex
        // A cancelled gesture has no side effects on the document or undo stack.
        if (commit) {
            val materialized = materializedTextBlocks()
            val from = materialized.indexOfFirst { it.id == id }
            if (from >= 0) {
                val next = materialized.toMutableList()
                val moved = next.removeAt(from)
                val target = destination.coerceIn(0, next.size)
                if (from != target) {
                    next.add(target, moved)
                    beginEditing()
                    recordChange(currentDraftState().copy(blocks = next))
                }
            }
        }
        draggedBlockId = null
        dragPreviewIds = null
    }

    LaunchedEffect(draggedBlockId) {
        while (draggedBlockId != null) {
            delay(24)
            val dragId = draggedBlockId ?: break
            val layout = editorListState.layoutInfo
            val y = dragFingerY
            val top = layout.viewportStartOffset + dragEdgePx
            val bottom = layout.viewportEndOffset - dragEdgePx
            val scroll = when {
                y < top -> -((top - y) / dragEdgePx).coerceIn(0.25f, 1f) * 19f
                y > bottom -> ((y - bottom) / dragEdgePx).coerceIn(0.25f, 1f) * 19f
                else -> 0f
            }
            if (scroll != 0f) {
                editorListState.scrollBy(scroll)
                updateDragPreview(dragId)
            }
        }
    }

    val displayedBlocks = dragPreviewIds?.let { order ->
        val indexed = blocks.associateBy(NoteBlock::id)
        order.mapNotNull(indexed::get).takeIf { it.size == blocks.size }
    } ?: blocks
    val otherBlockIds = if (draggedBlockId != null)
        blocks.map(NoteBlock::id).filterNot { it == draggedBlockId } else emptyList()
    val dropBeforeId = if (draggedBlockId != null)
        otherBlockIds.getOrNull(dragDropIndex) else null
    val dropAfterId = if (draggedBlockId != null && dropBeforeId == null)
        otherBlockIds.lastOrNull() else null"####;

const DRAG_LIST_START: &str = r####"        } else items(displayedBlocks, key = NoteBlock::id) { block ->
            Column(modifier = Modifier
                .zIndex(if (draggedBlockId == block.id) 1f else 0f)
                .animateItemPlacement()
            ) {
            if (dropBeforeId == block.id) {
                Spacer(Modifier.fillMaxWidth().height(2.dp).background(editorAccent))
            }
            val dragRow = editorListState.layoutInfo.visibleItemsInfo.firstOrNull { it.key == block.id }
            val offsetY = if (draggedBlockId == block.id && dragRow != null)
                dragFingerY - dragRow.offset - gripTopPx - dragTouchLocalY else 0f
            DocumentCanvasBlock("####;

const DRAG_CALLBACKS: &str = r####"                onOpenActions = { selectedBlockMenuId = block.id },
                onDragStart = { y -> startBlockDrag(block.id, y) },
                onDragBy = { delta -> dragBlockBy(block.id, delta) },
                onDragEnd = { finishBlockDrag(block.id, true) },
                onDragCancel = { finishBlockDrag(block.id, false) },
                dragging = draggedBlockId == block.id,
                dragOffsetY = offsetY,"####;

const DRAG_LIST_END: &str = r####"                onPreviewAttachment = { block.attachmentId?.let(onRequestPreviewAttachment) }
            )
            if (dropAfterId == block.id) {
                Spacer(Modifier.fillMaxWidth().height(2.dp).background(editorAccent))
            }
            }
        }"####;

const DRAG_HANDLE: &str = r####"                    .clickable(onClick = onOpenActions)
                    .pointerInput(block.id) {
                        detectDragGestures(
                            onDragStart = { position -> currentDragStart(position.y) },
                            onDragCancel = { currentDragCancel() },
                            onDragEnd = { currentDragEnd() },
                            onDrag = { change, amount ->
                                change.consume()
                                currentDragBy(amount.y)
                            }
                        )
                    }
                    .semantics { contentDescription = "拖动调整块顺序，点击打开操作菜单" },"####;

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> String {
        let base = super::super::kotlin_sources::SOURCES.iter()
            .find(|item| item.path == SCREEN).expect("document template").contents;
        let with_caret = super::super::android_document_caret::render(SCREEN, base)
            .expect("document focus state injection");
        super::super::android_document_image_insertion::render(SCREEN, &with_caret)
            .expect("image insertion signatures are applied before drag")
    }
    #[test]
    fn drag_mutation_only_targets_document_editor() {
        let initial = fixture();
        let result = render(SCREEN, &initial).expect("render document drag UI");
        assert!(result.contains("dragPreviewIds = remaining"));
        assert!(result.contains("materializedTextBlocks()"));
        assert!(result.contains("recordChange(currentDraftState().copy(blocks = next))"));
        assert!(result.contains("onDragCancel = { finishBlockDrag(block.id, false) }"));
        assert!(result.contains("currentDragBy(amount.y)"));
        assert!(result.contains("editorListState.scrollBy(scroll)"));
        assert_eq!(render("some/other/File.kt", &initial).unwrap(), initial);
        assert!(render(SCREEN, &result).is_err());
    }
    #[test]
    fn missing_document_handle_fails_closed() {
        let before = fixture();
        let changed = before.replacen(
            ".combinedClickable(onClick = onFocused, onLongClick = onOpenActions)",
            ".clickable(onClick = onOpenActions)",
            1
        );
        assert!(render(SCREEN, &changed).is_err());
    }
    #[test]
    fn missing_lazy_list_focus_state_fails_closed() {
        let before = fixture().replacen(
            "var editorViewportHeightPx by remember(note.id, workspaceKey) { mutableStateOf(0) }",
            "var otherState = false",
            1
        );
        assert!(render(SCREEN, &before).is_err());
    }
}
