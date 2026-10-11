//! Avoid inspecting document bodies when collection filters only need metadata.
//! Runs at the final source-writing stage, after localization and filter generation.

const SCREEN: &str = "com/ofairyo/gridtimer/ui/NoteStudioSheet.kt";
pub const TEST_PATH: &str = "com/ofairyo/gridtimer/ui/DocumentCollectionWorkTest.kt";

pub fn render(path: &str, source: &str) -> Result<String, String> {
    if path != SCREEN {
        return Ok(source.to_owned());
    }
    let mut result = source.to_owned();
    replace_once(&mut result, QUERY, LAZY_QUERY)?;
    replace_once(&mut result, FILTER, SCOPED_FILTER)?;
    Ok(result)
}

fn replace_once(source: &mut String, before: &str, after: &str) -> Result<(), String> {
    let count = source.matches(before).count();
    if count != 1 {
        return Err(format!(
            "document collection work: expected one filter anchor, found {count}: {}",
            before.lines().next().unwrap_or("")
        ));
    }
    *source = source.replacen(before, after, 1);
    Ok(())
}

const QUERY: &str = r####"internal fun noteMatchesQuery(
    query: String,
    note: NoteEntry,
    folder: NoteFolder?
): Boolean {
    if (note.isEncryptionLocked()) return query.isBlank()
    return historySearchMatches(
        query = query,
        note.displayTitle(),
        note.previewBody(),
        note.searchableText(),
        folder?.name.orEmpty()
    )
}"####;

const LAZY_QUERY: &str = r####"internal fun noteMatchesQuery(
    query: String,
    note: NoteEntry,
    folder: NoteFolder?
): Boolean = documentNoteQueryMatches(query, note.isEncryptionLocked()) {
    arrayOf(note.displayTitle(), note.previewBody(), note.searchableText(), folder?.name.orEmpty())
}

// An empty query accepts the existing scope without projecting private bodies.
// A locked note remains visible only in that unfiltered query scope.
internal fun documentNoteQueryMatches(
    query: String,
    locked: Boolean,
    fields: () -> Array<String>
): Boolean {
    if (query.isBlank()) return true
    if (locked) return false
    return historySearchMatches(query, *fields())
}"####;

const FILTER: &str = r####"internal fun NoteEntry.matchesQuickFilter(filter: NoteQuickFilter): Boolean {
    if (isEncryptionLocked()) return filter == NoteQuickFilter.ALL
    NativeOptimizerBridge.matchesNoteQuickFilter(
        filterCode = filter.ordinal,
        pinned = pinned,
        todoRemaining = checklistStats()?.remaining ?: 0,
        imageAttachmentCount = attachmentCount(NoteAttachmentKind.IMAGE),
        hasFolder = folderId != null
    )?.let { return it }
    return when (filter) {
        NoteQuickFilter.ALL -> true
        NoteQuickFilter.PINNED -> pinned
        NoteQuickFilter.TODO -> (checklistStats()?.remaining ?: 0) > 0
        NoteQuickFilter.IMAGE -> attachmentCount(NoteAttachmentKind.IMAGE) > 0
        NoteQuickFilter.UNFILED -> folderId == null
    }
}"####;

const SCOPED_FILTER: &str = r####"internal fun NoteEntry.matchesQuickFilter(filter: NoteQuickFilter): Boolean {
    if (isEncryptionLocked()) return filter == NoteQuickFilter.ALL
    return when (filter) {
        NoteQuickFilter.ALL -> true
        NoteQuickFilter.PINNED -> pinned
        NoteQuickFilter.UNFILED -> folderId == null
        NoteQuickFilter.TODO -> {
            val remaining = checklistStats()?.remaining ?: 0
            NativeOptimizerBridge.matchesNoteQuickFilter(
                filterCode = filter.ordinal,
                pinned = false,
                todoRemaining = remaining,
                imageAttachmentCount = 0,
                hasFolder = false
            ) ?: (remaining > 0)
        }
        NoteQuickFilter.IMAGE -> {
            val imageCount = attachmentCount(NoteAttachmentKind.IMAGE)
            NativeOptimizerBridge.matchesNoteQuickFilter(
                filterCode = filter.ordinal,
                pinned = false,
                todoRemaining = 0,
                imageAttachmentCount = imageCount,
                hasFolder = false
            ) ?: (imageCount > 0)
        }
    }
}"####;

pub const TEST_CONTENTS: &str = r####"package com.ofairyo.gridtimer.ui

import com.ofairyo.gridtimer.data.*
import org.junit.Assert.*
import org.junit.Test

class DocumentCollectionWorkTest {
    @Test fun blankQueryAcceptsReadableAndLockedNotesWithoutInspectingFields() {
        for (query in listOf("", " \n\t")) {
            for (locked in listOf(false, true)) {
                assertTrue(documentNoteQueryMatches(query, locked) {
                    error("An unfiltered collection must not inspect document bodies")
                })
            }
        }
    }

    @Test fun nonblankQueryCannotReadLockedContentAndMatchesReadableFields() {
        assertFalse(documentNoteQueryMatches("alpha", true) {
            error("Locked document fields must not be inspected")
        })
        var reads = 0
        assertTrue(documentNoteQueryMatches("alpha", false) {
            reads++
            arrayOf("Alpha plan", "")
        })
        assertEquals(1, reads)
        assertFalse(documentNoteQueryMatches("missing", false) { arrayOf("Alpha plan") })
    }

    @Test fun quickFiltersKeepMetadataTodoImageAndLockedBoundaries() {
        val blank = NoteEntry(id = "blank", kind = NoteEntryKind.DOCUMENT)
        val image = NoteAttachment(id = "image", kind = NoteAttachmentKind.IMAGE)
        val todo = blank.copy(
            pinned = true, folderId = "work",
            document = NoteDocument(blocks = listOf(NoteBlock(type = NoteBlockType.TEXT, text = "- [ ] task"))),
            attachments = listOf(image)
        )
        assertTrue(blank.matchesQuickFilter(NoteQuickFilter.ALL))
        assertTrue(blank.matchesQuickFilter(NoteQuickFilter.UNFILED))
        assertFalse(blank.matchesQuickFilter(NoteQuickFilter.PINNED))
        assertFalse(blank.matchesQuickFilter(NoteQuickFilter.TODO))
        assertFalse(blank.matchesQuickFilter(NoteQuickFilter.IMAGE))
        assertTrue(todo.matchesQuickFilter(NoteQuickFilter.PINNED))
        assertFalse(todo.matchesQuickFilter(NoteQuickFilter.UNFILED))
        assertTrue(todo.matchesQuickFilter(NoteQuickFilter.TODO))
        assertTrue(todo.matchesQuickFilter(NoteQuickFilter.IMAGE))
        val completed = todo.copy(document = NoteDocument(blocks = listOf(NoteBlock(type = NoteBlockType.TEXT, text = "- [x] task"))))
        assertFalse(completed.matchesQuickFilter(NoteQuickFilter.TODO))
        assertTrue(completed.matchesQuickFilter(NoteQuickFilter.IMAGE))
        val locked = todo.copy(encryption = NoteEncryptionEnvelope(keyId = "locked"), encryptionUnlocked = false)
        assertTrue(locked.matchesQuickFilter(NoteQuickFilter.ALL))
        for (filter in listOf(NoteQuickFilter.PINNED, NoteQuickFilter.UNFILED, NoteQuickFilter.TODO, NoteQuickFilter.IMAGE)) {
            assertFalse(locked.matchesQuickFilter(filter))
        }
    }
}
"####;
