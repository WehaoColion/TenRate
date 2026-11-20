// v3.0.2.5 - Distinguish conservative legacy fallback from healthy current-only fast path.
// v3.0.2.4 - Verify on-disk durability, bounded save latency and optional logcat proof with 7200 history rows.
//! Android API 35 signed-APK acceptance: new knowledge page must persist after restart.
//! The driver runs only against an installed signed APK, never a mocked UI.
use std::{env, fs, path::{Path, PathBuf}, process::{Command, Output}, thread::sleep, time::Duration};

const PACKAGE: &str = "com.ofairyo.gridtimer";
const ACTIVITY: &str = "com.ofairyo.gridtimer/.MainActivity";
const VERSION: &str = "3.0.2.5";
const CREATED: &str = "KnowledgeSaveProbe";
const SEED: &str = "KnowledgeSeedProbe";
const HISTORY_ROWS: usize = 7_200;
const HISTORY_PADDING_BYTES: usize = 16 * 1024;

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
        let _ = cmd(&["shell", "uiautomator", "dump", "/sdcard/knowledge_appearance.xml"]);
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
fn tap(node: &Node) -> Result<(), String> {
    let x = (node.bounds[0] + node.bounds[2]) / 2;
    let y = (node.bounds[1] + node.bounds[3]) / 2;
    cmd(&["shell", "input", "tap", &x.to_string(), &y.to_string()])?;
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
fn create_page(root: &Path, page_name: &str) -> Result<u128, String> {
    if !tap_any(&["新建子页面", "新建页面", "New subpage"])? {
        capture(root, "03_no_create_action");
        return Err("Knowledge directory has no accessible create-page control".into());
    }
    // Root screens may expose a direct button or a compact "新建" menu.
    // Require the actual dialog, not merely the menu's accessibility label.
    let before_dialog = dump()?;
    if !before_dialog.contains("新建页面") && !before_dialog.contains("New page") {
        if !tap_any(&["子页面", "Subpage"])? {
            return Err("Tapped create entry, but no page action was offered".into());
        }
    }
    capture(root, "03_page_dialog");
    let edit = elements(&dump()?).into_iter()
        .find(|node| node.class_name.ends_with("EditText")
            && node.bounds[2] - node.bounds[0] > 60)
        .ok_or("New-page title field not accessible")?;
    tap(&edit)?;
    cmd(&["shell", "input", "text", page_name])?;
    sleep(Duration::from_millis(450));
    capture(root, "04_title_entered");
    if !tap_any(&["确定", "Confirm", "OK"])? {
        return Err("New-page confirmation is missing".into());
    }
    let started = std::time::Instant::now();
    let mut saved = false;
    for tick in 0..28 {
        let current = dump()?;
        if tick == 2 || tick == 10 { capture(root, &format!("05_saving_{tick}")); }
        if current.contains(page_name) && !current.contains("正在保存") &&
            !current.contains("等待前序保存") && !current.contains("正在写入本地数据") {
            saved = true;
            break;
        }
        sleep(Duration::from_secs(1));
    }
    capture(root, "06_after_create");
    if !saved {
        return Err(format!(
            "Create-page confirmation failed to finish in the emulator ({} ms)",
            started.elapsed().as_millis()
        ));
    }
    if cmd(&["shell", "pidof", PACKAGE])?.trim().is_empty() {
        return Err("App process exited after knowledge page creation".into());
    }
    Ok(started.elapsed().as_millis())
}

fn persisted_after_restart(root: &Path, page_name: &str) -> Result<usize, String> {
    cmd(&["shell", "am", "force-stop", PACKAGE])?;
    // A visible "Saved" label is not proof: read the authoritative SQLite
    // snapshot after process death, including its WAL, before reopening the UI.
    let device = "/data/user/0/com.ofairyo.gridtimer/databases/app_state.db";
    let local = root.join("durable_app_state.db");
    let local_str = local.to_str().ok_or("Durability fixture path is not UTF-8")?;
    cmd(&["pull", device, local_str])?;
    for extension in ["-wal", "-shm"] {
        let _ = adb(&["pull", &format!("{device}{extension}"), &format!("{local_str}{extension}")]);
    }
    let escaped = page_name.replace('\'', "''");
    let matches = sqlite(&local,
        &format!("SELECT COUNT(*) FROM current_snapshot WHERE workspace_key='guest' AND instr(app_data_json, '{escaped}') > 0;"))?;
    if matches != "1" {
        return Err(format!("Visible Saved status was not durable in SQLite after force-stop: {matches} current matches"));
    }
    let history: usize = sqlite(&local,
        "SELECT COUNT(*) FROM snapshot_history WHERE workspace_key='guest';")?
        .parse().map_err(|_| "Durable history count is invalid")?;
    fs::write(root.join("durable_history_count.txt"), format!("{history}\n"))
        .map_err(|e| e.to_string())?;
    if history < HISTORY_ROWS {
        return Err(format!("History was unexpectedly removed during saving: {history}"));
    }
    cmd(&["shell", "am", "start", "-W", "-n", ACTIVITY])?;
    sleep(Duration::from_secs(7));
    directory(root)?;
    for attempt in 0..18 {
        if find(&[page_name])?.is_some() {
            capture(root, "07_persisted_after_restart");
            return Ok(history);
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
    let device = "/data/user/0/com.ofairyo.gridtimer/databases/app_state.db";
    let local = root.join("fixture_app_state.db");
    let local_str = local.to_str().ok_or("Fixture path is not UTF-8")?;
    cmd(&["pull", device, local_str])?;
    for extension in ["-wal", "-shm"] {
        let _ = adb(&["pull", &format!("{device}{extension}"), &format!("{local_str}{extension}")]);
    }
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
        "PRAGMA wal_checkpoint(TRUNCATE); \
         WITH RECURSIVE seq(n) AS (SELECT 1 UNION ALL SELECT n+1 FROM seq WHERE n < {HISTORY_ROWS}) \
         INSERT INTO snapshot_history (workspace_key, schema_version, revision, item_count, \
             app_data_json, sha256, envelope_sha256, created_at_epoch_millis) \
         SELECT c.workspace_key, c.schema_version, c.revision, c.item_count, \
             c.app_data_json || printf('%*s', {HISTORY_PADDING_BYTES}, ''), '{digest}', '', \
             c.updated_at_epoch_millis - {HISTORY_ROWS} + seq.n \
         FROM current_snapshot c CROSS JOIN seq WHERE c.workspace_key = 'guest'; \
         PRAGMA wal_checkpoint(TRUNCATE);"
    );
    sqlite(&local, &sql)?;
    let count: usize = sqlite(&local,
        "SELECT COUNT(*) FROM snapshot_history WHERE workspace_key='guest';")?
        .parse().map_err(|_| "Could not read seeded history row count")?;
    if count < HISTORY_ROWS {
        return Err(format!("History fixture contains {count} rows, expected at least {HISTORY_ROWS}"));
    }
    let temporary = "/data/local/tmp/tenrate_history_fixture.db";
    cmd(&["push", local_str, temporary])?;
    cmd(&["shell", "cp", temporary, device])?;
    let _ = cmd(&["shell", "rm", "-f", &format!("{device}-wal"), &format!("{device}-shm")]);
    let _ = cmd(&["shell", "restorecon", device]);
    cmd(&["shell", "rm", "-f", temporary])?;
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
    let device = "/data/user/0/com.ofairyo.gridtimer/files/diagnostic_events.log";
    let local = root.join("app_diagnostic_events.log");
    let local_str = local.to_str().ok_or("Diagnostic path is not UTF-8")?;
    cmd(&["pull", device, local_str])?;
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

fn run(root: &Path) -> Result<String, String> {
    cmd(&["wait-for-device"])?;
    let info = cmd(&["shell", "dumpsys", "package", PACKAGE])?;
    if !info.contains(&format!("versionName={VERSION}")) {
        return Err(format!("Installed package is not signed candidate {VERSION}"));
    }
    cmd(&["shell", "am", "start", "-W", "-n", ACTIVITY])?;
    sleep(Duration::from_secs(7));
    directory(root)?;
    let _ = create_page(root, SEED)?;
    seed_history(root)?;
    directory(root)?;
    cmd(&["logcat", "-c"])?;
    let saved_millis = create_page(root, CREATED)?;
    fs::write(root.join("save_elapsed_ms.txt"), format!("{saved_millis}\n"))
        .map_err(|e| e.to_string())?;
    let scope = inspect_preflight_logs(root)?;
    if saved_millis > 20_000 {
        return Err(format!("Saving a page with {HISTORY_ROWS} historical snapshots was too slow: {saved_millis} ms"));
    }
    let durable_history = persisted_after_restart(root, CREATED)?;
    Ok(format!(
        "PASS: Android {VERSION} saved a knowledge page with {durable_history} historical SQLite snapshots in {saved_millis} ms; on-disk current snapshot and UI both survived force-stop/restart. Preflight trace: {scope}. Legacy-envelope fixture retains historical data; current-only routing must be corroborated by diagnostics where available."
    ))
}
fn main() {
    let path = env::args_os().nth(1).map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("knowledge_save_evidence"));
    fs::create_dir_all(&path).expect("evidence directory");
    let result = run(&path);
    capture(&path, "08_final");
    let report = match &result {
        Ok(message) => format!("# Android knowledge save emulator acceptance\n\n{message}\n\nOverall: PASS\n"),
        Err(error) => format!("# Android knowledge save emulator acceptance\n\nFailure: {error}\n\nOverall: FAILED / NOT VERIFIED\n"),
    };
    fs::write(path.join("report.md"), &report).expect("report");
    println!("{report}");
    if result.is_err() { std::process::exit(1); }
}
