// Android's 100-day recycle-bin policy. The shared data format is unchanged.
// This function is deliberately opt-in: desktop and the sync server never schedule it.
pub const NOTE_TRASH_RETENTION_MILLIS: i64 = 100 * 24 * 60 * 60 * 1_000;

pub fn purge_expired_note_trash_app_data_json(raw: &str, now: i64) -> Option<String> {
    // Fail closed if the clock is invalid or subtraction would underflow.
    let cutoff = now.checked_sub(NOTE_TRASH_RETENTION_MILLIS)?;
    if cutoff < 0 {
        return None;
    }
    let mut data = parse_app_data_for_mutation(raw, now)?;
    let expired = |note: &NoteEntry| {
        note.deleted_at_epoch_millis
            .is_some_and(|deleted| deleted >= 0 && deleted <= cutoff)
    };
    let expired_ids = data.notes.iter()
        .filter(|note| expired(note))
        .map(|note| note.id.clone())
        .collect::<Vec<_>>();
    if expired_ids.is_empty() {
        // No new snapshot, write, or sync revision when nothing is due.
        return None;
    }

    // Apply the same hierarchy detachment and attachment-tombstone semantics as
    // the existing manual "empty trash" operation, but only to expired notes.
    for id in &expired_ids {
        if !knowledge_identity_conflict(&data, id) {
            knowledge_detach_children_for_delete(&mut data, id, now)?;
            knowledge_unlink_legacy_projection(&mut data, id, now)?;
        }
    }
    let removed_notes = data.notes.iter().filter(|note| expired(note))
        .cloned().collect::<Vec<_>>();
    data.notes.retain(|note| !expired(note));

    let mut removed_attachment_revisions = HashMap::<String, i64>::new();
    for note in removed_notes {
        let deletion_revision = next_deletion_revision(
            &data.tombstones,
            TOMBSTONE_ENTITY_NOTE,
            &note.id,
            note_revision_epoch_millis(&note),
            now,
        )?;
        upsert_data_tombstone(
            &mut data.tombstones,
            TOMBSTONE_ENTITY_NOTE,
            &note.id,
            deletion_revision,
        );
        for (attachment_id, revision) in note_attachment_revision_index(&note) {
            removed_attachment_revisions
                .entry(attachment_id)
                .and_modify(|existing| *existing = (*existing).max(revision))
                .or_insert(revision);
        }
    }
    for (attachment_id, attachment_revision) in removed_attachment_revisions {
        // Never delete media still used by a surviving active or trashed note.
        if !notes_reference_attachment(&data.notes, &attachment_id) {
            let deletion_revision = next_deletion_revision(
                &data.tombstones,
                TOMBSTONE_ENTITY_NOTE_ATTACHMENT,
                &attachment_id,
                attachment_revision,
                now,
            )?;
            upsert_data_tombstone(
                &mut data.tombstones,
                TOMBSTONE_ENTITY_NOTE_ATTACHMENT,
                &attachment_id,
                deletion_revision,
            );
            let media_revision = next_deletion_revision(
                &data.tombstones,
                TOMBSTONE_ENTITY_NOTE_MEDIA,
                &attachment_id,
                attachment_revision,
                now,
            )?;
            upsert_data_tombstone(
                &mut data.tombstones,
                TOMBSTONE_ENTITY_NOTE_MEDIA,
                &attachment_id,
                media_revision,
            );
        }
    }
    data.sanitized(now)
        .and_then(|sanitized| serde_json::to_string(&sanitized).ok())
}

#[cfg(test)]
mod trash_retention_tests {
    use super::*;
    use serde_json::{json, Value};

    const DAY: i64 = 24 * 60 * 60 * 1_000;
    const NOW: i64 = 1_800_000_000_000;

    fn add(raw: &str, id: &str) -> String {
        upsert_note_app_data_json(
            raw,
            &json!({
                "id": id,
                "title": id,
                "content": "retention test",
                "updatedAtEpochMillis": NOW - 150 * DAY
            }).to_string(),
            NOW - 150 * DAY,
        ).expect("create a note")
    }

    fn deleted(raw: &str, id: &str, when: i64) -> String {
        delete_note_app_data_json(raw, id, when).expect("move to trash")
    }

    fn has_note(raw: &str, id: &str) -> bool {
        let parsed: Value = serde_json::from_str(raw).unwrap();
        parsed["notes"].as_array().unwrap().iter().any(|note| note["id"] == id)
    }

    #[test]
    fn hundred_day_boundary_keeps_newer_notes_and_active_notes() {
        let data = default_app_data_json(NOW - 150 * DAY);
        let data = add(&data, "expired");
        let data = add(&data, "newer");
        let data = add(&data, "active");
        let data = deleted(&data, "expired", NOW - 100 * DAY);
        let data = deleted(&data, "newer", NOW - 100 * DAY + 1);
        let result = purge_expired_note_trash_app_data_json(&data, NOW).unwrap();
        assert!(!has_note(&result, "expired"));
        assert!(has_note(&result, "newer"));
        assert!(has_note(&result, "active"));
        let parsed: Value = serde_json::from_str(&result).unwrap();
        assert!(parsed["tombstones"].as_array().unwrap().iter().any(|tombstone|
            tombstone["entityType"] == "note" && tombstone["entityId"] == "expired"));
        assert!(purge_expired_note_trash_app_data_json(&result, NOW).is_none());
    }

    #[test]
    fn restored_notes_and_clock_anomalies_never_auto_purge() {
        let data = default_app_data_json(NOW - 150 * DAY);
        let data = add(&data, "restored");
        let data = deleted(&data, "restored", NOW - 120 * DAY);
        let data = restore_note_app_data_json(&data, "restored", NOW - 2 * DAY).unwrap();
        assert!(purge_expired_note_trash_app_data_json(&data, NOW).is_none());
        assert!(purge_expired_note_trash_app_data_json(&data, 0).is_none());
        assert!(purge_expired_note_trash_app_data_json("not json", NOW).is_none());
    }

    #[test]
    fn shared_attachments_survive_until_last_owner_is_removed() {
        let data = default_app_data_json(NOW - 150 * DAY);
        let mut data = data;
        for id in ["expired", "survivor"] {
            data = upsert_note_app_data_json(
                &data,
                &json!({
                    "id": id, "title": id,
                    "content": "shared media",
                    "updatedAtEpochMillis": NOW - 150 * DAY,
                    "attachments": [{
                        "id": "shared-image", "mimeType": "image/png",
                        "sha256": "a".repeat(64), "sizeBytes": 4,
                        "updatedAtEpochMillis": NOW - 150 * DAY
                    }],
                    "document": {"blocks": [{
                        "id": format!("{id}-image"), "type": "IMAGE",
                        "attachmentId": "shared-image"
                    }]}
                }).to_string(),
                NOW - 150 * DAY,
            ).unwrap();
        }
        let data = deleted(&data, "expired", NOW - 101 * DAY);
        let result = purge_expired_note_trash_app_data_json(&data, NOW).unwrap();
        assert!(has_note(&result, "survivor"));
        let parsed: Value = serde_json::from_str(&result).unwrap();
        assert!(!parsed["tombstones"].as_array().unwrap().iter().any(|tombstone|
            tombstone["entityType"] == "noteMedia" && tombstone["entityId"] == "shared-image"));
    }
}
