use serde_json::{json, Value};
use tenrate_ios_core::apply;
fn action(raw: &str, c: Value, now: i64) -> String {
    apply(raw, &c.to_string(), now).expect("accepted mutation")
}
#[test]
fn timer_relaunch_pause_archive_restore_preserves_history() {
    let base = action("", json!({"action":"default"}), 1000);
    let named = action(
        &base,
        json!({"action":"title","slotId":1,"text":"阅读 📚"}),
        2000,
    );
    let running = action(&named, json!({"action":"start","slotId":1}), 3000);
    // Projection after relaunch derives elapsed time from stored timestamps.
    let view: Value =
        serde_json::from_str(&action(&running, json!({"action":"project"}), 64000)).unwrap();
    assert_eq!(view["slots"][0]["accumulatedMillis"], 61000);
    let stopped = action(&running, json!({"action":"pause","slotId":1}), 64000);
    let archived = action(
        &stopped,
        json!({"action":"archive","slotId":1,"text":"archive-1"}),
        65000,
    );
    let restored = action(
        &archived,
        json!({"action":"restoreArchive","text":"archive-1"}),
        66000,
    );
    let data: Value = serde_json::from_str(&restored).unwrap();
    assert_eq!(data["slots"][0]["title"], "阅读 📚");
    assert_eq!(data["sessions"].as_array().unwrap().len(), 1);
    assert_eq!(data["slots"][0]["accumulatedMillis"], 61000);
}
#[test]
fn notes_unicode_trash_restore_and_protection_boundary() {
    let base = action("", json!({"action":"default"}), 1000);
    let note = json!({"id":"note-1","title":"想法","content":"第一行\n第二行 🪴","kind":"STICKY","createdAtEpochMillis":1000});
    let saved = action(&base, json!({"action":"saveNote","note":note}), 2000);
    let trashed = action(&saved, json!({"action":"deleteNote","text":"note-1"}), 3000);
    let restored = action(
        &trashed,
        json!({"action":"restoreNote","text":"note-1"}),
        4000,
    );
    let data: Value = serde_json::from_str(&restored).unwrap();
    assert_eq!(data["notes"][0]["content"], "第一行\n第二行 🪴");
    assert!(data["notes"][0]["deletedAtEpochMillis"].is_null());
    assert!(apply(
        &saved,
        &json!({"action":"saveNote","note":{"id":"note-1","encryption":{}}}).to_string(),
        5000
    )
    .is_none());
    assert!(apply(&saved, &json!({"action":"saveNote","note":{"id":"note-1","document":{"blocks":[{"text":"keep"}]}}}).to_string(), 5000).is_none());
}
#[test]
fn malformed_command_never_replaces_state() {
    assert!(apply("broken", "{}", 1).is_none());
    assert!(apply("{}", r#"{"action":"start","slotId":999}"#, 1).is_none());
}
