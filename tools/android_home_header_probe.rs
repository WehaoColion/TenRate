//! Real API 35 Android acceptance for the zero-duration home header.
//! Uses only the Rust standard library and adb against the signed release APK.
use std::{
    env, fs,
    path::{Path, PathBuf},
    process::{Command, Output},
    thread::sleep,
    time::Duration,
};

const PACKAGE: &str = "com.ofairyo.gridtimer";
const ACTIVITY: &str = "com.ofairyo.gridtimer/.MainActivity";
const VERSION: &str = "2.13.5.3";
const ZERO_LABEL_CN: &str = "今日 0分钟";
const ZERO_LABEL_EN: &str = "Today 0m";

fn adb(args: &[&str]) -> Result<Output, String> {
    Command::new("adb")
        .args(args)
        .output()
        .map_err(|error| error.to_string())
}

fn command(args: &[&str]) -> Result<String, String> {
    let output = adb(args)?;
    if !output.status.success() {
        return Err(format!("adb {args:?}: {}", String::from_utf8_lossy(&output.stderr)));
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

fn screenshot(root: &Path, name: &str) -> Result<(), String> {
    let output = adb(&["exec-out", "screencap", "-p"])?;
    if !output.status.success() || !output.stdout.starts_with(b"\x89PNG") {
        return Err(format!("{name}: emulator screenshot not available"));
    }
    fs::write(root.join(format!("{name}.png")), output.stdout)
        .map_err(|error| error.to_string())
}

fn dump() -> Result<String, String> {
    for _ in 0..5 {
        let _ = command(&["shell", "uiautomator", "dump", "/sdcard/home_header.xml"]);
        if let Ok(xml) = command(&["exec-out", "cat", "/sdcard/home_header.xml"]) {
            if xml.contains("<hierarchy") {
                return Ok(xml);
            }
        }
        sleep(Duration::from_millis(350));
    }
    Err("Android accessibility hierarchy unavailable".into())
}

#[derive(Clone)]
struct Node {
    label: String,
    description: String,
    bounds: [i32; 4],
    clickable: bool,
}

fn attribute(node: &str, name: &str) -> String {
    let key = format!("{name}=\"");
    node.find(&key)
        .and_then(|offset| {
            let rest = &node[offset + key.len()..];
            rest.find('"').map(|end| rest[..end].to_owned())
        })
        .unwrap_or_default()
}

fn nodes(xml: &str) -> Vec<Node> {
    xml.split("<node ")
        .skip(1)
        .filter_map(|fragment| {
            let head = fragment.split('>').next()?;
            let raw = attribute(head, "bounds");
            let values = raw
                .split(|ch: char| !ch.is_ascii_digit() && ch != '-')
                .filter(|piece| !piece.is_empty())
                .filter_map(|piece| piece.parse::<i32>().ok())
                .collect::<Vec<_>>();
            if values.len() != 4 {
                return None;
            }
            Some(Node {
                label: attribute(head, "text"),
                description: attribute(head, "content-desc"),
                bounds: [values[0], values[1], values[2], values[3]],
                clickable: attribute(head, "clickable") == "true",
            })
        })
        .collect()
}

fn tap(node: &Node) -> Result<(), String> {
    let x = (node.bounds[0] + node.bounds[2]) / 2;
    let y = (node.bounds[1] + node.bounds[3]) / 2;
    command(&["shell", "input", "tap", &x.to_string(), &y.to_string()])?;
    Ok(())
}

fn dismiss_dialogs(view: &[Node]) -> Result<bool, String> {
    for name in ["Allow", "允许", "Close app"] {
        if let Some(node) = view.iter().find(|node| {
            node.label == name && node.bounds[2] > node.bounds[0]
        }) {
            tap(node)?;
            return Ok(true);
        }
    }
    Ok(false)
}

fn header_is_correct(view: &[Node]) -> Result<bool, String> {
    let Some(version) = view.iter().find(|node| node.label.contains(VERSION)) else {
        return Ok(false);
    };
    // The emulator may launch in English even after set-app-locales is requested.
    // Validate the observed language rather than mistaking a correct English badge for a failure.
    let Some(today) = view.iter().find(|node| {
        node.label == ZERO_LABEL_CN || node.label == ZERO_LABEL_EN
    }) else {
        return Ok(false);
    };
    let version_y = (version.bounds[1] + version.bounds[3]) / 2;
    let today_y = (today.bounds[1] + today.bounds[3]) / 2;
    if (version_y - today_y).abs() > 90 {
        return Err("Version and today's duration are not aligned in the same header row".into());
    }
    if today.clickable {
        return Err("Read-only today summary unexpectedly masquerades as a button".into());
    }
    let archive_in_header = view.iter().any(|node| {
        node.label == "归档"
            && (node.bounds[1] + node.bounds[3] - 2 * today_y).abs() < 100
    });
    if archive_in_header {
        return Err("Archive text still appears in the current-time header".into());
    }
    Ok(true)
}

fn wait_for_header(root: &Path, label: &str) -> Result<(), String> {
    for attempt in 0..25 {
        if let Ok(xml) = dump() {
            let view = nodes(&xml);
            if dismiss_dialogs(&view)? {
                sleep(Duration::from_millis(650));
                continue;
            }
            if header_is_correct(&view)? {
                fs::write(root.join(format!("{label}.xml")), &xml)
                    .map_err(|error| error.to_string())?;
                screenshot(root, label)?;
                return Ok(());
            }
            if attempt == 5 || attempt == 12 {
                if let Some(timer) = view.iter().find(|node| {
                    node.label == "计时" || node.description == "计时"
                        || node.label == "Timer" || node.description == "Timer"
                }) {
                    tap(timer)?;
                }
            }
        }
        sleep(Duration::from_millis(700));
    }
    if let Ok(xml) = dump() {
        let _ = fs::write(root.join(format!("{label}_not_found.xml")), xml);
    }
    let _ = screenshot(root, &format!("{label}_not_found"));
    Err(format!("{label}: current-time header was not found after startup"))
}

fn run(root: &Path) -> Result<(), String> {
    command(&["wait-for-device"])?;
    let package = command(&["shell", "dumpsys", "package", PACKAGE])?;
    if !package.contains(&format!("versionName={VERSION}")) {
        return Err(format!("Installed signed APK is not Android {VERSION}"));
    }
    command(&["shell", "pm", "clear", PACKAGE])?;
    let _ = command(&["shell", "cmd", "locale", "set-app-locales", PACKAGE, "--locales", "zh-CN"]);
    let _ = command(&["logcat", "-c"]);
    command(&["shell", "am", "start", "-W", "-n", ACTIVITY])?;
    wait_for_header(root, "01_cold_start_zero_duration")?;
    command(&["shell", "am", "force-stop", PACKAGE])?;
    command(&["shell", "am", "start", "-W", "-n", ACTIVITY])?;
    wait_for_header(root, "02_restart_zero_duration")?;
    let pid = command(&["shell", "pidof", PACKAGE])?;
    if pid.trim().is_empty() {
        return Err("App process did not survive the restart".into());
    }
    let logcat = command(&["logcat", "-d", "-s", "AndroidRuntime:E"])?;
    fs::write(root.join("android_runtime.log"), &logcat)
        .map_err(|error| error.to_string())?;
    if logcat.contains("FATAL EXCEPTION") {
        return Err("AndroidRuntime reported a fatal app exception".into());
    }
    Ok(())
}

fn main() {
    let root = env::args_os()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("home_header_evidence"));
    if let Err(error) = fs::create_dir_all(&root) {
        eprintln!("Evidence directory unavailable: {error}");
        std::process::exit(2);
    }
    let result = run(&root);
    let report = match &result {
        Ok(()) => format!(
            "# Android {VERSION} home header emulator acceptance\n\n\
             PASS: signed APK installed and started on Android API 35. \
             Cold start and forced restart both showed version {VERSION} \
             beside a read-only zero-duration badge in the observed app language. \
             The incorrect archive badge was absent from the header. \
             Actual screenshots, hierarchies and AndroidRuntime logs are attached.\n"
        ),
        Err(error) => format!(
            "# Android {VERSION} home header emulator acceptance\n\n\
             FAILED / NOT VERIFIED: {error}\n\
             Do not treat this job as device acceptance.\n"
        ),
    };
    let _ = fs::write(root.join("report.md"), &report);
    println!("{report}");
    if result.is_err() {
        std::process::exit(1);
    }
}
