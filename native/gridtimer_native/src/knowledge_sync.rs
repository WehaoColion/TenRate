// v2.23.2.29 - Repair concurrent typed and legacy-folder parent cycles without losing content.
use serde_json::{json, Value};
use std::collections::{BTreeMap, HashMap, HashSet};

/// Keep the newest move in a cycle, detach the oldest edge, and retain the
/// original page as a named recovery point. No title, block or property changes.
fn effective_parent<'a>(page: &'a Value, folders: &HashSet<String>) -> Option<&'a str> {
    page["document"]["knowledge"]["parentId"]
        .as_str()
        .filter(|id| !id.is_empty())
        .or_else(|| {
            (page["kind"].as_str() == Some("DOCUMENT"))
                .then(|| page["folderId"].as_str())
                .flatten()
                .filter(|id| page["id"].as_str() != Some(*id) && folders.contains(*id))
        })
}

pub fn resolve_parent_cycles(data: &mut Value) {
    let folders = data["noteFolders"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|folder| folder["id"].as_str().map(str::to_string))
        .collect::<HashSet<_>>();
    let Some(notes) = data["notes"].as_array_mut() else {
        return;
    };
    let index = notes
        .iter()
        .enumerate()
        .filter(|(_, p)| p["encryption"].is_null() && p["deletedAtEpochMillis"].is_null())
        .filter_map(|(i, p)| Some((p["id"].as_str()?.to_string(), i)))
        .collect::<BTreeMap<_, _>>();
    let mut finished = HashSet::new();
    for start in index.keys() {
        let mut path = Vec::<String>::new();
        let mut positions = HashMap::new();
        let mut current = start.clone();
        while let Some(&i) = index.get(&current) {
            if finished.contains(&current) {
                break;
            }
            if let Some(&begin) = positions.get(&current) {
                let cycle = &path[begin..];
                let chosen = cycle
                    .iter()
                    .min_by_key(|id| {
                        (
                            notes[index[*id]]["updatedAtEpochMillis"]
                                .as_i64()
                                .unwrap_or(0),
                            *id,
                        )
                    })
                    .unwrap();
                let page = &mut notes[index[chosen]];
                let revision = page["updatedAtEpochMillis"].as_i64().unwrap_or(0);
                let parent = effective_parent(page, &folders).unwrap_or("").to_string();
                let revision_id = format!("parent-conflict-{}-{}-{}", chosen, parent, revision);
                let mut snapshot = json!({"id":revision_id,"label":"父级冲突前","capturedAtEpochMillis":revision,"updatedAtEpochMillis":revision});
                for key in [
                    "title",
                    "content",
                    "kind",
                    "document",
                    "accentSeed",
                    "pinned",
                    "folderId",
                    "attachments",
                ] {
                    if let Some(v) = page.get(key) {
                        snapshot[key] = v.clone();
                    }
                }
                let revisions = page
                    .as_object_mut()
                    .unwrap()
                    .entry("revisions")
                    .or_insert_with(|| json!([]));
                if let Some(revisions) = revisions.as_array_mut() {
                    if !revisions.iter().any(|r| r["id"] == revision_id) {
                        revisions.push(snapshot);
                    }
                }
                page["document"]["knowledge"]["parentId"] = Value::Null;
                // Otherwise the old flat-folder projection would immediately
                // recreate the edge after clearing the typed parent.
                page["folderId"] = Value::Null;
                break;
            }
            positions.insert(current.clone(), path.len());
            path.push(current.clone());
            let Some(parent) = effective_parent(&notes[i], &folders) else {
                break;
            };
            current = parent.into();
        }
        finished.extend(path);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn knowledge_hierarchy_legacy_projection_cycle_recovers_original_folder_and_body() {
        let mut data = json!({"noteFolders":[{"id":"a","name":"A"}],"notes":[
            {"id":"a","kind":"DOCUMENT","title":"A","updatedAtEpochMillis":20,
                "document":{"knowledge":{"parentId":"b"},"blocks":[{"id":"a-text","type":"TEXT","text":"Folder body"}]}},
            {"id":"b","kind":"DOCUMENT","title":"B","folderId":"a","updatedAtEpochMillis":10,
                "document":{"blocks":[{"id":"b-text","type":"TEXT","text":"Child body"}]}}
        ]});
        resolve_parent_cycles(&mut data);
        let child = &data["notes"][1];
        assert!(child["folderId"].is_null());
        assert!(child["document"]["knowledge"]["parentId"].is_null());
        assert_eq!(child["document"]["blocks"][0]["text"], "Child body");
        assert_eq!(child["revisions"][0]["folderId"], "a");
        assert_eq!(child["revisions"][0]["label"], "父级冲突前");
        let resolved = data.clone();
        resolve_parent_cycles(&mut data);
        assert_eq!(resolved, data);
    }

    #[test]
    fn knowledge_hierarchy_mixed_parent_moves_sync_converges_and_does_not_recreate_cycle() {
        use crate::{app_data, sync_core};
        let base = app_data::sanitize_app_data_json("{}", 1000).unwrap();
        let base = app_data::create_note_folder_app_data_json(&base, "a", "A", 1001).unwrap();
        let base = app_data::knowledge_container_action_app_data_json(
            &base,
            "promote_folder",
            "a",
            "",
            "",
            1002,
        )
        .unwrap();
        let base = app_data::knowledge_container_action_app_data_json(
            &base,
            "create_page",
            "b",
            "",
            "B",
            1003,
        )
        .unwrap();
        let left =
            app_data::knowledge_container_action_app_data_json(&base, "move", "a", "b", "", 1010)
                .unwrap();
        let right = app_data::move_note_to_folder_app_data_json(&base, "b", "a", 1005).unwrap();
        let merged = sync_core::merge_sync_app_data_json(&left, &right, 1020).unwrap();
        let reverse = sync_core::merge_sync_app_data_json(&right, &left, 1020).unwrap();
        assert_eq!(merged, reverse);
        let value: Value = serde_json::from_str(&merged).unwrap();
        let child = value["notes"]
            .as_array()
            .unwrap()
            .iter()
            .find(|p| p["id"] == "b")
            .unwrap();
        assert!(child["folderId"].is_null());
        assert!(child["document"]["knowledge"]["parentId"].is_null());
        assert!(child["revisions"]
            .as_array()
            .unwrap()
            .iter()
            .any(|r| r["folderId"] == "a" && r["label"] == "父级冲突前"));
        let again = sync_core::merge_sync_app_data_json(&merged, &right, 1021).unwrap();
        let again_value: Value = serde_json::from_str(&again).unwrap();
        // The generic merge may append another recovery version of the repaired
        // winner. Replaying old data must preserve the current records and every
        // existing recovery point, rather than recreate a hierarchy cycle.
        for original in value["notes"].as_array().unwrap() {
            let replayed = again_value["notes"]
                .as_array()
                .unwrap()
                .iter()
                .find(|page| page["id"] == original["id"])
                .unwrap();
            let mut original_current = original.clone();
            let mut replayed_current = replayed.clone();
            original_current
                .as_object_mut()
                .unwrap()
                .remove("revisions");
            replayed_current
                .as_object_mut()
                .unwrap()
                .remove("revisions");
            assert_eq!(original_current, replayed_current);
            for revision in original["revisions"].as_array().unwrap() {
                assert!(replayed["revisions"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|retained| retained == revision));
            }
        }
    }

    #[test]
    fn knowledge_concurrent_moves_keep_pages_and_recoverable_original_parent() {
        use crate::{app_data, sync_core};
        let base = app_data::sanitize_app_data_json("{}", 1000).unwrap();
        let pages = json!([
            {"id":"a","kind":"DOCUMENT","title":"A","document":{"knowledge":{},"blocks":[{"id":"a-text","type":"TEXT","text":"A body"}]}},
            {"id":"b","kind":"DOCUMENT","title":"B","document":{"knowledge":{},"blocks":[{"id":"b-text","type":"TEXT","text":"B body"}]}}
        ]);
        let base = app_data::upsert_knowledge_pages_app_data_json(&base, &pages.to_string(), 1001)
            .unwrap();
        let mut a: Value = serde_json::from_str(&base).unwrap();
        let mut b = a.clone();
        let note_a = a["notes"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|p| p["id"] == "a")
            .unwrap();
        note_a["document"]["knowledge"]["parentId"] = json!("b");
        let left = app_data::upsert_note_app_data_json(&base, &note_a.to_string(), 1002).unwrap();
        let note_b = b["notes"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|p| p["id"] == "b")
            .unwrap();
        note_b["document"]["knowledge"]["parentId"] = json!("a");
        let right = app_data::upsert_note_app_data_json(&base, &note_b.to_string(), 1003).unwrap();
        let merged = sync_core::merge_sync_app_data_json(&left, &right, 1004).unwrap();
        let reverse = sync_core::merge_sync_app_data_json(&right, &left, 1004).unwrap();
        let projection = |raw: &str| {
            let value: Value = serde_json::from_str(raw).unwrap();
            value["notes"]
                .as_array()
                .unwrap()
                .iter()
                .map(|p| (p["id"].as_str().unwrap().to_string(), p.clone()))
                .collect::<BTreeMap<_, _>>()
        };
        assert_eq!(projection(&merged), projection(&reverse));
        let pages = projection(&merged);
        assert!(pages["a"]["document"]["knowledge"]["parentId"].is_null());
        assert_eq!(pages["b"]["document"]["knowledge"]["parentId"], "a");
        assert_eq!(pages["a"]["document"]["blocks"][0]["text"], "A body");
        assert!(pages["a"]["revisions"]
            .as_array()
            .unwrap()
            .iter()
            .any(|r| r["label"] == "父级冲突前" && r["document"]["knowledge"]["parentId"] == "b"));
        assert_eq!(
            app_data::sanitize_app_data_json(&merged, 1005).unwrap(),
            merged
        );
        let again = sync_core::merge_sync_app_data_json(&merged, &right, 1006).unwrap();
        assert_eq!(projection(&again), projection(&merged));
    }
}

#[cfg(test)]
#[test]
fn knowledge_legacy_sync_cannot_flatten_a_page_but_explicit_restore_can() {
    use crate::{app_data, sync_core};
    let base = app_data::sanitize_app_data_json("{}", 1000).unwrap();
    let page = json!({"id":"structured","kind":"DOCUMENT","title":"完整页面","document":{"knowledge":{},"blocks":[{"id":"table","type":"TEXT","text":"表格","knowledge":{"kind":"table","table":[["列"],["原值"]]}}]}});
    let advanced = app_data::upsert_note_app_data_json(&base, &page.to_string(), 1001).unwrap();
    let mut legacy: Value = serde_json::from_str(&advanced).unwrap();
    let old = legacy["notes"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|p| p["id"] == "structured")
        .unwrap();
    old["document"].as_object_mut().unwrap().remove("knowledge");
    old["document"]["blocks"][0]
        .as_object_mut()
        .unwrap()
        .remove("knowledge");
    old["document"]["blocks"][0]["text"] = json!("旧端新增的文字");
    old["content"] = json!("旧端新增的文字");
    old["updatedAtEpochMillis"] = json!(1003);
    let merged = sync_core::merge_sync_app_data_json(&advanced, &legacy.to_string(), 1004).unwrap();
    let reverse =
        sync_core::merge_sync_app_data_json(&legacy.to_string(), &advanced, 1004).unwrap();
    assert_eq!(merged, reverse);
    let value: Value = serde_json::from_str(&merged).unwrap();
    let current = value["notes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["id"] == "structured")
        .unwrap();
    assert_eq!(
        current["document"]["blocks"][0]["knowledge"]["table"][1][0],
        "原值"
    );
    let recovered = current["revisions"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["document"]["blocks"][0]["text"] == "旧端新增的文字")
        .unwrap();
    let restored = app_data::restore_note_revision_app_data_json(
        &merged,
        "structured",
        recovered["id"].as_str().unwrap(),
        1005,
    )
    .unwrap();
    let resynced = sync_core::merge_sync_app_data_json(&restored, &merged, 1006).unwrap();
    let value: Value = serde_json::from_str(&resynced).unwrap();
    let current = value["notes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["id"] == "structured")
        .unwrap();
    assert_eq!(current["document"]["blocks"][0]["text"], "旧端新增的文字");
    assert!(current["document"]["knowledge"].is_object());
}
