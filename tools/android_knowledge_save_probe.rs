// v3.0.2.15 - Scale genuine SQLite history to 100,000 rows without reducing per-row payload or latency gates.
// v3.0.2.15 - Repeated nested-page body edits, real writer contention and immediate durable exit under historical load.
// v3.0.2.15 - Exercise folder-to-child-page persistence under the loaded history and assert actual latency.
// v3.0.2.14 - Exercise a missing-digest history row in a live workspace.
// v3.0.2.9 - Exercise root and nested folder creation after an ordinary page commit.
// v3.0.2.6 - Time real confirmation on a cold preflight after upgrading a large v5 database.
// v3.0.2.5 - Distinguish conservative legacy fallback from healthy current-only fast path.
// v3.0.2.4 - Verify on-disk durability, bounded save latency and optional logcat proof with 7200 history rows.
//! Android API 35 signed-APK acceptance: new knowledge page must persist after restart.
//! The driver runs only against an installed signed APK, never a mocked UI.
use std::{
    env, fs,
    path::{Path, PathBuf},
    process::{Command, Output},
    thread::{self, sleep, JoinHandle},
    time::Duration,
};

const PACKAGE: &str = "com.ofairyo.gridtimer";
const ACTIVITY: &str = "com.ofairyo.gridtimer/.MainActivity";
mod android_save_probe_fixture;
use android_save_probe_fixture::{installed_version, SqliteWriter, DATABASE};
// Numeric page titles commit on both Latin and Pinyin keyboards without
// depending on the input method's candidate selection or word boundaries.
const CREATED: &str = "7391804025006002";
const SEED: &str = "7391804025006001";
const CONTENDED: &str = "7391804025006003";
const ROOT_FOLDER: &str = "7391804025006004";
const NESTED_FOLDER: &str = "7391804025006005";
const NESTED_PAGE: &str = "7391804025006006";
// Digits are committed by both Pinyin and Latin IMEs without a composition candidate.
const EDIT_ROUNDS: usize = 12;
const BURST_ROUNDS: usize = 30;
const EDIT_EXIT_MARKER: &str = "98765432109876543210";
const EDIT_BUDGET_MS: u128 = 8_000;
const HISTORY_ROWS: usize = 100_000;
const HISTORY_PADDING_BYTES: usize = 16 * 1024;
const PRIMARY_MIRROR: &str =
    "/data/user/0/com.ofairyo.gridtimer/files/state_workspaces/guest/timer_state.json";
const DIAGNOSTIC_LOG: &str = "/data/user/0/com.ofairyo.gridtimer/files/diagnostic_events.log";

#[derive(Clone)]
struct Node {
    label: String,
    description: String,
    class_name: String,
    bounds: [i32; 4],
}

