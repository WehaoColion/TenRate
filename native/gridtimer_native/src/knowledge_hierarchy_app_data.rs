// v2.23.4 - Resolve a newly created container's legacy projection along its ancestor chain.\n// v2.23.2.29 - Lossless page/folder conversions and guarded shared hierarchy mutations.
// Folders use existing version-1 typed properties, so old clients keep their
// contents and hierarchy without accepting a new data or sync format.

fn knowledge_folder_marker(note: &NoteEntry) -> bool {
    note.document.knowledge.as_ref().is_some_and(|page| {
        matches!(page.properties.get(crate::knowledge::FOLDER_KIND_PROPERTY),
            Some(crate::knowledge::CellValue::Text(kind)) if kind == "folder")
    })
}

fn knowledge_legacy_folder_origin(note: &NoteEntry) -> bool {
    note.kind == NoteEntryKind::Document
        && note.document.knowledge.as_ref().is_some_and(|page| {
            page.properties
                .get(crate::knowledge::LEGACY_FOLDER_PROPERTY)
                == Some(&crate::knowledge::CellValue::Checkbox(true))
        })
}

fn knowledge_identity_conflict(data: &AppData, id: &str) -> bool {
    data.note_folders.iter().any(|folder| folder.id == id)
        && data
            .notes
            .iter()
            .find(|note| note.id == id)
            .is_some_and(|note| !knowledge_legacy_folder_origin(note))
}

fn knowledge_effective_parent<'a>(data: &'a AppData, note: &'a NoteEntry) -> Option<&'a str> {
    note.document
        .knowledge
        .as_ref()
        .and_then(|page| page.parent_id.as_deref())
        .filter(|id| !id.is_empty())
        .or_else(|| {
            note.folder_id.as_deref().filter(|id| {
                *id != note.id && data.note_folders.iter().any(|folder| folder.id == *id)
            })
        })
}

fn knowledge_container_mutable(note: &NoteEntry) -> bool {
    note.kind == NoteEntryKind::Document
        && note.deleted_at_epoch_millis.is_none()
        && note.encryption.is_none()
        && !note
            .document
            .knowledge
            .as_ref()
            .is_some_and(|page| page.locked)
}

fn knowledge_parent_allowed(data: &AppData, note_id: &str, parent_id: Option<&str>) -> bool {
    let mut seen = HashSet::from([note_id]);
    let mut current = parent_id;
    while let Some(id) = current {
        if !seen.insert(id) || knowledge_identity_conflict(data, id) {
            return false;
        }
        if let Some(parent) = data.notes.iter().find(|note| note.id == id) {
            if !knowledge_container_mutable(parent) {
                return false;
            }
            current = knowledge_effective_parent(data, parent);
        } else if data.note_folders.iter().any(|folder| folder.id == id) {
            return true;
        } else {
            return false;
        }
    }
    true
}

fn knowledge_has_live_children(data: &AppData, id: &str) -> bool {
    data.notes.iter().any(|note| {
        note.id != id
            && note.kind == NoteEntryKind::Document
            && note.deleted_at_epoch_millis.is_none()
            && knowledge_effective_parent(data, note) == Some(id)
    })
}

fn knowledge_ensure_page(note: &mut NoteEntry) -> &mut crate::knowledge::KnowledgePage {
    // Attach metadata only after resolving legacy body text into blocks.
    // Otherwise a knowledge-only document would replace the legacy body.
    note.document = note.resolved_document();
    note.document.knowledge.get_or_insert_with(Default::default)
}

fn knowledge_capture_structure(note: &mut NoteEntry, label: &str, now: i64) -> Option<()> {
    let revision = next_mutation_revision(note_revision_epoch_millis(note), now)?;
    let mut snapshot = note.snapshot_for_history(now.max(0), revision);
    snapshot.id = format!("hierarchy-{}-{revision}", note.id);
    snapshot.label = label.to_string();
    note.revisions.insert(0, snapshot);
    Some(())
}

