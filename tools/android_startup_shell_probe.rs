//! Signed-APK Android emulator acceptance for the first-frame startup shell.
//! A fast restore can bypass the transient shell between ADB screenshots;
//! in that case its layout and safety are verified by the Rust sourcegen tests.
use std::{
    env, fs,
    path::{Path, PathBuf},
    process::{Command, Output},
    thread::sleep,
    time::Duration,
};

const PACKAGE: &str = "com.ofairyo.gridtimer";
const ACTIVITY: &str = "com.ofairyo.gridtimer/.MainActivity";
const VERSION: &str = "2.23.4.14";

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

fn screenshot(root: &Path, name: &str) -> Result<(), String> {
    let result = adb(&["exec-out", "screencap", "-p"])?;
    if !result.status.success() || !result.stdout.starts_with(b"\x89PNG") {
        return Err(format!("missing emulator screenshot: {name}"));
    }
    fs::write(root.join(format!("{name}.png")), result.stdout)
        .map_err(|error| error.to_string())
}

fn hierarchy() -> Result<String, String> {
    for _ in 0..4 {
        let _ = command(&["shell", "uiautomator", "dump", "/sdcard/startup_shell.xml"]);
        if let Ok(xml) = command(&["exec-out", "cat", "/sdcard/startup_shell.xml"]) {
            if xml.contains("<hierarchy") {
                return Ok(xml);
            }
        }
        sleep(Duration::from_millis(350));
    }
    Err("UI hierarchy is not available".into())
}

fn dismiss_system_dialogs(xml: &str) -> Result<bool, String> {
    let action = if xml.contains("Pixel Launcher isn't responding") ||
        xml.contains("Pixel Launcher isn&#39;t responding")
    {
        Some("Close app")
    } else if xml.contains("Allow tenfold to send you notifications?") {
        Some("Allow")
    } else {
        None
    };
    let Some(label) = action else { return Ok(false) };
    let Some(part) = xml.split("<node ").skip(1).find(|node| {
        node.contains(&format!("text=\"{label}\""))
    }) else { return Ok(false) };
    let bounds = part.split("bounds=\"").nth(1)
        .and_then(|part| part.split('"').next()).unwrap_or("");
    let coordinates: Vec<i32> = bounds.split(|char: char|
        !char.is_ascii_digit() && char != '-')
        .filter(|part| !part.is_empty())
        .filter_map(|part| part.parse::<i32>().ok())
        .collect();
    if coordinates.len() != 4 { return Ok(false) }
    command(&["shell", "input", "tap",
        &((coordinates[0] + coordinates[2]) / 2).to_string(),
        &((coordinates[1] + coordinates[3]) / 2).to_string()])?;
    Ok(true)
}

fn wait_for_workbench(root: &Path, label: &str) -> Result<String, String> {
    for attempt in 0..12 {
        let xml = hierarchy()?;
        fs::write(root.join(format!("{label}_{attempt}.xml")), &xml)
            .map_err(|error| error.to_string())?;
        if dismiss_system_dialogs(&xml)? {
            sleep(Duration::from_millis(600));
            continue;
        }
        let has_workbench = ["计时", "Timer", "工作台", "Workspace", "便签", "Quick notes"]
            .iter().any(|text| xml.contains(text));
        if has_workbench {
            screenshot(root, label)?;
            return Ok(xml);
        }
        sleep(Duration::from_millis(650));
    }
    screenshot(root, &format!("{label}_not_ready"))?;
    Err(format!("{label}: workbench did not become visible"))
}

fn run(root: &Path) -> Result<(), String> {
    command(&["wait-for-device"])?;
    let package = command(&["shell", "dumpsys", "package", PACKAGE])?;
    if !package.contains(&format!("versionName={VERSION}")) {
        return Err(format!("signed APK is not Android {VERSION}"));
    }
    command(&["shell", "pm", "clear", PACKAGE])?;
    let _ = command(&["logcat", "-c"]);
    command(&["shell", "am", "start", "-n", ACTIVITY])?;
    screenshot(root, "01_immediate_frame")?;
    let first = wait_for_workbench(root, "02_first_launch_ready")?;
    if first.contains("正在恢复本机内容。可以先进入工作台") {
        return Err("obsolete oversized startup shell is still displayed".into());
    }

    command(&["shell", "am", "force-stop", PACKAGE])?;
    command(&["shell", "am", "start", "-n", ACTIVITY])?;
    screenshot(root, "03_restart_immediate_frame")?;
    let after = wait_for_workbench(root, "04_restart_ready")?;
    if after.contains("正在恢复本机内容。可以先进入工作台") {
        return Err("obsolete oversized shell reappeared after restart".into());
    }

    let pid = command(&["shell", "pidof", PACKAGE])?;
    if pid.trim().is_empty() {
        return Err("app process did not survive restart".into());
    }
    let logcat = command(&["logcat", "-d", "-s", "AndroidRuntime:E", "StartupLatency:I"])?;
    fs::write(root.join("startup_logcat.txt"), &logcat)
        .map_err(|error| error.to_string())?;
    if logcat.contains("FATAL EXCEPTION") {
        return Err("AndroidRuntime reported a fatal app exception".into());
    }
    Ok(())
}

fn main() {
    let root = env::args_os().nth(1).map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("startup_shell_evidence"));
    if let Err(error) = fs::create_dir_all(&root) {
        eprintln!("Unable to create evidence directory: {error}");
        std::process::exit(2);
    }
    let result = run(&root);
    let report = match &result {
        Ok(()) => format!(
            "# Android startup-shell emulator acceptance\n\nPASS: Android {VERSION} signed APK \
            installed and launched; UI visible after cold start and force-stop/restart; \
            app remained alive; no fatal AndroidRuntime exception.\n\n\
            A transient loading shell can complete before ADB captures its hierarchy. \
            Its structure and non-mutating state are checked by Rust sourcegen tests.\n"
        ),
        Err(error) => format!(
            "# Android startup-shell emulator acceptance\n\nFAIL: {error}\n\n\
            Do not interpret this as a passed startup test.\n"
        ),
    };
    let _ = fs::write(root.join("report.md"), &report);
    println!("{report}");
    if result.is_err() {
        std::process::exit(1);
    }
}