fn adb(args: &[&str]) -> Result<Output, String> {
    Command::new("adb").args(args).output().map_err(|e| e.to_string())
}
fn cmd(args: &[&str]) -> Result<String, String> {
    let out = adb(args)?;
    if !out.status.success() {
        return Err(format!("adb {args:?}: {}", String::from_utf8_lossy(&out.stderr)));
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}
fn attr(xml: &str, name: &str) -> String {
    let marker = format!("{name}=\"");
    xml.find(&marker).and_then(|index| {
        let tail = &xml[index + marker.len()..];
        tail.find('"').map(|stop| tail[..stop].to_owned())
    }).unwrap_or_default()
}
fn elements(xml: &str) -> Vec<Node> {
    xml.split("<node ").skip(1).filter_map(|entry| {
        let head = entry.split('>').next()?;
        let raw = attr(head, "bounds");
        let bounds: Vec<i32> = raw.split(|c: char| !c.is_ascii_digit() && c != '-')
            .filter(|part| !part.is_empty()).filter_map(|part| part.parse().ok()).collect();
        if bounds.len() != 4 { return None; }
        Some(Node {
            label: attr(head, "text"),
            description: attr(head, "content-desc"),
            class_name: attr(head, "class"),
            bounds: [bounds[0], bounds[1], bounds[2], bounds[3]],
        })
    }).collect()
}
fn dump() -> Result<String, String> {
    for _ in 0..6 {
        let _ = cmd(&["shell", "rm", "-f", "/sdcard/knowledge_appearance.xml"]);
        let dumped = cmd(&["shell", "uiautomator", "dump", "/sdcard/knowledge_appearance.xml"]);
        if dumped.is_err() { sleep(Duration::from_millis(550)); continue; }
        if let Ok(xml) = cmd(&["exec-out", "cat", "/sdcard/knowledge_appearance.xml"]) {
            if xml.contains("<hierarchy") { return Ok(xml); }
        }
        sleep(Duration::from_millis(550));
    }
    Err("Android accessibility hierarchy unavailable".into())
}
fn capture(dir: &Path, title: &str) {
    if let Ok(xml) = dump() { let _ = fs::write(dir.join(format!("{title}.xml")), xml); }
    if let Ok(out) = adb(&["exec-out", "screencap", "-p"]) {
        if out.status.success() && out.stdout.starts_with(b"\x89PNG") {
            let _ = fs::write(dir.join(format!("{title}.png")), out.stdout);
        }
    }
}
fn find(labels: &[&str]) -> Result<Option<Node>, String> {
    let view = elements(&dump()?);
    for exact in [true, false] {
        for label in labels {
            if let Some(node) = view.iter().find(|node| {
                let visible = node.bounds[2] > node.bounds[0] &&
                    node.bounds[3] > node.bounds[1];
                visible && if exact {
                    node.label == *label || node.description == *label
                } else {
                    node.label.contains(label) || node.description.contains(label)
                }
            }) { return Ok(Some(node.clone())); }
        }
    }
    Ok(None)
}
fn send_tap(node: &Node) -> Result<(), String> {
    let x = (node.bounds[0] + node.bounds[2]) / 2;
    let y = (node.bounds[1] + node.bounds[3]) / 2;
    cmd(&["shell", "input", "tap", &x.to_string(), &y.to_string()])?;
    Ok(())
}
fn tap(node: &Node) -> Result<(), String> {
    send_tap(node)?;
    sleep(Duration::from_millis(950));
    Ok(())
}
fn tap_any(labels: &[&str]) -> Result<bool, String> {
    if let Some(node) = find(labels)? { tap(&node)?; Ok(true) } else { Ok(false) }
}
fn swipe() -> Result<(), String> {
    cmd(&["shell", "input", "swipe", "535", "1820", "535", "720", "500"])?;
    sleep(Duration::from_millis(800));
    Ok(())
}
fn clear_system_dialog() -> Result<(), String> {
    for _ in 0..3 {
        let ui = dump()?;
        if ui.contains("Allow tenfold to send you notifications") {
            if tap_any(&["Allow", "允许"])? { continue; }
        }
        if ui.contains("Pixel Launcher") && tap_any(&["Close app"])? { continue; }
        break;
    }
    Ok(())
}
fn directory(root: &Path) -> Result<(), String> {
    for step in 0..13 {
        clear_system_dialog()?;
        let view = dump()?;
        if (view.contains("新建子页面") && view.contains("新建子文件夹")) ||
            (view.contains("New subpage") && view.contains("New subfolder")) {
            capture(root, "02_directory");
            return Ok(());
        }
        if step == 0 { capture(root, "01_started"); }
        if tap_any(&["文档库", "管理文档库", "页面与文件夹", "Knowledge directory", "Directory"])? { continue; }
        if tap_any(&["知识", "Knowledge", "Notes"])? { continue; }
        if tap_any(&["目录"])? { continue; }
        swipe()?;
    }
    capture(root, "02_directory_not_found");
    Err("Could not reach the Android knowledge hierarchy and its create actions".into())
}
fn create_page(
    root: &Path,
    page_name: &str,
    before_confirm: impl FnOnce() -> Result<(), String>,
) -> Result<u128, String> {
    create_node(root, page_name, false, before_confirm)
}

fn create_folder(root: &Path, title: &str) -> Result<u128, String> {
    create_node(root, title, true, || Ok(()))
}

fn create_node(
    root: &Path, page_name: &str, folder: bool,
    before_confirm: impl FnOnce() -> Result<(), String>,
) -> Result<u128, String> {
    let labels: &[&str] = if folder {
        &["新建子文件夹", "新建文件夹", "New subfolder"]
    } else { &["新建子页面", "新建页面", "New subpage"] };
    if !tap_any(labels)? {
        capture(root, "03_no_create_action");
        return Err("Knowledge directory has no accessible create-page control".into());
    }
    // Root screens may expose a direct button or a compact "新建" menu.
    // Require the actual dialog, not merely the menu's accessibility label.
    let before_dialog = dump()?;
    let dialog_label = if folder { "新建文件夹" } else { "新建页面" };
    let english_label = if folder { "New folder" } else { "New page" };
    if !before_dialog.contains(dialog_label) && !before_dialog.contains(english_label) {
        let menu: &[&str] = if folder { &["子文件夹", "Subfolder"] } else { &["子页面", "Subpage"] };
        if !tap_any(menu)? {
            return Err("Tapped create entry, but no requested action was offered".into());
        }
    }
    capture(root, "03_page_dialog");
    let edit = elements(&dump()?).into_iter()
        .find(|node| node.class_name.ends_with("EditText")
            && node.bounds[2] - node.bounds[0] > 60)
        .ok_or("New-page title field not accessible")?;
    tap(&edit)?;
    cmd(&["shell", "input", "text", page_name])?;
    let title_in_field = |view: &str| elements(view).iter()
        .any(|node| node.class_name.ends_with("EditText") && node.label == page_name);
    if !title_in_field(&dump()?) {
        cmd(&["shell", "input", "keyevent", "KEYCODE_ENTER"])?;
    }
    capture(root, "04_title_entered");
    if !title_in_field(&dump()?) {
        return Err("The IME did not commit the complete page name in the editable field".into());
    }
    let confirmation = find(&["确定", "Confirm", "OK"])?
        .ok_or("New-page confirmation is missing")?;
    before_confirm()?;
    let started = std::time::Instant::now();
    send_tap(&confirmation)?;
    let mut saved = false;
    for tick in 0..28 {
        let current = dump()?;
        if tick == 2 || tick == 10 { capture(root, &format!("05_saving_{tick}")); }
        let confirmation_open = title_in_field(&current) && elements(&current).iter()
            .any(|node| ["确定", "Confirm", "OK"].contains(&node.label.as_str()));
        if !confirmation_open && current.contains(page_name) && !current.contains("正在保存") &&
            !current.contains("等待前序保存") && !current.contains("正在写入本地数据") {
            saved = true;
            break;
        }
        sleep(Duration::from_secs(1));
    }
    let elapsed = started.elapsed().as_millis();
    capture(root, "06_after_create");
    if !saved {
        return Err(format!(
            "Create-page confirmation failed to finish in the emulator ({} ms)",
            elapsed
        ));
    }
    if cmd(&["shell", "pidof", PACKAGE])?.trim().is_empty() {
        return Err("App process exited after knowledge page creation".into());
    }
    Ok(elapsed)
}

fn pull_database(local: &Path) -> Result<(), String> {
    let local_str = local.to_str().ok_or("Database fixture path is not UTF-8")?;
    cmd(&["pull", DATABASE, local_str])?;
    for extension in ["-wal", "-shm"] {
        let _ = adb(&["pull", &format!("{DATABASE}{extension}"), &format!("{local_str}{extension}")]);
    }
    Ok(())
}

// Large fixtures are checked independently on the host, then removed before
// the next pull so repeated evidence collection cannot exhaust runner storage.
fn remove_local_database(local: &Path) -> Result<(), String> {
    fs::remove_file(local).map_err(|e| e.to_string())?;
    for extension in ["-wal", "-shm", "-journal"] {
        let sidecar = format!("{}{extension}", local.display());
        match fs::remove_file(sidecar) {
            Ok(()) => {},
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {},
            Err(e) => return Err(e.to_string()),
        }
    }
    Ok(())
}

fn has_save_state(xml: &str, state: &str) -> bool {
    xml.split("<node ").any(|node| attr(node.split('>').next().unwrap_or(""), "resource-id").ends_with(state))
}
fn nested_body_has(marker: &str) -> Result<bool, String> {
    let count = live_sqlite(&format!(
        "SELECT count(*) FROM current_snapshot s, json_each(s.app_data_json,'$.notes') n WHERE s.workspace_key='guest' AND json_extract(n.value,'$.title')='{NESTED_PAGE}' AND EXISTS (SELECT 1 FROM json_each(n.value,'$.document.blocks') b WHERE instr(json_extract(b.value,'$.text'),'{marker}')>0);"
    ))?;
    Ok(count.trim() == "1")
}
fn edit_nested_page(root: &Path) -> Result<(), String> {
    let view = dump()?;
    let mut fields: Vec<Node> = elements(&view).into_iter().filter(|n|
        n.class_name.ends_with("EditText") && n.bounds[2] - n.bounds[0] > 80).collect();
    fields.sort_by_key(|n| n.bounds[1]);
    let body = fields.last().ok_or("Nested page body not editable")?;
    if fields.len() < 2 { return Err("Nested page only exposes title, not body".into()); }
    tap(body)?;
    let mut measurements = String::from("round,elapsed_ms,writer_contention\n");
    let mut expected = String::new();
    for round in 0..EDIT_ROUNDS {
        let marker = format!("817{round:02}629");
        expected.push_str(&marker);
        let writer = if round % 4 == 1 { Some(hold_sqlite_writer()?) } else { None };
        let started = std::time::Instant::now();
        cmd(&["shell", "input", "text", &marker])?;
        if let Some(writer) = writer {
            writer.join().map_err(|_| "Body writer thread panicked")??;
        }
        let mut saved = false;
        while started.elapsed().as_millis() < EDIT_BUDGET_MS {
            let ui = dump()?;
            if ui.contains(&expected) && has_save_state(&ui, "document_save_local_saved")
                && nested_body_has(&expected)? {
                saved = true;
                break;
            }
            sleep(Duration::from_millis(120));
        }
        let elapsed = started.elapsed().as_millis();
        measurements.push_str(&format!("{round},{elapsed},{}\n", round % 4 == 1));
        fs::write(root.join("body_save_stress.csv"), &measurements).map_err(|e| e.to_string())?;
        if !saved || elapsed > EDIT_BUDGET_MS {
            capture(root, &format!("body_save_timeout_{round}"));
            return Err(format!("Nested page body save round {round} exceeded {EDIT_BUDGET_MS}ms: {elapsed}ms; UI and SQLite receipt required"));
        }
    }
    // Burst of real key injections without waiting for autosave between edits.
    // One adb shell command preserves the input order, but no app state is mocked.
    let burst = (0..BURST_ROUNDS).map(|i| format!("943{i:02}")).collect::<Vec<_>>();
    let mut script = burst.iter().map(|marker| format!("input text {marker}")).collect::<Vec<_>>();
    script.push(format!("input text {EDIT_EXIT_MARKER}"));
    cmd(&["shell", &script.join(" && ")])?;
    let started = std::time::Instant::now();
    // The toolbar is hidden while the IME is visible. Back first dismisses
    // the keyboard; never reuse a coordinate captured before the long edit.
    cmd(&["shell", "input", "keyevent", "4"])?;
    let mut exited = false;
    let mut back_sent = false;
    while started.elapsed().as_millis() < EDIT_BUDGET_MS {
        let ui = dump()?;
        let visible = elements(&ui);
        let is_parent = visible.iter().any(|n| n.label == NESTED_FOLDER)
            && visible.iter().any(|n| n.label == "新建子页面")
            && visible.iter().any(|n| n.description == "文件夹设置")
            && !visible.iter().any(|n| n.class_name.ends_with("EditText") ||
                n.description.contains("拖动调整块顺序"));
        if !is_parent && !back_sent {
            if let Some(back) = visible.iter().find(|n| n.description == "返回" || n.description == "Back") {
                send_tap(back)?;
                back_sent = true;
            }
        }
        if is_parent &&
            !has_save_state(&ui, "document_save_saving") && nested_body_has(&format!("{}{}{EDIT_EXIT_MARKER}", expected, burst.concat()))? {
            exited = true;
            break;
        }
        sleep(Duration::from_millis(120));
    }
    let elapsed = started.elapsed().as_millis();
    fs::write(root.join("body_exit_save_ms.txt"), format!("{elapsed}\n")).map_err(|e| e.to_string())?;
    if !exited || elapsed > EDIT_BUDGET_MS {
        capture(root, "body_exit_save_timeout");
        return Err(format!("Immediate nested body exit not durable within {EDIT_BUDGET_MS}ms: {elapsed}ms"));
    }
    capture(root, "nested_body_stress_committed");
    Ok(())
}

fn persisted_after_restart(root: &Path, page_name: &str) -> Result<(usize, usize), String> {
    // A visible "Saved" label is not proof: read the authoritative SQLite
    // snapshot after process death, including its WAL, before reopening the UI.
    let local = root.join("durable_app_state.db");
    pull_database(&local)?;
    let escaped = page_name.replace('\'', "''");
    let matches = sqlite(&local, &format!(
        "SELECT COUNT(DISTINCT json_extract(note.value, '$.title')) FROM current_snapshot AS s,
         json_each(s.app_data_json, '$.notes') AS note WHERE s.workspace_key='guest'
         AND json_extract(note.value, '$.title') IN ('{escaped}', '{CONTENDED}');"
    ))?;
    if matches != "2" {
        return Err(format!("Expected two distinct saved page titles in SQLite after force-stop, found {matches}"));
    }
    // Confirm exactly one typed node per operation and the durable parent ID.
    let folders = sqlite(&local, &format!(
        "WITH nodes AS (SELECT note.value AS n FROM current_snapshot AS s,
          json_each(s.app_data_json, '$.notes') AS note WHERE s.workspace_key='guest'),
          root AS (SELECT n FROM nodes WHERE json_extract(n,'$.title')='{ROOT_FOLDER}'),
          child AS (SELECT n FROM nodes WHERE json_extract(n,'$.title')='{NESTED_FOLDER}'),
          page AS (SELECT n FROM nodes WHERE json_extract(n,'$.title')='{NESTED_PAGE}')
         SELECT (SELECT count(*) FROM root) || ':' || (SELECT count(*) FROM child) || ':' ||
          (SELECT count(*) FROM page) || ':' ||
          (SELECT count(*) FROM root, child, page WHERE
            coalesce(json_extract(root.n,'$.document.knowledge.parentId'),'')='' AND
            json_extract(child.n,'$.document.knowledge.parentId')=json_extract(root.n,'$.id') AND
            json_extract(page.n,'$.document.knowledge.parentId')=json_extract(child.n,'$.id') AND
            coalesce(json_extract(page.n,'$.document.knowledge.properties.__tenrate_container_kind.value'),'')!='folder' AND
            json_extract(root.n,'$.document.knowledge.properties.__tenrate_container_kind.value')='folder' AND
            json_extract(child.n,'$.document.knowledge.properties.__tenrate_container_kind.value')='folder');"
    ))?;
    if folders != "1:1:1:1" {
        return Err(format!("Folder identity, type or parent was not durably preserved: {folders}"));
    }
    let expected = (0..EDIT_ROUNDS).map(|round| format!("817{round:02}629")).collect::<String>()
        + &(0..BURST_ROUNDS).map(|round| format!("943{round:02}")).collect::<Vec<_>>().concat();
    let all_body = sqlite(&local, &format!(
        "SELECT count(*) FROM current_snapshot s, json_each(s.app_data_json,'$.notes') n WHERE s.workspace_key='guest' AND json_extract(n.value,'$.title')='{NESTED_PAGE}' AND EXISTS (SELECT 1 FROM json_each(n.value,'$.document.blocks') b WHERE instr(json_extract(b.value,'$.text'),'{expected}{EDIT_EXIT_MARKER}')>0);"
    ))?;
    if all_body != "1" { return Err("One or more nested body edits were lost after restart".into()); }
    let history: usize = sqlite(&local,
        "SELECT COUNT(*) FROM snapshot_history WHERE workspace_key='guest';")?
        .parse().map_err(|_| "Durable history count is invalid")?;
    fs::write(root.join("durable_history_count.txt"), format!("{history}\n"))
        .map_err(|e| e.to_string())?;
    if history < HISTORY_ROWS {
        return Err(format!("History was unexpectedly removed during saving: {history}"));
    }
    let version = sqlite(&local, "PRAGMA user_version;")?;
    let indexes = sqlite(&local, "SELECT COUNT(*) FROM sqlite_master WHERE type='index' AND name IN ('snapshot_history_schema_guard','snapshot_history_digest_guard','snapshot_history_envelope_guard');")?;
    if version != "7" || indexes != "3" {
        return Err(format!("Database upgrade did not install all guards: version={version}, indexes={indexes}"));
    }
    let packed = packed_legacy_rows(&local)?;
    remove_local_database(&local)?;
    if packed == 0 || packed >= HISTORY_ROWS {
        return Err(format!("Expected work remaining after partial legacy authentication, got {packed} packed rows"));
    }
    cmd(&["shell", "am", "start", "-W", "-n", ACTIVITY])?;
    sleep(Duration::from_secs(7));
    directory(root)?;
    for attempt in 0..18 {
        if find(&[page_name])?.is_some() {
            capture(root, "07_persisted_after_restart");
            return Ok((history, packed));
        }
        if attempt == 9 {
            // Reverse scroll once, because the folder can restore a previous offset.
            cmd(&["shell", "input", "swipe", "535", "700", "535", "1700", "400"])?;
        } else {
            swipe()?;
        }
    }
    capture(root, "07_page_not_persisted");
    Err("New knowledge page was not found after app process restart".into())
}