/// Project the nearest legacy folder for clients with the original flat UI.
/// The typed parent remains authoritative, and all content stays in the note.
fn knowledge_refresh_folder_projection(
    data: &mut AppData,
    roots: &HashSet<String>,
    now: i64,
) -> Option<()> {
    let folders = data
        .note_folders
        .iter()
        .map(|folder| folder.id.as_str())
        .collect::<HashSet<_>>();
    let parents = data
        .notes
        .iter()
        .filter(|note| note.kind == NoteEntryKind::Document)
        .map(|note| (note.id.as_str(), knowledge_effective_parent(data, note)))
        .collect::<HashMap<_, _>>();
    let mut children = HashMap::<&str, Vec<&str>>::new();
    for (id, parent) in &parents {
        if let Some(parent) = parent {
            children.entry(parent).or_default().push(id);
        }
    }
    let mut scope = roots.iter().map(String::as_str).collect::<HashSet<_>>();
    let mut pending = scope.iter().copied().collect::<Vec<_>>();
    while let Some(id) = pending.pop() {
        if let Some(descendants) = children.get(id) {
            for child in descendants {
                if scope.insert(child) {
                    pending.push(child);
                }
            }
        }
    }
    let mut cache = HashMap::<&str, Option<String>>::new();
    let mut projections = HashMap::<String, Option<String>>::new();
    for note in data
        .notes
        .iter()
        .filter(|note| note.kind == NoteEntryKind::Document && scope.contains(note.id.as_str()))
    {
        let mut path = Vec::new();
        let mut seen = HashSet::new();
        let mut current = parents.get(note.id.as_str()).copied().flatten();
        let result = loop {
            let Some(id) = current else {
                break None;
            };
            if folders.contains(id) {
                break Some(id.to_string());
            }
            if let Some(cached) = cache.get(id) {
                break cached.clone();
            }
            if !seen.insert(id) {
                break None;
            }
            path.push(id);
            current = parents.get(id).copied().flatten();
        };
        for id in path {
            cache.insert(id, result.clone());
        }
        projections.insert(note.id.clone(), result);
    }
    for note in &mut data.notes {
        let Some(projected) = projections.get(&note.id) else {
            continue;
        };
        if &note.folder_id != projected {
            note.folder_id = projected.clone();
            note.updated_at_epoch_millis =
                next_mutation_revision(note_revision_epoch_millis(note), now)?;
        }
    }
    Some(())
}

/// A newly created container has no descendants. Compute only its own legacy
/// folder projection instead of constructing the entire workspace hierarchy.
/// The full-tree refresh remains authoritative for move/rename/conversion.
fn knowledge_refresh_new_container_projection(
    data: &mut AppData,
    id: &str,
    now: i64,
) -> Option<()> {
    let created = data.notes.iter().find(|note| note.id == id)?;
    let mut next = knowledge_effective_parent(data, created).map(str::to_owned);
    let mut visited = HashSet::from([id.to_owned()]);
    let mut legacy_folder = None;
    while let Some(parent_id) = next {
        if !visited.insert(parent_id.clone()) {
            return None;
        }
        // A promoted legacy folder also has a note of the same id. As in the
        // full projection, the legacy folder takes precedence.
        if data.note_folders.iter().any(|folder| folder.id == parent_id) {
            legacy_folder = Some(parent_id);
            break;
        }
        let parent = data.notes.iter().find(|note| note.id == parent_id)?;
        next = knowledge_effective_parent(data, parent).map(str::to_owned);
    }
    let created = data.notes.iter_mut().find(|note| note.id == id)?;
    if created.folder_id != legacy_folder {
        created.folder_id = legacy_folder;
        created.updated_at_epoch_millis =
            next_mutation_revision(note_revision_epoch_millis(created), now)?;
    }
    Some(())
}

fn knowledge_promote_legacy_folder(data: &mut AppData, id: &str, now: i64) -> Option<()> {
    if data.notes.iter().any(|note| note.id == id) {
        return None;
    }
    let folder = data
        .note_folders
        .iter()
        .find(|folder| folder.id == id)?
        .clone();
    let mut note = NoteEntry {
        id: folder.id,
        title: folder.name,
        kind: NoteEntryKind::Document,
        created_at_epoch_millis: folder.created_at_epoch_millis,
        ..Default::default()
    };
    knowledge_ensure_page(&mut note).properties.insert(
        crate::knowledge::FOLDER_KIND_PROPERTY.to_string(),
        crate::knowledge::CellValue::Text("folder".into()),
    );
    knowledge_ensure_page(&mut note).properties.insert(
        crate::knowledge::LEGACY_FOLDER_PROPERTY.to_string(),
        crate::knowledge::CellValue::Checkbox(true),
    );
    note = normalize_note_for_save(data, note, None, now)?;
    data.notes.push(note);
    Some(())
}

