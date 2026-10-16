// v1.1.0.8 Windows - Project only the readable current note into AI consent.
fn desktop_ai_current_note_text(content: &str, document: &DesktopNoteDocument) -> String {
    let explicit_document = document.knowledge.is_some()
        || !document.blocks.is_empty()
        || document.rich_text_enabled
        || document.markdown_enabled
        || !document.rich_text_plain_text.is_empty();
    if !explicit_document {
        return content.to_owned();
    }

    let mut parts = Vec::new();
    let mut push = |label: &str, value: &str| {
        if !value.trim().is_empty() {
            parts.push(if label.is_empty() {
                value.to_owned()
            } else {
                format!("{label}：{value}")
            });
        }
    };
    // Rich text has one authoritative plaintext projection. Raw HTML blocks and
    // legacy compatibility text are not additional current versions.
    if document.rich_text_enabled {
        push("", &document.rich_text_plain_text);
    }
    let mut deleted = document
        .blocks
        .iter()
        .filter(|block| {
            block
                .extra
                .get("deletedAtEpochMillis")
                .is_some_and(|value| !value.is_null())
        })
        .map(|block| block.id.as_str())
        .collect::<std::collections::BTreeSet<_>>();
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
    let mut pending = deleted.iter().copied().collect::<Vec<_>>();
    while let Some(parent) = pending.pop() {
        if let Some(ids) = children.get(parent) {
            for id in ids {
                if deleted.insert(id) {
                    pending.push(id);
                }
            }
        }
    }
    for block in document
        .blocks
        .iter()
        .filter(|block| !deleted.contains(block.id.as_str()))
    {
        if !document.rich_text_enabled || !block_is_plain_text(block) {
            push(
                "",
                &knowledge::block_plaintext(block.knowledge.as_ref(), &block.text),
            );
        }
        for (label, value) in [
            ("说明", block.caption.as_str()),
            ("联系人", block.contact_name.as_str()),
            ("单位", block.contact_organization.as_str()),
            ("通话联系人", block.call_contact_name.as_str()),
            ("通话号码", block.call_phone_number.as_str()),
        ] {
            push(label, value);
        }
        if let Some(meta) = block.knowledge.as_ref() {
            push("链接", &meta.url);
        }
        for phone in &block.contact_phones {
            let number = phone
                .get("number")
                .and_then(Value::as_str)
                .or_else(|| phone.as_str())
                .unwrap_or("");
            let label = phone.get("label").and_then(Value::as_str).unwrap_or("");
            push(
                "电话",
                &if label.is_empty() {
                    number.to_owned()
                } else {
                    format!("{label} {number}")
                },
            );
        }
        if !block.call_direction.trim().is_empty() {
            let direction = match block.call_direction.to_ascii_uppercase().as_str() {
                "INCOMING" => "呼入",
                "OUTGOING" => "呼出",
                "MISSED" => "未接",
                "REJECTED" => "拒接",
                "BLOCKED" => "已拦截",
                _ => block.call_direction.as_str(),
            };
            push("通话方向", direction);
        }
        if let Some(at) = block.call_occurred_at_epoch_millis {
            push("通话时间戳", &at.to_string());
        }
        if let Some(duration) = block.call_duration_millis {
            push("通话时长毫秒", &duration.to_string());
        }
    }
    if let Some(meta) = document.knowledge.as_ref() {
        for tag in &meta.tags {
            push("标签", tag);
        }
        for (id, value) in &meta.properties {
            if id.starts_with("__tenrate_") {
                continue;
            }
            let field = meta
                .database
                .as_ref()
                .and_then(|db| db.fields.iter().find(|field| &field.id == id));
            if field.is_some_and(|field| field.deleted) {
                continue;
            }
            let name = field.map(|field| field.name.as_str()).unwrap_or(id);
            push(
                &format!("属性 {name}"),
                &desktop_ai_current_cell_text(value),
            );
        }
        for comment in meta
            .comments
            .iter()
            .filter(|comment| comment.deleted_at.is_none())
        {
            // Comments anchored to a removed block are no longer readable there.
            if comment
                .block_id
                .as_deref()
                .is_some_and(|id| deleted.contains(id))
            {
                continue;
            }
            push(
                &if comment.author.is_empty() {
                    "评论".to_owned()
                } else {
                    format!("评论（{}）", comment.author)
                },
                &comment.body,
            );
        }
    }
    parts.join("\n")
}

