// Windows - Protect nested containers and local presentation order at state boundaries.

fn directory_fixture_note(id: &str, parent: Option<&str>, folder: bool) -> DesktopNote {
    let mut properties = BTreeMap::new();
    if folder {
        properties.insert(
            knowledge::FOLDER_KIND_PROPERTY.into(),
            knowledge::CellValue::Text("folder".into()),
        );
    }
    DesktopNote {
        id: id.into(),
        title: id.into(),
        kind: "DOCUMENT".into(),
        content: format!("body of {id}"),
        document: DesktopNoteDocument {
            knowledge: Some(knowledge::KnowledgePage {
                parent_id: parent.map(str::to_string),
                properties,
                ..Default::default()
            }),
            ..Default::default()
        },
        ..Default::default()
    }
}
#[test]
fn knowledge_directory_mixed_nesting_legacy_projection_and_conversion_keep_all_content() {
    let folders = vec![DesktopNoteFolder {
        id: "legacy".into(),
        name: "legacy title".into(),
        ..Default::default()
    }];
    let mut old = directory_fixture_note("old-child", None, false);
    old.folder_id = Some("legacy".into());
    let notes = vec![
        directory_fixture_note("page", None, false),
        directory_fixture_note("inner-folder", Some("page"), true),
        directory_fixture_note("child", Some("inner-folder"), false),
        old,
    ];
    let index = KnowledgeDirectoryIndex::build(&notes, &folders);
    assert_eq!(index.nodes.len(), 5);
    assert_eq!(index.path("child"), ["page", "inner-folder", "child"]);
    assert_eq!(index.children(Some("legacy")), ["old-child"]);
    assert!(index.nodes["inner-folder"].folder);
    assert!(index.nodes["legacy"].legacy && !index.nodes["legacy"].has_note);
    assert!(!index.may_move("page", Some("child")));
    assert!(index.may_move("child", Some("legacy")));
    assert!(index.may_create(Some("page")));
}
#[test]
fn knowledge_directory_orphans_cycles_inaccessible_and_conflicting_nodes_remain_reachable() {
    let mut secret = directory_fixture_note("secret", None, false);
    secret.encryption = Some(Default::default());
    let notes = vec![
        directory_fixture_note("a", Some("b"), false),
        directory_fixture_note("b", Some("a"), false),
        directory_fixture_note("orphan", Some("gone"), false),
        secret,
        directory_fixture_note("under-secret", Some("secret"), false),
        directory_fixture_note("collision", None, true),
    ];
    let folders = vec![DesktopNoteFolder {
        id: "collision".into(),
        name: "legacy original".into(),
        ..Default::default()
    }];
    let index = KnowledgeDirectoryIndex::build(&notes, &folders);
    assert_eq!(index.nodes.len(), 6);
    assert_eq!(index.nodes["a"].issue, Some(KnowledgeDirectoryIssue::Cycle));
    assert_eq!(index.nodes["b"].issue, Some(KnowledgeDirectoryIssue::Cycle));
    assert_eq!(
        index.nodes["orphan"].issue,
        Some(KnowledgeDirectoryIssue::MissingParent)
    );
    assert_eq!(
        index.nodes["under-secret"].issue,
        Some(KnowledgeDirectoryIssue::InaccessibleParent)
    );
    assert!(index.nodes["collision"].title.contains("legacy original"));
    assert_eq!(
        index.nodes["collision"].issue,
        Some(KnowledgeDirectoryIssue::IdentityConflict)
    );
    assert!(!index.nodes["collision"].writable);
    assert!(!index.may_create(Some("collision")));
    assert!(!index.may_move("a", Some("secret")));
    for id in ["a", "b", "orphan", "under-secret", "collision"] {
        assert!(index.children(None).iter().any(|root| root == id));
    }
}
#[test]
fn knowledge_directory_locked_ancestors_block_creating_and_reordering_without_hiding_children() {
    let mut locked = directory_fixture_note("locked", None, true);
    locked.document.knowledge.as_mut().unwrap().locked = true;
    let notes = vec![
        locked,
        directory_fixture_note("inside", Some("locked"), false),
        directory_fixture_note("grandchild", Some("inside"), false),
    ];
    let index = KnowledgeDirectoryIndex::build(&notes, &[]);
    assert_eq!(index.path("grandchild"), ["locked", "inside", "grandchild"]);
    assert!(!index.may_create(Some("inside")));
    assert!(!index.scope("workspace", Some("inside"), true).can_reorder);
    assert!(!index.may_move("grandchild", Some("inside")));
    assert!(index.may_move("grandchild", None));
}
#[test]
fn knowledge_directory_deleted_promoted_folder_is_not_resurrected_by_legacy_projection() {
    let mut promoted = directory_fixture_note("legacy", None, true);
    promoted
        .document
        .knowledge
        .as_mut()
        .unwrap()
        .properties
        .insert(
            knowledge::LEGACY_FOLDER_PROPERTY.into(),
            knowledge::CellValue::Checkbox(true),
        );
    let folders = vec![DesktopNoteFolder {
        id: "legacy".into(),
        ..Default::default()
    }];
    assert!(KnowledgeDirectoryIndex::build(&[promoted.clone()], &folders).nodes["legacy"].writable);
    promoted.deleted_at_epoch_millis = Some(1);
    assert!(!KnowledgeDirectoryIndex::build(&[promoted], &folders)
        .nodes
        .contains_key("legacy"));
}
#[test]
fn knowledge_directory_drag_rejects_changed_workspace_parent_path_permissions_and_membership() {
    let notes = vec![
        directory_fixture_note("a", None, false),
        directory_fixture_note("b", None, false),
        directory_fixture_note("c", None, false),
    ];
    let index = KnowledgeDirectoryIndex::build(&notes, &[]);
    let scope = index.scope("workspace-a", None, true);
    let drag = KnowledgeDirectoryDrag {
        scope: scope.clone(),
        initial: vec!["a".into(), "b".into(), "c".into()],
        dragged: "a".into(),
    };
    assert_eq!(
        knowledge_directory_reordered(&drag, &scope, "c"),
        Some(vec!["b".into(), "c".into(), "a".into()])
    );
    for changed in [
        KnowledgeDirectoryScope {
            workspace: "workspace-b".into(),
            ..scope.clone()
        },
        KnowledgeDirectoryScope {
            parent: Some("parent".into()),
            ..scope.clone()
        },
        KnowledgeDirectoryScope {
            path: vec!["ancestor".into()],
            ..scope.clone()
        },
        KnowledgeDirectoryScope {
            can_reorder: false,
            ..scope.clone()
        },
        KnowledgeDirectoryScope {
            items: BTreeMap::from([("a".into(), false), ("b".into(), true), ("c".into(), true)]),
            ..scope.clone()
        },
        KnowledgeDirectoryScope {
            items: BTreeMap::from([("a".into(), true), ("b".into(), true)]),
            ..scope.clone()
        },
    ] {
        assert!(knowledge_directory_reordered(&drag, &changed, "b").is_none());
    }
    let duplicate = KnowledgeDirectoryDrag {
        initial: vec!["a".into(), "a".into(), "c".into()],
        ..drag.clone()
    };
    assert!(knowledge_directory_reordered(&duplicate, &scope, "c").is_none());
    let locked = KnowledgeDirectoryDrag {
        dragged: "missing".into(),
        ..drag
    };
    assert!(knowledge_directory_reordered(&locked, &scope, "b").is_none());
}
#[test]
fn knowledge_directory_saved_order_survives_content_dates_and_discards_only_missing_members() {
    let saved = vec!["b".into(), "gone".into(), "b".into(), "a".into()];
    assert_eq!(
        knowledge_directory_ordered(&["a".into(), "b".into(), "new".into()], &saved),
        ["b", "a", "new"]
    );
    assert_eq!(
        knowledge_directory_ordered(&["new".into(), "b".into(), "a".into()], &saved),
        ["b", "a", "new"]
    );
    let invalid = KnowledgeDirectoryPreferences {
        parents: BTreeMap::from([("null".into(), saved)]),
    };
    assert!(!invalid.validate());
    assert_ne!(
        KnowledgeDirectoryPreferences::parent_key(None),
        KnowledgeDirectoryPreferences::parent_key(Some("null"))
    );
}
#[test]
fn knowledge_directory_drag_commits_only_local_order_and_preserves_dates_history_home_and_sync_data(
) {
    let dir = temp_test_dir("windows_knowledge_directory_local_order");
    let mut client = knowledge_test_client(&dir);
    client.knowledge_directory_index();
    client.desktop_ui.knowledge.directory.open = true;
    client.desktop_ui.knowledge.directory.parent = Some("folder-a".into());
    // Use the actual sibling scope, with a second page added through the same shared operation.
    let request = KnowledgeDirectoryRequest {
        workspace: client.background_job_workspace_fingerprint(),
        action: "create_page".into(),
        id: "directory-second".into(),
        parent: Some("folder-a".into()),
        title: "second".into(),
    };
    assert!(client.apply_knowledge_directory_action(&request));
    assert!(client.flush_note_draft().is_ok());
    client.desktop_ui.knowledge.directory.open = true;
    client.desktop_ui.knowledge.directory.parent = Some("folder-a".into());
    let index = client.knowledge_directory_index();
    let scope = index.scope(
        &client.background_job_workspace_fingerprint(),
        Some("folder-a"),
        true,
    );
    let before = client.state_json.clone();
    let version = client.data_version;
    client.rebuild_view_cache();
    let home = client
        .view_cache
        .notes
        .iter()
        .map(|p| (p.id.clone(), p.updated_at_epoch_millis))
        .collect::<Vec<_>>();
    let ids = index
        .children(Some("folder-a"))
        .iter()
        .rev()
        .cloned()
        .collect::<Vec<_>>();
    assert_eq!(ids.len(), 2);
    assert!(client.commit_knowledge_directory_order(&scope, &ids));
    assert_eq!(client.state_json, before);
    assert_eq!(client.data_version, version);
    client.rebuild_view_cache();
    assert_eq!(
        client
            .view_cache
            .notes
            .iter()
            .map(|p| (p.id.clone(), p.updated_at_epoch_millis))
            .collect::<Vec<_>>(),
        home
    );
    let path = client.knowledge_directory_preferences_path();
    let stored: KnowledgeDirectoryPreferences =
        serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
    assert_eq!(
        stored.parents[&KnowledgeDirectoryPreferences::parent_key(Some("folder-a"))],
        ids
    );
    assert!(client.reset_knowledge_directory_order(&scope));
    let reset: KnowledgeDirectoryPreferences =
        serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
    assert!(!reset
        .parents
        .contains_key(&KnowledgeDirectoryPreferences::parent_key(Some("folder-a"))));
    assert_eq!(client.state_json, before);
    assert_eq!(client.data_version, version);
    client.shutdown_state = ClientShutdownState::Draining {
        started: Instant::now(),
    };
    assert!(!client.commit_knowledge_directory_order(&scope, &ids));
    assert!(!client.open_knowledge_directory(None));
    assert_eq!(client.state_json, before);
    assert_eq!(client.data_version, version);
    let _ = index;
    drop(client);
    fs::remove_dir_all(dir).unwrap();
}
#[test]
fn knowledge_directory_local_write_failure_keeps_previous_order_and_workspace_boundary_blocks_old_action(
) {
    let dir = temp_test_dir("windows_knowledge_directory_failure");
    let mut client = knowledge_test_client(&dir);
    let index = client.knowledge_directory_index();
    client.desktop_ui.knowledge.directory.open = true;
    let scope = index.scope(&client.background_job_workspace_fingerprint(), None, true);
    let ids = index.children(None).to_vec();
    let previous = client
        .desktop_ui
        .knowledge
        .directory
        .preferences
        .parents
        .clone();
    let blocker = dir.join("not_a_directory");
    fs::write(&blocker, b"keep").unwrap();
    client.settings_path = blocker.join("settings.json");
    assert!(!client.commit_knowledge_directory_order(&scope, &ids));
    assert_eq!(
        client.desktop_ui.knowledge.directory.preferences.parents,
        previous
    );
    let before = client.state_json.clone();
    let stale = KnowledgeDirectoryRequest {
        workspace: "old-account".into(),
        action: "to_folder".into(),
        id: "doc-a".into(),
        parent: None,
        title: String::new(),
    };
    assert!(!client.apply_knowledge_directory_action(&stale));
    assert_eq!(client.state_json, before);
    drop(client);
    fs::remove_dir_all(dir).unwrap();
}
#[test]
fn knowledge_directory_conversion_reuses_shared_transaction_and_never_replaces_body_or_children() {
    let dir = temp_test_dir("windows_knowledge_directory_conversion");
    let mut client = knowledge_test_client(&dir);
    client.set_note_canvas_text("原始正文与刚编辑的内容");
    let workspace = client.background_job_workspace_fingerprint();
    let mut request = KnowledgeDirectoryRequest {
        workspace: workspace.clone(),
        action: "to_folder".into(),
        id: "doc-a".into(),
        parent: None,
        title: String::new(),
    };
    assert!(client.apply_knowledge_directory_action(&request));
    assert!(client.note_content_draft.contains("刚编辑的内容"));
    assert!(client.knowledge_directory_index().nodes["doc-a"].folder);
    request.action = "create_folder".into();
    request.id = "nested-folder".into();
    request.parent = Some("doc-a".into());
    request.title = "child".into();
    assert!(client.apply_knowledge_directory_action(&request));
    request.action = "to_page".into();
    request.id = "doc-a".into();
    request.parent = None;
    assert!(client.apply_knowledge_directory_action(&request));
    let index = client.knowledge_directory_index();
    assert!(!index.nodes["doc-a"].folder);
    assert_eq!(index.children(Some("doc-a")), ["nested-folder"]);
    let doc = client
        .data
        .notes
        .iter()
        .find(|note| note.id == "doc-a")
        .unwrap();
    assert!(note_body_text(doc).contains("刚编辑的内容"));
    assert!(doc.revisions.len() >= 2);
    let before = client.state_json.clone();
    client.workspace_persistence_ready = false;
    assert!(
        !client.apply_knowledge_directory_action(&KnowledgeDirectoryRequest {
            action: "to_folder".into(),
            ..request.clone()
        })
    );
    assert_eq!(client.state_json, before);
    client.workspace_persistence_ready = true;
    client.shutdown_state = ClientShutdownState::Draining {
        started: Instant::now(),
    };
    assert!(
        !client.apply_knowledge_directory_action(&KnowledgeDirectoryRequest {
            action: "to_folder".into(),
            ..request
        })
    );
    assert_eq!(client.state_json, before);
    drop(client);
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn knowledge_source_target_reveals_only_ancestors_without_mutating_saved_toggle_state() {
    let fixture = |id: &str, parent: Option<&str>, kind: knowledge::BlockKind| DesktopNoteBlock {
        id: id.into(),
        knowledge: Some(knowledge::KnowledgeBlock {
            parent_id: parent.map(str::to_string),
            kind,
            collapsed: true,
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut blocks = vec![
        fixture("outer", None, knowledge::BlockKind::Toggle),
        fixture("inner", Some("outer"), knowledge::BlockKind::Toggle),
        fixture("image", Some("inner"), knowledge::BlockKind::Paragraph),
        fixture("other", None, knowledge::BlockKind::Toggle),
    ];
    let before = blocks.clone();
    assert_eq!(
        knowledge_block_reveal_ancestors(&blocks, "image"),
        HashSet::from(["outer".into(), "inner".into()])
    );
    assert!(knowledge_block_reveal_ancestors(&blocks, "missing").is_empty());
    assert_eq!(blocks, before);
    blocks[0].knowledge.as_mut().unwrap().parent_id = Some("inner".into());
    assert_eq!(
        knowledge_block_reveal_ancestors(&blocks, "image"),
        HashSet::from(["outer".into(), "inner".into()])
    );
    assert_eq!(
        knowledge_block_reveal_ancestors(&blocks, "outer"),
        HashSet::from(["inner".into()])
    );
}