/// Atomic hierarchy operations. Empty parent means the knowledge root.
/// No operation removes a body, attachment, version, or descendants.
pub fn knowledge_container_action_app_data_json(
    raw: &str,
    action: &str,
    id: &str,
    parent_id: &str,
    title: &str,
    now: i64,
) -> Option<String> {
    if id.is_empty() || id.len() > 512 || parent_id.len() > 512 || title.chars().count() > 240 {
        return None;
    }
    let mut data = parse_app_data_for_mutation(raw, now)?;
    if knowledge_identity_conflict(&data, id) {
        return None;
    }
    let parent = (!parent_id.is_empty()).then_some(parent_id);
    match action {
        "create_page" | "create_folder" => {
            if data.notes.iter().any(|note| note.id == id)
                || data.note_folders.iter().any(|folder| folder.id == id)
                || !knowledge_parent_allowed(&data, id, parent)
                || (action == "create_folder" && title.trim().is_empty())
            {
                return None;
            }
            let mut note = NoteEntry {
                id: id.to_string(),
                title: title.to_string(),
                kind: NoteEntryKind::Document,
                ..Default::default()
            };
            let page = knowledge_ensure_page(&mut note);
            page.parent_id = parent.map(str::to_string);
            if action == "create_folder" {
                page.properties.insert(
                    crate::knowledge::FOLDER_KIND_PROPERTY.to_string(),
                    crate::knowledge::CellValue::Text("folder".into()),
                );
            }
            note = normalize_note_for_save(&data, note, None, now)?;
            data.notes.push(note);
        }
        "move" | "promote_folder" | "rename" | "to_folder" | "to_page" => {
            if !data.notes.iter().any(|note| note.id == id) {
                if matches!(action, "move" | "promote_folder" | "rename" | "to_page") {
                    knowledge_promote_legacy_folder(&mut data, id, now)?;
                } else {
                    return None;
                }
            } else if action == "promote_folder" {
                // Retrying promotion may return the existing folder, but may
                // never repurpose an unrelated colliding page ID.
                if !knowledge_folder_marker(data.notes.iter().find(|note| note.id == id)?)
                    || !data.note_folders.iter().any(|folder| folder.id == id)
                {
                    return None;
                }
                return serde_json::to_string(&data).ok();
            }
            let previous = data.notes.iter().find(|note| note.id == id)?.clone();
            if !knowledge_container_mutable(&previous) {
                return None;
            }
            let mut note = previous.clone();
            match action {
                "move" | "promote_folder" => {
                    if !knowledge_parent_allowed(&data, id, parent) {
                        return None;
                    }
                    knowledge_capture_structure(&mut note, "层级移动前", now)?;
                    knowledge_ensure_page(&mut note).parent_id = parent.map(str::to_string);
                    // Root moves must not regain the previous legacy parent.
                    note.folder_id = None;
                }
                "to_folder" | "to_page" => {
                    if action == "to_folder"
                        && note
                            .document
                            .knowledge
                            .as_ref()
                            .is_some_and(|page| page.database.is_some())
                    {
                        return None;
                    }
                    knowledge_capture_structure(
                        &mut note,
                        if action == "to_folder" {
                            "转为文件夹前"
                        } else {
                            "转为页面前"
                        },
                        now,
                    )?;
                    let page = knowledge_ensure_page(&mut note);
                    if action == "to_folder" {
                        page.properties.insert(
                            crate::knowledge::FOLDER_KIND_PROPERTY.to_string(),
                            crate::knowledge::CellValue::Text("folder".into()),
                        );
                    } else {
                        page.properties
                            .remove(crate::knowledge::FOLDER_KIND_PROPERTY);
                    }
                }
                "rename" => {
                    if title.trim().is_empty() {
                        return None;
                    }
                    knowledge_capture_structure(&mut note, "重命名前", now)?;
                    note.title = title.to_string();
                    if let Some(folder) =
                        data.note_folders.iter_mut().find(|folder| folder.id == id)
                    {
                        folder.name = title.to_string();
                        folder.updated_at_epoch_millis =
                            next_mutation_revision(folder.updated_at_epoch_millis, now)?;
                    }
                }
                _ => unreachable!(),
            }
            note = normalize_note_for_save(&data, note, Some(&previous), now)?;
            data.notes.retain(|stored| stored.id != id);
            data.notes.push(note);
        }
        _ => return None,
    }
    if matches!(action, "create_page" | "create_folder") {
        knowledge_refresh_new_container_projection(&mut data, id, now)?;
    } else {
        knowledge_refresh_folder_projection(&mut data, &HashSet::from([id.to_string()]), now)?;
    }
    serde_json::to_string(&data.sanitized(now)?).ok()
}

