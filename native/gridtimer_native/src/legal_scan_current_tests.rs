//! Specifications for scanning only each retained note's current content.
use super::*;

fn prepare_current(data: &Value, unlocked: &Value, attachments: &Value) -> LegalScanPrepared {
    prepare_scan(
        &data.to_string(),
        "current-content-workspace",
        500,
        &unlocked.to_string(),
        &attachments.to_string(),
    )
    .unwrap()
}

#[test]
fn latest_scope_uses_current_page_and_sticky_content_once_even_when_history_is_newer() {
    let current = json!({"notes":[
        {"id":"page","kind":"document","title":"current-page-title","content":"current-page-body",
         "updatedAtEpochMillis":100,"latestVersionId":"misleading-history",
         "document":{"blocks":[{"id":"b","text":"current-block-body"}],
            "knowledge":{"tags":["current-tag"],"properties":{"current-property":"current-cell"}}}},
        {"id":"sticky","kind":"sticky","title":"current-sticky-title","content":"current-sticky-body"}
    ]});
    let mut retained = current.clone();
    for note in retained["notes"].as_array_mut().unwrap() {
        note["revisions"] = json!([
            {"id":"revision-old","title":"private-old-revision-title","content":"private-old-revision-body",
             "capturedAtEpochMillis":999999,"document":{"blocks":[{"text":"private-old-block"}],
                "knowledge":{"tags":["private-old-tag"],"properties":{"private-old-property":"private-old-cell"}}}}
        ]);
        note["versions"] = json!([
            {"id":"misleading-history","isLatest":true,"sequence":999,"updatedAtEpochMillis":999999,
             "title":"private-old-version-title","content":"private-old-version-body"},
            {"id":"duplicate-current","content":note["content"].clone()}
        ]);
    }
    let expected = prepare_current(&current, &json!([]), &json!([]));
    let actual = prepare_current(&retained, &json!([]), &json!([]));
    assert_eq!(
        serde_json::to_value(&actual).unwrap(),
        serde_json::to_value(&expected).unwrap()
    );
    assert_eq!(actual.evidence.len(), 2);
    assert!(actual
        .evidence
        .iter()
        .any(|item| item.source_path == "note/page"
            && item.text.contains("current-page-body")
            && item.text.contains("current-block-body")
            && item.text.contains("current-tag")
            && item.text.contains("current-cell")));
    assert!(
        actual
            .evidence
            .iter()
            .any(|item| item.source_path == "note/sticky"
                && item.text.contains("current-sticky-body"))
    );
    assert!(!serde_json::to_string(&actual)
        .unwrap()
        .contains("private-old"));
}

#[test]
fn latest_scope_blank_current_content_never_falls_back_to_historical_content() {
    let data = json!({"notes":[{"id":"blank","title":"","content":"","document":{},
        "latestVersionId":"old","versions":[{"id":"old","isLatest":true,"sequence":99,
            "title":"private-old-title","content":"private-old-body"}],
        "revisions":[{"id":"r","content":"private-old-revision"}]}]});
    let prepared = prepare_current(&data, &json!([]), &json!([]));
    assert!(prepared.evidence.is_empty());
    assert!(prepared.batches.is_empty());
    assert_eq!(prepared.manifest.evidence_count, 0);
    assert_eq!(prepared.manifest.upload_bytes, 0);
    assert_eq!(prepared.manifest.estimated_calls, 0);
    assert!(prepared.manifest.omissions.is_empty());
}

#[test]
fn latest_scope_unlock_requires_current_payload_but_no_historical_payload() {
    let stored = json!({"notes":[{"id":"locked","title":"cipher-title","content":"cipher-body",
        "encryption":{"ciphertextBase64":"private-ciphertext"},
        "versions":[{"content":"private-stored-history"}]}]});
    let current = json!([{"noteId":"locked","id":"locked","title":"current-clear-title",
        "content":"current-clear-body","document":{},"attachments":[]}]);
    let prepared = prepare_current(&stored, &current, &json!([]));
    assert_eq!(prepared.evidence.len(), 1);
    assert_eq!(prepared.manifest.coverage["笔记容器"].unavailable, 0);
    assert!(prepared.evidence[0].text.contains("current-clear-body"));
    let mut full = current.clone();
    full[0]["revisions"] = json!([{"content":"private-unlocked-revision"}]);
    full[0]["versions"] = json!([{"content":"private-unlocked-version"}]);
    let with_history = prepare_current(&stored, &full, &json!([]));
    assert_eq!(
        serde_json::to_value(&prepared).unwrap(),
        serde_json::to_value(with_history).unwrap()
    );
    for field in ["title", "content", "document", "attachments"] {
        let mut incomplete = current.clone();
        incomplete[0].as_object_mut().unwrap().remove(field);
        let rejected = prepare_current(&stored, &incomplete, &json!([]));
        assert!(rejected.evidence.is_empty());
        assert_eq!(rejected.manifest.coverage["笔记容器"].unavailable, 1);
    }
    let mut mismatched = current.clone();
    mismatched[0]["id"] = json!("another-note");
    for unavailable in [json!([]), mismatched] {
        let rejected = prepare_current(&stored, &unavailable, &json!([]));
        assert!(rejected.evidence.is_empty());
        assert_eq!(rejected.manifest.coverage["笔记容器"].unavailable, 1);
        assert!(!serde_json::to_string(&rejected)
            .unwrap()
            .contains("private-"));
    }
}

