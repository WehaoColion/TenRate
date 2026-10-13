// v2.23.2.28 - Preserve knowledge editor identity during a bounded exit fade.
// Android presentation and its access policy are authored in this Rust emitter.

const STUDIO: &str = "com/ofairyo/gridtimer/ui/NoteStudioSheet.kt";
pub const STRUCTURED_PATH: &str = "com/ofairyo/gridtimer/ui/StructuredKnowledgeReader.kt";
pub const PATH: &str = "com/ofairyo/gridtimer/ui/KnowledgeDocumentTransition.kt";
pub const TEST_PATH: &str = "com/ofairyo/gridtimer/ui/KnowledgeDocumentTransitionTest.kt";

fn replace_once(source: &mut String, old: &str, new: &str) -> Result<(), String> {
    if source.matches(old).count() != 1 {
        return Err(format!(
            "knowledge document transition anchor mismatch: {}",
            old.lines().next().unwrap_or("")
        ));
    }
    *source = source.replacen(old, new, 1);
    Ok(())
}

pub fn render(path: &str, base: &str) -> Result<String, String> {
    let mut source = base.to_owned();
    if path == STRUCTURED_PATH {
        replace_once(&mut source,
            "    val workspace = LocalNoteMediaWorkspaceKey.current",
            "    val workspace = LocalNoteMediaWorkspaceKey.current\n    val transitionActive = LocalDocumentTransitionActive.current\n    val latestTransitionActive by rememberUpdatedState(transitionActive)\n    val transitionFocusManager = androidx.compose.ui.platform.LocalFocusManager.current")?;
        replace_once(
            &mut source,
            "    val editable = onWrite != null && note.canEditStructured()",
            "    val editable = transitionActive && onWrite != null && note.canEditStructured()",
        )?;
        replace_once(&mut source,
            "    LaunchedEffect(editable) {\n        if (!editable) { draft = null; insertOpen = false }\n    }",
            "    LaunchedEffect(editable, transitionActive) {\n        if (!editable) { draft = null; insertOpen = false }\n        if (!transitionActive) {\n            outlineOpen = false; scrollTarget = null\n            transitionFocusManager.clearFocus(force = true)\n        }\n    }")?;
        replace_once(&mut source,
            "        if (!editable || saving || request.noteId != latestNote.id) return",
            "        if (!latestTransitionActive || !editable || saving || request.noteId != latestNote.id) return")?;
        replace_once(
            &mut source,
            "        if (request == null || saving || !editable) return",
            "        if (!latestTransitionActive || request == null || saving || !editable) return",
        )?;
        replace_once(&mut source,
            "    fun locate(node: StructuredOutlineNode) {",
            "    fun locate(node: StructuredOutlineNode) {\n        if (!latestTransitionActive) return")?;
        replace_once(
            &mut source,
            "    if (outlineOpen) AlertDialog(",
            "    if (transitionActive && outlineOpen) AlertDialog(",
        )?;
        replace_once(
            &mut source,
            "    if (insertOpen) AlertDialog(",
            "    if (transitionActive && insertOpen) AlertDialog(",
        )?;
        replace_once(
            &mut source,
            "    if (currentDraft != null && editable) {",
            "    if (transitionActive && currentDraft != null && editable) {",
        )?;
        replace_once(&mut source,
            "    BackHandler(enabled = draft == null && !outlineOpen && !insertOpen) { if (!saving) onBack() }",
            "    BackHandler(enabled = transitionActive && draft == null && !outlineOpen && !insertOpen) { if (!saving) onBack() }")?;
        replace_once(&mut source,
            "            TextButton(enabled = !saving, onClick = onBack) { Text(\"返回\") }",
            "            TextButton(enabled = transitionActive && !saving, onClick = onBack) { Text(\"返回\") }")?;
        return Ok(source);
    }
    if path != STUDIO {
        return Ok(source);
    }
    replace_once(&mut source, "package com.ofairyo.gridtimer.ui\n",
        "package com.ofairyo.gridtimer.ui\nimport androidx.compose.animation.fadeIn\nimport androidx.compose.animation.fadeOut\nimport androidx.compose.animation.togetherWith\nimport androidx.compose.animation.core.tween\nimport androidx.compose.ui.semantics.clearAndSetSemantics\n")?;
    let start = source
        .find("internal fun NoteStudioSheet(")
        .ok_or("missing knowledge studio")?;
    let end = source[start..]
        .find("internal fun StickyNoteStudioSheet(")
        .map(|offset| start + offset)
        .ok_or("missing sticky studio boundary")?;
    let mut studio = source[start..end].to_owned();
    replace_once(&mut studio,
        "        } else AnimatedContent(targetState = selectedNoteId != null, label = \"note-studio-mode\") { editing ->\n            if (!editing || selectedNote == null) {",
        TRANSITION_START)?;
    replace_once(&mut studio,
        "                    onCreateNote = { preset ->",
        "                    onCreateNote = create@ { preset ->\n                        if (!isCurrentFrame()) return@create")?;
    replace_once(&mut studio,
        "                    onOpenNote = { note ->",
        "                    onOpenNote = open@ { note ->\n                        if (!isCurrentFrame()) return@open")?;
    replace_once(
        &mut studio,
        "                    onOpenCanvas = { canvasVisible = true },",
        "                    onOpenCanvas = { if (isCurrentFrame()) canvasVisible = true },",
    )?;
    replace_once(&mut studio,
        "                    onAskKnowledge = {\n                        knowledgePriorityNoteId = null",
        "                    onAskKnowledge = askCollection@ {\n                        if (!isCurrentFrame()) return@askCollection\n                        knowledgePriorityNoteId = null")?;
    replace_once(&mut studio,
        "                    onOpenStickySource = { note -> stickySourcePreview = note },",
        "                    onOpenStickySource = { note -> if (isCurrentFrame()) stickySourcePreview = note },")?;
    replace_once(&mut studio,
        "                    onManageFolders = { folderManagerVisible = true },",
        "                    onManageFolders = { if (isCurrentFrame()) folderManagerVisible = true },")?;
    replace_once(&mut studio,
        "                    onTogglePinned = { note -> viewModel.setNotePinned(note.id, !note.pinned) },",
        "                    onTogglePinned = { note -> if (isCurrentFrame()) viewModel.setNotePinned(note.id, !note.pinned) },")?;
    replace_once(&mut studio,
        "                    onRestoreNote = { note -> viewModel.restoreNote(note.id) },",
        "                    onRestoreNote = { note -> if (isCurrentFrame()) viewModel.restoreNote(note.id) },")?;
    replace_once(&mut studio,
        "                    onDeleteTrashedNote = { note -> deleteTarget = DeleteTarget.PermanentNote(note) },",
        "                    onDeleteTrashedNote = { note -> if (isCurrentFrame()) deleteTarget = DeleteTarget.PermanentNote(note) },")?;
    replace_once(&mut studio,
        "                    onEmptyTrash = {\n                        if (filteredTrashedNotes.isNotEmpty()) {",
        "                    onEmptyTrash = {\n                        if (isCurrentFrame() && filteredTrashedNotes.isNotEmpty()) {")?;
    replace_once(&mut studio,
        "            } else {\n                NoteEditorContent(\n                    note = selectedNote,",
        "            } else if (frameAccess.mayDrawEditor()) {\n                val documentNote = frame.note\n                androidx.compose.runtime.key(frame.entryKey) {\n                androidx.compose.runtime.CompositionLocalProvider(LocalDocumentTransitionActive provides interactiveFrame) {\n                NoteEditorContent(\n                    note = documentNote,")?;
    replace_once(&mut studio,
        "                    onBack = {\n                        selectedNote?.id?.let(viewModel::lockEncryptedNote)",
        "                    onBack = back@ {\n                        if (!isCurrentFrame() || !frameAccess.mayInteract()) return@back\n                        viewModel.lockEncryptedNote(documentNote.id)")?;
    replace_once(&mut studio,
        "                    onRequestDelete = { deleteTarget = DeleteTarget.TrashNote(selectedNote) },",
        "                    onRequestDelete = { if (isCurrentFrame() && frameAccess.mayInteract()) deleteTarget = DeleteTarget.TrashNote(documentNote) },")?;
    replace_once(&mut studio,
        "                        attachmentPreview = AttachmentPreviewState(selectedNote.id, attachmentId)",
        "                        if (isCurrentFrame() && frameAccess.mayInteract()) attachmentPreview = AttachmentPreviewState(documentNote.id, attachmentId)")?;
    replace_once(&mut studio,
        "                    onAskKnowledge = { note ->",
        "                    onAskKnowledge = ask@ { note ->\n                        if (!isCurrentFrame() || !frameAccess.mayInteract()) return@ask")?;
    replace_once(
        &mut studio,
        "                )\n            }\n        }\n    }\n\n    if (asPage) {",
        TRANSITION_END,
    )?;
    source.replace_range(start..end, &studio);
    Ok(source)
}

