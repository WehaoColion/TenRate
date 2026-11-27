// v3.0.2.46 - Disambiguate toolbar exit, app receipt and rendered directory in million-row probe.
// v3.0.2.45 - Require fresh, complete accessibility evidence.
// v3.0.2.44 - Require v9 schema guard and legacy recovery without eager payload header index.
// v3.0.2.37 - Current editor state owns composition, exact selection evidence, same-instant timer comparison.
// v3.0.2.35 - Preserve the baseline and add million-row stress with exact receipts.
// v3.0.2.34 - Consolidate historical safety guards without changing authentication.
// v3.0.2.33 - Share release identity and visible navigation evidence.
// v3.0.2.31 - Bound read-only log collection and preserve strict durable assertions.
// v3.0.2.29 - Use document-end key chord and preserve exact failure evidence.
// v3.0.2.25 - Exercise actual background/resume during body saves.
// v3.0.2.22 - Preserve exact pre-version input evidence and recover from Android share/launcher navigation.
// v3.0.2.18 - Seed 500,000 real authenticated SQLite history rows in place with bounded transactions.
// v3.0.2.16 - Reveal real version count before timing and log independent UI/SQLite receipts.
// v3.0.2.15 - Verify exact sticky authored lines in each durable version and in the actual PNG pixels.
// v3.0.2.15 - Validate stopped-app durable SQLite directly on the emulator without multi-GiB transfers.
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
mod android_directory_evidence;
mod android_stress_profile;
use android_directory_evidence::prove_directory;
use android_stress_profile::{ACTIVE, HISTORY_PADDING_BYTES, RECOVERY_BUDGET_SECONDS, SAVE_BUDGET_MS};
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
const EDIT_ROUNDS: usize = ACTIVE.edit_rounds;
const BURST_ROUNDS: usize = ACTIVE.burst_rounds;
const EDIT_EXIT_MARKER: &str = "98765432109876543210";
const EDIT_BUDGET_MS: u128 = SAVE_BUDGET_MS;
const HISTORY_ROWS: usize = ACTIVE.history_rows;
const HISTORY_BATCH_ROWS: usize = 10_000;
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
    // Retry only specific read-only evidence operations. Never replay a tap,
    // key event, SQL mutation, log clear or version creation.
    let diagnostic = args.first() == Some(&"logcat") && args.contains(&"-d");
    let share_listing = args.len() == 2 && args[0] == "shell"
        && args[1].starts_with("ls -1t /data/user/0/com.ofairyo.gridtimer/cache/note_shares/note_share_*.png ");
    let attempts = if diagnostic || share_listing { 3 } else { 1 };
    for attempt in 0..attempts {
        let out = adb(args)?;
        if out.status.success() {
            return Ok(String::from_utf8_lossy(&out.stdout).into_owned());
        }
        let error = format!("adb {args:?}: status={} stderr={} stdout_bytes={}",
            out.status, String::from_utf8_lossy(&out.stderr), out.stdout.len());
        if attempt + 1 == attempts { return Err(error); }
        eprintln!("Read-only diagnostic retry: {error}");
        sleep(Duration::from_millis(350 * (attempt as u64 + 1)));
    }
    unreachable!("at least one ADB attempt")
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
mod android_fresh_hierarchy;
fn dump() -> Result<String, String> {
    android_fresh_hierarchy::fresh_hierarchy(cmd, "/sdcard/knowledge_appearance.xml", 6)
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
// A very large legacy history may keep the verified startup gate visible
// longer than normal navigation. This is not part of the per-save budget.
// Wait only for the explicit recovery gate; do not bypass authentication.
fn wait_for_workspace_restore() -> Result<(), String> {
    let started = std::time::Instant::now();
    loop {
        let xml = dump()?;
        if !xml.contains("正在恢复本机数据") && !xml.contains("数据准备完成后自动进入工作台") {
            return Ok(());
        }
        if started.elapsed() >= Duration::from_secs(RECOVERY_BUDGET_SECONDS) {
            return Err("Verified workspace recovery still blocked after 600 seconds".into());
        }
        sleep(Duration::from_secs(2));
    }
}

fn directory(root: &Path) -> Result<(), String> {
    wait_for_workspace_restore()?;
    for step in 0..13 {
        clear_system_dialog()?;
        let view = dump()?;
        if prove_directory(&view, None) {
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

fn has_save_state(xml: &str, state: &str) -> bool {
    xml.split("<node ").any(|node| attr(node.split('>').next().unwrap_or(""), "resource-id").ends_with(state))
}
fn nested_body_has(marker: &str) -> Result<bool, String> {
    let count = live_sqlite(&format!(
        "SELECT count(*) FROM current_snapshot s, json_each(s.app_data_json,'$.notes') n WHERE s.workspace_key='guest' AND json_extract(n.value,'$.title')='{NESTED_PAGE}' AND EXISTS (SELECT 1 FROM json_each(n.value,'$.document.blocks') b WHERE instr(json_extract(b.value,'$.text'),'{marker}')>0);"
    ))?;
    Ok(count.trim() == "1")
}
// A real text-field selection receipt is independent of saved-state receipts.
// Do not start a timed append while the IME still owns a stale selection.
fn append_caret_proven(xml: &str, expected: &str) -> bool {
    let length = expected.encode_utf16().count();
    let tag = format!("document_body_cursor_{length}_{length}_{length}_committed");
    xml.split("<node ").skip(1).filter_map(|entry| entry.split('>').next()).any(|node| {
        attr(node, "class").ends_with("EditText")
            && attr(node, "focused") == "true"
            && attr(node, "resource-id").ends_with(&tag)
            && (expected.is_empty() || attr(node, "text") == expected)
    })
}

fn ensure_append_caret(root: &Path, expected: &str, label: &str) -> Result<(), String> {
    let mut xml = dump()?;
    if append_caret_proven(&xml, expected) { return Ok(()); }
    let mut fields: Vec<Node> = elements(&xml).into_iter().filter(|n|
        n.class_name.ends_with("EditText") && n.bounds[2] - n.bounds[0] > 80).collect();
    fields.sort_by_key(|n| n.bounds[1]);
    if fields.len() < 2 { return Err("Body not visible for real cursor positioning".into()); }
    tap(fields.last().unwrap())?;
    cmd(&["shell", "input", "keycombination", "KEYCODE_CTRL_LEFT", "KEYCODE_MOVE_END"])?;
    for attempt in 0..=3 {
        xml = dump()?;
        if append_caret_proven(&xml, expected) { return Ok(()); }
        if attempt == 3 { break; }
        // No replacement, paste or extra characters. Each pass is bounded by
        // the existing source length; an actual selection receipt is mandatory.
        let mut keys = vec!["shell", "input", "keyevent", "KEYCODE_MOVE_END"];
        keys.extend(std::iter::repeat("KEYCODE_DPAD_RIGHT").take(expected.encode_utf16().count() + 1));
        cmd(&keys)?;
        sleep(Duration::from_millis(200));
    }
    fs::write(root.join(format!("cursor_{label}_not_proven.xml")), &xml).map_err(|e| e.to_string())?;
    capture(root, &format!("cursor_{label}_not_proven"));
    Err(format!("Body cursor not confirmed at exact append position before {label}; no input or save attempted"))
}

fn edit_nested_page(root: &Path) -> Result<(), String> {
    let view = dump()?;
    let mut fields: Vec<Node> = elements(&view).into_iter().filter(|n|
        n.class_name.ends_with("EditText") && n.bounds[2] - n.bounds[0] > 80).collect();
    fields.sort_by_key(|n| n.bounds[1]);
    let body = fields.last().ok_or("Nested page body not editable")?;
    if fields.len() < 2 { return Err("Nested page only exposes title, not body".into()); }
    tap(body)?;
    let mut measurements = String::from("round,elapsed_ms,writer_contention,background_resume\n");
    let mut expected = String::new();
    for round in 0..EDIT_ROUNDS {
        ensure_append_caret(root, &expected, &format!("round_{round}"))?;
        let marker = format!("817{round:02}629");
        expected.push_str(&marker);
        let writer = if round % 4 == 1 { Some(hold_sqlite_writer()?) } else { None };
        let started = std::time::Instant::now();
        let lifecycle_round = round % 4 == 2;
        if lifecycle_round {
            // Reproduce the device report: leave the app immediately after
            // actual typing, then return before the save receipt is observed.
            cmd(&["shell", &format!("input text {marker} && input keyevent 3")])?;
            sleep(Duration::from_secs(1));
            cmd(&["shell", "am", "start", "-W", "-n", ACTIVITY])?;
        } else {
            cmd(&["shell", "input", "text", &marker])?;
        }
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
        measurements.push_str(&format!("{round},{elapsed},{},{lifecycle_round}\n", round % 4 == 1));
        fs::write(root.join("body_save_stress.csv"), &measurements).map_err(|e| e.to_string())?;
        if !saved || elapsed > EDIT_BUDGET_MS {
            capture(root, &format!("body_save_timeout_{round}"));
            // Diagnostic capture happens only after the original deadline.
            // Never repair input or reinterpret a reordered body as a pass.
            if let Ok(xml) = dump() {
                let mut fields: Vec<Node> = elements(&xml).into_iter().filter(|n|
                    n.class_name.ends_with("EditText") && n.bounds[2] - n.bounds[0] > 80).collect();
                fields.sort_by_key(|n| n.bounds[1]);
                let observed = fields.last().map(|n| n.label.as_str()).unwrap_or("");
                let visible_saved = has_save_state(&xml, "document_save_local_saved");
                let expected_durable = nested_body_has(&expected).unwrap_or(false);
                fs::write(root.join(format!("body_save_failure_comparison_{round}.txt")),
                    format!("expected={expected:?}\nobserved={observed:?}\nvisible_saved={visible_saved}\nexpected_durable={expected_durable}\n")).map_err(|e|e.to_string())?;
                if visible_saved && !observed.is_empty() && observed != expected && !expected_durable {
                    return Err(format!("Nested page body round {round} failed exact input order despite visible saved receipt; expected {expected:?}, observed {observed:?}; {elapsed}ms (not a proven write timeout)"));
                }
            }
            return Err(format!("Nested page body save round {round} exceeded {EDIT_BUDGET_MS}ms: {elapsed}ms; UI and SQLite receipt required"));
        }
        if lifecycle_round {
            // The next round must obtain a new real selection receipt after
            // returning from background. Never assume coordinate taps mean end.
            ensure_append_caret(root, &expected, &format!("resume_{round}"))?;
        }
    }
    ensure_append_caret(root, &expected, "burst")?;
    // Burst of real key injections without waiting for autosave between edits.
    // One adb shell command preserves the input order, but no app state is mocked.
    let burst = (0..BURST_ROUNDS).map(|i| format!("943{i:02}")).collect::<Vec<_>>();
    // Preserve the immediate final edit/Back sequence: clear the diagnostic
    // buffer BEFORE the burst, never between the final key and Back.
    cmd(&["logcat", "-c"])?;
    let mut script = burst.iter().map(|marker| format!("input text {marker}")).collect::<Vec<_>>();
    script.push(format!("input text {EDIT_EXIT_MARKER}"));
    cmd(&["shell", &script.join(" && ")])?;
    let started = std::time::Instant::now();
    // The toolbar is hidden while the IME is visible. Back first dismisses
    // the keyboard; never reuse a coordinate captured before the long edit.
    cmd(&["shell", "input", "keyevent", "4"])?;
    let mut exited = false;
    let mut back_sent = false;
    let mut observations = String::from("elapsed_ms,parent,editor,back_sent\n");
    while started.elapsed().as_millis() < EDIT_BUDGET_MS {
        let ui = dump()?;
        let is_parent = prove_directory(&ui, Some(NESTED_FOLDER));
        // A text-only toolbar label is as valid as a content description.
        // Require a real visible/enabled app control on the document, never
        // tap a parent-directory Back action or a hidden transition node.
        let editor = ui.contains("document_body_cursor_");
        if !is_parent && editor && !back_sent {
            let back = ui.split("<node ").skip(1).filter_map(|entry| entry.split('>').next())
                .find(|node| {
                    attr(node, "package") == PACKAGE && attr(node, "enabled") == "true" &&
                    (["返回", "Back"].contains(&attr(node, "text").as_str()) ||
                     ["返回", "Back"].contains(&attr(node, "content-desc").as_str())) &&
                    {
                        let bounds = attr(node, "bounds");
                        let coords: Vec<i32> = bounds.split(|c: char| !c.is_ascii_digit() && c != '-')
                            .filter(|part| !part.is_empty()).filter_map(|part| part.parse().ok()).collect();
                        coords.len() == 4 && coords[2] > coords[0] && coords[3] > coords[1]
                    }
                });
            if let Some(back) = back {
                let wrapped = format!("<node {back}>");
                if let Some(node) = elements(&wrapped).first() {
                    send_tap(node)?;
                    back_sent = true;
                }
            }
        }
        observations.push_str(&format!("{},{is_parent},{editor},{back_sent}\n", started.elapsed().as_millis()));
        if is_parent &&
            !has_save_state(&ui, "document_save_saving") && nested_body_has(&format!("{}{}{EDIT_EXIT_MARKER}", expected, burst.concat()))? {
            exited = true;
            break;
        }
        sleep(Duration::from_millis(120));
    }
    let elapsed = started.elapsed().as_millis();
    fs::write(root.join("body_exit_save_ms.txt"), format!("{elapsed}\n")).map_err(|e| e.to_string())?;
    fs::write(root.join("body_exit_observations.csv"), observations).map_err(|e| e.to_string())?;
    // Gather independent app-side timing AFTER the original wall-clock
    // measurement. Neither log retrieval nor diagnostics can turn failure
    // into success or extend the original 8-second deadline.
    let trace = cmd(&["logcat", "-d", "-s", "NoteSaveLatency:I"]);
    let trace = trace.unwrap_or_else(|error| format!("Exit diagnostic unavailable: {error}"));
    fs::write(root.join("body_exit_receipt_trace.txt"), &trace).map_err(|e| e.to_string())?;
    for line in trace.lines().filter(|line| line.contains("phase=document_exit_")) {
        println!("Nested exit app evidence: {line}");
    }
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
    require_stopped_app()?;
    let escaped = page_name.replace('\'', "''");
    let matches = stopped_sqlite(&format!(
        "SELECT COUNT(DISTINCT json_extract(note.value, '$.title')) FROM current_snapshot AS s,
         json_each(s.app_data_json, '$.notes') AS note WHERE s.workspace_key='guest'
         AND json_extract(note.value, '$.title') IN ('{escaped}', '{CONTENDED}');"
    ))?;
    if matches != "2" {
        return Err(format!("Expected two distinct saved page titles in SQLite after force-stop, found {matches}"));
    }
    // Confirm exactly one typed node per operation and the durable parent ID.
    let folders = stopped_sqlite(&format!(
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
    let all_body = stopped_sqlite(&format!(
        "SELECT count(*) FROM current_snapshot s, json_each(s.app_data_json,'$.notes') n WHERE s.workspace_key='guest' AND json_extract(n.value,'$.title')='{NESTED_PAGE}' AND EXISTS (SELECT 1 FROM json_each(n.value,'$.document.blocks') b WHERE instr(json_extract(b.value,'$.text'),'{expected}{EDIT_EXIT_MARKER}')>0);"
    ))?;
    if all_body != "1" { return Err("One or more nested body edits were lost after restart".into()); }
    let history: usize = stopped_sqlite(
        "SELECT COUNT(*) FROM snapshot_history WHERE workspace_key='guest';")?
        .parse().map_err(|_| "Durable history count is invalid")?;
    fs::write(root.join("durable_history_count.txt"), format!("{history}\n"))
        .map_err(|e| e.to_string())?;
    if history < HISTORY_ROWS {
        return Err(format!("History was unexpectedly removed during saving: {history}"));
    }
    let version = stopped_sqlite("PRAGMA user_version;")?;
    let schema_index = stopped_sqlite("SELECT COUNT(*) FROM sqlite_master WHERE type='index' AND name='snapshot_history_schema_v9';")?;
    let header_index = stopped_sqlite("SELECT COUNT(*) FROM sqlite_master WHERE type='index' AND name='snapshot_history_header_guard';")?;
    let obsolete = stopped_sqlite("SELECT COUNT(*) FROM sqlite_master WHERE type='index' AND name IN ('snapshot_history_schema_guard','snapshot_history_digest_guard','snapshot_history_envelope_guard');")?;
    if version != "9" || schema_index != "1" || header_index != "0" || obsolete != "0" {
        return Err(format!("Legacy upgrade must install the schema guard without an eager payload header index: version={version}, schema={schema_index}, header={header_index}, obsolete={obsolete}"));
    }
    let packed = packed_legacy_rows()?;
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

// Independently query the actual on-device SQLite file (including committed WAL)
// with the app stopped. Avoid copying a multi-GiB DB through adb or using app
// APIs for evidence. Read-only mode prevents the verifier from changing data.
fn require_stopped_app() -> Result<(), String> {
    let output = adb(&["shell", "pidof", PACKAGE])?;
    if output.status.success() || !String::from_utf8_lossy(&output.stdout).trim().is_empty() {
        return Err("App process must be stopped before durable SQLite verification".into());
    }
    if output.status.code() != Some(1) {
        return Err(format!("Could not verify stopped app: {}", String::from_utf8_lossy(&output.stderr)));
    }
    Ok(())
}

fn stopped_sqlite(statement: &str) -> Result<String, String> {
    require_stopped_app()?;
    let quote = |value: &str| format!("'{}'", value.replace('\'', "'\\''"));
    let command = format!("sqlite3 -readonly -batch -bail -cmd '.timeout 8000' {} {}",
        quote(DATABASE), quote(statement));
    let result = cmd(&["shell", &command])?;
    require_stopped_app()?;
    Ok(result.trim().to_owned())
}

fn packed_legacy_rows() -> Result<usize, String> {
    // The original v5 fixture rows have a valid digest; the separate live
    // fault fixture has neither a digest nor an envelope.
    let state = stopped_sqlite(
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
fn padded_history_digest(root: &Path) -> Result<String, String> {
    let encoded = stopped_sqlite(
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
    require_stopped_app()?;
    let current = stopped_sqlite("SELECT COUNT(*) FROM current_snapshot WHERE workspace_key='guest';")?;
    if current != "1" {
        return Err(format!("Expected one guest current snapshot before seeding, found {current}"));
    }
    let digest = padded_history_digest(root)?;
    let payload_bytes: u64 = fs::metadata(root.join("legacy_history_payload_sample.json"))
        .map_err(|e|e.to_string())?.len();
    let capacity = cmd(&["shell", "df", "-k", "/data"])?;
    let available_kib = capacity.lines().last()
        .and_then(|line|line.split_whitespace().nth(3))
        .and_then(|value|value.parse::<u64>().ok()).ok_or("Could not read emulator free space")?;
    // Allow for page/overflow overhead, indexes and bounded batch journals.
    let minimum = payload_bytes.saturating_mul(HISTORY_ROWS as u64)
        .saturating_mul(3)/2 + 3*1024*1024*1024;
    fs::write(root.join("fixture_storage.txt"),format!("target_rows={HISTORY_ROWS}\npayload_bytes={payload_bytes}\nminimum_free_bytes={minimum}\n{capacity}"))
        .map_err(|e|e.to_string())?;
    if available_kib.saturating_mul(1024) < minimum {
        return Err(format!("Insufficient emulator disk: need {minimum} bytes; available {} KiB",available_kib));
    }
    // Seed directly on the rooted test emulator. Preserve the original file
    // owner/SELinux label. No multi-GiB pull, push or second database copy.
    let quote = |value: &str| format!("'{}'", value.replace('\'', "'\\''"));
    let write = |statement: &str| -> Result<String,String> {
        require_stopped_app()?;
        let command = format!("sqlite3 -batch -bail -cmd '.timeout 8000' {} {}",quote(DATABASE),quote(statement));
        let result = cmd(&["shell",&command])?;
        require_stopped_app()?;
        Ok(result.trim().to_owned())
    };
    write("PRAGMA wal_checkpoint(TRUNCATE); PRAGMA journal_mode=DELETE; \
           DROP INDEX IF EXISTS snapshot_history_header_guard; \
           DROP INDEX IF EXISTS snapshot_history_schema_v9; \
           PRAGMA user_version=5;")?;
    // Identical payloads retain the original test's authenticated JSON and
    // independently computed SHA-256. Each row has its own SQLite identity.
    // Bounded transactions prevent a single giant journal or copy.
    for first in (1..=HISTORY_ROWS).step_by(HISTORY_BATCH_ROWS) {
        let last = (first + HISTORY_BATCH_ROWS - 1).min(HISTORY_ROWS);
        write(&format!(
            "BEGIN IMMEDIATE; \
             WITH RECURSIVE seq(n) AS (SELECT {first} UNION ALL SELECT n+1 FROM seq WHERE n < {last}) \
             INSERT INTO snapshot_history (workspace_key, schema_version, revision, item_count, \
                 app_data_json, sha256, envelope_sha256, created_at_epoch_millis) \
             SELECT c.workspace_key, c.schema_version, c.revision, c.item_count, \
                 c.app_data_json || printf('%*s', {HISTORY_PADDING_BYTES}, ''), '{digest}', '', \
                 c.updated_at_epoch_millis - {HISTORY_ROWS} + seq.n \
             FROM current_snapshot c CROSS JOIN seq WHERE c.workspace_key='guest'; COMMIT;"
        ))?;
        println!("Seeded historical rows {first}..={last} / {HISTORY_ROWS}");
    }
    write("PRAGMA journal_mode=WAL; PRAGMA wal_checkpoint(TRUNCATE);")?;
    let count: usize = stopped_sqlite("SELECT COUNT(*) FROM snapshot_history WHERE workspace_key='guest';")?
        .parse().map_err(|_| "Could not read seeded history count")?;
    let exact = stopped_sqlite(&format!(
        "SELECT COUNT(*) FROM snapshot_history WHERE workspace_key='guest' AND sha256='{digest}' \
         AND envelope_sha256='' AND length(CAST(app_data_json AS BLOB))={payload_bytes};"))?;
    if count < HISTORY_ROWS || exact.parse::<usize>() != Ok(HISTORY_ROWS) {
        return Err(format!("Historical fixture mismatch: total={count}, exact={exact}, expected={HISTORY_ROWS}"));
    }
    let fixture_bytes = cmd(&["shell","stat","-c","%s",DATABASE])?;
    let capacity = cmd(&["shell","df","-k","/data"])?;
    fs::write(root.join("fixture_storage.txt"),format!("history_rows={count}\nexact_fixture_rows={exact}\npayload_bytes={payload_bytes}\nfixture_bytes={}\n{capacity}",fixture_bytes.trim()))
        .map_err(|e|e.to_string())?;
    fs::write(root.join("fixture_history_count.txt"),format!("{count}\n"))
        .map_err(|e|e.to_string())?;
    println!("Historical fixture: {count} rows, {exact} exact fixture rows, {} bytes",fixture_bytes.trim());
    cmd(&["shell", "restorecon", DATABASE])?;
    cmd(&["shell", "am", "start", "-W", "-n", ACTIVITY])?;
    sleep(Duration::from_secs(7));
    Ok(())
}

fn inspect_preflight_logs(root: &Path) -> Result<&'static str, String> {
    // The signed release often omits Logcat. Use the application's durable
    // diagnostic store to prove which transaction-bound receipt was used.
    // Avoid transferring unrelated main-buffer traffic from a busy emulator.
    // The application diagnostic file remains the authoritative scope proof.
    let log = cmd(&["logcat", "-d", "-b", "main", "-v", "brief", "-s", "NoteSaveLatency:I", "StartupLatency:I", "SQLiteLog:W", "AndroidRuntime:E"])?;
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

// Run on the same signed APK and 500,000-row SQLite history after the knowledge
// checks. Do not replace the earlier assertions or accept a UI-only receipt.
const STICKY_TITLE: &str = "7391804025006010";
const STICKY_MARKER: &str = "12345678909876543210";
const STICKY_ROUNDS: usize = ACTIVE.sticky_rounds;
const STICKY_BUDGET_MS: u128 = SAVE_BUDGET_MS;

fn sticky_home() -> Result<(), String> {
    wait_for_workspace_restore()?;
    for _ in 0..8 {
        let xml = dump()?;
        if xml.contains("最近便签") && xml.contains("回收站") { return Ok(()); }
        // Sharing is an external Android activity. Back can leave the launcher
        // foreground after a process restart; never keep pressing Back there.
        if xml.contains("package=\"com.google.android.apps.nexuslauncher\"") ||
            xml.contains("package=\"com.android.launcher3\"") {
            cmd(&["shell","am","start","-W","-n",ACTIVITY])?;
            sleep(Duration::from_secs(2));
            wait_for_workspace_restore()?;
            continue;
        }
        if xml.contains("package=\"com.android.intentresolver\"") ||
            xml.contains("package=\"com.android.internal.app\"") {
            cmd(&["shell","input","keyevent","4"])?;
            sleep(Duration::from_millis(700));
            continue;
        }
        if tap_any(&["便签", "Sticky notes", "Quick notes"])? { continue; }
        cmd(&["shell", "input", "keyevent", "4"])?;
        sleep(Duration::from_millis(700));
    }
    Err("Could not reach the sticky note home".into())
}

// Same 240 digits as the original stress payload, now with intentional
// paragraph boundaries, one and two blank lines, and a trailing Enter.
fn sticky_authored_row(index: usize) -> String {
    // Unique line identities catch reordering or substitution, while keeping
    // the same 40-digit width and total 240-digit pressure payload.
    format!("{:02}{}", index+1, &STICKY_MARKER.repeat(2)[2..])
}
fn sticky_authored_body() -> String {
    let mut body = String::new();
    for (index, gap) in [1, 2, 3, 1, 1, 1].into_iter().enumerate() {
        body.push_str(&sticky_authored_row(index));
        body.push_str(&"\n".repeat(gap));
    }
    body
}

// Check the original real input, not a replacement or pasted fixture.
fn verify_sticky_authored_input(evidence: &Path, xml: &str) -> Result<(), String> {
    let expected = sticky_authored_body();
    let actual = elements(xml).into_iter()
        .filter(|n| n.class_name.ends_with("EditText"))
        .map(|n| n.label.replace("&#10;", "\n").replace("&#xA;", "\n")
            .replace("&#xa;", "\n"))
        .find(|text| text.contains(&sticky_authored_row(0)));
    let observed = actual.as_deref().unwrap_or("");
    fs::write(evidence.join("authored_input.xml"), xml).map_err(|e|e.to_string())?;
    fs::write(evidence.join("authored_input_comparison.txt"),
        format!("expected_bytes={} observed_bytes={}\nexpected={expected:?}\nobserved={observed:?}\n",
            expected.len(), observed.len())).map_err(|e|e.to_string())?;
    if observed != expected {
        capture(evidence,"authored_input_failure");
        return Err(format!("Sticky input integrity failed before version creation: expected {} bytes, observed {} bytes; no repair or version timing attempted",
            expected.len(), observed.len()));
    }
    Ok(())
}

// Long authored notes can leave the LazyColumn scrolled when the IME closes.
// The version action may be visible while its count is above the viewport.
// Reveal the real count before starting the latency clock; never substitute
// a database-only receipt or relax the eight-second operation deadline.
fn reveal_sticky_version_bar(expected: usize) -> Result<String, String> {
    let label = if expected == 1 { "第 1 版".to_owned() }
        else { format!("版本 {expected} / {expected}") };
    for attempt in 0..4 {
        let xml = dump()?;
        if xml.contains(&label) && xml.contains("下一个版本") { return Ok(xml); }
        if attempt < 3 {
            cmd(&["shell","input","swipe","535","760","535","1560","350"])?;
            sleep(Duration::from_millis(250));
        }
    }
    Err(format!("Could not reveal actual version header: {label}"))
}

fn sticky_stack(expected: usize) -> Result<bool, String> {
    // Numeric fixture, so literal LF is the only SQL escaping needed; hex
    // comparison avoids shell newline or JSON escaping ambiguities.
    let authored_hex = sticky_authored_body().as_bytes().iter()
        .map(|b| format!("{b:02X}")).collect::<String>();
    let result = live_sqlite(&format!(
        "SELECT count(*) FROM current_snapshot s, json_each(s.app_data_json,'$.notes') n \
         WHERE s.workspace_key='guest' AND json_extract(n.value,'$.title')='{STICKY_TITLE}' \
         AND json_extract(n.value,'$.kind')='STICKY' \
         AND hex(json_extract(n.value,'$.document.blocks[0].text'))='{authored_hex}' \
         AND json_array_length(n.value,'$.versions')={expected} \
         AND (SELECT count(DISTINCT json_extract(v.value,'$.id')) FROM json_each(n.value,'$.versions') v)={expected} \
         AND (SELECT count(*) FROM json_each(n.value,'$.versions') v \
           WHERE json_extract(v.value,'$.isLatest')=1 AND json_extract(v.value,'$.id')=json_extract(n.value,'$.latestVersionId'))=1 \
         AND (SELECT count(*) FROM json_each(n.value,'$.versions') v \
           WHERE hex(json_extract(v.value,'$.document.blocks[0].text'))='{authored_hex}')={expected};"
    ))?;
    Ok(result.trim()=="1")
}

fn sticky_export_newlines(evidence: &Path) -> Result<String, String> {
    // Actual Android share action, not a synthetic bitmap.
    const CACHE: &str = "/data/user/0/com.ofairyo.gridtimer/cache/note_shares";
    let listing = format!("ls -1t {CACHE}/note_share_*.png 2>/dev/null || true");
    let previous = cmd(&["shell",&listing])?;
    if !tap_any(&["更多","More"])? || !tap_any(&["分享","Share"])? ||
        !tap_any(&["分享长图","Share long image"])? {
        capture(evidence,"sticky_share_menu_missing");
        return Err("Could not reach real sticky share image action".into());
    }
    let png = evidence.join("sticky_authored_newlines.png");
    let mut complete = false;
    for _ in 0..40 {
        let current = cmd(&["shell",&listing])?;
        if let Some(path) = current.lines().find(|path| !previous.lines().any(|old| old==*path)) {
            let image = adb(&["exec-out","cat",path])?;
            if image.status.success() && image.stdout.starts_with(b"\x89PNG\r\n\x1a\n") &&
                image.stdout.ends_with(b"IEND\xaeB\x60\x82") {
                fs::write(&png,image.stdout).map_err(|e|e.to_string())?;
                complete=true;
                break;
            }
        }
        sleep(Duration::from_millis(500));
    }
    if !complete { return Err("Sticky share PNG did not complete".into()); }
    capture(evidence,"sticky_share_sheet");
    let image=Command::new("convert").arg(&png)
        .args(["-colorspace","Gray","-depth","8","pgm:-"]).output().map_err(|e|e.to_string())?;
    let mut fields=image.stdout.splitn(4,|&byte|byte==b'\n');
    let header=fields.next();
    let width:usize=fields.next().and_then(|v|std::str::from_utf8(v).ok()?.split_whitespace().next()?.parse().ok())
        .ok_or("Sticky PNG width unavailable")?;
    let depth=fields.next();
    let pixels=fields.next().unwrap_or_default();
    if !image.status.success() || header!=Some(b"P5") || depth!=Some(b"255") ||
        width<20 || pixels.is_empty() || pixels.len()%width!=0 {
        return Err("Could not decode sticky share PNG".into());
    }
    let mut bands:Vec<(usize,usize)>=Vec::new();
    for (y,row) in pixels.chunks_exact(width).enumerate() {
        let content=&row[width/20..width*19/20];
        if content.iter().filter(|&&v|v<160).take(8).count()<8 { continue; }
        match bands.last_mut() {
            Some((_,bottom)) if y<=*bottom+3=>*bottom=y,
            _=>bands.push((y,y)),
        }
    }
    // Title and timestamp precede the six equal-width numeric rows.
    let rows=bands.get(2..8).ok_or_else(||format!("Missing authored rows in PNG: {bands:?}"))?;
    let gaps:Vec<usize>=rows.windows(2).map(|w|w[1].0-w[0].0).collect();
    let ordinary=gaps[0];
    if ordinary==0 || gaps[1]*10<ordinary*17 || gaps[1]*10>ordinary*23 ||
        gaps[2]*10<ordinary*27 || gaps[2]*10>ordinary*33 ||
        gaps[3]*10<ordinary*9 || gaps[3]*10>ordinary*11 ||
        gaps[4]*10<ordinary*9 || gaps[4]*10>ordinary*11 {
        return Err(format!("Sticky exported lines or blank paragraphs were collapsed: {gaps:?}; bands={bands:?}"));
    }
    fs::write(evidence.join("sticky_share_line_gaps.txt"),format!("{gaps:?}\n")).map_err(|e|e.to_string())?;
    Ok(format!("PASS: real sticky PNG preserves six authored rows and blank-line spacing, measured row gaps {gaps:?} px after durable version creation and restart."))
}

// A titled sticky note containing only Enter presses is a real persisted
// note, not an empty abandoned draft. Verify both versions byte-for-byte.
fn sticky_blank_body(root: &Path) -> Result<(), String> {
    const TITLE: &str = "7391804025006011";
    let evidence = root.join("sticky_blank_body");
    fs::create_dir_all(&evidence).map_err(|e|e.to_string())?;
    cmd(&["shell","am","start","-W","-n",ACTIVITY])?;
    sleep(Duration::from_secs(7));
    sticky_home()?;
    if !tap_any(&["新建","New"])? { return Err("Blank-body sticky create action missing".into()); }
    let mut fields: Vec<Node> = elements(&dump()?).into_iter().filter(|n|
        n.class_name.ends_with("EditText") && n.bounds[2]>n.bounds[0] && n.bounds[3]>n.bounds[1]).collect();
    fields.sort_by_key(|n|n.bounds[1]);
    if fields.len()<2 { capture(&evidence,"missing_editor_fields"); return Err("Blank-body editor not available".into()); }
    tap(&fields[0])?;
    cmd(&["shell","input","text",TITLE])?;
    let body = elements(&dump()?).into_iter().filter(|n|n.class_name.ends_with("EditText")
        && n.bounds[2]>n.bounds[0] && n.bounds[3]>n.bounds[1]).max_by_key(|n|n.bounds[1])
        .ok_or("Blank-body sticky body unavailable")?;
    tap(&body)?;
    for _ in 0..3 { cmd(&["shell","input","keyevent","KEYCODE_ENTER"])?; }
    cmd(&["shell","input","keyevent","4"])?;
    sleep(Duration::from_millis(600));
    let action = find(&["下一个版本","Next version"])?.ok_or("Blank-body version create action unavailable")?;
    let query = format!(
        "SELECT count(*) FROM current_snapshot s, json_each(s.app_data_json,'$.notes') n \
         WHERE s.workspace_key='guest' AND json_extract(n.value,'$.title')='{TITLE}' \
         AND hex(json_extract(n.value,'$.document.blocks[0].text'))='0A0A0A' \
         AND json_array_length(n.value,'$.versions')=2 \
         AND (SELECT count(*) FROM json_each(n.value,'$.versions') v \
           WHERE hex(json_extract(v.value,'$.document.blocks[0].text'))='0A0A0A')=2;"
    );
    let started = std::time::Instant::now();
    send_tap(&action)?;
    let mut confirmed = false;
    while started.elapsed().as_millis() <= STICKY_BUDGET_MS {
        let ui = dump()?;
        if ui.contains("版本 2 / 2") && !ui.contains("正在创建新版本") && live_sqlite(&query)?.trim()=="1" {
            confirmed = true;
            break;
        }
        sleep(Duration::from_millis(150));
    }
    if !confirmed || started.elapsed().as_millis()>STICKY_BUDGET_MS {
        capture(&evidence,"blank_body_failure");
        return Err("Three blank authored lines did not survive version creation within budget".into());
    }
    capture(&evidence,"blank_body_committed");
    cmd(&["shell","am","force-stop",PACKAGE])?;
    if stopped_sqlite(&query)?.trim()!="1" { return Err("Blank-only body lost after force-stop".into()); }
    cmd(&["shell","am","start","-W","-n",ACTIVITY])?;
    sleep(Duration::from_secs(7));
    sticky_home()?;
    if live_sqlite(&query)?.trim()!="1" { return Err("Blank-only body lost after restart".into()); }
    fs::write(evidence.join("result.txt"), "PASS: three Enter presses retained in both versions after force-stop and restart.\n").map_err(|e|e.to_string())?;
    Ok(())
}

fn sticky_version_pressure(root: &Path, rounds: usize) -> Result<String, String> {
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
    for (index, gap) in [1, 2, 3, 1, 1, 1].into_iter().enumerate() {
        cmd(&["shell","input","text",&sticky_authored_row(index)])?;
        for _ in 0..gap {
            cmd(&["shell","input","keyevent","KEYCODE_ENTER"])?;
        }
        println!("Typed sticky authored row {}", index+1);
    }
    cmd(&["shell","input","keyevent","4"])?;
    sleep(Duration::from_millis(600));
    // Validate input separately from persistence. Preserve the same rapid
    // input sequence and fail rather than retrying or repairing lost digits.
    let authored = reveal_sticky_version_bar(1)?;
    verify_sticky_authored_input(&evidence, &authored)?;
    // The real version panel is the same control shown in the reported screenshot.
    let mut metrics = String::new();
    for round in 1..=rounds {
        if let Err(error) = reveal_sticky_version_bar(round) {
            capture(&evidence,"version_header_not_visible");
            return Err(error);
        }
        let action = find(&["下一个版本","Next version"])?.ok_or("Sticky version create action unavailable")?;
        let writer = if round%4==0 { Some(hold_sqlite_writer()?) } else { None };
        let started = std::time::Instant::now();
        send_tap(&action)?;
        let expected=round+1;
        let label=format!("版本 {expected} / {expected}");
        let mut confirmed=false;
        while started.elapsed().as_millis()<=STICKY_BUDGET_MS {
            let xml=dump()?;
            let durable = sticky_stack(expected)?;
            let visible = xml.contains(&label) && !xml.contains("正在创建新版本");
            println!("Sticky version {expected}: elapsed={}ms visible={visible} durable={durable}", started.elapsed().as_millis());
            if visible && durable {
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
    if !sticky_stack(rounds+1)? { return Err("Sticky version stack lost or duplicated after force-stop".into()); }
    cmd(&["shell","am","start","-W","-n",ACTIVITY])?;
    sleep(Duration::from_secs(7));
    sticky_home()?;
    if !tap_any(&[STICKY_TITLE])? { return Err("Persisted sticky note not visible after restart".into()); }
    let expected=rounds+1;
    let xml=reveal_sticky_version_bar(expected)?;
    if !xml.contains(&format!("版本 {expected} / {expected}")) || !sticky_stack(expected)? {
        capture(&evidence,"restart_failure");
        return Err("Sticky version stack did not reopen intact".into());
    }
    capture(&evidence,"reopened_versions");
    let share_report = sticky_export_newlines(&evidence)?;
    println!("{share_report}");
    Ok(format!("Sticky: {rounds} real UI version creates including {} competing SQLite writers, each <= {STICKY_BUDGET_MS}ms with unique durable IDs, exact latest pointer and retained marker in every version; force-stop and restart verified. {share_report}", rounds / 4))
}

// Exercise the real WebView commit bridge after TXT -> RTF conversion,
// rather than asserting only the Rust-generated HTML or the displayed label.
fn sticky_rich_roundtrip(root: &Path, prior_versions: usize) -> Result<String, String> {
    let evidence = root.join("sticky_rich_roundtrip");
    fs::create_dir_all(&evidence).map_err(|e|e.to_string())?;
    // Dismiss the real share sheet before killing its originating activity.
    let foreground = dump()?;
    if foreground.contains("package=\"com.android.intentresolver\"") {
        cmd(&["shell","input","keyevent","4"])?;
        sleep(Duration::from_millis(700));
    }
    cmd(&["shell","am","force-stop",PACKAGE])?;
    cmd(&["shell","am","start","-W","-n",ACTIVITY])?;
    sleep(Duration::from_secs(7));
    sticky_home()?;
    if !tap_any(&[STICKY_TITLE])? { return Err("Sticky note missing before RTF roundtrip".into()); }
    if !tap_any(&["更多","More"])? || !tap_any(&["切到富文本"])? {
        capture(&evidence,"rich_mode_missing");
        return Err("Could not switch real sticky editor to RTF".into());
    }
    sleep(Duration::from_millis(900));
    let xml = dump()?;
    if !xml.contains("RTF") || !xml.contains("android.webkit.WebView") {
        capture(&evidence,"rich_editor_missing");
        return Err("RTF editor did not become active".into());
    }
    let expected = prior_versions + 1;
    let authored_hex = sticky_authored_body().as_bytes().iter()
        .map(|b|format!("{b:02X}")).collect::<String>();
    // The prior latest becomes RTF when the editor commits before creating the
    // next version. All earlier historical snapshots must retain exact text.
    let query = format!(
        "SELECT count(*) FROM current_snapshot s, json_each(s.app_data_json,'$.notes') n \
         WHERE s.workspace_key='guest' AND json_extract(n.value,'$.title')='{STICKY_TITLE}' \
         AND json_extract(n.value,'$.document.richTextEnabled')=1 \
         AND hex(json_extract(n.value,'$.document.richTextPlainText'))='{authored_hex}' \
         AND json_array_length(n.value,'$.versions')={expected} \
         AND (SELECT count(DISTINCT json_extract(v.value,'$.id')) FROM json_each(n.value,'$.versions') v)={expected} \
         AND (SELECT count(*) FROM json_each(n.value,'$.versions') v WHERE \
           hex(CASE WHEN json_extract(v.value,'$.document.richTextEnabled')=1 \
           THEN json_extract(v.value,'$.document.richTextPlainText') ELSE json_extract(v.value,'$.document.blocks[0].text') END)='{authored_hex}')={expected} \
         AND (SELECT count(*) FROM json_each(n.value,'$.versions') v WHERE json_extract(v.value,'$.isLatest')=1 \
           AND json_extract(v.value,'$.id')=json_extract(n.value,'$.latestVersionId') \
           AND json_extract(v.value,'$.document.richTextEnabled')=1)=1;"
    );
    reveal_sticky_version_bar(prior_versions)?;
    let action = find(&["下一个版本","Next version"])?.ok_or("RTF version create action missing")?;
    let started = std::time::Instant::now();
    send_tap(&action)?;
    let mut confirmed = false;
    while started.elapsed().as_millis() <= STICKY_BUDGET_MS {
        let ui = dump()?;
        let durable = live_sqlite(&query)?.trim()=="1";
        let visible = ui.contains(&format!("版本 {expected} / {expected}")) && !ui.contains("正在创建新版本");
        println!("RTF version {expected}: elapsed={}ms visible={visible} durable={durable}", started.elapsed().as_millis());
        if visible && durable {
            confirmed = true;
            break;
        }
        sleep(Duration::from_millis(150));
    }
    let elapsed = started.elapsed().as_millis();
    fs::write(evidence.join("version_elapsed_ms.txt"),format!("{elapsed}\n")).map_err(|e|e.to_string())?;
    if !confirmed || elapsed>STICKY_BUDGET_MS {
        capture(&evidence,"rtf_version_failure");
        return Err(format!("RTF version commit changed authored lines or exceeded budget: {elapsed}ms"));
    }
    capture(&evidence,"rich_committed");
    cmd(&["shell","am","force-stop",PACKAGE])?;
    if stopped_sqlite(&query)?.trim()!="1" { return Err("RTF body or historical versions changed after force-stop".into()); }
    cmd(&["shell","am","start","-W","-n",ACTIVITY])?;
    sleep(Duration::from_secs(7));
    sticky_home()?;
    if !tap_any(&[STICKY_TITLE])? { return Err("RTF note missing after restart".into()); }
    let xml = reveal_sticky_version_bar(expected)?;
    if !xml.contains("RTF") || !xml.contains(&format!("版本 {expected} / {expected}"))
        || live_sqlite(&query)?.trim()!="1" {
        capture(&evidence,"rtf_restart_failure");
        return Err("RTF note did not reopen with exact original line boundaries".into());
    }
    capture(&evidence,"rich_reopened");
    let report = sticky_export_newlines(&evidence)?;
    Ok(format!("PASS: RTF WebView version creation and restart retained every authored line in {expected} versions, {elapsed}ms. {report}"))
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
    println!("{HISTORY_ROWS} historical rows fixture preparation plus startup navigation: {elapsed} ms; ready={}", ready.is_ok());
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
        if prove_directory(&current, Some(NESTED_FOLDER)) {
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
    require_stopped_app()?;
    let quarantined_history = stopped_sqlite(&format!(
        "SELECT COUNT(*) FROM snapshot_quarantine WHERE workspace_key='guest' \
         AND source_table='snapshot_history' AND source_row_id='{unchecked_history}' \
         AND stored_sha256='' AND reason='sha256_mismatch' AND length(app_data_json)>0;"
    ))?;
    if quarantined_history != "1" {
        return Err("Startup recovery did not preserve the unchecked history row in quarantine".into());
    }
    let packed_after = packed_legacy_rows()?;
    if packed_after <= packed_before {
        return Err(format!("Legacy authentication did not resume after restart: {packed_before} to {packed_after} packed rows"));
    }
    let sticky_report = sticky_version_pressure(root, STICKY_ROUNDS)?;
    println!("{sticky_report}");
    Ok(format!(
        "PASS: Android {version} upgraded a v5 database with {durable_history} retained SQLite snapshots. With an unchecked historical row inserted after startup, click-to-visible save took {saved_millis} ms with {scope}, and the row remained in history until startup recovery moved it to quarantine. With a competing SQLite writer and a read-only mirror directory, save took {contended_millis} ms. Both pages were durable after force-stop while the JSON mirror was still stale. Root folder, nested folder and nested page took {root_folder_ms}/{nested_folder_ms}/{nested_page_ms} ms and survived with exactly one node each and the correct parent IDs. Nested page passed {EDIT_ROUNDS} real body edits (including independent writer contention and actual background/resume), each with UI and SQLite receipt within {EDIT_BUDGET_MS} ms; {BURST_ROUNDS} additional rapid input events followed by immediate final edit/Back and all body markers survived process death. Restart repaired the mirror and reopened the first page. Background authentication resumed from {packed_before} to {packed_after} packed legacy rows, preserving all {HISTORY_ROWS} original unsealed histories. {sticky_report}"
    ))
}
fn main() {
    let path = env::args_os().nth(1).map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("knowledge_save_evidence"));
    fs::create_dir_all(&path).expect("evidence directory");
    let workload = format!("profile={}\nhistory_rows={HISTORY_ROWS}\nhistory_padding_bytes={HISTORY_PADDING_BYTES}\nedit_rounds={EDIT_ROUNDS}\nburst_rounds={BURST_ROUNDS}\nsticky_rounds={STICKY_ROUNDS}\nsave_budget_ms={SAVE_BUDGET_MS}\nrecovery_budget_seconds={RECOVERY_BUDGET_SECONDS}\n", ACTIVE.name);
    fs::write(path.join("stress_profile.txt"), &workload).expect("workload evidence");
    println!("{workload}");
    let result = if env::args().nth(2).as_deref() == Some("--sticky-newlines") {
        // Standalone signed APK regression on clean state, before heavy load.
        // Still exercises real editing, four version commits (one competing
        // writer), exact SQLite bytes after process death and exported pixels.
        cmd(&["wait-for-device"])
            .and_then(|_| cmd(&["shell", "dumpsys", "package", PACKAGE]))
            .and_then(|info| installed_version(&info).map(|_| ()))
            .and_then(|_| cmd(&["root"]))
            .and_then(|_| cmd(&["wait-for-device"]))
            .and_then(|_| sticky_blank_body(&path))
            .and_then(|_| sticky_version_pressure(&path, 4))
            .and_then(|plain| sticky_rich_roundtrip(&path, 5).map(|rich| format!("{plain}\n{rich}")))
    } else {
        run(&path)
    };
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