#[test]
fn latest_scope_excludes_deleted_notes_but_preserves_retained_archived_records() {
    let data = json!({"notes":[
        {"id":"alive","content":"current-retained-body","revisions":[{"content":"private-old-body"}]},
        {"id":"deleted","content":"private-deleted-body","deletedAtEpochMillis":1,
            "attachments":[{"id":"removed","sha256":"missing"}]},
        {"id":"tombstoned","content":"private-tombstoned-body","updatedAtEpochMillis":10,
            "versions":[{"content":"private-tombstoned-history"}]}
    ],"tombstones":[{"entityType":"note","entityId":"tombstoned","deletedAtEpochMillis":20}],
    "archivedTasks":[{"id":"archived","title":"retained-archived-task","archivedAtEpochMillis":30}]
    });
    let prepared = prepare_current(&data, &json!([]), &json!([]));
    assert!(prepared
        .evidence
        .iter()
        .any(|item| item.source_path == "note/alive"));
    assert!(prepared
        .evidence
        .iter()
        .any(|item| item.source_path == "archivedTask/archived"
            && item.text.contains("retained-archived-task")));
    assert_eq!(prepared.manifest.coverage["笔记容器"].excluded_deleted, 2);
    assert!(prepared.manifest.omissions.is_empty());
    assert!(!serde_json::to_string(&prepared)
        .unwrap()
        .contains("private-"));
}

struct CurrentAttachmentFiles(std::path::PathBuf);

impl CurrentAttachmentFiles {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "tenrate_legal_current_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir(&path).unwrap();
        Self(path)
    }

    fn descriptor(&self, id: &str, text: &str) -> (Value, Value) {
        let name = format!("{id}.txt");
        let path = self.0.join(&name);
        std::fs::write(&path, text).unwrap();
        let digest = format!("{:x}", Sha256::digest(text.as_bytes()));
        (
            json!({"id":id,"sha256":digest,"fileName":name,"mimeType":"text/plain"}),
            json!({"noteId":"n","attachmentId":id,"sha256":digest,"path":path,
            "mimeType":"text/plain","sizeBytes":text.len()}),
        )
    }
}

impl Drop for CurrentAttachmentFiles {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn latest_scope_current_attachments_are_deduplicated_and_history_files_do_not_reduce_coverage() {
    let files = CurrentAttachmentFiles::new();
    let (shared, shared_input) = files.descriptor("shared", "current-shared-file");
    let (standalone, standalone_input) = files.descriptor("standalone", "current-standalone-file");
    let (old, old_input) = files.descriptor("old", "private-old-file-content");
    let mut data = json!({"notes":[{"id":"n","title":"current-note",
        "attachments":[shared,standalone.clone(),old],"attachmentIds":["shared"],
        "document":{"blocks":[{"id":"a","attachmentId":"shared"},{"id":"b","attachmentId":"shared"}]},
        "revisions":[{"id":"r","attachmentIds":["old","shared","missing-old-revision"]}],
        "versions":[{"id":"old-version","content":"private-old-body",
            "attachmentIds":["old","missing-old-version"],
            "document":{"blocks":[{"id":"missing-old-block","attachmentId":"missing-old-block-file"}]}}]
    }]});
    let inputs = json!([shared_input, standalone_input, old_input]);
    let prepared = prepare_current(&data, &json!([]), &inputs);
    let attachments = prepared
        .evidence
        .iter()
        .filter(|item| item.category == "附件")
        .collect::<Vec<_>>();
    assert_eq!(attachments.len(), 1);
    assert!(attachments
        .iter()
        .any(|item| item.text.contains("current-shared-file")));
    assert!(!attachments
        .iter()
        .any(|item| item.text.contains("current-standalone-file")));
    assert!(attachments
        .iter()
        .all(|item| item.source_path.starts_with("note/n/current/")));
    assert_eq!(prepared.manifest.coverage["附件"].candidates, 1);
    assert_eq!(prepared.manifest.coverage["附件"].included, 1);
    assert_eq!(prepared.manifest.coverage["附件"].unavailable, 0);
    assert!(prepared.manifest.omissions.is_empty());
    assert!(!serde_json::to_string(&prepared)
        .unwrap()
        .contains("private-old"));

