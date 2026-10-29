
// Compare the incremental create projection to the old full-tree implementation,
// including its exact version/revision and legacy-folder semantics.
#[test]
fn new_container_projection_matches_full_tree_across_ancestor_shapes() {
    let raw = create_note_folder_app_data_json(&state(), "legacy", "老文件夹", 1001).unwrap();
    let raw = action(&raw, "create_page", "legacy_child", "legacy", "旧目录子页", 1002);
    let raw = action(&raw, "create_folder", "legacy_nested", "legacy_child", "深层", 1003);
    let raw = action(&raw, "create_page", "plain_root", "", "根页面", 1004);
    let raw = action(&raw, "create_folder", "plain_nested", "plain_root", "普通子目录", 1005);

    for (index, parent_id) in [
        "",
        "legacy",
        "legacy_child",
        "legacy_nested",
        "plain_root",
        "plain_nested",
    ]
    .iter()
    .enumerate()
    {
        for kind in ["create_page", "create_folder"] {
            let id = format!("fresh_{index}_{kind}");
            let now = 1010 + index as i64;
            let mut base = parse_app_data_for_mutation(&raw, now).unwrap();
            let mut created = NoteEntry {
                id: id.clone(),
                title: "新建项".to_owned(),
                kind: NoteEntryKind::Document,
                ..Default::default()
            };
            let page = knowledge_ensure_page(&mut created);
            page.parent_id = (!parent_id.is_empty()).then(|| (*parent_id).to_owned());
            if kind == "create_folder" {
                page.properties.insert(
                    crate::knowledge::FOLDER_KIND_PROPERTY.to_owned(),
                    crate::knowledge::CellValue::Text("folder".into()),
                );
            }
            created = normalize_note_for_save(&base, created, None, now).unwrap();
            base.notes.push(created);
            let mut full = base.clone();
            knowledge_refresh_folder_projection(&mut full, &HashSet::from([id.clone()]), now)
                .unwrap();
            knowledge_refresh_new_container_projection(&mut base, &id, now).unwrap();
            assert_eq!(base, full, "create projection differs for {kind} under {parent_id}");
            let saved = action(&raw, kind, &id, parent_id, "新建项", now);
            let expected = serde_json::to_string(&full.sanitized(now).unwrap()).unwrap();
            assert_eq!(
                serde_json::from_str::<Value>(&saved).unwrap(),
                serde_json::from_str::<Value>(&expected).unwrap(),
                "persisted create differs for {kind} under {parent_id}"
            );
        }
    }
}

// v2.23.2.29 - Hierarchy specifications cover lossless content and rejected transitions.
use super::*;
use serde_json::{json, Value};