fn desktop_ai_current_cell_text(value: &knowledge::CellValue) -> String {
    use knowledge::CellValue;
    match value {
        CellValue::Empty => String::new(),
        CellValue::Text(value) | CellValue::Select(value) | CellValue::Url(value) => value.clone(),
        CellValue::Number(value) => value.to_string(),
        CellValue::MultiSelect(values) | CellValue::Relation(values) => values.join("、"),
        CellValue::Date(value) => {
            if value.end.is_empty() || value.start == value.end {
                value.start.clone()
            } else {
                format!("{} 至 {}", value.start, value.end)
            }
        }
        CellValue::Checkbox(checked) => if *checked { "已勾选" } else { "未勾选" }.into(),
    }
}

#[cfg(test)]
mod desktop_ai_current_content_tests {
    use super::*;

    fn document(value: Value) -> DesktopNoteDocument {
        serde_json::from_value(value).unwrap()
    }

    #[test]
    fn current_block_projection_excludes_old_mirrors_and_removed_subtrees_but_preserves_semantics()
    {
        let doc = document(json!({
            "richTextPlainText":"old-rich-mirror",
            "blocks":[
                {"id":"todo","text":"already done","knowledge":{"kind":"todo","checked":true}},
                {"id":"table","text":"old-table-mirror","knowledge":{"kind":"table","table":[["item","value"],["renewal","58.80"]]}},
                {"id":"deleted","text":"removed-secret","deletedAtEpochMillis":1},
                {"id":"child","text":"removed-child","knowledge":{"parentId":"deleted"}},
                {"id":"live","text":"current body"}
            ]
        }));
        let text = desktop_ai_current_note_text("old-compat-secret", &doc);
        assert!(text.contains("- [x] already done"));
        assert!(text.contains("renewal | 58.80"));
        assert!(text.contains("current body"));
        for removed in [
            "old-rich-mirror",
            "old-table-mirror",
            "old-compat-secret",
            "removed-secret",
            "removed-child",
        ] {
            assert!(!text.contains(removed), "{removed}");
        }
    }

    #[test]
    fn rich_text_and_explicit_empty_documents_never_fall_back_to_legacy_content() {
        let rich = document(json!({
            "richTextEnabled":true,
            "richTextPlainText":"current rich body",
            "blocks":[{"id":"html","text":"<p>old-html-mirror</p>"}]
        }));
        assert_eq!(
            desktop_ai_current_note_text("old-compat-secret", &rich),
            "current rich body"
        );
        let empty = document(json!({"knowledge":{}}));
        assert!(desktop_ai_current_note_text("removed-compat-secret", &empty).is_empty());
        let empty_rich = document(json!({"richTextEnabled":true}));
        assert!(desktop_ai_current_note_text("removed-compat-secret", &empty_rich).is_empty());
        assert_eq!(
            desktop_ai_current_note_text("legacy current note", &DesktopNoteDocument::default()),
            "legacy current note"
        );
    }

    #[test]
    fn current_visible_properties_comments_and_contact_fields_are_included_without_arbitrary_payloads(
    ) {
        let doc = document(json!({
            "knowledge":{
                "tags":["deadline"],
                "properties":{"paid":{"kind":"checkbox","value":false}},
                "comments":[
                    {"id":"c","body":"current comment"},
                    {"id":"d","body":"removed-comment","deletedAt":1}
                ]
            },
            "blocks":[{
                "id":"contact","type":"CONTACT","contactName":"Alice",
                "contactPhones":[{"label":"work","number":"123456","privatePayload":"not-readable-secret"}]
            }]
        }));
        let text = desktop_ai_current_note_text("old-body", &doc);
        for visible in [
            "deadline",
            "paid",
            "未勾选",
            "current comment",
            "Alice",
            "123456",
        ] {
            assert!(text.contains(visible), "{visible}");
        }
        for hidden in ["old-body", "removed-comment", "not-readable-secret"] {
            assert!(!text.contains(hidden), "{hidden}");
        }
    }
}
