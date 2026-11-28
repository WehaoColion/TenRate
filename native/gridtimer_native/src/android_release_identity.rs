//! Gradle is the packaging identity owner. All Rust consumers derive the same
//! values, including standalone simulator and publication tools.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AndroidReleaseIdentity {
    version_name: &'static str,
    version_code: i32,
}

const fn whitespace(c: u8) -> bool { matches!(c, b' ' | b'\t' | b'\r') }

const fn trim(line: &str) -> &str {
    let bytes = line.as_bytes();
    let mut first = 0;
    let mut last = bytes.len();
    while first < last && whitespace(bytes[first]) { first += 1; }
    while last > first && whitespace(bytes[last - 1]) { last -= 1; }
    line.split_at(first).1.split_at(last - first).0
}

const fn has_key(line: &str, key: &str) -> bool {
    let bytes = line.as_bytes();
    let token = key.as_bytes();
    if bytes.len() <= token.len() || !whitespace(bytes[token.len()]) { return false; }
    let mut i = 0;
    while i < token.len() {
        if bytes[i] != token[i] { return false; }
        i += 1;
    }
    true
}

// A duplicate, malformed or missing declaration is a compile-time error.
const fn unique_value(source: &'static str, key: &str) -> &'static str {
    let mut rest = source;
    let mut found = None;
    while !rest.is_empty() {
        let bytes = rest.as_bytes();
        let mut end = 0;
        while end < bytes.len() && bytes[end] != b'\n' { end += 1; }
        let (line, tail) = rest.split_at(end);
        rest = if tail.is_empty() { tail } else { tail.split_at(1).1 };
        let line = trim(line);
        if has_key(line, key) {
            if found.is_some() {
                panic!("Duplicate Android release declaration: keep one versionName and versionCode in app/build.gradle");
            }
            found = Some(trim(line.split_at(key.len()).1));
        }
    }
    match found {
        Some(value) => value,
        None => panic!("Missing Android release identity: set versionName and versionCode in app/build.gradle"),
    }
}

pub const fn parse_android_release(source: &'static str) -> AndroidReleaseIdentity {
    let quoted = unique_value(source, "versionName");
    let bytes = quoted.as_bytes();
    if bytes.len() < 3 || bytes[0] != b'\'' || bytes[bytes.len() - 1] != b'\'' {
        panic!("Android versionName must be a quoted numeric release in app/build.gradle");
    }
    let version_name = quoted.split_at(1).1.split_at(quoted.len() - 2).0;
    let mut parts = 1;
    let mut digits = 0;
    let mut i = 0;
    let version = version_name.as_bytes();
    while i < version.len() {
        let ch = version[i];
        if ch >= b'0' && ch <= b'9' { digits += 1; }
        else if ch == b'.' && digits > 0 { parts += 1; digits = 0; }
        else { panic!("Invalid Android versionName in app/build.gradle"); }
        i += 1;
    }
    if digits == 0 || parts < 2 || parts > 4 { panic!("Invalid Android versionName segments in app/build.gradle"); }
    let code = unique_value(source, "versionCode").as_bytes();
    let mut number: i32 = 0;
    let mut j = 0;
    while j < code.len() {
        let ch = code[j];
        if ch < b'0' || ch > b'9' || number > (i32::MAX - (ch - b'0') as i32) / 10 {
            panic!("Invalid Android versionCode in app/build.gradle");
        }
        number = number * 10 + (ch - b'0') as i32;
        j += 1;
    }
    if number <= 0 { panic!("Android versionCode must be positive in app/build.gradle"); }
    AndroidReleaseIdentity { version_name, version_code: number }
}

pub const ANDROID_RELEASE: AndroidReleaseIdentity = parse_android_release(include_str!("../../../app/build.gradle"));
pub const ANDROID_APP_VERSION: &str = ANDROID_RELEASE.version_name;
pub const ANDROID_VERSION_CODE: i32 = ANDROID_RELEASE.version_code;

impl AndroidReleaseIdentity {
    pub fn require_observed(self, name: &str, code: i32) -> Result<(), String> {
        if name == self.version_name && code == self.version_code { return Ok(()); }
        Err(format!("Android identity mismatch: expected {} / {}, observed {name} / {code}; rebuild/install the exact checked-out app/build.gradle release", self.version_name, self.version_code))
    }

    pub fn require_history_head(self, history: &str) -> Result<(), String> {
        let mut versions = history.lines().filter_map(|line| {
            let (field, value) = line.trim().split_once('=')?;
            if field.trim() != "version" { return None; }
            value.trim().strip_suffix(',')?.trim().strip_prefix('"')?.strip_suffix('"')
        });
        let head = versions.next().ok_or("Android update history is empty; add the current release as the first entry")?;
        if head != self.version_name {
            return Err(format!("Android update history is stale: {head}, expected {}; append the current release as the first AndroidUpdateEntry", self.version_name));
        }
        let previous = versions.next().ok_or("Android update history must retain the preceding release")?;
        let current = version_parts(head)?;
        let prior = version_parts(previous)?;
        if current[..3] != prior[..3] || prior[3].checked_add(1) != Some(current[3]) {
            return Err(format!("Android revision {previous} -> {head} is not +0.0.0.1; retain the preceding release and increment only the fourth segment (an approved version lane requires a separate policy revision)"));
        }
        Ok(())
    }

    pub fn candidate_tag(self, commit: &str) -> Result<String, String> {
        if commit.len() != 40 || !commit.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err("Candidate publication requires the exact 40-character build commit".into());
        }
        Ok(format!("v{}-candidate-{}", self.version_name, &commit[..12]))
    }
}

fn version_parts(version: &str) -> Result<[u32; 4], String> {
    let mut result = [0; 4];
    let mut count = 0;
    for (i, part) in version.split('.').enumerate() {
        if i >= result.len() || part.is_empty() || !part.bytes().all(|b| b.is_ascii_digit()) {
            return Err(format!("Invalid numeric release {version}"));
        }
        result[i] = part.parse().map_err(|_| format!("Invalid numeric release {version}"))?;
        count += 1;
    }
    if count < 2 { return Err(format!("Invalid numeric release {version}")); }
    Ok(result)
}

pub fn installed_version(info: &str) -> Result<&'static str, String> {
    let mut names = info.lines().filter_map(|line| line.trim().strip_prefix("versionName="));
    let name = names.next().ok_or("Installed package has no versionName; install the exact signed candidate")?;
    let mut codes = info.lines().filter_map(|line| {
        line.trim().strip_prefix("versionCode=")?.split_whitespace().next()?.parse::<i32>().ok()
    });
    let code = codes.next().ok_or("Installed package has no versionCode; install the exact signed candidate")?;
    ANDROID_RELEASE.require_observed(name, code)?;
    if names.any(|value| value != name) || codes.any(|value| value != code) {
        return Err("Ambiguous installed package identity; reinstall the exact signed candidate".into());
    }
    Ok(ANDROID_APP_VERSION)
}