fn sqlite(db: &Path, statement: &str) -> Result<String, String> {
    let out = Command::new("sqlite3").arg(db).arg(statement)
        .output().map_err(|e| format!("sqlite3 is unavailable: {e}"))?;
    if !out.status.success() {
        return Err(format!("SQLite history fixture failed: {}", String::from_utf8_lossy(&out.stderr)));
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_owned())
}

fn packed_legacy_rows(db: &Path) -> Result<usize, String> {
    // The original v5 fixture rows have a valid digest; the separate live
    // fault fixture has neither a digest nor an envelope.
    let state = sqlite(db,
        "SELECT COUNT(*) || ':' || COUNT(CASE WHEN storage_encoding=1 AND app_data_json GLOB '!gridtimer-history-v1:*' THEN 1 END) \
         FROM snapshot_history WHERE workspace_key='guest' AND envelope_sha256='' AND sha256<>'';")?;
    let (total, packed) = state.split_once(':').ok_or("Invalid legacy row counts")?;
    if total.parse::<usize>() != Ok(HISTORY_ROWS) {
        return Err(format!("Legacy history was deleted or promoted: expected {HISTORY_ROWS} unsealed rows, got {total}"));
    }
    packed.parse().map_err(|_| "Invalid packed legacy row count".into())
}