const TRANSITION_START: &str = r####"        } else androidx.compose.runtime.key(workspaceKey) {
            // The target carries the editor record. The outgoing editor must not
            // consult selectedNote after selection has already returned to null.
            val transitionEntryKey = remember(workspaceKey, selectedNoteId) { java.util.UUID.randomUUID().toString() }
            val latestTransitionEntryKey = rememberUpdatedState(transitionEntryKey)
            val latestTransitionSelectedNoteId = rememberUpdatedState(selectedNoteId)
            val transitionTarget = selectedNote?.let { KnowledgeDocumentTransitionTarget(it, transitionEntryKey) }
            val renderFrame: @Composable (KnowledgeDocumentTransitionTarget?) -> Unit = { frame ->
                fun isCurrentFrame(): Boolean = knowledgeDocumentFrameIsCurrent(
                    frameNoteId = frame?.note?.id,
                    frameEntryKey = frame?.entryKey ?: transitionEntryKey,
                    selectedNoteId = latestTransitionSelectedNoteId.value,
                    currentEntryKey = latestTransitionEntryKey.value,
                    workspaceMatches = viewModel.currentWorkspaceKey() == workspaceKey
                )
                val currentFrame = isCurrentFrame()
                val repositoryRecord = frame?.note?.id?.let { id -> appData.notes.firstOrNull { it.id == id } }
                val frameAccess = KnowledgeDocumentTransitionAccess(
                    workspaceMatches = viewModel.currentWorkspaceKey() == workspaceKey,
                    currentEntry = currentFrame,
                    snapshotProtected = frame?.note?.encryption != null,
                    recordPresent = repositoryRecord != null,
                    recordProtected = repositoryRecord?.encryption != null,
                    recordDeleted = repositoryRecord?.isDeleted() == true
                )
                val interactiveFrame = currentFrame && (frame == null || frameAccess.mayInteract())
                Box(modifier = if (interactiveFrame) Modifier.fillMaxSize() else
                    Modifier.fillMaxSize().clearAndSetSemantics {}) {
            if (frame == null) {"####;

const TRANSITION_END: &str = r####"                )
                }
                }
            }
                if (!interactiveFrame) DocumentEditorInputBarrier()
                }
            }
            // An unlocked protected record is never retained as an outgoing
            // animation target. Locking or changing workspace disposes it now.
            if (selectedNote?.encryption != null) {
                renderFrame(transitionTarget)
            } else {
                AnimatedContent(
                    targetState = transitionTarget,
                    modifier = Modifier.fillMaxSize(),
                    transitionSpec = {
                        (fadeIn(animationSpec = tween(160)) togetherWith
                            fadeOut(animationSpec = tween(120))).using(null)
                    },
                    contentKey = { (it != null) to it?.note?.id },
                    label = "knowledge-document-transition"
                ) { frame -> renderFrame(frame) }
            }
        }
    }

    if (asPage) {"####;

pub const CONTENTS: &str = r####"package com.ofairyo.gridtimer.ui

import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.runtime.Composable
import androidx.compose.runtime.compositionLocalOf
import androidx.compose.ui.Modifier
import androidx.compose.ui.input.pointer.PointerEventPass
import androidx.compose.ui.input.pointer.pointerInput
import com.ofairyo.gridtimer.data.NoteEntry

internal data class KnowledgeDocumentTransitionTarget(val note: NoteEntry, val entryKey: String)

internal val LocalDocumentTransitionActive = compositionLocalOf { true }

internal fun knowledgeDocumentFrameIsCurrent(
    frameNoteId: String?, frameEntryKey: String?, selectedNoteId: String?,
    currentEntryKey: String, workspaceMatches: Boolean
): Boolean = workspaceMatches && frameNoteId == selectedNoteId &&
    frameEntryKey == currentEntryKey

// This policy grants presentation only. Durable save and error handling remain
// in the retained editor save gate. Protected or deleted snapshots cannot outlive
// their active selection, and a workspace change revokes all access immediately.
internal data class KnowledgeDocumentTransitionAccess(
    val workspaceMatches: Boolean,
    val currentEntry: Boolean,
    val snapshotProtected: Boolean,
    val recordPresent: Boolean,
    val recordProtected: Boolean,
    val recordDeleted: Boolean
) {
    fun mayDrawEditor(): Boolean {
        if (!workspaceMatches || recordDeleted) return false
        if (currentEntry) {
            return if (snapshotProtected) recordPresent && recordProtected else !recordProtected
        }
        return !snapshotProtected && recordPresent && !recordProtected
    }

    fun mayInteract(): Boolean = currentEntry && mayDrawEditor()
}

// A transparent input barrier leaves the editor's layout and pixels untouched.
// It can also be used by the current editor while its captured exit is saving.
@Composable
internal fun DocumentEditorInputBarrier(modifier: Modifier = Modifier) {
    Box(modifier = modifier.fillMaxSize().pointerInput(Unit) {
        awaitPointerEventScope {
            while (true) {
                awaitPointerEvent(PointerEventPass.Initial).changes.forEach { it.consume() }
            }
        }
    })
}
"####;

pub const TEST_CONTENTS: &str = r####"package com.ofairyo.gridtimer.ui

import org.junit.Assert.*
import org.junit.Test

class KnowledgeDocumentTransitionTest {
    private fun access(current: Boolean = false, snapshotProtected: Boolean = false,
        present: Boolean = true, protectedNow: Boolean = false, deleted: Boolean = false,
        workspace: Boolean = true) = KnowledgeDocumentTransitionAccess(
            workspace, current, snapshotProtected, present, protectedNow, deleted)

    @Test fun ordinaryExitMayDrawButCannotInteract() {
        val outgoing = access()
        assertTrue(outgoing.mayDrawEditor())
        assertFalse(outgoing.mayInteract())
    }

    @Test fun protectedEditorRequiresItsCurrentSelection() {
        assertTrue(access(current = true, snapshotProtected = true, protectedNow = true).mayInteract())
        assertFalse(access(snapshotProtected = true, protectedNow = true).mayDrawEditor())
    }

    @Test fun newlyProtectedRecordRevokesOldPlainSnapshot() {
        assertFalse(access(protectedNow = true).mayDrawEditor())
        assertFalse(access(current = true, protectedNow = true).mayDrawEditor())
    }

    @Test fun workspaceChangeRevokesBothActiveAndOutgoingEditors() {
        assertFalse(access(workspace = false).mayDrawEditor())
        assertFalse(access(current = true, workspace = false).mayInteract())
    }

    @Test fun deletedOrMissingRecordCannotRemainAsOutgoingContent() {
        assertFalse(access(deleted = true).mayDrawEditor())
        assertFalse(access(present = false).mayDrawEditor())
        assertFalse(access(current = true, deleted = true).mayDrawEditor())
    }

    @Test fun unsavedCurrentPlainDraftCanStillBeEdited() {
        assertTrue(access(current = true, present = false).mayInteract())
        assertFalse(access(current = true, snapshotProtected = true, present = false).mayDrawEditor())
    }

    @Test fun onlyCurrentEntryMayNavigateAgain() {
        assertTrue(access(current = true).mayInteract())
        assertFalse(access(current = false).mayInteract())
    }

    @Test fun oldCallbackCannotCloseAReopenedRecordOrAnotherWorkspace() {
        assertTrue(knowledgeDocumentFrameIsCurrent("A", "entry-2", "A", "entry-2", true))
        assertFalse(knowledgeDocumentFrameIsCurrent("A", "entry-1", "A", "entry-2", true))
        assertFalse(knowledgeDocumentFrameIsCurrent("A", "entry-2", "B", "entry-2", true))
        assertFalse(knowledgeDocumentFrameIsCurrent("A", "entry-2", "A", "entry-2", false))
        assertTrue(knowledgeDocumentFrameIsCurrent(null, "collection", null, "collection", true))
        assertFalse(knowledgeDocumentFrameIsCurrent(null, "collection", "A", "entry-A", true))
        assertFalse(knowledgeDocumentFrameIsCurrent(null, "collection-1", null, "collection-2", true))
    }
}
"####;
