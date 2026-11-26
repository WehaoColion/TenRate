// v3.0.2.39 - Compact AI conversation layout, independent history and real emulator gate.
//! Real Android emulator smoke test for AI conversation entry and recovery controls.
//! Without a configured provider, this verifies the interface, not remote inference.
use std::{env, fs, path::{Path, PathBuf}, process::{Command, Output}, thread::sleep, time::Duration};

const PACKAGE: &str = "com.ofairyo.gridtimer";
const ACTIVITY: &str = "com.ofairyo.gridtimer/.MainActivity";
#[allow(dead_code)]
#[path = "../native/gridtimer_native/src/android_release_identity.rs"]
mod android_release_identity;
use android_release_identity::installed_version;

fn adb(args: &[&str]) -> Result<Output, String> {
    Command::new("adb").args(args).output().map_err(|e| e.to_string())
}
fn cmd(args: &[&str]) -> Result<String, String> {
    let output = adb(args)?;
    if !output.status.success() {
        return Err(format!("adb {:?}: {}", args, String::from_utf8_lossy(&output.stderr)));
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}
fn dump() -> Result<String, String> {
    for _ in 0..6 {
        let _ = cmd(&["shell", "uiautomator", "dump", "/sdcard/ai_chat.xml"]);
        if let Ok(xml) = cmd(&["exec-out", "cat", "/sdcard/ai_chat.xml"]) {
            if xml.contains("<hierarchy") { return Ok(xml); }
        }
        sleep(Duration::from_millis(500));
    }
    Err("No Android UI hierarchy".into())
}
#[derive(Clone)]
struct Item {
    label: String,
    bounds: [i32; 4],
}
fn attribute(s: &str, key: &str) -> String {
    let prefix = format!("{key}=\"");
    s.find(&prefix).and_then(|n| {
        let value = &s[n + prefix.len()..];
        value.find('"').map(|end| value[..end].to_owned())
    }).unwrap_or_default()
}
fn items(xml: &str) -> Vec<Item> {
    xml.split("<node ").skip(1).filter_map(|x| {
        let head = x.split('>').next()?;
        let text = attribute(head, "text");
        let description = attribute(head, "content-desc");
        let raw = attribute(head, "bounds");
        let numbers: Vec<i32> = raw.split(|c: char| !c.is_ascii_digit() && c != '-')
            .filter(|w| !w.is_empty()).filter_map(|w| w.parse().ok()).collect();
        if numbers.len() != 4 { return None; }
        Some(Item {
            label: if text.is_empty() { description } else { text },
            bounds: [numbers[0], numbers[1], numbers[2], numbers[3]],
        })
    }).collect()
}
fn tap_label(labels: &[&str]) -> Result<bool, String> {
    let list = items(&dump()?);
    let item = labels.iter().find_map(|query| list.iter().find(|item| {
        item.label == *query && item.bounds[2] > item.bounds[0]
    })).or_else(|| labels.iter().find_map(|query| {
        list.iter().find(|item| item.label.contains(query))
    }));
    if let Some(item) = item {
        let x = (item.bounds[0] + item.bounds[2]) / 2;
        let y = (item.bounds[1] + item.bounds[3]) / 2;
        cmd(&["shell", "input", "tap", &x.to_string(), &y.to_string()])?;
        sleep(Duration::from_millis(900));
        return Ok(true);
    }
    Ok(false)
}
fn capture(dir: &Path, label: &str) {
    if let Ok(xml) = dump() { let _ = fs::write(dir.join(format!("{label}.xml")), xml); }
    if let Ok(output) = adb(&["exec-out", "screencap", "-p"]) {
        if output.status.success() && output.stdout.starts_with(b"\x89PNG") {
            let _ = fs::write(dir.join(format!("{label}.png")), output.stdout);
        }
    }
}
fn run(dir: &Path) -> Result<String, String> {
    cmd(&["wait-for-device"])?;
    let metadata = cmd(&["shell", "dumpsys", "package", PACKAGE])?;
    let version = installed_version(&metadata)?;
    println!("Verifying installed Android {version}");
    cmd(&["shell", "am", "start", "-W", "-n", ACTIVITY])?;
    sleep(Duration::from_secs(7));
    capture(dir, "01_app_started");
    for _ in 0..16 {
        let ui = dump()?;
        if (ui.contains("新对话") || ui.contains("New chat")) &&
            (ui.contains("历史") || ui.contains("History")) { break; }
        if tap_label(&["AI chat", "Start chat", "AI 问答", "AI 对话", "开始对话"])? { continue; }
        if tap_label(&["知识", "Knowledge", "Notes"])? { continue; }
        if tap_label(&["文档库", "页面与文件夹"])? { continue; }
        cmd(&["shell", "input", "swipe", "500", "1700", "500", "650", "430"])?;
        sleep(Duration::from_millis(500));
    }
    let opened = dump()?;
    capture(dir, "02_ai_dialog");
    if !((opened.contains("AI 问答") || opened.contains("AI chat") || opened.contains("AI Q&A")) &&
        (opened.contains("新对话") || opened.contains("New chat")) &&
        (opened.contains("历史") || opened.contains("History"))) {
        return Err("AI dialog or historical-conversation controls were not visible".into());
    }
    // Composer and attachment/send controls must be visible without scrolling.
    if !opened.contains("输入问题") && !opened.contains("继续追问") {
        return Err("Compact composer is not visible on initial chat viewport".into());
    }
    if !opened.contains("＋ 图片") && !opened.contains("图片") {
        return Err("Attachment action is not visible beside composer".into());
    }
    if !tap_label(&["历史", "History"])? { return Err("History action is not tappable".into()); }
    sleep(Duration::from_millis(700));
    capture(dir, "03_history");
    let history = dump()?;
    if !(history.contains("历史对话") || history.contains("Chat history")) {
        return Err("History list was not displayed".into());
    }
    if history.contains("输入问题") || history.contains("继续追问…") {
        return Err("History view still displays the chat composer".into());
    }
    if !tap_label(&["返回对话", "Back to chat"])? { return Err("History return action is not tappable".into()); }
    capture(dir, "03b_returned_chat");
    if !tap_label(&["新对话", "New chat"])? { return Err("New-chat action is not tappable".into()); }
    capture(dir, "04_new_chat");
    if !(dump()?.contains("新对话") || dump()?.contains("New chat")) {
        return Err("New-chat did not keep the AI interface open".into());
    }
    if !tap_label(&["输入问题", "继续追问…"])? { return Err("Composer is not tappable".into()); }
    cmd(&["shell","input","text","TenRateUIcheck"])?;
    let editing = dump()?;
    capture(dir, "05_composer_ime");
    if !editing.contains("TenRateUIcheck") {
        return Err("Real keyboard input did not reach composer".into());
    }
    cmd(&["shell", "input", "keyevent", "4"])?;
    if cmd(&["shell", "pidof", PACKAGE])?.trim().is_empty() {
        return Err("App process exited while navigating the AI conversation".into());
    }
    Ok("Signed candidate installed and launched; compact composer, independent history, return, new-chat and real keyboard input verified. Real multi-turn inference and persistence require configured API credentials.".into())
}
fn main() {
    let dir = env::args_os().nth(1).map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("ai_chat_evidence"));
    fs::create_dir_all(&dir).expect("create evidence folder");
    let result = run(&dir);
    capture(&dir, "05_final");
    let report = match &result {
        Ok(info) => format!("# AI conversation emulator acceptance\n\n{info}\n\nOverall: PASS (UI smoke only)\n"),
        Err(error) => format!("# AI conversation emulator acceptance\n\n{error}\n\nOverall: FAILED\n"),
    };
    fs::write(dir.join("report.md"), &report).expect("write report");
    println!("{report}");
    if result.is_err() { std::process::exit(1); }
}
