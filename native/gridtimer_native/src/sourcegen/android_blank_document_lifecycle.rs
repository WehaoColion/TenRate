// v2.23.2.59 - Untouched blank knowledge pages are read-only visits, not save/delete operations.
// New blank pages remain transient until edited; existing blank pages remain intact.

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
            // Only the knowledge-document studio is changed. Sticky notes must
            // preserve their own creation/admission and recovery contract.
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
                "                        // A new blank page exists in pendingCreatedNote.\n                        // Do not commit a whole AppData snapshot just to show it.\n                        if (preset != NoteDraftPreset.BLANK) {\n                            viewModel.upsertNoteAndFlushResult(\n                            note = freshNote,"
            )?;
            replace_exact(
                &mut creation,
                "                        }\n                    },",
                "                        }\n                        }\n                    },"
            )?;
            studio.replace_range(create_from..create_until, &creation);
            source.replace_range(from..until, &studio);
        }
        DOCUMENT => {
            // Back is navigation, not an explicit request to delete an existing
            // empty note. An untouched entry should never wait on another
            // caller's IO or destructively delete its already persisted record.
            // canLeaveUnmodified still requires matching content/workspace,
            // editGeneration == 0, no pending persistence and no restore.
            // Edited, reverted or modified documents retain the original
            // durable save / blank-deletion path below this early return.
            replace_exact(
                &mut source,
                "        val visitNeedsBlankDeletion = deleteBlankDraft && blankDraftDeletionEligibleAtEntry &&\n            !blankDraftDeleted.value",
                "        // Unchanged navigation does not request deletion of a blank page.\n        val visitNeedsBlankDeletion = false"
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
        assert!(doc.contains("pendingCreatedNote = freshNote"));
        assert!(doc.contains("viewModel.upsertNoteAndFlushResult("));
        assert_eq!(sticky, original.split("internal fun StickyNoteStudioSheet()").nth(1).unwrap());
    }

    #[test]
    fn existing_and_new_untouched_blank_pages_can_exit_without_deletion() {
        let base = "        val visitNeedsBlankDeletion = deleteBlankDraft && blankDraftDeletionEligibleAtEntry &&\n            !blankDraftDeleted.value";
        let result = render(DOCUMENT, base).unwrap();
        assert!(result.contains("val visitNeedsBlankDeletion = false"));
        assert!(!result.contains("!blankDraftDeleted.value"));
        // The session's independent content, edit-generation and pending-write
        // checks still reject any modified/reverted/undurable draft.
        assert!(!result.contains("onComplete(true)"));
    }

    #[test]
    fn missing_or_ambiguous_anchors_fail_closed() {
        assert!(render(STUDIO, "a different screen").is_err());
        assert!(render(DOCUMENT, "a different editor").is_err());
        let anchor = "                        viewModel.upsertNoteAndFlushResult(\n                            note = freshNote,";
        let repeated = studio_sample().replacen(
            anchor,
            &format!("{anchor}\n{anchor}"),
            1
        );
        assert!(render(STUDIO, &repeated).is_err());
        assert_eq!(render("other/file.kt", "untouched").unwrap(), "untouched");
    }
}