// Expand only the historical JSON with legal whitespace, retaining the exact
// AppData object. An independently calculated digest keeps each row valid;
// padding invalid JSON or copying the old digest would test quarantine instead.
fn padded_history_digest(db: &Path, root: &Path) -> Result<String, String> {
    let encoded = sqlite(db,
        "SELECT hex(app_data_json) FROM current_snapshot WHERE workspace_key = 'guest';")?;
    if encoded.is_empty() || encoded.len() % 2 != 0 {
        return Err("Seed current JSON is empty or has invalid hexadecimal encoding".into());
    }
    let mut payload = Vec::with_capacity(encoded.len() / 2 + HISTORY_PADDING_BYTES);
    for pair in encoded.as_bytes().chunks_exact(2) {
        let digits = std::str::from_utf8(pair).map_err(|error| error.to_string())?;
        payload.push(u8::from_str_radix(digits, 16).map_err(|error| error.to_string())?);
    }
    payload.extend(std::iter::repeat(b' ').take(HISTORY_PADDING_BYTES));
    let candidate = root.join("legacy_history_payload_sample.json");
    fs::write(&candidate, &payload).map_err(|error| error.to_string())?;
    let hashed = Command::new("sha256sum").arg(&candidate).output()
        .map_err(|error| format!("sha256sum unavailable: {error}"))?;
    if !hashed.status.success() {
        return Err(format!("Cannot digest history fixture: {}", String::from_utf8_lossy(&hashed.stderr)));
    }
    let text = String::from_utf8_lossy(&hashed.stdout);
    let digest = text.split_whitespace().next().ok_or("No SHA-256 fixture digest")?;
    if digest.len() != 64 || !digest.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err("History fixture SHA-256 is malformed".into());
    }
    fs::write(root.join("history_payload_bytes.txt"), format!("{}\n", payload.len()))
        .map_err(|error| error.to_string())?;
    Ok(digest.to_owned())
}

