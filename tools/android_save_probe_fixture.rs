// v3.0.2.33 - Share release identity and visible navigation evidence.
use std::{
    io::{BufRead, BufReader, Write},
    process::{Child, ChildStdin, Command, Stdio},
};

pub const DATABASE: &str = "/data/user/0/com.ofairyo.gridtimer/databases/app_state.db";

#[allow(dead_code)]
#[path = "../native/gridtimer_native/src/android_release_identity.rs"]
mod android_release_identity;
pub use self::android_release_identity::installed_version;

// Drive a real competing writer in the emulator. Closing stdin also releases the
// transaction if a UI assertion returns early.
pub struct SqliteWriter {
    process: Child,
    input: ChildStdin,
}

impl SqliteWriter {
    pub fn acquire() -> Result<Self, String> {
        let mut process = Command::new("adb")
            .args(["shell", "-T", "sqlite3", "-batch", DATABASE])
            .stdin(Stdio::piped()).stdout(Stdio::piped()).spawn().map_err(|e| e.to_string())?;
        let mut input = process.stdin.take().ok_or("Missing SQLite input")?;
        let mut output = BufReader::new(process.stdout.take().ok_or("Missing SQLite output")?);
        input.write_all(b".bail on\n.timeout 8000\nBEGIN IMMEDIATE;\n.print writer-ready\n")
            .map_err(|e| e.to_string())?;
        let mut ready = String::new();
        output.read_line(&mut ready).map_err(|e| e.to_string())?;
        if ready.trim() != "writer-ready" {
            return Err(format!("SQLite writer did not acquire the transaction: {ready}"));
        }
        Ok(Self { process, input })
    }

    pub fn release(mut self) -> Result<(), String> {
        self.input.write_all(b"ROLLBACK;\n.quit\n").map_err(|e| e.to_string())?;
        drop(self.input);
        if self.process.wait().map_err(|e| e.to_string())?.success() {
            Ok(())
        } else {
            Err("SQLite writer failed to release the transaction".into())
        }
    }
}
