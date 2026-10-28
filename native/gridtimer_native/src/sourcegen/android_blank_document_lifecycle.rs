// v2.23.2.59 - A brand-new untouched blank knowledge page is a transient editor draft.
// Do not queue a full-workspace creation write before its first real edit.
// Existing pages and nonblank presets retain the verified durable-save path.

const STUDIO: &str = "com/ofairyo/gridtimer/ui/NoteStudioSheet.kt";
const DOCUMENT: &str = "com/ofairyo/gridtimer/ui/NoteDocumentEditor.kt";

fn replace_exact(source: &mut String, before: &str, after: &str) -> Result<(), String> {
    if source.matches(before).count() != 1 {
        return Err(format!(
            "blank document lifecycle anchor missing or duplicated: {}",
            before.lines().next().unwrap_or("")
        ));
    }
    *source = source.replacen(before, after, 1);
    Ok(())
}

pub fn render(path: &str, base: &str) -> Result<String, String> {
    let mut source = base.to_owned();
    match path {
        STUDIO => {
            // Limit the change to the DOCUMENT studio. Sticky-note creation must
            // continue using its existing synchronous admission and recovery policy.
            let from = source.find("internal fun NoteStudioSheet(")
                .ok_or("missing document studio")?;
            let until = source[from..].find("internal fun StickyNoteStudioSheet(")
                .map(|offset| from + offset)
                .ok_or("missing sticky studio")?;
            let mut studio = source[from..until].to_owned();
            let create_from = studio.find("onCreateNote = create@ { preset ->")
                .ok_or("missing document creation callback")?;
            let create_until = studio[create_from..].find("onOpenNote = open@ { note ->")
                .map(|offset| create_from + offset)
                .ok_or("missing document open callback")?;
            let mut creation = studio[create_from..create_until].to_owned();
            replace_exact(
                &mut creation,
                "                        viewModel.upsertNoteAndFlushResult(\n                            note = freshNote,",
                "                        // New blank pages exist only in pendingCreatedNote until edited.\n                        // Otherwise opening and backing out causes two expensive full-snapshot writes.\n                        if (preset != NoteDraftPreset.BLANK) {\n                        viewModel.upsertNoteAndFlushResult(\n                            note = freshNote,"
            )?;
            replace_exact(
                &mut creation,
                "                        }\n                    },",
                "                        }\n                        }\n                    },"
            )?;
            studio.replace_range(create_from..create_until, &creation);
            replace_exact(
                &mut studio,
                "                    note = documentNote,",
                "                    note = documentNote,\n                    // A pending blank page was never submitted to the repository.\n                    // Revoke this fast path as soon as a real record is published.\n                    transientBlankAtEntry = pendingCreatedNote?.id == documentNote.id &&\n                        appData.notes.none { it.id == documentNote.id },"
            )?;
            source.replace_range(from..until, &studio);
        }
        DOCUMENT => {
            replace_exact(
                &mut source,
                "    onAskKnowledge: (NoteEntry) -> Unit = {}\n) {",
                "    onAskKnowledge: (NoteEntry) -> Unit = {},\n    transientBlankAtEntry: Boolean = false\n) {"
            )?;
            // canLeaveUnmodified already proves zero accepted editor mutations,
            // exact entry-content equality, matching workspace and no restore.
            // The sole exception for ordinary blank pages is required deletion;
            // a transient blank has no row to delete and must not enter the queue.
            replace_exact(
                &mut source,
                "        val visitNeedsBlankDeletion = deleteBlankDraft && blankDraftDeletionEligibleAtEntry &&\n            !blankDraftDeleted.value",
                "        val visitNeedsBlankDeletion = deleteBlankDraft && blankDraftDeletionEligibleAtEntry &&\n            !transientBlankAtEntry && !blankDraftDeleted.value"
            )?;
        }
        _ => {}
    }
    Ok(source)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn studio_sample() -> String {
        r####"internal fun NoteStudioSheet() {
                    onCreateNote = create@ { preset ->
                        pendingCreatedNote = freshNote
                        viewModel.upsertNoteAndFlushResult(
                            note = freshNote,
                            reason = "note_create"
                        ) { result ->
                            report(result)
                        }
                    },
                    onOpenNote = open@ { note ->
                    },
                    note = documentNote,
}
internal fun StickyNoteStudioSheet() {
                        viewModel.upsertNoteAndFlushResult(
                            note = freshNote,
                            reason = "note_create"
                        )
}"####.to_owned()
    }

    #[test]
    fn only_blank_document_creation_is_deferred_and_sticky_path_is_untouched() {
        let original = studio_sample();
        let changed = render(STUDIO, &original).unwrap();
        let doc = changed.split("internal fun StickyNoteStudioSheet()").next().unwrap();
        let sticky = changed.split("internal fun StickyNoteStudioSheet()").nth(1).unwrap();
        assert!(doc.contains("if (preset != NoteDraftPreset.BLANK) {"));
        assert!(doc.contains("transientBlankAtEntry = pendingCreatedNote?.id == documentNote.id"));
        assert!(doc.contains("appData.notes.none { it.id == documentNote.id }"));
        assert_eq!(sticky, original.split("internal fun StickyNoteStudioSheet()").nth(1).unwrap());
    }

    #[test]
    fn document_exit_skips_storage_only_for_unpersisted_and_unmodified_visit() {
        let base = "    onAskKnowledge: (NoteEntry) -> Unit = {}\n) {\n        val visitNeedsBlankDeletion = deleteBlankDraft && blankDraftDeletionEligibleAtEntry &&\n            !blankDraftDeleted.value";
        let result = render(DOCUMENT, base).unwrap();
        assert!(result.contains("transientBlankAtEntry: Boolean = false"));
        assert!(result.contains("!transientBlankAtEntry && !blankDraftDeleted.value"));
        // An edited page still fails the existing canLeaveUnmodified gate and
        // therefore reaches its durable commit/deletion path.
        assert!(!result.contains("onComplete(true)"));
    }

    #[test]
    fn missing_or_ambiguous_anchors_fail_closed() {
        assert!(render(STUDIO, "a different screen").is_err());
        assert!(render(DOCUMENT, "a different editor").is_err());
        let repeated = studio_sample().replace(
            "                    note = documentNote,",
            "                    note = documentNote,\n                    note = documentNote,"
        );
        assert!(render(STUDIO, &repeated).is_err());
        assert_eq!(render("other/file.kt", "untouched").unwrap(), "untouched");
    }
}