// The emulator is rooted only for seeding a true on-disk historical workload.
// Production APK still runs with its normal signature, sandbox and persistence code.
fn seed_history(root: &Path) -> Result<(), String> {
    cmd(&["shell", "am", "force-stop", PACKAGE])?;
    cmd(&["root"])?;
    cmd(&["wait-for-device"])?;
    let local = root.join("fixture_app_state.db");
    let local_str = local.to_str().ok_or("Fixture path is not UTF-8")?;
    pull_database(&local)?;
    let current = sqlite(&local, "SELECT COUNT(*) FROM current_snapshot WHERE workspace_key = 'guest';")?;
    if current != "1" {
        return Err(format!("Expected one guest current snapshot before seeding, found {current}"));
    }
    // Preserve the payload/digest from an already authenticated current row.
    // Legacy history has a valid digest and a blank envelope; only startup
    // recovery must scan these unchanged rows. Save must preserve them while
    // authenticating the current snapshot without rereading all history.
    // Blank legacy history envelopes are supported by the existing native
    // history verifier, so no fabricated signature is introduced.
    let digest = padded_history_digest(&local, root)?;
    let sql = format!(
        "PRAGMA wal_checkpoint(TRUNCATE); PRAGMA journal_mode=DELETE; \
         DROP INDEX snapshot_history_schema_guard; \
         DROP INDEX snapshot_history_digest_guard; \
         DROP INDEX snapshot_history_envelope_guard; \
         PRAGMA user_version=5; \
         WITH RECURSIVE seq(n) AS (SELECT 1 UNION ALL SELECT n+1 FROM seq WHERE n < {HISTORY_ROWS}) \
         INSERT INTO snapshot_history (workspace_key, schema_version, revision, item_count, \
             app_data_json, sha256, envelope_sha256, created_at_epoch_millis) \
         SELECT c.workspace_key, c.schema_version, c.revision, c.item_count, \
             c.app_data_json || printf('%*s', {HISTORY_PADDING_BYTES}, ''), '{digest}', '', \
             c.updated_at_epoch_millis - {HISTORY_ROWS} + seq.n \
         FROM current_snapshot c CROSS JOIN seq WHERE c.workspace_key = 'guest'; \
         PRAGMA journal_mode=WAL; PRAGMA wal_checkpoint(TRUNCATE);"
    );
    sqlite(&local, &sql)?;
    let count: usize = sqlite(&local,
        "SELECT COUNT(*) FROM snapshot_history WHERE workspace_key='guest';")?
        .parse().map_err(|_| "Could not read seeded history row count")?;
    if count < HISTORY_ROWS {
        return Err(format!("History fixture contains {count} rows, expected at least {HISTORY_ROWS}"));
    }
    let temporary = "/data/local/tmp/tenrate_history_fixture.db";
    let fixture_bytes = fs::metadata(&local).map_err(|e| e.to_string())?.len();
    let capacity = cmd(&["shell", "df", "-k", "/data"])?;
    fs::write(root.join("fixture_storage.txt"), format!("history_rows={count}\nfixture_bytes={fixture_bytes}\n{capacity}"))
        .map_err(|e| e.to_string())?;
    println!("Historical fixture: {count} rows, {fixture_bytes} bytes");
    cmd(&["push", local_str, temporary])?;
    cmd(&["shell", "cp", temporary, DATABASE])?;
    let _ = cmd(&["shell", "rm", "-f", &format!("{DATABASE}-wal"), &format!("{DATABASE}-shm")]);
    let _ = cmd(&["shell", "restorecon", DATABASE]);
    cmd(&["shell", "rm", "-f", temporary])?;
    remove_local_database(&local)?;
    fs::write(root.join("fixture_history_count.txt"), format!("{count}\n"))
        .map_err(|e| e.to_string())?;
    cmd(&["shell", "am", "start", "-W", "-n", ACTIVITY])?;
    sleep(Duration::from_secs(7));
    Ok(())
}

fn inspect_preflight_logs(root: &Path) -> Result<&'static str, String> {
    // The signed release often omits Logcat. Use the application's durable
    // diagnostic store to prove which transaction-bound receipt was used.
    let log = cmd(&["logcat", "-d", "-b", "main", "-v", "brief"])?;
    let matching_logcat = log.lines()
        .filter(|line| line.contains("NoteSaveLatency")
            || line.contains("savePreflight scope"))
        .take(200).collect::<Vec<_>>().join("\n");
    fs::write(root.join("save_logcat_main.txt"), &log).map_err(|e| e.to_string())?;
    fs::write(root.join("save_preflight_logcat.txt"), &matching_logcat)
        .map_err(|e| e.to_string())?;
    let local = root.join("app_diagnostic_events.log");
    let local_str = local.to_str().ok_or("Diagnostic path is not UTF-8")?;
    cmd(&["pull", DIAGNOSTIC_LOG, local_str])?;
    let recorded = fs::read_to_string(&local)
        .map_err(|error| format!("App's durable scope proof unavailable: {error}"))?;
    let last = recorded.lines()
        .filter(|line| line.contains("| category=notes.local_save.preflight | scope="))
        .last().ok_or("No durable Android save-preflight scope was recorded")?;
    fs::write(root.join("last_preflight_scope.txt"), format!("{last}\n"))
        .map_err(|error| error.to_string())?;
    if last.contains("scope=current") {
        Ok("authenticated current-only (application diagnostic confirmed)")
    } else {
        Err(format!("The measured save did not use current-only preflight: {last}"))
    }
}

