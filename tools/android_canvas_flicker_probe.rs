use std::{
    env, fs,
    path::{Path, PathBuf},
    process::{Command, Output},
    thread::sleep,
    time::Duration,
};

const PACKAGE: &str = "com.ofairyo.gridtimer";
const ACTIVITY: &str = "com.ofairyo.gridtimer/.MainActivity";
const FRAME_BYTES: usize = 320 * 720 * 3;

#[derive(Clone)]
struct Node {
    text: String,
    description: String,
    class: String,
    left: i32,
    top: i32,
    right: i32,
    bottom: i32,
}

fn adb(args: &[&str]) -> Result<Output, String> {
    Command::new("adb").args(args).output().map_err(|e| e.to_string())
}

fn command(args: &[&str]) -> Result<String, String> {
    let output = adb(args)?;
    if !output.status.success() {
        return Err(format!("adb {:?}: {}", args, String::from_utf8_lossy(&output.stderr)));
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

fn attribute(chunk: &str, name: &str) -> String {
    let key = format!("{name}=\"");
    chunk.find(&key).and_then(|offset| {
        let remaining = &chunk[offset + key.len()..];
        remaining.find('"').map(|end| remaining[..end].to_owned())
    }).unwrap_or_default()
}

fn ui_nodes(xml: &str) -> Vec<Node> {
    xml.split("<node ").skip(1).filter_map(|part| {
        let head = part.split('>').next()?;
        let coordinates: Vec<i32> = attribute(head, "bounds")
            .split(|c: char| !c.is_ascii_digit() && c != '-')
            .filter(|value| !value.is_empty())
            .filter_map(|value| value.parse().ok()).collect();
        if coordinates.len() != 4 { return None; }
        Some(Node {
            text: attribute(head, "text"),
            description: attribute(head, "content-desc"),
            class: attribute(head, "class"),
            left: coordinates[0], top: coordinates[1],
            right: coordinates[2], bottom: coordinates[3],
        })
    }).collect()
}

fn hierarchy() -> Result<String, String> {
    for _ in 0..4 {
        let _ = command(&["shell", "uiautomator", "dump", "/sdcard/canvas_probe.xml"]);
        if let Ok(xml) = command(&["exec-out", "cat", "/sdcard/canvas_probe.xml"]) {
            if xml.contains("<hierarchy") { return Ok(xml); }
        }
        sleep(Duration::from_millis(500));
    }
    Err("Android accessibility hierarchy unavailable".into())
}

fn matching_node(xml: &str, labels: &[&str]) -> Option<Node> {
    for label in labels {
        if let Some(node) = ui_nodes(xml).into_iter().find(|node| {
            node.right > node.left && node.bottom > node.top &&
            (node.text == *label || node.description == *label ||
             node.text.contains(*label) || node.description.contains(*label))
        }) {
            return Some(node);
        }
    }
    None
}

fn tap(node: &Node) -> Result<(), String> {
    command(&["shell", "input", "tap",
        &((node.left + node.right) / 2).to_string(),
        &((node.top + node.bottom) / 2).to_string()]).map(|_| ())
}

// Android emulator's first boot can show a Pixel Launcher ANR above the app.
// It is an emulator-infrastructure overlay, not a TenRate failure.
fn dismiss_emulator_overlays() -> Result<(), String> {
    for _ in 0..6 {
        let xml = hierarchy()?;
        if xml.contains("Pixel Launcher isn't responding") || xml.contains("Pixel Launcher isn&#39;t responding") {
            let button = matching_node(&xml, &["Close app"])
                .ok_or("Pixel Launcher ANR is present but its dismissal button is missing")?;
            tap(&button)?;
            sleep(Duration::from_millis(800));
            continue;
        }
        if xml.contains("Allow tenfold to send you notifications?") {
            let button = matching_node(&xml, &["Allow"])
                .ok_or("notification permission overlay cannot be dismissed")?;
            tap(&button)?;
            sleep(Duration::from_millis(800));
            continue;
        }
        return Ok(());
    }
    Err("emulator system dialog persists after repeated dismissal".into())
}

fn tap_label(labels: &[&str]) -> Result<bool, String> {
    dismiss_emulator_overlays()?;
    let xml = hierarchy()?;
    match matching_node(&xml, labels) {
        Some(node) => {
            tap(&node)?;
            sleep(Duration::from_millis(750));
            Ok(true)
        }
        None => Ok(false),
    }
}

fn evidence(root: &Path, label: &str) {
    if let Ok(image) = adb(&["exec-out", "screencap", "-p"]) {
        if image.status.success() && image.stdout.starts_with(b"\x89PNG") {
            let _ = fs::write(root.join(format!("{label}.png")), image.stdout);
        }
    }
    if let Ok(xml) = hierarchy() {
        let _ = fs::write(root.join(format!("{label}.xml")), xml);
    }
}

fn canvas_visible() -> bool {
    hierarchy().map(|xml| {
        xml.contains("新建画布") || xml.contains("全部画布") ||
        xml.contains("知识画布，") || xml.contains("Canvas") && xml.contains("卡片")
    }).unwrap_or(false)
}

fn open_canvas(root: &Path) -> Result<(), String> {
    dismiss_emulator_overlays()?;
    if canvas_visible() { return Ok(()); }
    for _ in 0..3 {
        evidence(root, "navigation_step");
        if tap_label(&["画布", "Canvas", "卡片与连线"])? && canvas_visible() {
            return Ok(());
        }
        if tap_label(&["知识库", "知识", "Knowledge", "笔记", "Notes"])? && canvas_visible() {
            return Ok(());
        }
    }
    if canvas_visible() { Ok(()) }
    else { evidence(root, "canvas_navigation_failed"); Err("Canvas not reachable after dismissing system overlays; inspect navigation evidence".into()) }
}

fn create_card() -> Result<(), String> {
    if !tap_label(&["＋ 便签", "+ 便签", "新建便签", "＋ Note", "+ Note"])? {
        return Err("New note action missing on canvas".into());
    }
    let xml = hierarchy()?;
    let field = ui_nodes(&xml).into_iter().find(|node| {
        node.class.ends_with("EditText") && node.right - node.left > 30
    }).ok_or("New note dialog has no editable field")?;
    tap(&field)?;
    command(&["shell", "input", "text", "CanvasGestureProbe"])?;
    if !tap_label(&["保存", "Save"])? {
        return Err("New note dialog has no Save action".into());
    }
    sleep(Duration::from_millis(850));
    let xml = hierarchy()?;
    if !xml.contains("Cards 1") && !xml.contains("卡片 1") && !xml.contains("1 张卡片") {
        return Err("New card did not appear in canvas after Save".into());
    }
    Ok(())
}

fn record_gestures(root: &Path) -> Result<PathBuf, String> {
    let remote = "/sdcard/canvas_pan_check.mp4";
    let _ = command(&["shell", "rm", "-f", remote]);
    let mut recorder = Command::new("adb")
        .args(["shell", "screenrecord", "--time-limit", "9", "--bit-rate", "8000000", remote])
        .spawn().map_err(|e| format!("screenrecord: {e}"))?;
    sleep(Duration::from_millis(1000));
    for cycle in 0..3 {
        let (from_x, to_x) = if cycle % 2 == 0 { ("185", "345") } else { ("345", "185") };
        command(&["shell", "input", "swipe", from_x, "570", to_x, "650", "510"])?;
        sleep(Duration::from_millis(350));
    }
    // A freshly created card is initially centered in the viewport. This tests
    // card dragging, not only an empty-canvas pan, on an actual Android input stack.
    let xml = hierarchy()?;
    let surface = ui_nodes(&xml).into_iter().find(|node| {
        node.description.contains("知识画布") && node.description.contains("张卡片")
    }).ok_or("Canvas surface bounds unavailable for card drag")?;
    let center_x = (surface.left + surface.right) / 2;
    let center_y = (surface.top + surface.bottom) / 2;
    let to_x = (center_x + 120).min(surface.right - 20);
    let to_y = (center_y + 70).min(surface.bottom - 20);
    command(&["shell", "input", "swipe", &center_x.to_string(), &center_y.to_string(),
        &to_x.to_string(), &to_y.to_string(), "600"])?;
    sleep(Duration::from_millis(400));
    command(&["shell", "input", "swipe", &to_x.to_string(), &to_y.to_string(),
        &center_x.to_string(), &center_y.to_string(), "600"])?;
    sleep(Duration::from_millis(350));
    let status = recorder.wait().map_err(|e| e.to_string())?;
    if !status.success() { return Err(format!("Android screenrecord exited: {status}")); }
    let local = root.join("canvas_pan_check.mp4");
    command(&["pull", remote, local.to_string_lossy().as_ref()])?;
    if fs::metadata(&local).map(|info| info.len()).unwrap_or(0) < 1000 {
        return Err("Canvas gesture recording is missing or empty".into());
    }
    Ok(local)
}

fn quantify_header_stability(video: &Path) -> Result<(usize, f64), String> {
    let output = Command::new("ffmpeg")
        .args(["-nostdin", "-hide_banner", "-loglevel", "error", "-i"])
        .arg(video)
        .args(["-vf", "fps=12,scale=320:720", "-pix_fmt", "rgb24", "-f", "rawvideo", "pipe:1"])
        .output().map_err(|e| format!("ffmpeg missing: {e}"))?;
    if !output.status.success() {
        return Err(format!("ffmpeg decode failed: {}", String::from_utf8_lossy(&output.stderr)));
    }
    let frames: Vec<&[u8]> = output.stdout.chunks_exact(FRAME_BYTES).collect();
    if frames.len() < 18 {
        return Err(format!("Too few decoded gesture frames: {}", frames.len()));
    }
    let baseline = frames[0];
    let mut max_fraction = 0.0_f64;
    for frame in &frames {
        let mut changed = 0usize;
        let mut total = 0usize;
        // Only the fixed action header is compared. Canvas content is meant to move.
        for row in 53..166 {
            for col in 16..305 {
                let pos = (row * 320 + col) * 3;
                let difference = (0..3)
                    .map(|channel| frame[pos + channel].abs_diff(baseline[pos + channel]) as u16)
                    .sum::<u16>();
                if difference > 75 { changed += 1; }
                total += 1;
            }
        }
        max_fraction = max_fraction.max(changed as f64 / total as f64);
    }
    Ok((frames.len(), max_fraction))
}

fn main() {
    let root = env::args().nth(1).map(PathBuf::from).unwrap_or_else(|| PathBuf::from("canvas_emulator_evidence"));
    fs::create_dir_all(&root).expect("create evidence directory");
    let mut report = String::from("# Android canvas gesture emulator test\n\n");
    let result = (|| -> Result<(), String> {
        command(&["shell", "am", "start", "-W", "-n", ACTIVITY])?;
        sleep(Duration::from_secs(5));
        dismiss_emulator_overlays()?;
        evidence(&root, "app_started");
        let process = command(&["shell", "pidof", PACKAGE])?;
        if process.trim().is_empty() { return Err("APK process not running after launch".into()); }
        report.push_str("PASS: signed APK installed, app process launched.\n\n");
        open_canvas(&root)?;
        evidence(&root, "canvas_open");
        report.push_str("PASS: canvas controls observed in accessibility tree.\n\n");
        create_card()?;
        evidence(&root, "canvas_before_pan");
        report.push_str("PASS: created and saved a real canvas note in the current locale.\n\n");
        let video = record_gestures(&root)?;
        evidence(&root, "canvas_after_pan");
        let (frames, max_fraction) = quantify_header_stability(&video)?;
        report.push_str(&format!("Decoded {frames} gesture frames; largest changing share of fixed header = {:.2}%.\n\n", max_fraction * 100.0));
        if max_fraction > 0.08 {
            return Err(format!("Fixed action header changed by {:.2}% during pan; inspect recorded frames", max_fraction * 100.0));
        }
        let process = command(&["shell", "pidof", PACKAGE])?;
        if process.trim().is_empty() { return Err("App process died during canvas gestures".into()); }
        report.push_str("PASS: no large header flash detected during canvas pans and card drags; app remains alive.\n");
        Ok(())
    })();
    match &result {
        Ok(()) => report.push_str("\nOverall: PASS\n"),
        Err(error) => report.push_str(&format!("\nOverall: NOT VERIFIED / FAIL\n\nReason: {error}\n")),
    }
    report.push_str("\nThis probes emulator rendering and launch, not physical-device display behavior. Video and UI dumps are retained as evidence.\n");
    fs::write(root.join("report.md"), &report).expect("write test report");
    println!("{report}");
    if result.is_err() { std::process::exit(1); }
}