fn state() -> String {
    sanitize_app_data_json("{}", 1000).unwrap()
}
fn action(raw: &str, kind: &str, id: &str, parent: &str, title: &str, now: i64) -> String {
    knowledge_container_action_app_data_json(raw, kind, id, parent, title, now).unwrap()
}
fn note(raw: &str, id: &str) -> Value {
    serde_json::from_str::<Value>(raw).unwrap()["notes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|note| note["id"] == id)
        .unwrap()
        .clone()
}

#[test]
fn knowledge_hierarchy_page_folder_conversion_preserves_body_id_children_and_history() {
    let raw = upsert_note_app_data_json(
        &state(),
        &json!({"id":"page","kind":"DOCUMENT","title":"正文","content":"保留旧正文"}).to_string(),
        1001,
    )
    .unwrap();
    let raw = action(&raw, "create_page", "child", "page", "子页面", 1002);
    let converted = action(&raw, "to_folder", "page", "", "", 1003);
    let folder = note(&converted, "page");
    assert_eq!(folder["content"], "保留旧正文");
    assert_eq!(folder["document"]["blocks"][0]["text"], "保留旧正文");
    assert_eq!(
        folder["document"]["knowledge"]["properties"][crate::knowledge::FOLDER_KIND_PROPERTY],
        json!({"kind":"text","value":"folder"})
    );
    assert_eq!(
        note(&converted, "child")["document"]["knowledge"]["parentId"],
        "page"
    );
    let restored = action(&converted, "to_page", "page", "", "", 1004);
    let page = note(&restored, "page");
    assert!(
        page["document"]["knowledge"]["properties"][crate::knowledge::FOLDER_KIND_PROPERTY]
            .is_null()
    );
    assert_eq!(page["content"], "保留旧正文");
    assert_eq!(page["latestVersionId"], folder["latestVersionId"]);
    assert_eq!(
        page["versions"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v["id"].clone())
            .collect::<Vec<_>>(),
        folder["versions"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v["id"].clone())
            .collect::<Vec<_>>()
    );
    assert!(page["revisions"]
        .as_array()
        .unwrap()
        .iter()
        .any(|snapshot| snapshot["label"] == "转为页面前"
            && snapshot["document"]["knowledge"]["properties"]
                [crate::knowledge::FOLDER_KIND_PROPERTY]["value"]
                == "folder"));
    assert!(page["revisions"]
        .as_array()
        .unwrap()
        .iter()
        .any(|snapshot| snapshot["label"] == "转为文件夹前"));
}

#[test]
fn knowledge_hierarchy_nested_pages_folders_and_cycle_rejection_are_atomic() {
    let raw = action(&state(), "create_folder", "root", "", "资料", 1001);
    let raw = action(&raw, "create_page", "page", "root", "页面", 1002);
    let raw = action(&raw, "create_folder", "nested", "page", "子文件夹", 1003);
    let raw = action(&raw, "create_page", "leaf", "nested", "子页面", 1004);
    for (id, parent) in [("root", "leaf"), ("page", "page"), ("nested", "missing")] {
        assert!(
            knowledge_container_action_app_data_json(&raw, "move", id, parent, "", 1005).is_none()
        );
    }
    assert_eq!(
        note(&raw, "page")["document"]["knowledge"]["parentId"],
        "root"
    );
    let moved = action(&raw, "move", "nested", "root", "", 1006);
    assert_eq!(
        note(&moved, "nested")["document"]["knowledge"]["parentId"],
        "root"
    );
    assert_eq!(
        note(&moved, "leaf")["document"]["knowledge"]["parentId"],
        "nested"
    );
}

#[test]
fn knowledge_hierarchy_legacy_folders_remain_compatible_and_promotion_rejects_descendant_cycle() {
    let raw = create_note_folder_app_data_json(&state(), "legacy", "老资料", 1001).unwrap();
    let raw = action(&raw, "create_page", "child", "legacy", "原资料", 1002);
    let raw = action(&raw, "create_folder", "sub", "child", "嵌套文件夹", 1003);
    let raw = action(&raw, "create_page", "leaf", "sub", "正文", 1004);
    assert_eq!(note(&raw, "leaf")["folderId"], "legacy");
    assert!(
        knowledge_container_action_app_data_json(&raw, "move", "legacy", "leaf", "", 1005)
            .is_none()
    );
    let promoted = action(&raw, "promote_folder", "legacy", "", "", 1005);
    let old_roundtrip =
        crate::legacy_app_data_fixture::sanitize_app_data_json(&promoted, 1006).unwrap();
    assert_eq!(
        note(&old_roundtrip, "legacy")["document"]["knowledge"],
        note(&promoted, "legacy")["document"]["knowledge"]
    );
    assert_eq!(note(&old_roundtrip, "leaf")["folderId"], "legacy");
    assert_eq!(
        serde_json::from_str::<Value>(&old_roundtrip).unwrap()["noteFolders"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    let moved = action(&promoted, "move", "child", "", "", 1007);
    assert!(note(&moved, "child")["folderId"].is_null());
    assert!(note(&moved, "leaf")["folderId"].is_null());
}

#[test]
fn knowledge_hierarchy_stale_editor_cannot_undo_move_or_folder_conversion() {
    let raw = action(&state(), "create_page", "page", "", "页面", 1001);
    let stale = note(&raw, "page");
    let converted = action(&raw, "to_folder", "page", "", "", 1002);
    assert!(upsert_note_app_data_json(&converted, &stale.to_string(), 1003).is_none());
    let raw = action(&converted, "create_folder", "parent", "", "父级", 1004);
    let stale = note(&raw, "page");
    let moved = action(&raw, "move", "page", "parent", "", 1005);
    assert!(upsert_note_app_data_json(&moved, &stale.to_string(), 1006).is_none());
    let mut fresh = note(&moved, "page");
    fresh["title"] = json!("新标题");
    assert!(upsert_note_app_data_json(&moved, &fresh.to_string(), 1007).is_some());
}

#[test]
fn knowledge_hierarchy_locked_or_deleted_parent_blocks_child_creation_and_moves() {
    let raw = action(&state(), "create_page", "parent", "", "父级", 1001);
    let raw = action(&raw, "create_page", "child", "parent", "子页", 1002);
    let mut locked = note(&raw, "parent");
    locked["document"]["knowledge"]["locked"] = json!(true);
    let locked = upsert_note_app_data_json(&raw, &locked.to_string(), 1003).unwrap();
    assert!(knowledge_container_action_app_data_json(
        &locked,
        "create_page",
        "new",
        "child",
        "",
        1004
    )
    .is_none());
    assert!(
        knowledge_container_action_app_data_json(&locked, "to_folder", "parent", "", "", 1004)
            .is_none()
    );
    let deleted = delete_note_app_data_json(&raw, "parent", 1003).unwrap();
    assert!(knowledge_container_action_app_data_json(
        &deleted,
        "create_page",
        "new",
        "parent",
        "",
        1004
    )
    .is_none());
    assert!(knowledge_container_action_app_data_json(
        &deleted,
        "to_folder",
        "parent",
        "",
        "",
        1004
    )
    .is_none());
}

#[test]
fn knowledge_hierarchy_container_encryption_requires_relocating_live_children() {
    let raw = action(&state(), "create_page", "parent", "", "父级", 1001);
    let raw = action(&raw, "create_page", "child", "parent", "子页", 1002);
    let data: AppData = serde_json::from_str(&raw).unwrap();
    let previous = data.notes.iter().find(|note| note.id == "parent").unwrap();
    let mut protected = previous.clone();
    protected.encryption = Some(NoteEncryptionEnvelope::default());
    assert!(!knowledge_save_allowed(&data, Some(previous), &protected));
    let moved = action(&raw, "move", "child", "", "", 1003);
    let data: AppData = serde_json::from_str(&moved).unwrap();
    assert!(knowledge_save_allowed(&data, Some(previous), &protected));
}

#[test]
fn knowledge_hierarchy_delete_preserves_children_and_history_at_former_parent() {
    let raw = action(&state(), "create_page", "outer", "", "外层", 1001);
    let raw = action(&raw, "create_folder", "parent", "outer", "文件夹", 1002);
    let raw = action(&raw, "create_page", "child", "parent", "正文", 1003);
    let raw = action(
        &raw,
        "create_folder",
        "grandchild",
        "child",
        "子文件夹",
        1004,
    );
    let deleted = delete_note_app_data_json(&raw, "parent", 1005).unwrap();
    assert_eq!(
        note(&deleted, "child")["document"]["knowledge"]["parentId"],
        "outer"
    );
    assert_eq!(
        note(&deleted, "grandchild")["document"]["knowledge"]["parentId"],
        "child"
    );
    assert!(note(&deleted, "child")["revisions"]
        .as_array()
        .unwrap()
        .iter()
        .any(|snapshot| snapshot["label"] == "父级删除前"
            && snapshot["document"]["knowledge"]["parentId"] == "parent"));
    let restored = restore_note_app_data_json(&deleted, "parent", 1006).unwrap();
    assert_eq!(
        note(&restored, "child")["document"]["knowledge"]["parentId"],
        "outer"
    );
    let hard = delete_note_permanently_app_data_json(&deleted, "parent", 1006).unwrap();
    assert_eq!(note(&hard, "child")["title"], "正文");
}

#[test]
fn knowledge_hierarchy_legacy_delete_keeps_flat_notes_and_nested_content_reachable() {
    let raw = create_note_folder_app_data_json(&state(), "legacy", "旧文件夹", 1001).unwrap();
    let raw=upsert_note_app_data_json(&raw,&json!({"id":"old-child","kind":"DOCUMENT","title":"旧页面","content":"旧正文","folderId":"legacy"}).to_string(),1002).unwrap();
    let raw = action(
        &raw,
        "create_folder",
        "new-folder",
        "legacy",
        "新文件夹",
        1003,
    );
    let deleted = delete_note_folder_app_data_json(&raw, "legacy", 1004).unwrap();
    assert_eq!(note(&deleted, "old-child")["content"], "旧正文");
    assert!(note(&deleted, "old-child")["folderId"].is_null());
    assert!(note(&deleted, "new-folder")["document"]["knowledge"]["parentId"].is_null());
}

#[test]
fn knowledge_hierarchy_colliding_ids_database_conversion_and_unknown_actions_are_rejected() {
    let raw = action(&state(), "create_page", "same", "", "页面", 1001);
    assert!(knowledge_container_action_app_data_json(
        &raw,
        "create_folder",
        "same",
        "",
        "目录",
        1002
    )
    .is_none());
    let legacy = create_note_folder_app_data_json(&raw, "same", "原文件夹", 1002).unwrap();
    assert!(knowledge_container_action_app_data_json(
        &legacy,
        "promote_folder",
        "same",
        "",
        "",
        1003
    )
    .is_none());
    let mut page = note(&raw, "same");
    page["document"]["knowledge"]["database"] =
        serde_json::to_value(crate::knowledge::KnowledgeDatabase::task_database()).unwrap();
    let database = upsert_note_app_data_json(&raw, &page.to_string(), 1002).unwrap();
    assert!(
        knowledge_container_action_app_data_json(&database, "to_folder", "same", "", "", 1003)
            .is_none()
    );
    assert!(
        knowledge_container_action_app_data_json(&raw, "purge", "same", "", "", 1003).is_none()
    );
}

#[test]
fn knowledge_hierarchy_sync_concurrent_moves_keep_folder_metadata_and_all_content() {
    let raw = action(&state(), "create_folder", "a", "", "A", 1001);
    let raw = action(&raw, "create_folder", "b", "", "B", 1002);
    let left = action(&raw, "move", "a", "b", "", 1003);
    let right = action(&raw, "move", "b", "a", "", 1004);
    let merged = crate::sync_core::merge_sync_app_data_json(&left, &right, 1005).unwrap();
    let reverse = crate::sync_core::merge_sync_app_data_json(&right, &left, 1005).unwrap();
    assert_eq!(merged, reverse);
    for id in ["a", "b"] {
        assert_eq!(
            note(&merged, id)["document"]["knowledge"]["properties"]
                [crate::knowledge::FOLDER_KIND_PROPERTY]["value"],
            "folder"
        );
    }
    assert!(note(&merged, "a")["document"]["knowledge"]["parentId"].is_null());
    assert_eq!(note(&merged, "b")["document"]["knowledge"]["parentId"], "a");
    assert_eq!(sanitize_app_data_json(&merged, 1006).unwrap(), merged);
}

#[test]
fn knowledge_hierarchy_deleting_promoted_page_does_not_resurrect_legacy_folder() {
    let raw = create_note_folder_app_data_json(&state(), "legacy", "旧文件夹", 1001).unwrap();
    let raw = action(&raw, "promote_folder", "legacy", "", "", 1002);
    let raw = action(&raw, "create_page", "child", "legacy", "子页", 1003);
    let raw = action(&raw, "to_page", "legacy", "", "", 1004);
    let deleted = delete_note_app_data_json(&raw, "legacy", 1005).unwrap();
    assert!(
        serde_json::from_str::<Value>(&deleted).unwrap()["noteFolders"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    assert!(note(&deleted, "child")["document"]["knowledge"]["parentId"].is_null());
    assert!(note(&deleted, "child")["folderId"].is_null());
    assert_eq!(note(&deleted, "child")["title"], "子页");
}

#[test]
fn knowledge_hierarchy_unrelated_orphan_projection_is_preserved_and_promotion_is_idempotent() {
    let raw=upsert_note_app_data_json(&state(),&json!({"id":"orphan","kind":"DOCUMENT","content":"保留","folderId":"temporarily-missing-folder"}).to_string(),1001).unwrap();
    let raw = action(&raw, "create_page", "parent", "", "父页", 1002);
    assert_eq!(
        note(&raw, "orphan")["folderId"],
        "temporarily-missing-folder"
    );
    let deleted = delete_note_app_data_json(&raw, "parent", 1003).unwrap();
    assert_eq!(
        note(&deleted, "orphan")["folderId"],
        "temporarily-missing-folder"
    );
    let raw = create_note_folder_app_data_json(&raw, "legacy", "旧文件夹", 1003).unwrap();
    let promoted = action(&raw, "promote_folder", "legacy", "parent", "", 1004);
    let retry = action(&promoted, "promote_folder", "legacy", "", "", 1005);
    assert_eq!(
        note(&retry, "legacy")["document"]["knowledge"]["parentId"],
        "parent"
    );
    assert_eq!(retry, promoted);
}

#[test]
fn knowledge_hierarchy_parent_delete_reparents_locked_child_without_changing_content_or_lock() {
    let raw = action(&state(), "create_folder", "parent", "", "目录", 1001);
    let raw=upsert_note_app_data_json(&raw,&json!({"id":"child","kind":"DOCUMENT","title":"已锁定","content":"锁定正文","document":{"knowledge":{"parentId":"parent","locked":true},"blocks":[{"id":"body","type":"TEXT","text":"锁定正文"}]}}).to_string(),1002).unwrap();
    let before = note(&raw, "child");
    let deleted = delete_note_app_data_json(&raw, "parent", 1003).unwrap();
    let after = note(&deleted, "child");
    assert_eq!(after["title"], before["title"]);
    assert_eq!(after["content"], before["content"]);
    assert_eq!(after["document"]["blocks"], before["document"]["blocks"]);
    assert_eq!(after["document"]["knowledge"]["locked"], true);
    assert!(after["document"]["knowledge"]["parentId"].is_null());
    assert_eq!(after["attachments"], before["attachments"]);
}

#[test]
fn knowledge_hierarchy_stale_legacy_parent_cannot_undo_root_move() {
    let raw = create_note_folder_app_data_json(&state(), "legacy", "旧文件夹", 1001).unwrap();
    let raw=upsert_note_app_data_json(&raw,&json!({"id":"page","kind":"DOCUMENT","title":"页面","folderId":"legacy","document":{"knowledge":{},"blocks":[{"id":"body","type":"TEXT","text":"正文"}]}}).to_string(),1002).unwrap();
    let stale = note(&raw, "page");
    let moved = action(&raw, "move", "page", "", "", 1003);
    assert!(note(&moved, "page")["folderId"].is_null());
    assert!(note(&moved, "page")["document"]["knowledge"]["parentId"].is_null());
    assert!(upsert_note_app_data_json(&moved, &stale.to_string(), 1004).is_none());
    assert_eq!(note(&moved, "page")["content"], "正文");
}

#[test]
fn knowledge_hierarchy_nested_page_or_folder_cannot_lose_position_on_first_encryption() {
    let raw = action(&state(), "create_folder", "parent", "", "目录", 1001);
    let raw = action(&raw, "create_page", "child", "parent", "子页面", 1002);
    let data: AppData = serde_json::from_str(&raw).unwrap();
    for id in ["parent", "child"] {
        let previous = data.notes.iter().find(|note| note.id == id).unwrap();
        let mut protected = previous.clone();
        protected.encryption = Some(NoteEncryptionEnvelope::default());
        assert!(!knowledge_save_allowed(&data, Some(previous), &protected));
    }
    let standalone = action(&state(), "create_folder", "folder", "", "空文件夹", 1001);
    let data: AppData = serde_json::from_str(&standalone).unwrap();
    let previous = data.notes.iter().find(|note| note.id == "folder").unwrap();
    let mut protected = previous.clone();
    protected.encryption = Some(NoteEncryptionEnvelope::default());
    assert!(!knowledge_save_allowed(&data, Some(previous), &protected));
    let moved = action(&raw, "move", "child", "", "", 1003);
    let data: AppData = serde_json::from_str(&moved).unwrap();
    let previous = data.notes.iter().find(|note| note.id == "child").unwrap();
    let mut protected = previous.clone();
    protected.encryption = Some(NoteEncryptionEnvelope::default());
    assert!(knowledge_save_allowed(&data, Some(previous), &protected));
    assert!(knowledge_save_allowed(&data, Some(&protected), &protected));
}

#[test]
fn knowledge_hierarchy_ambiguous_legacy_identity_is_read_only_and_preserves_both_records() {
    let raw = action(&state(), "create_page", "conflict", "", "独立页面", 1001);
    let raw = create_note_folder_app_data_json(&raw, "conflict", "独立旧文件夹", 1002).unwrap();
    for kind in ["move", "rename", "to_folder", "to_page", "promote_folder"] {
        assert!(
            knowledge_container_action_app_data_json(&raw, kind, "conflict", "", "新名", 1003)
                .is_none()
        );
    }
    assert!(knowledge_container_action_app_data_json(
        &raw,
        "create_page",
        "new",
        "conflict",
        "子页",
        1003
    )
    .is_none());
    let mut changed = note(&raw, "conflict");
    changed["title"] = json!("不能覆盖");
    assert!(upsert_note_app_data_json(&raw, &changed.to_string(), 1003).is_none());
    assert!(delete_note_app_data_json(&raw, "conflict", 1003).is_none());
    assert!(delete_note_folder_app_data_json(&raw, "conflict", 1003).is_none());
    assert!(delete_note_permanently_app_data_json(&raw, "conflict", 1003).is_none());
    assert_eq!(note(&raw, "conflict")["title"], "独立页面");
    assert_eq!(
        serde_json::from_str::<Value>(&raw).unwrap()["noteFolders"][0]["name"],
        "独立旧文件夹"
    );
    assert_eq!(sanitize_app_data_json(&raw, 1004).unwrap(), raw);
}

#[test]
fn knowledge_hierarchy_promoted_origin_cannot_be_removed_or_hidden_by_encryption() {
    let raw = create_note_folder_app_data_json(&state(), "legacy", "旧文件夹", 1001).unwrap();
    let raw = action(&raw, "to_page", "legacy", "", "", 1002);
    let data: AppData = serde_json::from_str(&raw).unwrap();
    let previous = data.notes.iter().find(|note| note.id == "legacy").unwrap();
    let mut protected = previous.clone();
    protected.encryption = Some(NoteEncryptionEnvelope::default());
    assert!(!knowledge_save_allowed(&data, Some(previous), &protected));
    let mut changed = note(&raw, "legacy");
    changed["document"]["knowledge"]["properties"]
        .as_object_mut()
        .unwrap()
        .remove(crate::knowledge::LEGACY_FOLDER_PROPERTY);
    assert!(upsert_note_app_data_json(&raw, &changed.to_string(), 1003).is_none());
    let renamed = action(&raw, "rename", "legacy", "", "新页面名", 1003);
    assert_eq!(note(&renamed, "legacy")["title"], "新页面名");
    // A copied page has no same-ID legacy projection. Its inherited provenance
    // must not invent an encryption restriction after moving to the root.
    let mut clone = previous.clone();
    clone.id = "new-copy".to_string();
    clone.encryption = Some(NoteEncryptionEnvelope::default());
    let mut plain_clone = previous.clone();
    plain_clone.id = clone.id.clone();
    assert!(knowledge_save_allowed(&data, Some(&plain_clone), &clone));
}

#[test]
fn creating_a_previously_missing_parent_keeps_dangling_children_compatible() {
    let raw = create_note_folder_app_data_json(&state(), "legacy", "旧目录", 1001).unwrap();
    let raw = action(&raw, "create_page", "orphan", "", "孤立子页", 1002);
    let mut value = serde_json::from_str::<Value>(&raw).unwrap();
    let orphan = value["notes"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|note| note["id"] == "orphan")
        .unwrap();
    orphan["document"]["knowledge"]["parentId"] = json!("future");
    let raw = sanitize_app_data_json(&value.to_string(), 1003).unwrap();
    let mut expected = parse_app_data_for_mutation(&raw, 1004).unwrap();
    let mut parent = NoteEntry {
        id: "future".to_owned(),
        title: "补建父页面".to_owned(),
        kind: NoteEntryKind::Document,
        ..Default::default()
    };
    knowledge_ensure_page(&mut parent).parent_id = Some("legacy".to_owned());
    parent = normalize_note_for_save(&expected, parent, None, 1004).unwrap();
    expected.notes.push(parent);
    knowledge_refresh_folder_projection(&mut expected, &HashSet::from(["future".into()]), 1004)
        .unwrap();
    let expected = serde_json::to_value(expected.sanitized(1004).unwrap()).unwrap();
    let actual = action(&raw, "create_page", "future", "legacy", "补建父页面", 1004);
    assert_eq!(serde_json::from_str::<Value>(&actual).unwrap(), expected);
    assert_eq!(note(&actual, "orphan")["folderId"], "legacy");
}