fn live_sqlite(statement: &str) -> Result<String, String> {
    cmd(&["shell", &format!("sqlite3 -batch -cmd '.timeout 8000' {DATABASE} \"{statement}\"")])
}

fn insert_history_without_digest() -> Result<i64, String> {
    let id = live_sqlite(
        "INSERT INTO snapshot_history (workspace_key, schema_version, revision, item_count, \
         app_data_json, sha256, envelope_sha256, created_at_epoch_millis) \
         SELECT workspace_key, schema_version, revision, item_count, app_data_json, '', '', \
         updated_at_epoch_millis FROM current_snapshot WHERE workspace_key = 'guest'; \
         SELECT last_insert_rowid();",
    )?.trim().parse().map_err(|_| "Missing history fixture row ID")?;
    if id > 0 { Ok(id) } else { Err("Could not insert history fixture in the open workspace".into()) }
}

fn hold_sqlite_writer() -> Result<JoinHandle<Result<(), String>>, String> {
    let writer = SqliteWriter::acquire()?;
    Ok(thread::spawn(move || {
        sleep(Duration::from_millis(2_000));
        writer.release()
    }))
}

// Run on the same signed APK and 100,000-row SQLite history after the knowledge
// checks. Do not replace the earlier assertions or accept a UI-only receipt.
const STICKY_TITLE: &str = "7391804025006010";
const STICKY_MARKER: &str = "12345678909876543210";
const STICKY_ROUNDS: usize = 24;
const STICKY_BUDGET_MS: u128 = 8_000;

fn sticky_home() -> Result<(), String> {
    for _ in 0..8 {
        let xml = dump()?;
        if xml.contains("最近便签") && xml.contains("回收站") { return Ok(()); }
        if tap_any(&["便签", "Sticky notes", "Quick notes"])? { continue; }
        cmd(&["shell", "input", "keyevent", "4"])?;
        sleep(Duration::from_millis(700));
    }
    Err("Could not reach the sticky note home".into())
}

fn sticky_stack(expected: usize) -> Result<bool, String> {
    let result = live_sqlite(&format!(
        "SELECT count(*) FROM current_snapshot s, json_each(s.app_data_json,'$.notes') n \
         WHERE s.workspace_key='guest' AND json_extract(n.value,'$.title')='{STICKY_TITLE}' \
         AND json_extract(n.value,'$.kind')='STICKY' \
         AND json_array_length(n.value,'$.versions')={expected} \
         AND (SELECT count(DISTINCT json_extract(v.value,'$.id')) FROM json_each(n.value,'$.versions') v)={expected} \
         AND (SELECT count(*) FROM json_each(n.value,'$.versions') v \
           WHERE json_extract(v.value,'$.isLatest')=1 AND json_extract(v.value,'$.id')=json_extract(n.value,'$.latestVersionId'))=1 \
         AND (SELECT count(*) FROM json_each(n.value,'$.versions') v \
           WHERE instr(v.value,'{STICKY_MARKER}')>0)={expected};"
    ))?;
    Ok(result.trim()=="1")
}

fn sticky_version_pressure(root: &Path) -> Result<String, String> {
    let evidence = root.join("sticky_version_pressure");
    fs::create_dir_all(&evidence).map_err(|e|e.to_string())?;
    cmd(&["shell","am","start","-W","-n",ACTIVITY])?;
    sleep(Duration::from_secs(7));
    sticky_home()?;
    if !tap_any(&["新建","New"])? { return Err("No sticky create action".into()); }
    let mut fields: Vec<Node> = elements(&dump()?).into_iter().filter(|n|
        n.class_name.ends_with("EditText") && n.bounds[2]>n.bounds[0] && n.bounds[3]>n.bounds[1]).collect();
    fields.sort_by_key(|n|n.bounds[1]);
    if fields.len()<2 { capture(&evidence,"missing_editor_fields"); return Err("Sticky editor did not expose title and body".into()); }
    tap(&fields[0])?;
    cmd(&["shell","input","text",STICKY_TITLE])?;
    let body = elements(&dump()?).into_iter().filter(|n|n.class_name.ends_with("EditText")
        && n.bounds[2]>n.bounds[0] && n.bounds[3]>n.bounds[1]).max_by_key(|n|n.bounds[1])
        .ok_or("Sticky body unavailable")?;
    tap(&body)?;
    for _ in 0..12 { cmd(&["shell","input","text",STICKY_MARKER])?; }
    cmd(&["shell","input","keyevent","4"])?;
    sleep(Duration::from_millis(600));
    // The real version panel is the same control shown in the reported screenshot.
    let mut metrics = String::new();
    for round in 1..=STICKY_ROUNDS {
        let action = find(&["下一个版本","Next version"])?.ok_or("Sticky version create action unavailable")?;
        let writer = if round%4==0 { Some(hold_sqlite_writer()?) } else { None };
        let started = std::time::Instant::now();
        send_tap(&action)?;
        let expected=round+1;
        let label=format!("版本 {expected} / {expected}");
        let mut confirmed=false;
        while started.elapsed().as_millis()<=STICKY_BUDGET_MS {
            let xml=dump()?;
            if xml.contains(&label) && !xml.contains("正在创建新版本") && sticky_stack(expected)? {
                confirmed=true;
                break;
            }
            sleep(Duration::from_millis(150));
        }
        if let Some(writer)=writer { writer.join().map_err(|_|"Sticky writer thread panicked")??; }
        let elapsed=started.elapsed().as_millis();
        metrics.push_str(&format!("{round},{elapsed},{confirmed}\n"));
        fs::write(evidence.join("version_elapsed_ms.csv"), &metrics).map_err(|e|e.to_string())?;
        if !confirmed || elapsed>STICKY_BUDGET_MS {
            capture(&evidence,"version_failure");
            return Err(format!("Sticky version {expected} did not receive exact UI and SQLite receipt within {STICKY_BUDGET_MS} ms: {elapsed}ms"));
        }
    }
    capture(&evidence,"all_versions_committed");
    cmd(&["shell","am","force-stop",PACKAGE])?;
    if !sticky_stack(STICKY_ROUNDS+1)? { return Err("Sticky version stack lost or duplicated after force-stop".into()); }
    cmd(&["shell","am","start","-W","-n",ACTIVITY])?;
    sleep(Duration::from_secs(7));
    sticky_home()?;
    if !tap_any(&[STICKY_TITLE])? { return Err("Persisted sticky note not visible after restart".into()); }
    let expected=STICKY_ROUNDS+1;
    let xml=dump()?;
    if !xml.contains(&format!("版本 {expected} / {expected}")) || !sticky_stack(expected)? {
        capture(&evidence,"restart_failure");
        return Err("Sticky version stack did not reopen intact".into());
    }
    capture(&evidence,"reopened_versions");
    Ok(format!("Sticky: {STICKY_ROUNDS} real UI version creates including six competing SQLite writers, each <= {STICKY_BUDGET_MS}ms with unique durable IDs, exact latest pointer and retained marker in every version; force-stop and restart verified."))
}

