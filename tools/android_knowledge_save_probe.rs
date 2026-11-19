//! Android API 35 signed-APK acceptance: new knowledge page must persist after restart.
//! The driver runs only against an installed signed APK, never a mocked UI.
use std::{env, fs, path::{Path, PathBuf}, process::{Command, Output}, thread::sleep, time::Duration};

const PACKAGE: &str = "com.ofairyo.gridtimer";
const ACTIVITY: &str = "com.ofairyo.gridtimer/.MainActivity";
const VERSION: &str = "3.0.2.2";
const CREATED: &str = "KnowledgeSaveProbe";

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
fn create_page(root: &Path) -> Result<u128, String> {
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
    cmd(&["shell", "input", "text", CREATED])?;
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
        if current.contains(CREATED) && !current.contains("正在保存") &&
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

fn persisted_after_restart(root: &Path) -> Result<(), String> {
    cmd(&["shell", "am", "force-stop", PACKAGE])?;
    cmd(&["shell", "am", "start", "-W", "-n", ACTIVITY])?;
    sleep(Duration::from_secs(7));
    directory(root)?;
    for attempt in 0..18 {
        if find(&[CREATED])?.is_some() {
            capture(root, "07_persisted_after_restart");
            return Ok(());
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

fn run(root: &Path) -> Result<String, String> {
    cmd(&["wait-for-device"])?;
    let info = cmd(&["shell", "dumpsys", "package", PACKAGE])?;
    if !info.contains(&format!("versionName={VERSION}")) {
        return Err(format!("Installed package is not signed candidate {VERSION}"));
    }
    cmd(&["shell", "am", "start", "-W", "-n", ACTIVITY])?;
    sleep(Duration::from_secs(7));
    directory(root)?;
    let saved_millis = create_page(root)?;
    persisted_after_restart(root)?;
    Ok(format!(
        "PASS: Android {VERSION} created the knowledge page and confirmed all saving-stage indicators cleared after {saved_millis} ms, then force-stopped and restarted with the page still durable."
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
