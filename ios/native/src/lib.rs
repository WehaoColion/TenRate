//! Small C ABI around the existing product core. No server or JNI dependencies.
#![allow(dead_code)]
#[path = "../../../native/gridtimer_native/src/app_data.rs"]
pub mod app_data;
#[path = "../../../native/gridtimer_native/src/finance_money.rs"]
mod finance_money;
#[path = "../../../native/gridtimer_native/src/finance_precision_guard.rs"]
mod finance_precision_guard;
#[path = "../../../native/gridtimer_native/src/finance_profile.rs"]
mod finance_profile;
#[path = "../../../native/gridtimer_native/src/knowledge.rs"]
mod knowledge;
#[path = "../../../native/gridtimer_native/src/note_documents.rs"]
mod note_documents;
#[path = "../../../native/gridtimer_native/src/rich_text_security.rs"]
mod rich_text_security;

use serde_json::Value;
use std::ffi::{c_char, CStr, CString};

pub fn apply(raw: &str, command: &str, now: i64) -> Option<String> {
    let c: Value = serde_json::from_str(command).ok()?;
    let id = c.get("slotId").and_then(Value::as_i64).unwrap_or(0);
    let text = c.get("text").and_then(Value::as_str).unwrap_or("");
    let slot = i32::try_from(id).ok()?;
    match c.get("action")?.as_str()? {
        "default" => Some(app_data::default_app_data_json(now)),
        "validate" => app_data::sanitize_app_data_json(raw, now),
        "project" => app_data::project_timer_views_json(raw, now).ok(),
        "start" if (1..=14).contains(&slot) => app_data::start_slot_app_data_json(raw, slot, now),
        "pause" if (1..=14).contains(&slot) => {
            app_data::pause_slots_app_data_json(raw, &[slot], now)
        }
        "title" if (1..=14).contains(&slot) => {
            app_data::update_slot_title_app_data_json(raw, slot, text, now)
        }
        "category" if (1..=14).contains(&slot) => {
            app_data::set_slot_category_app_data_json(raw, slot, text, now)
        }
        "reset" if (1..=14).contains(&slot) => app_data::reset_slot_app_data_json(raw, slot, now),
        "archive" if (1..=14).contains(&slot) => {
            app_data::archive_slot_app_data_json(raw, slot, text, now)
        }
        "restoreArchive" => app_data::restore_archived_task_app_data_json(raw, text, now),
        "saveNote" => {
            // Only plain text notes are editable in this first iOS release.
            // Reject protected/structured documents rather than flattening them.
            let mut note = c.get("note")?.clone();
            let data: Value = serde_json::from_str(raw).ok()?;
            let note_id = note.get("id")?.as_str()?;
            let existing = data
                .get("notes")?
                .as_array()?
                .iter()
                .find(|n| n.get("id").and_then(Value::as_str) == Some(note_id));
            for item in existing.into_iter().chain(std::iter::once(&note)) {
                if item.get("encryption").is_some_and(|v| !v.is_null())
                    || item
                        .get("kind")
                        .and_then(Value::as_str)
                        .is_some_and(|v| v != "STICKY")
                    || item
                        .pointer("/document/blocks")
                        .and_then(Value::as_array)
                        .is_some_and(|v| !plain_blocks(v))
                    || item
                        .get("attachments")
                        .and_then(Value::as_array)
                        .is_some_and(|v| !v.is_empty())
                    || item
                        .pointer("/document/richTextEnabled")
                        .and_then(Value::as_bool)
                        == Some(true)
                    || item
                        .pointer("/document/knowledge")
                        .is_some_and(|v| !v.is_null())
                {
                    return None;
                }
            }
            // The shared core turns legacy plain text into one canonical text
            // block. Update that block together with content on later saves.
            let content = note
                .get("content")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string();
            if let Some(block) = note.pointer_mut("/document/blocks/0") {
                block["text"] = Value::String(content);
            }
            app_data::upsert_note_app_data_json(raw, &note.to_string(), now)
        }
        "deleteNote" => app_data::delete_note_app_data_json(raw, text, now),
        "restoreNote" => app_data::restore_note_app_data_json(raw, text, now),
        _ => None,
    }
}

fn plain_blocks(blocks: &[Value]) -> bool {
    blocks.is_empty()
        || (blocks.len() == 1
            && blocks[0].get("id").and_then(Value::as_str) == Some("legacy-text")
            && blocks[0].get("type").and_then(Value::as_str) == Some("TEXT")
            && blocks[0].get("knowledge").is_none_or(Value::is_null)
            && blocks[0].get("attachmentId").is_none_or(Value::is_null))
}

/// All strings are UTF-8; returned ownership is released with tenrate_string_free.
/// Invalid requests and caught Rust panics return null. Inputs must remain valid
/// NUL-terminated strings for the duration of this call.
#[no_mangle]
pub unsafe extern "C" fn tenrate_apply(
    raw: *const c_char,
    command: *const c_char,
    now: i64,
) -> *mut c_char {
    if raw.is_null() || command.is_null() {
        return std::ptr::null_mut();
    }
    std::panic::catch_unwind(|| {
        let raw = CStr::from_ptr(raw).to_str().ok()?;
        let command = CStr::from_ptr(command).to_str().ok()?;
        CString::new(apply(raw, command, now)?)
            .ok()
            .map(CString::into_raw)
    })
    .ok()
    .flatten()
    .unwrap_or(std::ptr::null_mut())
}

#[no_mangle]
pub unsafe extern "C" fn tenrate_string_free(value: *mut c_char) {
    if !value.is_null() {
        drop(CString::from_raw(value));
    }
}
