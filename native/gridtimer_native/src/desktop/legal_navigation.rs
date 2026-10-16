// Resolve report links against current records before changing any screen.
#[derive(Clone, Debug, PartialEq, Eq)]
enum DesktopLegalDestination {
    Note {
        id: String,
        block: Option<String>,
        attachment: Option<String>,
    },
    Slot(i32),
    History {
        archives: bool,
        id: String,
    },
    Finance {
        monthly: bool,
        period: String,
        entry: Option<(String, String, usize)>,
    },
    FinanceOverview,
    Folder(String),
    Category(String),
}

fn desktop_legal_explicit_document(document: &DesktopNoteDocument) -> bool {
    !document.blocks.is_empty()
        || document.knowledge.is_some()
        || document.rich_text_enabled
        || document.markdown_enabled
        || !document.rich_text_plain_text.is_empty()
}

// A removed container removes its entire retained subtree from the current view.
fn desktop_legal_visible_blocks(document: &DesktopNoteDocument) -> Vec<&DesktopNoteBlock> {
    let mut hidden = document
        .blocks
        .iter()
        .filter(|block| {
            block
                .extra
                .get("deletedAtEpochMillis")
                .is_some_and(|v| !v.is_null())
        })
        .map(|block| block.id.as_str())
        .collect::<HashSet<_>>();
    let mut children = BTreeMap::<&str, Vec<&str>>::new();
    for block in &document.blocks {
        if let Some(parent) = block
            .knowledge
            .as_ref()
            .and_then(|meta| meta.parent_id.as_deref())
        {
            children.entry(parent).or_default().push(&block.id);
        }
    }
    let mut pending = hidden.iter().copied().collect::<Vec<_>>();
    while let Some(parent) = pending.pop() {
        if let Some(ids) = children.get(parent) {
            for id in ids {
                if hidden.insert(id) {
                    pending.push(id);
                }
            }
        }
    }
    document
        .blocks
        .iter()
        .filter(|block| !hidden.contains(block.id.as_str()))
        .collect()
}

fn desktop_legal_current_scan_note(note: &DesktopNote) -> Result<Value, String> {
    let mut current = serde_json::to_value(note).map_err(|error| error.to_string())?;
    let blocks = desktop_legal_visible_blocks(&note.document);
    current["document"]["blocks"] =
        serde_json::to_value(&blocks).map_err(|error| error.to_string())?;
    current["attachments"] = serde_json::to_value(desktop_legal_current_attachments(note))
        .map_err(|error| error.to_string())?;
    current["revisions"] = json!([]);
    current["versions"] = json!([]);
    // The shared scanner recognizes blocks/knowledge as an explicit document.
    // Retain that marker when all blocks were removed or a rich document is empty.
    if desktop_legal_explicit_document(&note.document)
        && note.document.knowledge.is_none()
        && blocks.is_empty()
    {
        current["document"]["knowledge"] = json!({});
    }
    if let Some(comments) = current
        .get_mut("document")
        .and_then(|doc| doc.get_mut("knowledge"))
        .and_then(|meta| meta.get_mut("comments"))
        .and_then(Value::as_array_mut)
    {
        let hidden = note
            .document
            .blocks
            .iter()
            .filter(|block| !blocks.iter().any(|visible| visible.id == block.id))
            .map(|block| block.id.as_str())
            .collect::<HashSet<_>>();
        comments.retain(|comment| {
            !comment.get("deletedAt").is_some_and(|v| !v.is_null())
                && !comment
                    .get("blockId")
                    .and_then(Value::as_str)
                    .is_some_and(|id| hidden.contains(id))
        });
    }
    Ok(current)
}

fn desktop_legal_current_scan_snapshot(raw: &str, data: &DesktopAppData) -> Result<String, String> {
    let mut snapshot: Value = serde_json::from_str(raw).map_err(|error| error.to_string())?;
    if let Some(notes) = snapshot.get_mut("notes").and_then(Value::as_array_mut) {
        if notes.len() != data.notes.len() {
            return Err("扫描快照的笔记数量不一致，请重新准备".into());
        }
        for (value, note) in notes.iter_mut().zip(&data.notes) {
            if value.get("id").and_then(Value::as_str).unwrap_or("") != note.id {
                return Err("扫描快照的笔记身份不一致，请重新准备".into());
            }
            *value = desktop_legal_current_scan_note(note)?;
        }
    }
    serde_json::to_string(&snapshot).map_err(|error| error.to_string())
}

fn desktop_legal_current_attachments(note: &DesktopNote) -> Vec<&DesktopNoteAttachment> {
    let document = &note.document;
    let explicit = desktop_legal_explicit_document(document);
    if !explicit {
        return note
            .attachments
            .iter()
            .filter(|item| {
                !item
                    .extra
                    .get("deletedAtEpochMillis")
                    .is_some_and(|v| !v.is_null())
            })
            .collect();
    }
    let mut ids = HashSet::new();
    for block in desktop_legal_visible_blocks(document) {
        if let Some(id) = &block.attachment_id {
            ids.insert(id.clone());
        }
        if document.rich_text_enabled && matches!(block.block_type.as_str(), "" | "TEXT" | "text") {
            ids.extend(gridtimer_native::desktop_rich_text_attachment_ids(
                &block.text,
            ));
        }
    }
    note.attachments
        .iter()
        .filter(|item| {
            ids.contains(&item.id)
                && !item
                    .extra
                    .get("deletedAtEpochMillis")
                    .is_some_and(|v| !v.is_null())
        })
        .collect()
}