fn run(root: &Path) -> Result<String, String> {
    cmd(&["wait-for-device"])?;
    let info = cmd(&["shell", "dumpsys", "package", PACKAGE])?;
    let version = installed_version(&info)?;
    cmd(&["shell", "am", "start", "-W", "-n", ACTIVITY])?;
    sleep(Duration::from_secs(7));
    directory(root)?;
    create_page(root, SEED, || Ok(()))?;
    cmd(&["logcat", "-c"])?;
    let startup = std::time::Instant::now();
    seed_history(root)?;
    let ready = directory(root);
    let elapsed = startup.elapsed().as_millis();
    println!("100k fixture preparation plus startup navigation: {elapsed} ms; ready={}", ready.is_ok());
    fs::write(root.join("history_startup_elapsed_ms.txt"), format!("{elapsed}\n"))
        .map_err(|e| e.to_string())?;
    let log = cmd(&["logcat", "-d", "-s", "StartupLatency:I", "SQLiteLog:W", "AndroidRuntime:E"])?;
    fs::write(root.join("history_startup_logcat.txt"), &log).map_err(|e| e.to_string())?;
    println!("{log}");
    ready?;
    cmd(&["logcat", "-c"])?;
    let mut unchecked_history = None;
    let saved_millis = create_page(root, CREATED, || {
        // Add after startup so the local write itself encounters the old full-scan trigger.
        unchecked_history = Some(insert_history_without_digest()?);
        cmd(&["shell", "touch", "-m", "-t", "200001010000.00", PRIMARY_MIRROR])?;
        cmd(&["shell", &format!(": > {DIAGNOSTIC_LOG}")])?;
        Ok(())
    })?;
    fs::write(root.join("save_elapsed_ms.txt"), format!("{saved_millis}\n"))
        .map_err(|e| e.to_string())?;
    let scope = inspect_preflight_logs(root)?;
    let unchecked_history = unchecked_history.ok_or("Missing live history fixture")?;
    let still_retained = live_sqlite(&format!(
        "SELECT COUNT(*) FROM snapshot_history WHERE id = {unchecked_history} AND sha256 = '';"
    ))?;
    if still_retained.trim() != "1" {
        return Err("Foreground save consumed the unchecked history row instead of preserving it for recovery".into());
    }
    if saved_millis > 20_000 {
        return Err(format!("Saving a page with {HISTORY_ROWS} historical snapshots was too slow: {saved_millis} ms"));
    }
    // Follow a page commit with root and nested folders under the same history load.
    directory(root)?;
    let folder_evidence = root.join("folder_creation");
    fs::create_dir_all(&folder_evidence).map_err(|e| e.to_string())?;
    let root_folder_ms = create_folder(&folder_evidence, ROOT_FOLDER)?;
    capture(&folder_evidence, "root_folder_committed");
    let nested_folder_ms = create_folder(&folder_evidence, NESTED_FOLDER)?;
    capture(&folder_evidence, "nested_folder_committed");
    // Reproduce the reported folder -> new child page sequence under real
    // historical load. A timeout that merely unlocks the dialog is not success.
    let nested_page_ms = create_page(&folder_evidence, NESTED_PAGE, || Ok(()))?;
    capture(&folder_evidence, "nested_page_committed");
    fs::write(folder_evidence.join("creation_elapsed_ms.txt"),
        format!("root_folder={root_folder_ms}\nnested_folder={nested_folder_ms}\nnested_page={nested_page_ms}\n"))
        .map_err(|e| e.to_string())?;
    if [root_folder_ms, nested_folder_ms, nested_page_ms].iter().any(|&elapsed| elapsed > 20_000) {
        return Err(format!("Folder -> child page save exceeded 20s: {root_folder_ms}/{nested_folder_ms}/{nested_page_ms} ms"));
    }
    edit_nested_page(&folder_evidence)?;
    let mut returned_to_folder = false;
    for _ in 0..12 {
        let current = dump()?;
        let visible = elements(&current);
        if visible.iter().any(|n| n.label == NESTED_FOLDER)
            && visible.iter().any(|n| n.label == "新建子页面")
            && visible.iter().any(|n| n.description == "文件夹设置")
            && !visible.iter().any(|n| n.class_name.ends_with("EditText")) {
            returned_to_folder = true;
            break;
        }
        sleep(Duration::from_millis(300));
    }
    if !returned_to_folder {
        capture(&folder_evidence, "child_page_return_failed");
        return Err("New child page did not return to its parent directory".into());
    }
    for _ in 0..2 {
        if !tap_any(&["返回", "Back"])? {
            return Err("Could not navigate from created folder to its parent".into());
        }
    }
    directory(root)?;
    let mirror_dir = PRIMARY_MIRROR.rsplit_once('/').unwrap().0;
    let original_mode = cmd(&["shell", "stat", "-c", "%a", mirror_dir])?;
    cmd(&["shell", "chmod", "0555", mirror_dir])?;
    let mut writer = None;
    let contended_save = create_page(root, CONTENDED, || {
        cmd(&["shell", &format!(": > {DIAGNOSTIC_LOG}")])?;
        cmd(&["logcat", "-c"])?;
        writer = Some(hold_sqlite_writer()?);
        Ok(())
    });
    if let Some(writer) = writer {
        writer.join().map_err(|_| "SQLite writer thread panicked")??;
    }
    cmd(&["shell", "am", "force-stop", PACKAGE])?;
    cmd(&["shell", "chmod", original_mode.trim(), mirror_dir])?;
    let contended_millis = contended_save?;
    if cmd(&["exec-out", "cat", PRIMARY_MIRROR])?.contains(CONTENDED) {
        return Err("Expected the blocked recovery copy to lag behind the visible save".into());
    }
    let trace = cmd(&["logcat", "-d", "-s", "NoteSaveLatency:I"])?;
    fs::write(root.join("contended_receipt_trace.txt"), &trace).map_err(|e| e.to_string())?;
    if !trace.contains("phase=sqlite_receipt mirrors=queued") {
        return Err("The confirmed save did not exercise deferred recovery mirrors".into());
    }
    let diagnostics = cmd(&["exec-out", "cat", DIAGNOSTIC_LOG])?;
    fs::write(root.join("contended_save_diagnostics.txt"), &diagnostics)
        .map_err(|error| error.to_string())?;
    if !diagnostics.contains("phase=outer_sqlite_begin_wait")
        && !diagnostics.contains("phase=hierarchy_write_lock")
    {
        return Err("The competing SQLite writer did not exercise a recorded save wait".into());
    }
    let (durable_history, packed_before) = persisted_after_restart(root, CREATED)?;
    let mut repaired = false;
    for _ in 0..25 {
        if cmd(&["exec-out", "cat", PRIMARY_MIRROR])?.contains(CONTENDED) {
            repaired = true;
            break;
        }
        sleep(Duration::from_millis(400));
    }
    if !repaired { return Err("Verified startup did not repair the stale recovery copy".into()); }
    sleep(Duration::from_secs(4));
    cmd(&["shell", "am", "force-stop", PACKAGE])?;
    let after = root.join("resumed_app_state.db");
    pull_database(&after)?;
    let quarantined_history = sqlite(&after, &format!(
        "SELECT COUNT(*) FROM snapshot_quarantine WHERE workspace_key='guest' \
         AND source_table='snapshot_history' AND source_row_id='{unchecked_history}' \
         AND stored_sha256='' AND reason='sha256_mismatch' AND length(app_data_json)>0;"
    ))?;
    if quarantined_history != "1" {
        return Err("Startup recovery did not preserve the unchecked history row in quarantine".into());
    }
    let packed_after = packed_legacy_rows(&after)?;
    remove_local_database(&after)?;
    if packed_after <= packed_before {
        return Err(format!("Legacy authentication did not resume after restart: {packed_before} to {packed_after} packed rows"));
    }
    let sticky_report = sticky_version_pressure(root)?;
    println!("{sticky_report}");
    Ok(format!(
        "PASS: Android {version} upgraded a v5 database with {durable_history} retained SQLite snapshots. With an unchecked historical row inserted after startup, click-to-visible save took {saved_millis} ms with {scope}, and the row remained in history until startup recovery moved it to quarantine. With a competing SQLite writer and a read-only mirror directory, save took {contended_millis} ms. Both pages were durable after force-stop while the JSON mirror was still stale. Root folder, nested folder and nested page took {root_folder_ms}/{nested_folder_ms}/{nested_page_ms} ms and survived with exactly one node each and the correct parent IDs. Nested page passed {EDIT_ROUNDS} real body edits (including independent writer contention), each with UI and SQLite receipt within {EDIT_BUDGET_MS} ms; {BURST_ROUNDS} additional rapid input events followed by immediate final edit/Back and all body markers survived process death. Restart repaired the mirror and reopened the first page. Background authentication resumed from {packed_before} to {packed_after} packed legacy rows, preserving all {HISTORY_ROWS} original unsealed histories. {sticky_report}"
    ))
}
fn main() {
    let path = env::args_os().nth(1).map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("knowledge_save_evidence"));
    fs::create_dir_all(&path).expect("evidence directory");
    let result = run(&path);
    capture(&path, "08_final");
    if result.is_err() {
        if let Ok(trace) = cmd(&["logcat", "-d", "-s", "StartupLatency:I", "SQLiteLog:W", "AndroidRuntime:E", "NoteSaveLatency:I"]) {
            let _ = fs::write(path.join("failure_logcat.txt"), &trace);
            println!("{trace}");
        }
        if let Ok(trace) = cmd(&["exec-out", "cat", DIAGNOSTIC_LOG]) {
            let _ = fs::write(path.join("failure_diagnostics.txt"), trace);
        }
    }
    let report = match &result {
        Ok(message) => format!("# Android knowledge save emulator acceptance\n\n{message}\n\nOverall: PASS\n"),
        Err(error) => format!("# Android knowledge save emulator acceptance\n\nFailure: {error}\n\nOverall: FAILED / NOT VERIFIED\n"),
    };
    fs::write(path.join("report.md"), &report).expect("report");
    println!("{report}");
    if result.is_err() { std::process::exit(1); }
}