    data["notes"][0]["document"]["blocks"]
        .as_array_mut()
        .unwrap()
        .push(json!({"id":"missing-current-block","attachmentId":"missing-current-file"}));
    let missing_current = prepare_current(&data, &json!([]), &inputs);
    assert_eq!(missing_current.manifest.coverage["附件"].unavailable, 1);
    assert_eq!(missing_current.manifest.omissions.len(), 1);
    assert!(missing_current.manifest.omissions[0]
        .source_path
        .contains("missing-current-file"));

    // Legacy notes show their attachment catalog without structured blocks.
    let mut legacy =
        json!({"notes":[{"id":"n","content":"legacy-current-body","attachments":[standalone]}]});
    let legacy_prepared = prepare_current(&legacy, &json!([]), &inputs);
    assert_eq!(legacy_prepared.manifest.coverage["附件"].included, 1);
    assert!(legacy_prepared
        .evidence
        .iter()
        .any(|item| item.text.contains("current-standalone-file")));
    assert!(legacy_prepared.manifest.omissions.is_empty());
    legacy["notes"][0]["document"] = json!({"richTextEnabled":true,"blocks":[],"knowledge":null});
    let legacy_rich = prepare_current(&legacy, &json!([]), &inputs);
    assert_eq!(legacy_rich.manifest.coverage["附件"].included, 1);
    assert!(legacy_rich.manifest.omissions.is_empty());

    let rich_text = json!({"notes":[{"id":"n","content":"current-rich-body",
    "attachments":data["notes"][0]["attachments"].clone(),
    "document":{"richTextEnabled":true,"richTextPlainText":"current-rich-body","blocks":[
        {"id":"rich-current","type":"TEXT",
            "text":"<figure data-note-image=\"shared\"><img src=\"note-image://shared\"></figure>"},
        {"id":"rich-deleted","type":"TEXT","deletedAtEpochMillis":1,
            "text":"<img src=\"note-image://old\">"}
    ]}}]});
    let rich_prepared = prepare_current(&rich_text, &json!([]), &inputs);
    assert_eq!(rich_prepared.manifest.coverage["附件"].candidates, 1);
    assert_eq!(rich_prepared.manifest.coverage["附件"].included, 1);
    assert!(rich_prepared
        .evidence
        .iter()
        .any(|item| item.category == "附件" && item.text.contains("current-shared-file")));
    assert!(!serde_json::to_string(&rich_prepared)
        .unwrap()
        .contains("private-old"));
    assert!(rich_prepared.manifest.omissions.is_empty());
}

#[test]
fn latest_scope_old_evidence_ids_and_old_quotes_cannot_enter_new_reports() {
    let data = json!({"notes":[{"id":"n","content":"current-supported-fact",
        "versions":[{"id":"old","content":"private-old-fact"}]}]});
    let prepared = prepare_current(&data, &json!([]), &json!([]));
    assert_eq!(prepared.evidence.len(), 1);
    let current_id = &prepared.evidence[0].id;
    let finding = |id: &str, quote: &str| {
        json!({"title":"待核查","area":"合同债务","fact":"待核查事实",
        "evidence":[{"evidenceId":id,"quote":quote}],"eventAtEpochMillis":null,
        "missingFacts":"仍需原始凭证","recommendation":"核对事实","lawIds":[]})
    };
    let raw = json!({"findings":[finding("E000002","private-old-fact"),
        finding(current_id,"private-old-fact"),finding(current_id,"current-supported-fact")]});
    let findings = validate_batch_response(
        &raw.to_string(),
        &prepared.batches[0],
        &prepared.evidence,
        &[],
    )
    .unwrap();
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].evidence.len(), 1);
    assert_eq!(findings[0].evidence[0].source_path, "note/n");
    assert_eq!(findings[0].evidence[0].quote, "current-supported-fact");
}
