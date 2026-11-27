// v3.0.2.45 - Shared fail-closed Android accessibility evidence.
use std::{thread::sleep, time::Duration};

/// A failed read-only observation is retryable; an old XML file is never
/// evidence of the current screen. Do not retry taps or keyboard events here.
pub fn fresh_hierarchy(
    mut command: impl FnMut(&[&str]) -> Result<String, String>,
    path: &str,
    attempts: usize,
) -> Result<String, String> {
    let mut last_error = String::from("No hierarchy attempts");
    for attempt in 0..attempts {
        let result = (|| {
            command(&["shell", "rm", "-f", path])?;
            command(&["shell", "uiautomator", "dump", path])?;
            let xml = command(&["exec-out", "cat", path])?;
            if !xml.contains("<hierarchy") || !xml.contains("</hierarchy>") {
                return Err("Incomplete Android accessibility hierarchy".to_owned());
            }
            Ok(xml)
        })();
        match result {
            Ok(xml) => return Ok(xml),
            Err(error) => last_error = error,
        }
        if attempt + 1 < attempts { sleep(Duration::from_millis(550)); }
    }
    Err(format!("Fresh Android accessibility hierarchy unavailable: {last_error}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn failed_dump_cannot_reuse_previous_screen() {
        let mut read = false;
        let result = fresh_hierarchy(|args| {
            match args[1] {
                "rm" => Ok(String::new()),
                "uiautomator" => Err("simulated real dump failure".into()),
                "cat" => {
                    read = true;
                    Ok("<hierarchy><node text=\"old heading\"/></hierarchy>".into())
                }
                other => panic!("unexpected command {other}"),
            }
        }, "/sdcard/probe.xml", 1);
        assert!(result.is_err());
        assert!(!read, "A failed dump must not read a previously captured screen");
    }

    #[test]
    fn successful_dump_requires_complete_new_hierarchy() {
        let mut calls = Vec::new();
        let result = fresh_hierarchy(|args| {
            calls.push(args[1].to_owned());
            Ok(if args[1] == "cat" { "<hierarchy><node text=\"new heading\"/></hierarchy>".into() } else { String::new() })
        }, "/sdcard/probe.xml", 1).unwrap();
        assert_eq!(calls, ["rm", "uiautomator", "cat"]);
        assert!(result.contains("new heading"));
        assert!(fresh_hierarchy(|args| Ok(if args[1] == "cat" { "<hierarchy>".into() } else { String::new() }), "/sdcard/probe.xml", 1).is_err());
    }
}