fn desktop_legal_live_note<'a>(data: &'a DesktopAppData, id: &str) -> Option<&'a DesktopNote> {
    data.notes.iter().find(|note| {
        note.id == id
            && note.deleted_at_epoch_millis.is_none()
            && !legal_source_tombstoned(data, "note", id, note.updated_at_epoch_millis)
    })
}

fn desktop_legal_note_destination(
    data: &DesktopAppData,
    id: &str,
    suffix: &[&str],
) -> Option<DesktopLegalDestination> {
    let note = desktop_legal_live_note(data, id)?;
    let suffix = suffix.strip_prefix(&["current"]).unwrap_or(suffix);
    let (block, attachment) = match suffix {
        [] => (None, None),
        ["block", block] => (Some(*block), None),
        ["attachment", attachment] => (None, Some(*attachment)),
        ["block", block, "attachment", attachment] => (Some(*block), Some(*attachment)),
        _ => return None,
    };
    // An old report must not silently open an unrelated current block or file.
    if let Some(id) = block {
        let visible = desktop_legal_visible_blocks(&note.document)
            .iter()
            .any(|b| b.id == id);
        if !visible {
            return None;
        }
    }
    if attachment.is_some_and(|id| {
        !desktop_legal_current_attachments(note).iter().any(|item| {
            item.id == id
                && !legal_source_tombstoned(
                    data,
                    "noteAttachment",
                    id,
                    item.updated_at_epoch_millis,
                )
        })
    }) {
        return None;
    }
    if let (Some(block), Some(attachment)) = (block, attachment) {
        let item = note.document.blocks.iter().find(|item| item.id == block)?;
        let direct = item.attachment_id.as_deref() == Some(attachment);
        let embedded = note.document.rich_text_enabled
            && gridtimer_native::desktop_rich_text_attachment_ids(&item.text)
                .iter()
                .any(|id| id == attachment);
        if !direct && !embedded {
            return None;
        }
    }
    Some(DesktopLegalDestination::Note {
        id: id.into(),
        block: block.map(str::to_string),
        attachment: attachment.map(str::to_string),
    })
}

fn desktop_legal_destination(data: &DesktopAppData, path: &str) -> Option<DesktopLegalDestination> {
    let segments = path.split('/').collect::<Vec<_>>();
    if segments
        .iter()
        .any(|part| part.is_empty() || *part == "." || *part == "..")
    {
        return None;
    }
    match segments.as_slice() {
        ["note", id, suffix @ ..] => desktop_legal_note_destination(data, id, suffix),
        ["slot", id] => id
            .parse::<i32>()
            .ok()
            .filter(|id| data.slots.iter().any(|slot| slot.id == *id))
            .map(DesktopLegalDestination::Slot),
        ["session", id] | ["archivedTask", id]
            if legal_source_available_in_data(data, path)
                && if segments[0] == "archivedTask" {
                    data.archived_tasks.iter().any(|item| item.id == *id)
                } else {
                    data.sessions.iter().any(|item| item.id == *id)
                } =>
        {
            Some(DesktopLegalDestination::History {
                archives: segments[0] == "archivedTask",
                id: (*id).into(),
            })
        }
        ["finance", "overview"] => Some(DesktopLegalDestination::FinanceOverview),
        ["finance", kind @ ("day" | "month"), period, tail @ ..] => {
            let monthly = *kind == "month";
            let collection = if monthly {
                "monthlySnapshots"
            } else {
                "dailyLedgers"
            };
            if !finance_record_key_valid(collection, period)
                || !legal_source_available_in_data(data, path)
            {
                return None;
            }
            let entry = match tail {
                [] => None,
                [list, id] => {
                    let rows = data
                        .finance_profile
                        .extra
                        .get(collection)?
                        .get(*period)?
                        .get(*list)?
                        .as_array()?;
                    let matching = rows
                        .iter()
                        .enumerate()
                        .filter(|(index, row)| {
                            row.get("id")
                                .and_then(Value::as_str)
                                .filter(|id| !id.is_empty())
                                .map_or_else(|| *id == index.to_string(), |actual| actual == *id)
                        })
                        .collect::<Vec<_>>();
                    if matching.len() != 1 {
                        return None;
                    }
                    let (index, row) = matching[0];
                    if row
                        .get("deletedAtEpochMillis")
                        .is_some_and(|v| !v.is_null())
                    {
                        return None;
                    }
                    let actual_id = rows[index].get("id").and_then(Value::as_str).unwrap_or("");
                    Some((
                        (*list).into(),
                        actual_id.into(),
                        if actual_id.is_empty() { index } else { 0 },
                    ))
                }
                _ => return None,
            };
            Some(DesktopLegalDestination::Finance {
                monthly,
                period: (*period).into(),
                entry,
            })
        }
        ["noteFolder", id]
            if data.note_folders.iter().any(|folder| {
                folder.id == *id
                    && !legal_source_tombstoned(
                        data,
                        "noteFolder",
                        id,
                        folder.updated_at_epoch_millis,
                    )
                    && !folder
                        .extra
                        .get("deletedAtEpochMillis")
                        .is_some_and(|v| !v.is_null())
            }) =>
        {
            Some(DesktopLegalDestination::Folder((*id).into()))
        }
        ["category", id] if data.categories.iter().any(|category| category.id == *id) => {
            Some(DesktopLegalDestination::Category((*id).into()))
        }
        _ => None,
    }
}
