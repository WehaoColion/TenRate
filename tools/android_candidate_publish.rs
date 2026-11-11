// Publish a signed Android candidate to a commit-scoped immutable prerelease.
// This prevents later CI runs for the same Android version from replacing
// artifacts that were signed and published by an earlier commit.
use std::{env, fs, path::{Path, PathBuf}, process::{Command, Output}};

fn gh(repo: &str, args: &[&str]) -> Result<Output, String> {
    Command::new("gh")
        .args(args)
        .arg("--repo")
        .arg(repo)
        .output()
        .map_err(|e| format!("gh {args:?}: {e}"))
}

fn checked(repo: &str, args: &[&str]) -> Result<Output, String> {
    let output = gh(repo, args)?;
    if !output.status.success() {
        return Err(format!(
            "gh {args:?}: {}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    Ok(output)
}

fn only_apk(folder: &Path) -> Result<PathBuf, String> {
    let mut files = fs::read_dir(folder)
        .map_err(|e| e.to_string())?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "apk"))
        .collect::<Vec<_>>();
    if files.len() != 1 {
        return Err(format!("expected one signed APK, found {}", files.len()));
    }
    Ok(files.remove(0))
}

fn publish() -> Result<(), String> {
    let repo = env::var("GITHUB_REPOSITORY").map_err(|e| e.to_string())?;
    let commit = env::var("GITHUB_SHA").map_err(|e| e.to_string())?;
    if commit.len() != 40 || !commit.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err("invalid Git commit id".into());
    }
    let apk = only_apk(Path::new("package"))?;
    let filename = apk.file_name().and_then(|name| name.to_str()).ok_or("APK filename not UTF-8")?;
    let version = filename.strip_prefix("tenfold_v")
        .and_then(|rest| rest.strip_suffix(".apk"))
        .ok_or("unexpected APK filename")?;
    if !(3..=4).contains(&version.split('.').count())
        || version.split('.').any(|segment| segment.is_empty() || !segment.bytes().all(|b| b.is_ascii_digit())) {
        return Err("invalid APK version".into());
    }
    let checksum = apk.with_file_name(format!("{filename}.sha256"));
    let checksum_text = fs::read_to_string(&checksum).map_err(|e| e.to_string())?;
    let digest_out = Command::new("sha256sum").arg(&apk).output().map_err(|e| e.to_string())?;
    if !digest_out.status.success() { return Err("sha256sum failed".into()); }
    let digest_text = String::from_utf8(digest_out.stdout).map_err(|e| e.to_string())?;
    let digest = digest_text.split_whitespace().next().ok_or("missing computed checksum")?;
    let expected = checksum_text.split_whitespace().collect::<Vec<_>>();
    if expected != [digest, filename] { return Err("APK checksum mismatch".into()); }

    let tag = format!("v{version}-candidate-{}", &commit[..12]);
    let old = gh(&repo, &["release", "view", &tag, "--json", "tagName"])?;
    if old.status.success() {
        let previous = Path::new("previous");
        fs::create_dir_all(previous).map_err(|e| e.to_string())?;
        for file in [&apk, &checksum] {
            let name = file.file_name().and_then(|name| name.to_str()).ok_or("invalid asset name")?;
            checked(&repo, &["release", "download", &tag, "--pattern", name, "--dir", "previous"])?;
            let prior = fs::read(previous.join(name)).map_err(|e| e.to_string())?;
            if prior != fs::read(file).map_err(|e| e.to_string())? {
                return Err(format!("immutable release asset mismatch: {name}"));
            }
        }
        println!("Verified existing immutable candidate {tag} ({digest})");
    } else {
        let err = format!("{}{}", String::from_utf8_lossy(&old.stdout), String::from_utf8_lossy(&old.stderr));
        if !err.to_lowercase().contains("not found") { return Err(err); }
        let title = format!("Android {version} candidate {}", &commit[..12]);
        let notes = format!("Signed candidate APK from commit {commit}. SHA256: {digest}. Emulator acceptance is independently reported in the build workflow.");
        checked(&repo, &[
            "release", "create", &tag,
            apk.to_str().ok_or("non-UTF-8 apk path")?,
            checksum.to_str().ok_or("non-UTF-8 checksum path")?,
            "--target", &commit, "--prerelease", "--title", &title, "--notes", &notes
        ])?;
        println!("Published immutable candidate {tag} ({digest})");
    }
    Ok(())
}

fn main() {
    if let Err(error) = publish() {
        eprintln!("Candidate publication failed: {error}");
        std::process::exit(1);
    }
}
