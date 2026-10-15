// v1.1.0.8 Windows - Exit only after an exact successful local save.
#[test]
fn desktop_exit_unchanged_note_does_not_wait_for_unrelated_work() {
    let dir = temp_test_dir("desktop_exit_clean");
    let mut client =
        test_client_for_account_scope(&dir, "account-a", history_test_state(now_millis()));
    client.tab = AppTab::Knowledge;
    client.note_dirty = false;
    client.desktop_ui.navigation.editor_visible = true;
    client.workspace_persistence_ready = false;
    assert!(client.return_to_document_list());
    assert!(!client.desktop_ui.navigation.editor_visible);
}

fn desktop_exit_test_binding(client: &TimerWindowsClient) -> DesktopDocumentExit {
    DesktopDocumentExit {
        workspace: client.background_job_workspace_fingerprint(),
        note_id: client.selected_note_id.clone(),
        editor_epoch: client.desktop_ui.navigation.editor_epoch,
        revision: client.persistence.revisions.note,
        started: Instant::now(),
        property_inputs: client.document_exit_property_inputs(),
    }
}

#[test]
fn desktop_exit_newer_edit_or_other_workspace_never_closes_editor() {
    for changed_workspace in [false, true] {
        let dir = temp_test_dir("desktop_exit_changed");
        let mut client =
            test_client_for_account_scope(&dir, "account-a", history_test_state(now_millis()));
        client.tab = AppTab::Knowledge;
        client.desktop_ui.navigation.editor_visible = true;
        let mut binding = desktop_exit_test_binding(&client);
        if changed_workspace {
            binding.workspace.push_str("-other");
        } else {
            binding.revision += 1;
        }
        client.desktop_ui.navigation.pending_exit = Some(binding);
        client.note_dirty = false;
        client.poll_document_exit();
        assert!(client.desktop_ui.navigation.editor_visible);
        assert!(client.desktop_ui.navigation.pending_exit.is_none());
    }
}

#[test]
fn desktop_exit_save_failure_keeps_draft_and_editor() {
    let dir = temp_test_dir("desktop_exit_failure");
    let mut client = knowledge_test_client(&dir);
    client.tab = AppTab::Knowledge;
    client.select_note_by_id("doc-a");
    client.desktop_ui.navigation.editor_visible = true;
    client.note_title_draft = "unsaved edit".into();
    client.mark_note_dirty();
    // Fail the durable transaction's configuration write, not the best-effort
    // JSON mirror, whose failure intentionally leaves a valid journal receipt.
    let blocker = dir.join("blocked-parent");
    fs::write(&blocker, b"not a directory").unwrap();
    client.sync_path = blocker.join("sync_account.json");
    client.desktop_ui.navigation.pending_exit = Some(desktop_exit_test_binding(&client));
    let ctx = workspace_test_context();
    client.submit_draft_snapshot(&ctx, true);
    let deadline = Instant::now() + Duration::from_secs(10);
    while client.persistence.pending() && Instant::now() < deadline {
        thread::sleep(Duration::from_millis(10));
        client.poll_draft_persistence();
    }
    assert!(client.persistence.failed());
    client.poll_document_exit();
    assert!(client.desktop_ui.navigation.editor_visible);
    assert!(client.note_dirty);
    assert_eq!(client.note_title_draft, "unsaved edit");
}

#[test]
fn desktop_exit_successful_exact_receipt_returns_to_list() {
    let dir = temp_test_dir("desktop_exit_saved");
    let mut client =
        test_client_for_account_scope(&dir, "account-a", history_test_state(now_millis()));
    client.tab = AppTab::Knowledge;
    client.desktop_ui.navigation.editor_visible = true;
    client.desktop_ui.navigation.pending_exit = Some(desktop_exit_test_binding(&client));
    client.note_dirty = false;
    client.poll_document_exit();
    assert!(!client.desktop_ui.navigation.editor_visible);
    assert!(client.desktop_ui.navigation.pending_exit.is_none());
}

#[test]
fn desktop_exit_dirty_request_waits_for_real_durable_receipt() {
    let dir = temp_test_dir("desktop_exit_durable");
    let mut client = knowledge_test_client(&dir);
    client.tab = AppTab::Knowledge;
    client.select_note_by_id("doc-a");
    client.desktop_ui.navigation.editor_visible = true;
    client.note_title_draft = "durably saved before exit".into();
    client.mark_note_dirty();
    client.sync_apply.frame_context = Some(workspace_test_context());
    assert!(!client.return_to_document_list());
    assert!(client.desktop_ui.navigation.editor_visible);
    let deadline = Instant::now() + Duration::from_secs(10);
    while client.desktop_ui.navigation.pending_exit.is_some() && Instant::now() < deadline {
        thread::sleep(Duration::from_millis(10));
        client.poll_draft_persistence();
        client.poll_document_exit();
    }
    assert!(!client.note_dirty);
    assert!(!client.desktop_ui.navigation.editor_visible);
    assert_eq!(
        client
            .data
            .notes
            .iter()
            .find(|n| n.id == "doc-a")
            .unwrap()
            .title,
        "durably saved before exit"
    );
}

#[test]
fn desktop_exit_new_property_input_cancels_navigation_and_retains_text() {
    let dir = temp_test_dir("desktop_exit_property_change");
    let mut client = knowledge_test_client(&dir);
    client.tab = AppTab::Knowledge;
    client.desktop_ui.navigation.editor_visible = true;
    client.desktop_ui.navigation.pending_exit = Some(desktop_exit_test_binding(&client));
    client.desktop_ui.knowledge.cell_inputs.insert(
        "page:doc-a:field".into(),
        KnowledgeCellInput {
            source: "old".into(),
            text: "still editing".into(),
        },
    );
    client.note_dirty = false;
    client.poll_document_exit();
    assert!(client.desktop_ui.navigation.editor_visible);
    assert!(client.desktop_ui.navigation.pending_exit.is_none());
    assert_eq!(
        client.desktop_ui.knowledge.cell_inputs["page:doc-a:field"].text,
        "still editing"
    );
}
