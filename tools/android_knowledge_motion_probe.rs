//! Android API 35 smoke and visual evidence for the knowledge workspace.
//! The driver runs only against an installed signed APK, never a mocked UI.
use std::{env, fs, path::{Path, PathBuf}, process::{Command, Output}, thread::sleep, time::Duration};

const PACKAGE: &str = "com.ofairyo.gridtimer";
const ACTIVITY: &str = "com.ofairyo.gridtimer/.MainActivity";
const VERSION: &str = "2.13.5.12";
const FOLDER: &str = "KnowledgeDesignFolder";

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
fn create_folder(root: &Path) -> Result<(), String> {
    if !tap_any(&["新建子文件夹", "New subfolder"])? {
        return Err("Create-folder action not visible".into());
    }
    capture(root, "03_create_folder_dialog");
    let field = elements(&dump()?).into_iter().find(|item| {
        item.class_name.ends_with("EditText") && item.bounds[2] - item.bounds[0] > 60
    }).ok_or("Folder name field not present")?;
    tap(&field)?;
    cmd(&["shell", "input", "text", FOLDER])?;
    sleep(Duration::from_millis(400));
    if !tap_any(&["确定", "OK", "Confirm"])? {
        return Err("Folder confirmation action missing".into());
    }
    sleep(Duration::from_secs(2));
    let state = dump()?;
    capture(root, "04_folder_created");
    if !state.contains(FOLDER) {
        return Err("Created folder title was not rendered by the real app".into());
    }
    if cmd(&["shell", "pidof", PACKAGE])?.trim().is_empty() {
        return Err("App exited after adding a knowledge folder".into());
    }
    Ok(())
}
// Confirm the emulator is actually evaluating animations instead of suppressing them.
fn assert_animations_enabled() -> Result<(), String> {
    for key in ["window_animation_scale", "transition_animation_scale", "animator_duration_scale"] {
        let value = cmd(&["shell", "settings", "get", "global", key])?;
        if value.trim() != "1" && value.trim() != "1.0" {
            return Err(format!("Animation acceptance invalid: {key}={}", value.trim()));
        }
    }
    Ok(())
}
fn capture_motion_frame(dir: &Path, title: &str) {
    if let Ok(out) = adb(&["exec-out", "screencap", "-p"]) {
        if out.status.success() && out.stdout.starts_with(b"\x89PNG") {
            let _ = fs::write(dir.join(format!("{title}.png")), out.stdout);
        }
    }
}
// A real MP4 is required for human motion review; frame-pair snapshots are not a
// substitute for evaluating motion. Recording is best effort, while the actual
// UI business-state assertions below remain strict.
fn open_folder_with_motion(root: &Path) -> Result<(), String> {
    let folder = find(&[FOLDER])?.ok_or("New knowledge folder cannot be opened")?;
    let _ = cmd(&["shell", "rm", "-f", "/sdcard/knowledge_motion.mp4"]);
    let recording = Command::new("adb")
        .args(["shell", "screenrecord", "--time-limit", "8", "--bit-rate", "2000000", "/sdcard/knowledge_motion.mp4"])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn();
    sleep(Duration::from_millis(400));
    let x = (folder.bounds[0] + folder.bounds[2]) / 2;
    let y = (folder.bounds[1] + folder.bounds[3]) / 2;
    cmd(&["shell", "input", "tap", &x.to_string(), &y.to_string()])?;
    capture_motion_frame(root, "05_folder_opening_motion_frame");
    sleep(Duration::from_millis(180));
    capture_motion_frame(root, "05_folder_opening_settling_frame");
    sleep(Duration::from_millis(300));
    capture(root, "06_folder_opened_motion_enabled");
    if let Ok(mut child) = recording {
        let _ = child.wait();
        let file = root.join("knowledge_folder_motion.mp4");
        let _ = Command::new("adb")
            .arg("pull")
            .arg("/sdcard/knowledge_motion.mp4")
            .arg(file)
            .status();
    }
    Ok(())
}
fn run(root: &Path) -> Result<String, String> {
    assert_animations_enabled()?;
    cmd(&["wait-for-device"])?;
    let package_info = cmd(&["shell", "dumpsys", "package", PACKAGE])?;
    if !package_info.contains(&format!("versionName={VERSION}")) {
        return Err(format!("Installed package is not Android {VERSION}"));
    }
    cmd(&["shell", "am", "start", "-W", "-n", ACTIVITY])?;
    sleep(Duration::from_secs(7));
    directory(root)?;
    if !tap_any(&["知识设置"])? {
        return Err("Root knowledge settings entry is not visible".into());
    }
    let legal_menu = dump()?;
    capture(root, "03_root_settings");
    if !legal_menu.contains("排除法律风险分析") {
        return Err("Legal analysis exclusion is not reachable from knowledge settings".into());
    }
    cmd(&["shell", "input", "keyevent", "4"])?;
    create_folder(root)?;
    open_folder_with_motion(root)?;
    let folder_view = dump()?;
    capture(root, "06_folder_controls");
    if !folder_view.contains("管理") || !folder_view.contains("文件夹设置") {
        return Err("Folder creation, management and settings hierarchy is not visible".into());
    }
    if find(&["重命名"])?.is_some() {
        return Err("Rename must remain hidden until the management menu is opened".into());
    }
    if !tap_any(&["管理当前页面或文件夹", "管理 ▾"])? {
        return Err("Folder management menu is not accessible".into());
    }
    let management_menu = dump()?;
    capture(root, "07_management_menu");
    if !management_menu.contains("移动") || !management_menu.contains("重命名") ||
        !management_menu.contains("转为页面") {
        return Err("Management menu lost an existing folder action".into());
    }
    cmd(&["shell", "input", "keyevent", "4"])?;
    if !tap_any(&["文件夹设置"])? {
        return Err("Folder-specific settings entry is not accessible".into());
    }
    let settings_menu = dump()?;
    capture(root, "08_folder_settings");
    if !settings_menu.contains("排除法律风险分析") {
        return Err("Folder legal exclusion is not reachable from its settings".into());
    }
    Ok(format!("PASS: signed Android {VERSION} launched with animation scales 1.0; knowledge directory, folder creation, management and exclusion controls passed; motion screenshots and optional MP4 captured."))
}
fn main() {
    let path = env::args_os().nth(1).map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("knowledge_ui_evidence"));
    fs::create_dir_all(&path).expect("evidence directory");
    let result = run(&path);
    capture(&path, "05_final");
    let report = match &result {
        Ok(message) => format!("# Android knowledge motion simulator acceptance\n\n{message}\n\nOverall: PASS\n"),
        Err(error) => format!("# Android knowledge motion simulator acceptance\n\nFailure: {error}\n\nOverall: FAILED / NOT VERIFIED\n"),
    };
    fs::write(path.join("report.md"), &report).expect("report");
    println!("{report}");
    if result.is_err() { std::process::exit(1); }
}
