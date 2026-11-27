//! Verify heading hierarchy from the pixels written by the Android share action.
#[path = "android_heading_ink.rs"]
mod android_heading_ink;
use android_heading_ink::select_heading_ink_sequence;
use super::{adb, command, evidence, key, seek, tap, tap_label, write_text, NO_JUMP_TEXT, PARAGRAPH_TEXT};
use std::{fs, path::Path, process::Command, str, thread::sleep, time::Duration};

const CACHE: &str = "/data/user/0/com.ofairyo.gridtimer/cache/note_shares";

pub(super) fn export_and_check(out: &Path) -> Result<String, String> {
    let paragraph_marker = format!("{}{}", PARAGRAPH_TEXT.as_str(), NO_JUMP_TEXT.as_str());
    let paragraph = seek(&[paragraph_marker.as_str()])?.ok_or("Reopened paragraph is missing")?;
    tap(&paragraph)?;
    key("KEYCODE_MOVE_END")?;
    // Use the same glyphs at every level so the visible ink heights are comparable.
    let lines = [
        ("#%s", "MMMMMMMM11"),
        ("##%s", "MMMMMMMM22"),
        ("", "MMMMMMMM33"),
        ("#%s", "MMMMMMMM44"),
        ("##%s", "MMMMMMMM55"),
        ("", "MMMMMMMM66"),
    ];
    for (prefix, text) in lines {
        key("KEYCODE_ENTER")?;
        write_text(&format!("'{prefix}{text}'"))?;
        if seek(&[text])?.is_none() {
            return Err(format!("Cannot find {text} in the document editor"));
        }
    }

    if command(&["shell", "dumpsys", "input_method"])?.contains("mInputShown=true") {
        key("KEYCODE_BACK")?;
    }
    for _ in 0..3 {
        if seek(&["返回", "Back"])?.is_some() {
            break;
        }
        command(&["shell", "input", "swipe", "540", "450", "540", "1420", "320"])?;
        sleep(Duration::from_millis(750));
    }
    if !tap_label(&["更多", "More"])?
        || !tap_label(&["分享与导出", "Share and export", "Share & export"])?
    {
        return Err("Could not open the knowledge page's share menu".into());
    }
    evidence(out, "09_share_export_menu");
    let previous = share_images()?;
    if !tap_label(&["分享长图", "Share long image"])? {
        return Err("Share image action could not be reached".into());
    }

    let png = out.join("knowledge_share_headings.png");
    for _ in 0..24 {
        let current = share_images()?;
        if let Some(path) = current
            .lines()
            .find(|path| !previous.lines().any(|old| old == *path))
        {
            let file = adb(&["exec-out", "cat", path])?;
            if file.status.success()
                && file.stdout.starts_with(b"\x89PNG\r\n\x1a\n")
                && file.stdout.ends_with(b"IEND\xaeB\x60\x82")
            {
                fs::write(&png, file.stdout).map_err(|error| error.to_string())?;
                evidence(out, "10_android_share_sheet");
                return check_heading_hierarchy(&png);
            }
        }
        sleep(Duration::from_millis(500));
    }
    Err("The share action did not generate a complete PNG".into())
}

fn share_images() -> Result<String, String> {
    command(&["shell", &format!("ls -1t {CACHE}/note_share_*.png 2>/dev/null || true")])
}

fn check_heading_hierarchy(png: &Path) -> Result<String, String> {
    let image = Command::new("convert")
        .arg(png)
        .args(["-colorspace", "Gray", "-depth", "8", "pgm:-"])
        .output()
        .map_err(|error| format!("Could not decode the share PNG: {error}"))?;
    let mut image_parts = image.stdout.splitn(4, |&byte| byte == b'\n');
    let header = image_parts.next();
    let width: usize = image_parts
        .next()
        .and_then(|line| str::from_utf8(line).ok()?.split_whitespace().next()?.parse().ok())
        .ok_or("The share PNG has no decoded width")?;
    let depth = image_parts.next();
    let pixels = image_parts.next().unwrap_or_default();
    if !image.status.success() || header != Some(b"P5") || depth != Some(b"255")
        || width < 20 || pixels.is_empty() || pixels.len() % width != 0
    {
        return Err("Could not read the share card pixels".into());
    }

    let mut bands: Vec<(usize, usize)> = Vec::new();
    for (y, row) in pixels.chunks_exact(width).enumerate() {
        let content = &row[width / 20..width * 19 / 20];
        if content.iter().filter(|&&pixel| pixel < 160).take(8).count() < 8 {
            continue;
        }
        match bands.last_mut() {
            Some((_, bottom)) if y <= *bottom + 3 => *bottom = y,
            _ => bands.push((y, y)),
        }
    }
    // Locate the six authored sample rows by their contiguous H1/H2/body pattern.
    // The original paragraph may wrap, so a fixed band offset is not evidence.
    let heights: Vec<_> = bands.iter().map(|(top, bottom)| bottom - top + 1).collect();
    let sample = select_heading_ink_sequence(&heights)
        .map_err(|error| format!("{error}; bands={bands:?}; heights={heights:?}"))?;
    Ok(format!(
        "PASS: the Android share action generated a PNG with H1 ink heights [{}, {}] px, H2 [{}, {}] px and body [{}, {}] px.",
        sample.first_h1, sample.second_h1,
        sample.first_h2, sample.second_h2,
        sample.first_body, sample.second_body
    ))
}
