#[cfg(test)]
mod desktop_legal_destination_tests {
    use super::*;
    #[test]
    fn report_jump_checks_current_note_block_attachment_and_deletion() {
        let mut data: DesktopAppData = serde_json::from_value(json!({"notes":[{
            "id":"n","title":"当前","content":"正文","kind":"DOCUMENT","updatedAtEpochMillis":5,
            "attachments":[{"id":"current"},{"id":"other"},{"id":"history-only"}],
            "document":{"blocks":[{"id":"b","type":"IMAGE","attachmentId":"current"},{"id":"c","type":"IMAGE","attachmentId":"other"}]},
            "versions":[{"id":"old","attachments":[{"id":"history-only"}]}]
        }]}))
        .unwrap();
        assert!(matches!(
            desktop_legal_destination(&data, "note/n/current/block/b/attachment/current"),
            Some(DesktopLegalDestination::Note { .. })
        ));
        for path in [
            "note/n/current/block/missing",
            "note/n/current/block/b/attachment/other",
            "note/n/current/attachment/history-only",
            "note/n/versions/old",
            "note/n/../b",
            "note/n/",
        ] {
            assert_eq!(desktop_legal_destination(&data, path), None, "{path}");
        }
        data.notes[0].deleted_at_epoch_millis = Some(6);
        assert_eq!(desktop_legal_destination(&data, "note/n"), None);
        data.notes[0].deleted_at_epoch_millis = None;
        data.extra.insert(
            "tombstones".into(),
            json!([{"entityType":"note","entityId":"n","deletedAtEpochMillis":6}]),
        );
        assert_eq!(desktop_legal_destination(&data, "note/n"), None);
    }
    #[test]
    fn attachments_only_follow_current_visible_references() {
        let note: DesktopNote = serde_json::from_value(json!({"id":"n","title":"","content":"", "attachments":[{"id":"visible"},{"id":"old"},{"id":"removed"}],
            "document":{"richTextEnabled":true,"blocks":[{"id":"text","text":"<img data-note-image=\"visible\">"},
                {"id":"removed-block","attachmentId":"removed","deletedAtEpochMillis":1}]},
            "revisions":[{"id":"r","attachments":[{"id":"old"}]}]})).unwrap();
        assert_eq!(
            desktop_legal_current_attachments(&note)
                .iter()
                .map(|a| a.id.as_str())
                .collect::<Vec<_>>(),
            vec!["visible"]
        );
    }
    #[test]
    fn evidence_navigation_preserves_selection_when_scope_or_source_is_invalid() {
        let root = temp_test_dir("legal_evidence_scope_navigation");
        let mut client = knowledge_test_client(&root);
        client.desktop_ui.legal_risk.scope = client.legal_scope();
        let before = client.selected_note_id.clone();
        client.desktop_ui.legal_risk.scope.push_str("-old-account");
        client.open_legal_evidence("note/doc-b");
        assert_eq!(client.selected_note_id, before);
        client.desktop_ui.legal_risk.scope = client.legal_scope();
        client.open_legal_evidence("note/deleted-or-missing");
        assert_eq!(client.selected_note_id, before);
        client.open_legal_evidence("note/doc-b");
        assert!(matches!(client.tab, AppTab::Knowledge));
        assert_eq!(client.selected_note_id, "doc-b");
        assert!(client.desktop_ui.navigation.editor_visible);
        assert!(client.selected_note_version_id.is_empty());
        drop(client);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn exact_history_source_does_not_match_another_id_and_scope_change_clears_it() {
        let root = temp_test_dir("legal_exact_history_source");
        let mut client = knowledge_test_client(&root);
        let mut raw: Value = serde_json::from_str(&client.state_json).unwrap();
        raw["sessions"] = json!([
            {"id":"s1","slotTitle":"first","slotId":1,"startedAtEpochMillis":1,"endedAtEpochMillis":10,"durationMillis":9},
            {"id":"s10","slotTitle":"similar","slotId":1,"startedAtEpochMillis":10,"endedAtEpochMillis":20,"durationMillis":10}
        ]);
        client.state_json = raw.to_string();
        client.data = serde_json::from_str(&client.state_json).unwrap();
        client.desktop_ui.history_version = u64::MAX;
        client.desktop_ui.legal_risk.history_source = Some((
            client.background_job_workspace_fingerprint(),
            false,
            "s1".into(),
        ));
        client.refresh_history_index(now_millis());
        assert_eq!(client.desktop_ui.history_session_rows.len(), 1);
        let index = client.desktop_ui.history_index.as_ref().unwrap();
        assert_eq!(
            index.sessions[client.desktop_ui.history_session_rows[0]].id,
            "s1"
        );
        client
            .desktop_ui
            .legal_risk
            .history_source
            .as_mut()
            .unwrap()
            .0
            .push_str("-other");
        client.refresh_history_index(now_millis());
        assert!(client.desktop_ui.legal_risk.history_source.is_none());
        assert_eq!(client.desktop_ui.history_session_rows.len(), 2);
        drop(client);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn finance_links_select_exact_existing_entry_and_reject_invalid_periods() {
        let mut data: DesktopAppData = serde_json::from_value(json!({"financeProfile":{"dailyLedgers":{"2026-10-15":{"expenses":[{"id":"a"},{"id":"b"}]}}}})).unwrap();
        assert_eq!(
            desktop_legal_destination(&data, "finance/day/2026-10-15/expenses/b"),
            Some(DesktopLegalDestination::Finance {
                monthly: false,
                period: "2026-10-15".into(),
                entry: Some(("expenses".into(), "b".into(), 0))
            })
        );
        assert_eq!(
            desktop_legal_destination(&data, "finance/day/2026-10-15/expenses/missing"),
            None
        );
        assert_eq!(
            desktop_legal_destination(&data, "finance/day/2026-99-00"),
            None
        );
        assert_eq!(desktop_legal_destination(&data, "session/missing"), None);
        data.finance_profile.extra.get_mut("dailyLedgers").unwrap()["2026-10-15"]["expenses"] = json!([
            {"id":"b","deletedAtEpochMillis":2},{"id":"b"}
        ]);
        assert_eq!(
            desktop_legal_destination(&data, "finance/day/2026-10-15/expenses/b"),
            None
        );
    }
    #[test]
    fn removed_block_subtrees_are_neither_scanned_enumerated_nor_report_destinations() {
        let raw = json!({"notes":[{
            "id":"n","title":"current note","content":"",
            "attachments":[{"id":"removed-file","fileName":"private-removed-file.txt"}],
            "document":{
                "knowledge":{"comments":[{"id":"c","blockId":"child","body":"private-child-comment"}]},
                "blocks":[
                    {"id":"parent","text":"private-removed-parent","deletedAtEpochMillis":1},
                    {"id":"child","text":"private-removed-child","attachmentId":"removed-file","knowledge":{"parentId":"parent"}},
                    {"id":"grandchild","text":"private-removed-grandchild","knowledge":{"parentId":"child"}},
                    {"id":"live","text":"retained current body"}
                ]
            }
        }]}).to_string();
        let data: DesktopAppData = serde_json::from_str(&raw).unwrap();
        assert!(desktop_legal_current_attachments(&data.notes[0]).is_empty());
        for path in [
            "note/n/current/block/parent",
            "note/n/current/block/child",
            "note/n/current/block/grandchild",
            "note/n/current/block/child/attachment/removed-file",
            "note/n/current/attachment/removed-file",
        ] {
            assert_eq!(desktop_legal_destination(&data, path), None, "{path}");
        }
        assert!(desktop_legal_destination(&data, "note/n/current/block/live").is_some());
        let snapshot = desktop_legal_current_scan_snapshot(&raw, &data).unwrap();
        let prepared = gridtimer_native::legal_scan::prepare_scan(
            &snapshot,
            "synthetic-current-scope",
            500,
            "[]",
            "[]",
        )
        .unwrap();
        let prepared_json = serde_json::to_string(&prepared).unwrap();
        assert!(prepared_json.contains("retained current body"));
        assert!(!prepared_json.contains("private-"));
        assert!(!prepared_json.contains("removed-file"));
        assert!(raw.contains("private-removed-child"));
        assert_eq!(data.notes[0].document.blocks.len(), 4);
    }

    #[test]
    fn explicit_empty_rich_document_never_scans_or_opens_a_retained_old_attachment_catalog() {
        let raw = json!({"notes":[{
            "id":"n","title":"current empty rich document","content":"",
            "attachments":[{"id":"old-file","fileName":"private-old-file.txt"}],
            "attachmentIds":["old-file"],
            "document":{"richTextEnabled":true,"blocks":[]}
        }]})
        .to_string();
        let data: DesktopAppData = serde_json::from_str(&raw).unwrap();
        assert!(desktop_legal_current_attachments(&data.notes[0]).is_empty());
        assert!(desktop_legal_destination(&data, "note/n/current/attachment/old-file").is_none());
        let snapshot = desktop_legal_current_scan_snapshot(&raw, &data).unwrap();
        let prepared = gridtimer_native::legal_scan::prepare_scan(
            &snapshot,
            "synthetic-current-scope",
            500,
            "[]",
            "[]",
        )
        .unwrap();
        assert!(prepared.manifest.omissions.is_empty());
        assert!(!serde_json::to_string(&prepared)
            .unwrap()
            .contains("old-file"));

        let legacy: DesktopNote = serde_json::from_value(json!({
            "id":"legacy","title":"legacy current note","content":"legacy body","attachments":[{"id":"live-file"}]
        }))
        .unwrap();
        assert_eq!(desktop_legal_current_attachments(&legacy).len(), 1);
        let value = desktop_legal_current_scan_note(&legacy).unwrap();
        assert!(value["document"].get("knowledge").is_none());
    }
}
