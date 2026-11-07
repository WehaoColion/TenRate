//! Emulator smoke test: launch signed app, exclude the knowledge root,
//! force-stop/reopen, verify durable exclusion, and restore normal participation.
use std::{
    env, fs,
    path::{Path, PathBuf},
    process::{Command, Output},
    thread::sleep,
    time::Duration,
};

const ACTIVITY: &str = "com.ofairyo.gridtimer/.MainActivity";
const PACKAGE: &str = "com.ofairyo.gridtimer";

#[derive(Debug)]
struct Node {
    text: String,
    description: String,
    bounds: [i32; 4],
}

fn adb(args: &[&str]) -> Result<Output, String> {
    Command::new("adb").args(args).output().map_err(|error| error.to_string())
}

fn command(args: &[&str]) -> Result<String, String> {
    let result = adb(args)?;
    if !result.status.success() {
        return Err(format!("adb {args:?}: {}", String::from_utf8_lossy(&result.stderr)));
    }
    Ok(String::from_utf8_lossy(&result.stdout).into_owned())
}

fn attr(node: &str, key: &str) -> String {
    let prefix = format!("{key}=\"");
    node.find(&prefix).and_then(|start| {
        let tail = &node[start + prefix.len()..];
        tail.find('"').map(|end| tail[..end].to_string())
    }).unwrap_or_default()
}

fn nodes(xml: &str) -> Vec<Node> {
    xml.split("<node ").skip(1).filter_map(|record| {
        let head = record.split('>').next()?;
        let bounds: Vec<i32> = attr(head, "bounds")
            .split(|part: char| !part.is_ascii_digit() && part != '-')
            .filter(|part| !part.is_empty())
            .filter_map(|part| part.parse().ok()).collect();
        if bounds.len() != 4 { return None; }
        Some(Node {
            text: attr(head, "text"),
            description: attr(head, "content-desc"),
            bounds: [bounds[0], bounds[1], bounds[2], bounds[3]],
        })
    }).collect()
}

fn hierarchy() -> Result<String, String> {
    for _ in 0..4 {
        let _ = command(&["shell", "uiautomator", "dump", "/sdcard/legal_exclusion_ui.xml"]);
        if let Ok(xml) = command(&["exec-out", "cat", "/sdcard/legal_exclusion_ui.xml"]) {
            if xml.contains("<hierarchy") { return Ok(xml); }
        }
        sleep(Duration::from_millis(600));
    }
    Err("Unable to dump emulator accessibility hierarchy".into())
}

fn save_evidence(path: &Path, label: &str) {
    if let Ok(xml) = hierarchy() {
        let _ = fs::write(path.join(format!("{label}.xml")), xml);
    }
    if let Ok(image) = adb(&["exec-out", "screencap", "-p"]) {
        if image.status.success() && image.stdout.starts_with(b"\x89PNG") {
            let _ = fs::write(path.join(format!("{label}.png")), image.stdout);
        }
    }
}

fn tap_label(labels: &[&str]) -> Result<bool, String> {
    let xml = hierarchy()?;
    for label in labels {
        if let Some(node) = nodes(&xml).into_iter().find(|node| {
            node.bounds[2] > node.bounds[0] && node.bounds[3] > node.bounds[1] &&
                (node.text.contains(label) || node.description.contains(label))
        }) {
            command(&["shell", "input", "tap",
                &((node.bounds[0] + node.bounds[2]) / 2).to_string(),
                &((node.bounds[1] + node.bounds[3]) / 2).to_string()])?;
            sleep(Duration::from_millis(850));
            return Ok(true);
        }
    }
    Ok(false)
}

fn clear_system_overlays() -> Result<(), String> {
    for _ in 0..5 {
        let xml = hierarchy()?;
        if xml.contains("Pixel Launcher isn't responding") ||
            xml.contains("Pixel Launcher isn&#39;t responding")
        {
            if !tap_label(&["Close app"])? { break; }
            continue;
        }
        if xml.contains("Allow tenfold to send you notifications?") {
            if !tap_label(&["Allow"])? { break; }
            continue;
        }
        return Ok(());
    }
    Ok(())
}

fn open_knowledge_root(evidence: &Path) -> Result<(), String> {
    for attempt in 0..8 {
        clear_system_overlays()?;
        let xml = hierarchy()?;
        if xml.contains("排除整个知识模块") || xml.contains("恢复参与法律分析") {
            return Ok(());
        }
        save_evidence(evidence, &format!("navigation_{attempt}"));
        let _ = tap_label(&["知识目录", "页面与文件夹", "知识库", "知识", "Knowledge", "Notes", "目录"])?;
        sleep(Duration::from_millis(650));
    }
    Err("Could not open knowledge root from signed app; inspect navigation screenshots".into())
}

fn run(root: &Path) -> Result<(), String> {
    command(&["wait-for-device"])?;
    let version = command(&["shell", "dumpsys", "package", PACKAGE])?;
    if !version.contains("versionName=2.23.4.13") {
        return Err("Installed package is not the candidate 2.23.4.13".into());
    }
    command(&["shell", "am", "start", "-n", ACTIVITY])?;
    sleep(Duration::from_secs(6));
    save_evidence(root, "01_app_started");
    open_knowledge_root(root)?;
    save_evidence(root, "02_knowledge_root");
    if !tap_label(&["排除整个知识模块"])? {
        return Err("Root exclusion action missing".into());
    }
    save_evidence(root, "03_confirmation");
    if !tap_label(&["确认排除"])? {
        return Err("User confirmation action missing".into());
    }
    save_evidence(root, "04_excluded");
    if !hierarchy()?.contains("恢复参与法律分析") {
        return Err("Knowledge root was not marked excluded".into());
    }
    command(&["shell", "am", "force-stop", PACKAGE])?;
    command(&["shell", "am", "start", "-n", ACTIVITY])?;
    sleep(Duration::from_secs(5));
    open_knowledge_root(root)?;
    save_evidence(root, "05_after_restart");
    if !hierarchy()?.contains("恢复参与法律分析") {
        return Err("Exclusion was lost after Android process restart".into());
    }
    if !tap_label(&["恢复参与法律分析"])? {
        return Err("Restore participation action is missing".into());
    }
    if !tap_label(&["确认恢复"])? {
        return Err("Restore confirmation is missing".into());
    }
    save_evidence(root, "06_restored");
    if !hierarchy()?.contains("排除整个知识模块") {
        return Err("Restored knowledge did not rejoin legal analysis scope".into());
    }
    Ok(())
}

fn main() {
    let root = env::args_os().nth(1).map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("legal_exclusion_emulator_evidence"));
    if let Err(error) = fs::create_dir_all(&root) {
        eprintln!("Cannot create evidence directory: {error}");
        std::process::exit(2);
    }
    let result = run(&root);
    save_evidence(&root, "07_final");
    let message = match &result {
        Ok(()) => "PASS: signed APK launched; root deny/confirm; restart retained deny; restore confirmed.",
        Err(_) => "FAIL: emulator acceptance is not established. Review evidence and job logs.",
    };
    let details = format!("# Android legal knowledge exclusion emulator acceptance\n\n{message}\n\n{}\n",
        result.as_ref().err().map(String::as_str).unwrap_or(""));
    let _ = fs::write(root.join("report.md"), &details);
    println!("{details}");
    if result.is_err() { std::process::exit(1); }
}
