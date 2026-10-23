//! Keep the insertion anchor after imported image blocks, including after draft
//! mutations, and expose a direct way to continue writing below an image.
const SCREEN: &str = "com/ofairyo/gridtimer/ui/NoteDocumentEditor.kt";
pub const TEST_PATH: &str = "com/ofairyo/gridtimer/ui/DocumentImageInsertionTest.kt";

fn replace_once(source: &mut String, before: &str, after: &str) -> Result<(), String> {
    let count = source.matches(before).count();
    if count != 1 {
        return Err(format!(
            "image block insertion: expected one editor anchor, found {count}: {}",
            before.lines().next().unwrap_or("")
        ));
    }
    *source = source.replacen(before, after, 1);
    Ok(())
}

pub fn render(path: &str, original: &str) -> Result<String, String> {
    if path != SCREEN {
        return Ok(original.to_owned());
    }
    let mut source = original.to_owned();

    // A selected image is a valid INSERT anchor, but is not a keyboard/caret
    // target. The old text-only normalization silently reset it on each
    // caption edit or other recordChange, causing inserts before the image.
    replace_once(&mut source,
        r####"        activeTextBlockId = draft.activeTextBlockId
            ?.takeIf { targetId -> blocks.any { it.id == targetId && it.type == NoteBlockType.TEXT } }
            ?: blocks.firstOrNull { it.type == NoteBlockType.TEXT }?.id
        pendingFocusBlockId = if (markdownPreview) null else activeTextBlockId"####,
        r####"        activeTextBlockId = documentSelectedBlockId(blocks, draft.activeTextBlockId)
        pendingFocusBlockId = if (markdownPreview) null else documentTextFocusBlockId(blocks, activeTextBlockId)"####)?;

    replace_once(&mut source,
        r####"        val index = blocks.indexOfFirst { it.id == activeTextBlockId }
        val insertIndex = if (index >= 0) index + 1 else blocks.size"####,
        r####"        val insertIndex = documentInsertAfterIndex(blocks, activeTextBlockId)"####)?;

    replace_once(&mut source,
        r####"    fun insertTextBlockAfterActive(text: String = "") {
        insertBlockAfterActive(NoteBlock(type = NoteBlockType.TEXT, text = text))
    }"####,
        r####"    fun insertTextBlockAfterActive(text: String = "") {
        insertBlockAfterActive(NoteBlock(type = NoteBlockType.TEXT, text = text))
    }

    fun insertTextBlockAfter(blockId: String) {
        // The direct image affordance has its own stable anchor. Do not use
        // whichever text field happened to be active before the image.
        if (blocks.none { it.id == blockId }) return
        beginEditing()
        val newBlock = NoteBlock(type = NoteBlockType.TEXT)
        val nextBlocks = materializedTextBlocks().toMutableList().apply {
            add(documentInsertAfterIndex(this, blockId), newBlock)
        }
        recordChange(currentDraftState().copy(blocks = nextBlocks))
        activeTextBlockId = newBlock.id
        pendingFocusBlockId = newBlock.id
    }"####)?;

    replace_once(&mut source,
        r####"        val activeIndex = baseBlocks.indexOfFirst { it.id == activeTextBlockId }
        val insertIndex = if (activeIndex >= 0) activeIndex + 1 else baseBlocks.size"####,
        r####"        val insertIndex = documentInsertAfterIndex(baseBlocks, activeTextBlockId)"####)?;

    // The image id survives both pending -> imported replacement and undo
    // bookkeeping. Anchor later toolbar commands after the last picked image,
    // and do not allow a stale text focus effect to reclaim the selection.
    replace_once(&mut source,
        r####"        recordChange(currentDraftState().copy(blocks = nextBlocks))

        uris.zip(placeholders)"####,
        r####"        recordChange(currentDraftState().copy(blocks = nextBlocks))
        focusManager.clearFocus(force = true)
        activeTextBlockId = placeholders.last().id
        pendingFocusBlockId = null

        uris.zip(placeholders)"####)?;

    replace_once(&mut source,
        r####"                onOpenActions = { selectedBlockMenuId = block.id },"####,
        r####"                onOpenActions = { selectedBlockMenuId = block.id },
                onInsertAfter = { insertTextBlockAfter(block.id) },"####)?;

    replace_once(&mut source,
        r####"                onCaptionChanged = { updated -> updateBlock(block.id) { it.copy(caption = updated) } },"####,
        r####"                onCaptionChanged = { updated ->
                    activeTextBlockId = block.id
                    updateBlock(block.id) { it.copy(caption = updated) }
                },"####)?;

    replace_once(&mut source,
        r####"    onOpenActions: () -> Unit,
    onTextChanged: (TextFieldValue) -> Unit,"####,
        r####"    onOpenActions: () -> Unit,
    onInsertAfter: () -> Unit,
    onTextChanged: (TextFieldValue) -> Unit,"####)?;

    replace_once(&mut source,
        r####"                        DocumentInlineField(
                            value = block.caption,
                            placeholder = "添加图片说明",
                            onValueChange = onCaptionChanged
                        )
                    }"####,
        r####"                        DocumentInlineField(
                            value = block.caption,
                            placeholder = "添加图片说明",
                            onValueChange = onCaptionChanged
                        )
                        Text(
                            text = "＋ 在图片下方添加段落",
                            modifier = Modifier
                                .fillMaxWidth()
                                .semantics { contentDescription = "在图片后插入文本块" }
                                .clickable(onClick = onInsertAfter)
                                .padding(vertical = 10.dp),
                            style = MaterialTheme.typography.bodySmall.copy(color = accent)
                        )
                    }"####)?;

    source.push_str(HELPERS);
    Ok(source)
}

const HELPERS: &str = r####"

// An image is a valid insertion anchor. Only TEXT blocks are focusable.
// Keep this policy independent of pending-attachment status and media IO.
internal fun documentSelectedBlockId(blocks: List<NoteBlock>, candidate: String?): String? =
    candidate?.takeIf { id -> blocks.any { it.id == id } }
        ?: blocks.firstOrNull { it.type == NoteBlockType.TEXT }?.id

internal fun documentTextFocusBlockId(blocks: List<NoteBlock>, candidate: String?): String? =
    candidate?.takeIf { id -> blocks.any { it.id == id && it.type == NoteBlockType.TEXT } }

internal fun documentInsertAfterIndex(blocks: List<NoteBlock>, anchorId: String?): Int {
    val index = blocks.indexOfFirst { it.id == anchorId }
    return if (index >= 0) index + 1 else blocks.size
}
"####;

pub const TEST_CONTENTS: &str = r####"package com.ofairyo.gridtimer.ui

import com.ofairyo.gridtimer.data.NoteBlock
import com.ofairyo.gridtimer.data.NoteBlockType
import org.junit.Assert.*
import org.junit.Test

class DocumentImageInsertionTest {
    private val blocks = listOf(
        NoteBlock(id = "paragraph", type = NoteBlockType.TEXT, text = "前文"),
        NoteBlock(id = "photo-1", type = NoteBlockType.IMAGE, attachmentId = "pending:photo-1"),
        NoteBlock(id = "photo-2", type = NoteBlockType.IMAGE, attachmentId = "saved-2")
    )

    @Test fun pickerLastImageBecomesNextInsertionAnchor() {
        assertEquals(3, documentInsertAfterIndex(blocks, "photo-2"))
        assertEquals(2, documentInsertAfterIndex(blocks, "photo-1"))
        assertEquals(1, documentInsertAfterIndex(blocks, "paragraph"))
    }

    @Test fun insertingAfterImageDoesNotReplaceOrDeleteIt() {
        val next = blocks.toMutableList().apply {
            add(documentInsertAfterIndex(this, "photo-1"),
                NoteBlock(id = "new-text", type = NoteBlockType.TEXT))
        }
        assertEquals(listOf("paragraph", "photo-1", "new-text", "photo-2"), next.map { it.id })
        assertEquals("pending:photo-1", next[1].attachmentId)
        assertEquals("saved-2", next[3].attachmentId)
    }

    @Test fun captionUpdatesAndDraftReapplicationKeepImageAsAnchor() {
        assertEquals("photo-2", documentSelectedBlockId(blocks, "photo-2"))
        assertNull(documentTextFocusBlockId(blocks, "photo-2"))
        assertEquals("paragraph", documentTextFocusBlockId(blocks, "paragraph"))
    }

    @Test fun removedAnchorNeverRetainsInvalidSelection() {
        assertEquals("paragraph", documentSelectedBlockId(blocks, "deleted-photo"))
        assertEquals(3, documentInsertAfterIndex(blocks, "deleted-photo"))
        assertNull(documentSelectedBlockId(emptyList(), "photo-2"))
        assertNull(documentTextFocusBlockId(blocks, "deleted-photo"))
    }

    @Test fun imageOnlyDocumentRemainsAppendable() {
        val images = blocks.drop(1)
        assertEquals("photo-2", documentSelectedBlockId(images, "photo-2"))
        assertNull(documentTextFocusBlockId(images, "photo-2"))
        assertEquals(2, documentInsertAfterIndex(images, "photo-2"))
    }
}
"####;

#[cfg(test)]
mod tests {
    use super::*;

    fn template() -> &'static str {
        super::super::kotlin_sources::SOURCES.iter()
            .find(|entry| entry.path == SCREEN).expect("actual Android editor template").contents
    }

    #[test]
    fn real_editor_receives_import_anchor_and_inline_action() {
        let edited = render(SCREEN, template()).unwrap();
        assert!(edited.contains("activeTextBlockId = placeholders.last().id"));
        assert!(edited.contains("pendingFocusBlockId = null\n\n        uris.zip(placeholders)"));
        assert!(edited.contains("onInsertAfter = { insertTextBlockAfter(block.id) }"));
        assert!(edited.contains("onInsertAfter: () -> Unit"));
        assert!(edited.contains("documentSelectedBlockId(blocks, draft.activeTextBlockId)"));
        assert!(edited.contains("documentTextFocusBlockId(blocks, activeTextBlockId)"));
        assert!(edited.contains(".clickable(onClick = onInsertAfter)"));
    }

    #[test]
    fn deleted_or_duplicate_anchor_fails_generation() {
        for anchor in [
            "        activeTextBlockId = draft.activeTextBlockId",
            "    fun insertTextBlockAfterActive(text: String = \"\") {",
            "        val activeIndex = baseBlocks.indexOfFirst { it.id == activeTextBlockId }",
            "        uris.zip(placeholders)",
            "                onOpenActions = { selectedBlockMenuId = block.id },",
            "    onOpenActions: () -> Unit,",
            "                            placeholder = \"添加图片说明\",",
        ] {
            let absent = template().replacen(anchor, "removed anchor", 1);
            assert!(render(SCREEN, &absent).is_err(), "missing anchor accepted: {anchor}");
        }
        let duplicated = template().replacen(
            "                onOpenActions = { selectedBlockMenuId = block.id },",
            "                onOpenActions = { selectedBlockMenuId = block.id },\n                onOpenActions = { selectedBlockMenuId = block.id },", 1
        );
        assert!(render(SCREEN, &duplicated).is_err());
    }

    #[test]
    fn second_application_is_rejected_and_other_sources_unchanged() {
        let first = render(SCREEN, template()).unwrap();
        assert!(render(SCREEN, &first).is_err());
        assert_eq!(render("other.kt", "unchanged").unwrap(), "unchanged");
    }
}
