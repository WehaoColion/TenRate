// Business boundary specifications use synthetic results; they never call a provider.
fn desktop_agent_test_client(root: &Path) -> TimerWindowsClient {
    let mut client = knowledge_test_client(root);
    client.sync.ai_api_key = "synthetic-never-sent".into();
    client.sync.ai_base_url = "https://api.example.test".into();
    client.sync.ai_model = "synthetic-agent-test".into();
    client.open_desktop_agent_entry(Some("doc-a".into()));
    client.desktop_agent.question = "整理待办".into();
    client
}
fn desktop_agent_test_result() -> Value {
    json!({"ok":true,"answer":"已整理合成测试资料", "requests":4,"toolCalls":6,"reviewCompleted":true,
        "draft":{"title":"合成测试草稿","content":"原始正文", "actionItems":[],"sourceIds":["doc-a"]},
        "verification":{"passed":true,"checks": (["run_complete","draft_complete","tool_chain","sources","tasks","deep_review","write_boundary"].map(|id| json!({"id":id,"passed":true})))}})
}
fn desktop_agent_install_synthetic_draft(client: &mut TimerWindowsClient) {
    let context = client.desktop_agent_context().unwrap();
    client.desktop_agent.draft = Some(DesktopAgentDraft {
        context,
        result: desktop_agent_test_result(),
        title: "合成测试草稿".into(),
        body: "原始正文".into(),
        generated_title: "合成测试草稿".into(),
        generated_body: "原始正文".into(),
        verified: true,
        saved_id: None,
    });
}
#[test]
fn desktop_agent_unacknowledged_changed_scope_and_unwritable_preview_never_start_a_request() {
    let root = temp_test_dir("desktop_agent_send_denial");
    let mut client = desktop_agent_test_client(&root);
    client.prepare_desktop_agent();
    assert!(client.desktop_agent.preview.is_some());
    client.send_desktop_agent();
    assert!(client.desktop_agent.active.is_none());
    client.prepare_desktop_agent();
    client.desktop_agent.preview.as_mut().unwrap().acknowledged = true;
    client.desktop_agent.question = "其他目标".into();
    client.send_desktop_agent();
    assert!(client.desktop_agent.active.is_none());
    client.prepare_desktop_agent();
    client.desktop_agent.preview.as_mut().unwrap().acknowledged = true;
    client.workspace_persistence_ready = false;
    client.send_desktop_agent();
    assert!(client.desktop_agent.active.is_none());
    assert_eq!(client.task_supervisor.active_count(), 0);
    drop(client);
    fs::remove_dir_all(root).unwrap();
}
#[test]
fn desktop_agent_cancel_after_worker_completion_still_discards_the_queued_result() {
    let root = temp_test_dir("desktop_agent_cancel_completion");
    let mut client = desktop_agent_test_client(&root);
    let context = client.desktop_agent_context().unwrap();
    let (tx, rx) = mpsc::channel();
    let handle = gridtimer_native::desktop_agent::DesktopAgentRun::new("queued-test".into());
    tx.send(DesktopAgentCompletion {
        task_id: "queued-test".into(),
        context: context.clone(),
        result: desktop_agent_test_result(),
    })
    .unwrap();
    client.desktop_agent.active = Some(DesktopAgentActive {
        task_id: "queued-test".into(),
        context,
        cancelled: false,
        handle,
        rx,
    });
    client.cancel_desktop_agent();
    assert!(client.desktop_agent.active.is_some());
    client.poll_desktop_agent();
    assert!(client.desktop_agent.active.is_none());
    assert!(client.desktop_agent.draft.is_none());
    assert_eq!(client.data.notes.len(), 3);
    drop(client);
    fs::remove_dir_all(root).unwrap();
}
#[test]
fn desktop_agent_current_sources_and_verified_writable_context_are_required_to_save() {
    let root = temp_test_dir("desktop_agent_save_scope");
    let mut client = desktop_agent_test_client(&root);
    desktop_agent_install_synthetic_draft(&mut client);
    assert_eq!(client.desktop_agent_save_state(), AgentSaveState::Ready);
    client.workspace_persistence_ready = false;
    let before = client.state_json.clone();
    client.save_desktop_agent_draft();
    assert_eq!(client.state_json, before);
    assert!(client
        .desktop_agent
        .draft
        .as_ref()
        .unwrap()
        .saved_id
        .is_none());
    client.workspace_persistence_ready = true;
    let note = client
        .data
        .notes
        .iter_mut()
        .find(|note| note.id == "doc-a")
        .unwrap();
    note.content = "来源已改变".into();
    note.document = DesktopNoteDocument::default();
    client.data_version += 1;
    client.refresh_desktop_agent_sources();
    assert_eq!(
        client.desktop_agent_save_state(),
        AgentSaveState::ScopeChanged
    );
    client.save_desktop_agent_draft();
    assert_eq!(client.state_json, before);
    drop(client);
    fs::remove_dir_all(root).unwrap();
}
#[test]
fn desktop_agent_saves_once_as_a_new_document_without_changing_sources() {
    let root = temp_test_dir("desktop_agent_save_new_document");
    let mut client = desktop_agent_test_client(&root);
    let sources = client.data.notes.clone();
    desktop_agent_install_synthetic_draft(&mut client);
    client.save_desktop_agent_draft();
    let saved = client
        .desktop_agent
        .draft
        .as_ref()
        .unwrap()
        .saved_id
        .clone()
        .expect("accepted new-page commit");
    let note = client
        .data
        .notes
        .iter()
        .find(|note| note.id == saved)
        .unwrap();
    assert_eq!(note.kind, "DOCUMENT");
    assert!(note_body_text(note).contains("原始正文"));
    for source in &sources {
        assert_eq!(
            client.data.notes.iter().find(|note| note.id == source.id),
            Some(source)
        );
    }
    let count = client.data.notes.len();
    client.save_desktop_agent_draft();
    assert_eq!(client.data.notes.len(), count);
    assert_eq!(client.desktop_agent_save_state(), AgentSaveState::Saved);
    drop(client);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn desktop_document_ai_reads_only_current_page_and_never_falls_back_when_it_disappears() {
    let root = temp_test_dir("desktop_current_page_ai_scope");
    let mut client = desktop_agent_test_client(&root);
    client.desktop_agent.open = false;
    client.open_desktop_document_ai("doc-a");
    client.knowledge_ai_question_draft = "整理这篇记录".into();
    client.prepare_desktop_ai_preview().unwrap();
    let preview = client.desktop_ai.preview.as_ref().unwrap();
    assert_eq!(preview.mode, QueryMode::Knowledge);
    assert_eq!(preview.sources.len(), 1);
    assert_eq!(preview.sources[0].0, "doc-a");
    client
        .data
        .notes
        .iter_mut()
        .find(|note| note.id == "doc-a")
        .unwrap()
        .deleted_at_epoch_millis = Some(now_millis());
    client.data_version += 1;
    assert!(client.prepare_desktop_ai_preview().is_err());
    assert!(client.desktop_ai.preview.is_none());
    assert!(!client.knowledge_ai_pending);
    assert_eq!(client.task_supervisor.active_count(), 0);
    drop(client);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn desktop_agent_temporary_sync_lock_retains_edited_draft_but_blocks_saving() {
    let root = temp_test_dir("desktop_agent_temporary_sync_lock");
    let mut client = desktop_agent_test_client(&root);
    desktop_agent_install_synthetic_draft(&mut client);
    client.desktop_agent.draft.as_mut().unwrap().body = "用户已编辑的草稿".into();
    client.sync_task = Some(SyncTaskState {
        kind: SyncTaskKind::Download,
        phase: SyncTaskPhase::AppData,
        started_at_epoch_millis: now_millis(),
    });
    client.refresh_desktop_agent_sources();
    assert_eq!(
        client.desktop_agent.draft.as_ref().unwrap().body,
        "用户已编辑的草稿"
    );
    assert_eq!(
        client.desktop_agent_save_state(),
        AgentSaveState::ScopeChanged
    );
    client.sync_task = None;
    client.refresh_desktop_agent_sources();
    assert_eq!(client.desktop_agent_save_state(), AgentSaveState::Ready);
    drop(client);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn desktop_agent_dirty_selected_source_invalidates_cached_preview_result_and_save() {
    let root = temp_test_dir("desktop_agent_dirty_selected_source");
    let mut client = desktop_agent_test_client(&root);
    client.prepare_desktop_agent();
    let context = client
        .desktop_agent
        .preview
        .as_ref()
        .unwrap()
        .context
        .clone();
    desktop_agent_install_synthetic_draft(&mut client);
    let (tx, rx) = mpsc::channel();
    let handle = gridtimer_native::desktop_agent::DesktopAgentRun::new("dirty-queued-test".into());
    tx.send(DesktopAgentCompletion {
        task_id: "dirty-queued-test".into(),
        context: context.clone(),
        result: desktop_agent_test_result(),
    })
    .unwrap();
    client.desktop_agent.active = Some(DesktopAgentActive {
        task_id: "dirty-queued-test".into(),
        context,
        cancelled: false,
        handle: handle.clone(),
        rx,
    });
    let cached_version = client.desktop_agent.cached_version;
    client.selected_note_id = "doc-a".into();
    client.note_dirty = true;
    client.refresh_desktop_agent_sources();
    assert_eq!(client.desktop_agent.cached_version, cached_version);
    assert!(client.desktop_agent.active.as_ref().unwrap().cancelled);
    assert_eq!(handle.progress()["cancelRequested"], true);
    assert!(client.desktop_agent.preview.is_none());
    assert_eq!(client.desktop_agent_save_state(), AgentSaveState::Running);
    client.poll_desktop_agent();
    assert!(client.desktop_agent.active.is_none());
    assert_eq!(
        client.desktop_agent_save_state(),
        AgentSaveState::ScopeChanged
    );
    assert_eq!(
        client.desktop_agent.draft.as_ref().unwrap().body,
        "原始正文"
    );
    assert_eq!(client.data.notes.len(), 3);
    drop(client);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn desktop_knowledge_ai_changed_persisted_or_dirty_source_rejects_queued_answer() {
    for dirty in [false, true] {
        let root = temp_test_dir(if dirty {
            "desktop_ai_dirty_source"
        } else {
            "desktop_ai_source_generation"
        });
        let mut client = desktop_agent_test_client(&root);
        client.open_desktop_document_ai("doc-a");
        client.knowledge_ai_question_draft = "整理这篇记录".into();
        client.prepare_desktop_ai_preview().unwrap();
        let mut preview = client.desktop_ai.preview.take().unwrap();
        let id = client
            .desktop_ai
            .boundary
            .begin(&mut preview.consent, &preview.binding, true)
            .unwrap();
        client.desktop_ai.active_knowledge_scope = Some((
            preview.notes_version,
            ["doc-a".into()].into_iter().collect(),
        ));
        let (tx, rx) = mpsc::channel();
        tx.send(KnowledgeAiTaskResult {
            request_id: id,
            recipient_host: "synthetic.test".into(),
            model: "synthetic".into(),
            origin_workspace: client.ai_workspace_identity(),
            question: preview.question,
            source_ids: vec!["doc-a".into()],
            source_titles: vec!["项目方案".into()],
            source_folders: vec!["项目资料".into()],
            result: ai_client::AiCompletionResult::ok("过期合成回答，不得接受".into()),
        })
        .unwrap();
        client.knowledge_ai_result_rx = Some(rx);
        client.knowledge_ai_pending = true;
        if dirty {
            client.selected_note_id = "doc-a".into();
            client.note_dirty = true;
        } else {
            client.data_version += 1;
        }
        client.observe_desktop_ai_scope();
        assert!(
            client.desktop_ai.boundary.busy(),
            "in-flight worker ownership is retained"
        );
        assert!(client.knowledge_ai_pending);
        client.poll_knowledge_ai_result();
        assert!(!client.desktop_ai.boundary.busy());
        assert!(!client.knowledge_ai_pending);
        assert!(client.knowledge_ai_answer.is_none());
        assert_eq!(client.data.notes.len(), 3);
        drop(client);
        fs::remove_dir_all(root).unwrap();
    }
}

#[test]
fn desktop_agent_source_navigation_flushes_edit_and_stops_at_a_failed_barrier() {
    let root = temp_test_dir("desktop_agent_source_navigation");
    let mut client = desktop_agent_test_client(&root);
    let block = client.note_blocks_draft[0].clone();
    client.apply_note_block_text_edit(&block, "刚编辑的知识正文".into());
    client.workspace_persistence_ready = false;
    client.open_desktop_agent_source("sticky-a");
    assert_eq!(client.selected_note_id, "doc-a");
    assert!(client.note_dirty);
    client.workspace_persistence_ready = true;
    client.open_desktop_agent_source("sticky-a");
    assert_eq!(client.selected_note_id, "sticky-a");
    assert!(client.tab == AppTab::Notes);
    assert_eq!(
        client
            .data
            .notes
            .iter()
            .find(|note| note.id == "doc-a")
            .unwrap()
            .document
            .blocks[0]
            .text,
        "刚编辑的知识正文"
    );
    let block = client.note_blocks_draft[0].clone();
    client.apply_note_block_text_edit(&block, "刚编辑的便签正文".into());
    client.open_desktop_agent_source("sticky-a");
    assert_eq!(client.note_content_draft, "刚编辑的便签正文");
    assert!(!client.note_dirty);
    drop(client);
    fs::remove_dir_all(root).unwrap();
}
