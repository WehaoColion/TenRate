// v2.13.5.6 - 在 GitHub API35 模拟器真实启动客户收付款逐笔记录 UI。
use std::{env, fs, path::{Path, PathBuf}, process::{Command, Output}, thread, time::Duration};

const APP: &str = "com.ofairyo.gridtimer";

#[derive(Clone)]
struct Element { text: String, description: String, class: String, bounds: [i32; 4] }

fn adb(args: &[&str]) -> Result<Output, String> {
    Command::new("adb").args(args).output().map_err(|e| e.to_string())
}
fn run(args: &[&str]) -> Result<String, String> {
    let out = adb(args)?;
    if !out.status.success() {
        return Err(format!("adb {:?}: {}", args, String::from_utf8_lossy(&out.stderr)));
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}
fn attr(source: &str, key: &str) -> String {
    let prefix = format!("{key}=\"");
    source.find(&prefix).and_then(|start| {
        let tail = &source[start + prefix.len()..];
        tail.find('"').map(|end| tail[..end].to_owned())
    }).unwrap_or_default()
}
fn elements(source: &str) -> Vec<Element> {
    source.split("<node ").skip(1).filter_map(|section| {
        let tag = section.split('>').next()?;
        let coords: Vec<i32> = attr(tag, "bounds")
            .split(|ch: char| !ch.is_ascii_digit() && ch != '-')
            .filter(|p| !p.is_empty()).filter_map(|p| p.parse().ok()).collect();
        if coords.len() != 4 { return None; }
        Some(Element {
            text: attr(tag, "text"),
            description: attr(tag, "content-desc"),
            class: attr(tag, "class"),
            bounds: [coords[0], coords[1], coords[2], coords[3]],
        })
    }).collect()
}
fn tree() -> Result<String, String> {
    for _ in 0..5 {
        let _ = run(&["shell", "uiautomator", "dump", "/sdcard/client_payment_detail.xml"]);
        if let Ok(xml) = run(&["exec-out", "cat", "/sdcard/client_payment_detail.xml"]) {
            if xml.contains("<hierarchy") { return Ok(xml); }
        }
        thread::sleep(Duration::from_millis(400));
    }
    Err("Accessibility tree unavailable".into())
}
fn target(labels: &[&str]) -> Result<Option<Element>, String> {
    let xml = tree()?;
    let options = elements(&xml);
    Ok(labels.iter().find_map(|label| options.iter().find(|item|
        item.bounds[2] > item.bounds[0] && item.bounds[3] > item.bounds[1] &&
        (item.text.contains(label) || item.description.contains(label))).cloned()))
}
fn tap(element: &Element) -> Result<(), String> {
    run(&["shell", "input", "tap",
        &((element.bounds[0] + element.bounds[2]) / 2).to_string(),
        &((element.bounds[1] + element.bounds[3]) / 2).to_string()])?;
    thread::sleep(Duration::from_millis(750));
    Ok(())
}
fn click(labels: &[&str]) -> Result<bool, String> {
    if let Some(found) = target(labels)? { tap(&found)?; Ok(true) } else { Ok(false) }
}
fn required(labels: &[&str]) -> Result<(), String> {
    for _ in 0..4 {
        if click(labels)? { return Ok(()); }
        thread::sleep(Duration::from_millis(450));
    }
    Err(format!("Required button not visible: {labels:?}"))
}
fn edit(index: usize, value: &str) -> Result<(), String> {
    let fields: Vec<_> = elements(&tree()?).into_iter()
        .filter(|node| node.class.ends_with("EditText") && node.bounds[2] - node.bounds[0] > 50)
        .collect();
    let field = fields.get(index).ok_or_else(|| format!("Editable field {index} missing; count {}", fields.len()))?;
    tap(field)?;
    run(&["shell", "input", "text", value])?;
    thread::sleep(Duration::from_millis(350));
    Ok(())
}
fn snapshot(folder: &Path, title: &str) {
    if let Ok(png) = adb(&["exec-out", "screencap", "-p"]) {
        if png.status.success() && png.stdout.starts_with(b"\x89PNG") {
            let _ = fs::write(folder.join(format!("{title}.png")), png.stdout);
        }
    }
    if let Ok(xml) = tree() {
        let _ = fs::write(folder.join(format!("{title}.xml")), xml);
    }
}
fn dismiss_system_dialogs() -> Result<(), String> {
    for _ in 0..5 {
        let xml = tree()?;
        if xml.contains("Pixel Launcher isn't responding") || xml.contains("Pixel Launcher isn&#39;t responding") {
            required(&["Close app"])?;
        } else if xml.contains("Allow tenfold to send you notifications?") {
            required(&["Allow"])?;
        } else { return Ok(()); }
    }
    Ok(())
}
fn open_canvas(folder: &Path) -> Result<(), String> {
    for _ in 0..5 {
        dismiss_system_dialogs()?;
        if target(&["＋ 添加第一个客户", "知识画布，", "我的画布", "＋ 添加"])?.is_some() {
            return Ok(());
        }
        let _ = click(&["画布", "卡片与连线"])?;
        if target(&["＋ 添加第一个客户", "知识画布，", "我的画布", "＋ 添加"])?.is_some() {
            return Ok(());
        }
        let _ = click(&["知识库", "知识", "Knowledge", "笔记"])?;
        snapshot(folder, "navigation");
    }
    Err("Unable to reach the canvas".into())
}
fn open_first_card() -> Result<(), String> {
    let surface = elements(&tree()?).into_iter()
        .find(|n| n.description.contains("知识画布，") && n.description.contains("张卡片"))
        .ok_or("Canvas card surface not found")?;
    run(&["shell", "input", "tap",
        &((surface.bounds[0] + surface.bounds[2]) / 2).to_string(),
        &((surface.bounds[1] + surface.bounds[3]) / 2).to_string()])?;
    thread::sleep(Duration::from_millis(800));
    Ok(())
}
fn verify(label: &str) -> Result<(), String> {
    for _ in 0..5 {
        if tree()?.contains(label) { return Ok(()); }
        thread::sleep(Duration::from_millis(500));
    }
    Err(format!("Expected UI text absent: {label}"))
}
fn scenario(folder: &Path, report: &mut String) -> Result<(), String> {
    run(&["shell", "am", "start", "-W", "-n", "com.ofairyo.gridtimer/.MainActivity"])?;
    thread::sleep(Duration::from_secs(6));
    dismiss_system_dialogs()?;
    snapshot(folder, "01_启动");
    if run(&["shell", "pidof", APP])?.trim().is_empty() {
        return Err("Process did not start".into());
    }
    open_canvas(folder)?;
    snapshot(folder, "02_画布");
    required(&["＋ 添加第一个客户", "＋ 添加"])?;
    if elements(&tree()?).iter().all(|n| !n.class.ends_with("EditText")) {
        required(&["＋ 新建客户", "＋ 更多卡片：客户"])?;
    }
    edit(0, "ClientDetail")?;
    run(&["shell", "input", "keyevent", "KEYCODE_ENTER"])?;
    required(&["创建", "Create"])?;
    verify("1 张卡片")?;
    snapshot(folder, "03_客户已建立");
    open_first_card()?;
    snapshot(folder, "04_客户详情");
    required(&["收付款信息", "查看明细"])?;
    verify("收付款明细")?;
    snapshot(folder, "05_收付款空态");
    required(&["新增款项项目"])?;
    edit(0, "PaperService")?;
    edit(1, "1220.00")?;
    run(&["shell", "input", "keyevent", "KEYCODE_ENTER"])?;
    snapshot(folder, "06_新建款项表单");
    required(&["保存"])?;
    verify("PaperService")?;
    snapshot(folder, "07_款项已保存");
    required(&["＋ 收款"])?;
    edit(2, "Online")?; // Amount and receipt date are prefilled.
    run(&["shell", "input", "keyevent", "KEYCODE_ENTER"])?;
    snapshot(folder, "08_收款表单");
    required(&["保存"])?;
    verify("已结清")?;
    verify("1,220.00")?;
    snapshot(folder, "09_收款已结清");
    report.push_str("PASS: New client, charge project, received RMB amount and payment details were created on the actual emulator.\n\n");
    required(&["完成"])?;
    run(&["shell", "am", "force-stop", APP])?;
    run(&["shell", "am", "start", "-W", "-n", "com.ofairyo.gridtimer/.MainActivity"])?;
    thread::sleep(Duration::from_secs(5));
    open_canvas(folder)?;
    open_first_card()?;
    required(&["收付款信息", "查看明细"])?;
    verify("1,220.00")?;
    verify("已结清")?;
    snapshot(folder, "10_重启仍保留收款");
    report.push_str("PASS: Forced application restart preserved the recorded charge and receipt.\n\n");
    Ok(())
}
fn main() {
    let folder = env::args().nth(1).map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("client_payment_detail_evidence"));
    fs::create_dir_all(&folder).expect("Evidence folder");
    let mut report = String::from("# Android 2.13.5.6 customer payment details\n\n");
    match scenario(&folder, &mut report) {
        Ok(()) => report.push_str("Overall: PASS\n"),
        Err(error) => {
            snapshot(&folder, "失败现场");
            report.push_str(&format!("Overall: FAILED / NOT VERIFIED\n\nReason: {error}\n"));
        }
    }
    fs::write(folder.join("report.md"), &report).expect("Report");
    println!("{report}");
    if !report.contains("Overall: PASS") { std::process::exit(1); }
}
