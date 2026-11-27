// v3.0.2.45 - Require fresh, complete accessibility evidence.
// v3.0.2.41 - Preserve independent motion gate and emit Android frame bottleneck counters.
// v3.0.2.33 - Share release identity and visible navigation evidence.
//! Android API 35 smoke and visual evidence for the knowledge workspace.
//! The driver runs only against an installed signed APK, never a mocked UI.
use std::{env, fs, path::{Path, PathBuf}, process::{Command, Output}, thread::sleep, time::Duration};

const PACKAGE: &str = "com.ofairyo.gridtimer";
const ACTIVITY: &str = "com.ofairyo.gridtimer/.MainActivity";
#[allow(dead_code)]
#[path = "../native/gridtimer_native/src/android_release_identity.rs"]
mod android_release_identity;
mod android_directory_evidence;
use android_release_identity::installed_version;
use android_directory_evidence::prove_directory;
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
// Record multiple navigation directions with animations enabled. The recording and
// actual Android frame timing report are required evidence, not optional screenshots.
fn open_folder_with_motion(root: &Path) -> Result<(), String> {
    // Creation automatically opens the folder; start the recording at its parent
    // so the first tap is a real navigation, not a tap on the current title.
    if prove_directory(&dump()?, Some(FOLDER)) {
        cmd(&["shell", "input", "keyevent", "4"])?;
        sleep(Duration::from_millis(800));
    }
    let folder = find(&[FOLDER])?.ok_or("New knowledge folder cannot be opened")?;
    let x = ((folder.bounds[0] + folder.bounds[2]) / 2).to_string();
    let y = ((folder.bounds[1] + folder.bounds[3]) / 2).to_string();
    cmd(&["shell", "rm", "-f", "/sdcard/knowledge_motion.mp4"])?;
    cmd(&["shell", "dumpsys", "gfxinfo", PACKAGE, "reset"])?;
    let mut recording = Command::new("adb")
        .args(["shell", "screenrecord", "--time-limit", "16", "--bit-rate", "6000000", "/sdcard/knowledge_motion.mp4"])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn().map_err(|e| e.to_string())?;
    sleep(Duration::from_millis(750));
    let result = (|| -> Result<(), String> {
        cmd(&["shell", "input", "tap", &x, &y])?;
        sleep(Duration::from_millis(800));
        if !prove_directory(&dump()?, Some(FOLDER)) {
            return Err("Forward directory transition failed".into());
        }
        capture_motion_frame(root, "05_folder_opened");
        cmd(&["shell", "input", "keyevent", "4"])?;
        sleep(Duration::from_millis(800));
        let returned = find(&[FOLDER])?.ok_or("Back navigation did not restore parent")?;
        let rx = ((returned.bounds[0] + returned.bounds[2]) / 2).to_string();
        let ry = ((returned.bounds[1] + returned.bounds[3]) / 2).to_string();
        cmd(&["shell", "input", "tap", &rx, &ry])?;
        sleep(Duration::from_millis(800));
        if !prove_directory(&dump()?, Some(FOLDER)) {
            return Err("Re-entry after back failed".into());
        }
        capture_motion_frame(root, "06_folder_reentered");
        Ok(())
    })();
    let frames = cmd(&["shell", "dumpsys", "gfxinfo", PACKAGE, "framestats"])?;
    fs::write(root.join("knowledge_motion_framestats.txt"), &frames).map_err(|e| e.to_string())?;
    let status = recording.wait().map_err(|e| e.to_string())?;
    if !status.success() { return Err("Android screenrecord failed".into()); }
    let file = root.join("knowledge_folder_motion.mp4");
    let local = file.to_str().ok_or("Invalid evidence path")?;
    cmd(&["pull", "/sdcard/knowledge_motion.mp4", local])?;
    if fs::metadata(&file).map_err(|e| e.to_string())?.len() < 100_000 {
        return Err("Motion recording is missing or too small".into());
    }
    let rendered = frames.lines().find_map(|line| line.trim().strip_prefix("Total frames rendered:")
        .and_then(|n| n.trim().parse::<u32>().ok())).unwrap_or(0);
    if rendered == 0 || !frames.contains("Janky frames:") {
        return Err("Android did not report usable frame timing".into());
    }
    fs::write(root.join("motion_metrics.txt"), frames.lines().filter(|line|
        line.contains("Total frames rendered:") || line.contains("Janky frames:") ||
        line.contains("percentile:")).collect::<Vec<_>>().join("\n")).map_err(|e| e.to_string())?;
    result?;
    // Separate performance from screen recording and accessibility dumps. Both
    // stress SwiftShader and can hide the real application animation behavior.
    cmd(&["shell", "input", "keyevent", "4"])?;
    sleep(Duration::from_millis(900));
    let row = find(&[FOLDER])?.ok_or("Folder missing from parent before timing")?;
    let rx = ((row.bounds[0] + row.bounds[2]) / 2).to_string();
    let ry = ((row.bounds[1] + row.bounds[3]) / 2).to_string();
    cmd(&["shell", "dumpsys", "gfxinfo", PACKAGE, "reset"])?;
    for _ in 0..6 {
        cmd(&["shell", "input", "tap", &rx, &ry])?;
        sleep(Duration::from_millis(900));
        cmd(&["shell", "input", "keyevent", "4"])?;
        sleep(Duration::from_millis(900));
    }
    cmd(&["shell", "input", "tap", &rx, &ry])?;
    sleep(Duration::from_millis(900));
    let timing = cmd(&["shell", "dumpsys", "gfxinfo", PACKAGE, "framestats"])?;
    fs::write(root.join("motion_unrecorded_framestats.txt"), &timing).map_err(|e| e.to_string())?;
    let metric = |prefix: &str| timing.lines().find_map(|line|
        line.trim().strip_prefix(prefix)?.trim().split(|c: char| !c.is_ascii_digit()).next()?.parse::<u32>().ok());
    let total = metric("Total frames rendered:").ok_or("Missing unrecorded frame count")?;
    let janky = metric("Janky frames:").ok_or("Missing unrecorded janky frames")?;
    let p90 = metric("90th percentile:").ok_or("Missing unrecorded p90")?;
    let summary = format!("Unrecorded navigation: {total} frames, {janky} janky, p90 {p90}ms");
    fs::write(root.join("motion_unrecorded_metrics.txt"), &summary).map_err(|e| e.to_string())?;
    println!("{summary}");
    // Keep the original gate; these independent Android counters distinguish
    // scheduling, input, UI thread, upload and draw pressure for the next fix.
    let counters = timing.lines().filter(|line| {
        let line = line.trim();
        ["Number Missed Vsync:", "Number High input latency:", "Number Slow UI thread:",
         "Number Slow bitmap uploads:", "Number Slow issue draw commands:",
         "Number Frame deadline missed:", "Number Frame deadline missed (legacy):"]
            .iter().any(|prefix| line.starts_with(prefix))
    }).collect::<Vec<_>>().join("\n");
    println!("Unrecorded frame bottlenecks:\n{counters}");
    fs::write(root.join("motion_unrecorded_bottlenecks.txt"), &counters).map_err(|e| e.to_string())?;
    if !prove_directory(&dump()?, Some(FOLDER)) { return Err("Repeated navigation did not return to the folder".into()); }
    // Conservative emulator gate; this is not a claim of production 60fps.
    // Do not pass a sparse or overwhelmingly janky sample.
    if total < 40 || janky.saturating_mul(100) > total.saturating_mul(25) || p90 > 50 {
        return Err(format!("Motion frame budget not met: {summary}"));
    }
    Ok(())
}
fn run(root: &Path) -> Result<String, String> {
    assert_animations_enabled()?;
    cmd(&["wait-for-device"])?;
    let package_info = cmd(&["shell", "dumpsys", "package", PACKAGE])?;
    let version = installed_version(&package_info)?;
    cmd(&["shell", "am", "start", "-W", "-n", ACTIVITY])?;
    sleep(Duration::from_secs(7));
    directory(root)?;
    create_folder(root)?;
    open_folder_with_motion(root)?;
    if !prove_directory(&dump()?, Some(FOLDER)) {
        return Err("Folder navigation did not leave the app in the opened folder".into());
    }
    Ok(format!("PASS: signed Android {version} launched with animation scales 1.0; directory discovery and folder creation passed; forward/back/re-entry passed; MP4 captured and independent unrecorded frame budget passed. Visual smoothness still requires reviewing evidence."))
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