// Remove a compatibility projection only when this note was explicitly
// promoted from that legacy folder. Coincident IDs alone are not authority.
fn knowledge_unlink_legacy_projection(data: &mut AppData, id: &str, now: i64) -> Option<()> {
    let promoted = data
        .notes
        .iter()
        .find(|note| note.id == id)
        .and_then(|note| note.document.knowledge.as_ref())
        .is_some_and(|page| {
            page.properties
                .get(crate::knowledge::LEGACY_FOLDER_PROPERTY)
                == Some(&crate::knowledge::CellValue::Checkbox(true))
        });
    if !promoted {
        return Some(());
    }
    if let Some(folder) = data.note_folders.iter().find(|folder| folder.id == id) {
        let revision = next_deletion_revision(
            &data.tombstones,
            TOMBSTONE_ENTITY_NOTE_FOLDER,
            id,
            note_folder_revision_epoch_millis(folder),
            now,
        )?;
        data.note_folders.retain(|folder| folder.id != id);
        upsert_data_tombstone(
            &mut data.tombstones,
            TOMBSTONE_ENTITY_NOTE_FOLDER,
            id,
            revision,
        );
        for note in &mut data.notes {
            if note.folder_id.as_deref() == Some(id) {
                note.folder_id = None;
                note.updated_at_epoch_millis =
                    next_mutation_revision(note_revision_epoch_millis(note), now)?;
            }
        }
        if data.note_preferences.selected_folder_id.as_deref() == Some(id) {
            data.note_preferences.selected_folder_id = None;
            data.note_preferences_updated_at_epoch_millis =
                next_mutation_revision(data.note_preferences_updated_at_epoch_millis, now)?;
        }
    }
    Some(())
}

/// Deleting a container removes only that container. Direct children keep
/// their contents and move up one level, with a recoverable structure snapshot.
fn knowledge_detach_children_for_delete(data: &mut AppData, id: &str, now: i64) -> Option<()> {
    let former_parent = data
        .notes
        .iter()
        .find(|note| note.id == id)
        .and_then(|note| knowledge_effective_parent(data, note))
        .map(str::to_string);
    let parent = former_parent
        .as_deref()
        .filter(|parent| knowledge_parent_allowed(data, id, Some(parent)))
        .map(str::to_string);
    let children = data
        .notes
        .iter()
        .filter(|note| {
            note.id != id
                && note.kind == NoteEntryKind::Document
                && note.deleted_at_epoch_millis.is_none()
                && knowledge_effective_parent(data, note) == Some(id)
        })
        .map(|note| note.id.clone())
        .collect::<HashSet<_>>();
    for note in &mut data.notes {
        if children.contains(&note.id) && note.encryption.is_none() {
            knowledge_capture_structure(note, "父级删除前", now)?;
            knowledge_ensure_page(note).parent_id = parent.clone();
            note.folder_id = None;
            note.updated_at_epoch_millis =
                next_mutation_revision(note_revision_epoch_millis(note), now)?;
        }
    }
    knowledge_refresh_folder_projection(data, &children, now)
}

#[cfg(test)]
#[path = "knowledge_hierarchy_persistence_tests.rs"]
mod knowledge_hierarchy_persistence_tests;
