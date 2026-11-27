//! Signed app locale switching, with actual screen evidence for each language.
use std::{fs, path::Path, process::Command, thread::sleep, time::Duration};
mod android_fresh_hierarchy;
mod android_probe_text;
const PACKAGE: &str = "com.ofairyo.gridtimer";
const ACTIVITY: &str = "com.ofairyo.gridtimer/.MainActivity";
const CATALOG: &str = include_str!("../native/gridtimer_native/src/sourcegen/ui_locales/core_navigation.tsv");
const LANGUAGES: [&str; 10] = ["en-US","hi-IN","es-ES","ar","fr-FR","bn-BD","pt-BR","id-ID","ur-PK","ja-JP"];
fn cmd(args: &[&str]) -> Result<String,String> {
    let output = Command::new("adb").args(args).output().map_err(|e|e.to_string())?;
    if !output.status.success() { return Err(format!("adb {args:?}: {}", String::from_utf8_lossy(&output.stderr))); }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}
fn run(root: &Path) -> Result<(),String> {
    for (index, locale) in LANGUAGES.iter().enumerate() {
        cmd(&["shell","am","force-stop",PACKAGE])?;
        cmd(&["shell","cmd","locale","set-app-locales",PACKAGE,"--locales",locale])?;
        cmd(&["shell","am","start","-W","-n",ACTIVITY])?;
        let expected = CATALOG.lines().find_map(|line| {
            let mut fields = line.split('\t');
            if fields.next()? == "知识" { fields.nth(index) } else { None }
        }).ok_or("missing expected knowledge label")?;
        let mut matched: Option<String> = None;
        for _ in 0..12 {
            sleep(Duration::from_secs(2));
            let xml = android_fresh_hierarchy::fresh_hierarchy(cmd,"/sdcard/locale_acceptance.xml",3)?;
            fs::write(root.join(format!("{locale}.xml")), &xml).map_err(|e|e.to_string())?;
            // Check a real translated navigation label, not a locale setting alone.
            if let Some(label) = android_probe_text::matched_navigation_label(locale, expected, &xml) {
                matched = Some(label.to_owned());
                break;
            }
        }
        let png = Command::new("adb").args(["exec-out","screencap","-p"]).output().map_err(|e|e.to_string())?;
        if png.status.success() && png.stdout.starts_with(b"\x89PNG") {
            fs::write(root.join(format!("{locale}.png")),png.stdout).map_err(|e|e.to_string())?;
        }
        let matched = matched.ok_or_else(|| format!("{locale}: translated Knowledge navigation label {expected:?} not found"))?;
        println!("PASS {locale}: {matched} (catalog: {expected})");
    }
    Ok(())
}
fn main() {
    let root = Path::new("locale_evidence");
    fs::create_dir_all(root).expect("create locale evidence");
    if let Err(error) = run(root) { eprintln!("{error}"); std::process::exit(1); }
}
