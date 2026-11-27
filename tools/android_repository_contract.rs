// v3.0.2.45 - Replay stale accessibility evidence failure before signing.
// v3.0.2.39 - Compact AI conversation layout, independent history and real emulator gate.
// v3.0.2.35 - Preserve the baseline and add million-row stress with exact receipts.
//! Cheap fail-closed checks run before any signing or simulator work.
#[allow(dead_code)]
#[path = "../native/gridtimer_native/src/android_release_identity.rs"]
mod android_release_identity;
#[cfg(test)]
mod android_directory_evidence;
#[cfg(test)]
mod android_fresh_hierarchy;
use android_release_identity::{ANDROID_APP_VERSION, ANDROID_RELEASE};
#[allow(dead_code)]
mod android_stress_profile;
use android_stress_profile::{BASELINE, EXTREME, HISTORY_PADDING_BYTES, SAVE_BUDGET_MS, RECOVERY_BUDGET_SECONDS};

const WORKFLOW: &str = include_str!("../.github/workflows/android-release.yml");
const HISTORY: &str = include_str!("../native/gridtimer_native/src/sourcegen/android_update_history_source.rs");
const RELEASE_INDEX: &str = include_str!("../documents/releases/README.md");
const CANDIDATE_PUBLISHER: &str = include_str!("android_candidate_publish.rs");
#[cfg(test)]
#[path = "android_ai_probe_text.rs"]
mod android_ai_probe_text;
#[cfg(test)]
#[path = "android_locale_probe_text.rs"]
mod android_locale_probe_text;
#[cfg(test)]
const AI_PROBE_SOURCE: &str = include_str!("android_ai_conversation_probe.rs");
#[cfg(test)]
const LOCALE_PROBE_SOURCE: &str = include_str!("android_locale_probe.rs");
#[cfg(test)]
#[path = "android_ime_marker.rs"]
mod android_ime_marker;
#[cfg(test)]
#[path = "android_heading_ink.rs"]
mod android_heading_ink;
#[cfg(test)]
const DOCUMENT_TYPING_PROBE: &str = include_str!("android_document_typing_probe.rs");
#[cfg(test)]
const SHARE_PROBE: &str = include_str!("android_knowledge_share_probe.rs");
#[cfg(test)]
const MOTION_PROBE: &str = include_str!("android_knowledge_motion_probe.rs");
#[cfg(test)]
#[path = "android_save_exit_policy.rs"]
mod android_save_exit_policy;
#[cfg(test)]
const SAVE_PROBE: &str = include_str!("android_knowledge_save_probe.rs");


fn require_release_index_head(index: &str, version: &str) -> Result<(), String> {
    let first = index.lines().find(|line| !line.trim().is_empty())
        .ok_or("Android release index is empty; add the current release as its first entry")?;
    let expected = format!("[Android {version} ");
    if !first.starts_with(&expected) {
        return Err(format!("Android release index is stale: expected {version} as its first entry; never infer the current version from a PR title or branch name"));
    }
    let relative = format!("android/release_notes_v{version}.md");
    if !first.contains(&relative) {
        return Err(format!("Android release index head must link {relative}"));
    }
    let note = std::path::Path::new("documents/releases").join(&relative);
    if !note.is_file() {
        return Err(format!("Missing current Android release note: {}", note.display()));
    }
    Ok(())
}

fn require_candidate_publisher_identity(source: &str) -> Result<(), String> {
    for required in [
        "env::var(\"GITHUB_SHA\")",
        "ANDROID_RELEASE.candidate_tag(&commit)?",
        "version != ANDROID_APP_VERSION",
        "\"--target\", &commit",
    ] {
        if !source.contains(required) {
            return Err(format!("Candidate publisher must use the checked-out Gradle identity and exact build SHA; missing {required}"));
        }
    }
    for forbidden in ["GITHUB_HEAD_REF", "GITHUB_REF_NAME", "pull_request.title"] {
        if source.contains(forbidden) {
            return Err(format!("Candidate publisher must not infer Android version from {forbidden}; use app/build.gradle at the checked-out commit"));
        }
    }
    Ok(())
}

fn indentation(line: &str) -> usize { line.len() - line.trim_start().len() }

fn job_lines<'a>(yaml: &'a str, name: &str) -> Result<Vec<&'a str>, String> {
    let title = format!("{name}:");
    let mut lines = yaml.lines();
    let header = lines.find(|line| line.trim() == title)
        .ok_or_else(|| format!("Missing required Android workflow job {name}; restore the core emulator gate"))?;
    let indent = indentation(header);
    Ok(lines.take_while(|line| line.trim().is_empty() || line.trim_start().starts_with('#') || indentation(line) > indent)
        .filter(|line| !line.trim().is_empty() && !line.trim_start().starts_with('#')).collect())
}

fn require_core_job(yaml: &str, name: &str, dual_api: bool) -> Result<(), String> {
    let lines = job_lines(yaml, name)?;
    let field_indent = lines.iter().map(|line| indentation(line)).min().ok_or("Empty required job")?;
    if lines.iter().any(|line| indentation(line) == field_indent && line.trim_start().starts_with("if:")) {
        return Err(format!("{name} is conditional; remove the job-level if so all signed builds execute emulator acceptance"));
    }
    if dual_api {
        let mut api = lines.iter().filter_map(|line| line.trim().strip_prefix("api:"));
        let value = api.next().ok_or("Knowledge save matrix is missing; require both API 35 and 36")?;
        let compact: String = value.chars().filter(|c| !c.is_whitespace()).collect();
        if (compact != "[35,36]" && compact != "[36,35]") || api.next().is_some() {
            return Err("Knowledge save matrix must include API 35 and 36 for every signed build; remove branch-specific matrix expressions".into());
        }
    }
    Ok(())
}

fn require_stress_profiles(yaml: &str) -> Result<(), String> {
    let lines = job_lines(yaml, "knowledge-save-emulator")?;
    let history = lines.iter().filter_map(|line| line.trim().strip_prefix("history:")).collect::<Vec<_>>();
    if history.len() != 1 || history[0].chars().filter(|c| !c.is_whitespace()).collect::<String>() != "[baseline,extreme]" {
        return Err("Knowledge save must run both baseline and extreme on each API; restore history: [baseline, extreme]".into());
    }
    if BASELINE.history_rows < 500_000 || EXTREME.history_rows < 1_000_000
        || BASELINE.edit_rounds < 12 || EXTREME.edit_rounds < 48
        || BASELINE.burst_rounds < 30 || EXTREME.burst_rounds < 90
        || BASELINE.sticky_rounds < 24 || EXTREME.sticky_rounds < 48
        || HISTORY_PADDING_BYTES < 16 * 1024 || SAVE_BUDGET_MS > 8_000 || RECOVERY_BUDGET_SECONDS > 600 {
        return Err("Stress workload or deadline was weakened; restore the baseline and million-row profile in tools/android_stress_profile.rs".into());
    }
    Ok(())
}

fn check_repository() -> Result<(), String> {
    ANDROID_RELEASE.require_history_head(HISTORY)?;
    require_release_index_head(RELEASE_INDEX, ANDROID_APP_VERSION)?;
    require_candidate_publisher_identity(CANDIDATE_PUBLISHER)?;
    require_core_job(WORKFLOW, "knowledge-save-emulator", true)?;
    require_core_job(WORKFLOW, "knowledge-motion-emulator", false)?;
    require_core_job(WORKFLOW, "ai-conversation-emulator", true)?;
    require_core_job(WORKFLOW, "localization-emulator", true)?;
    require_stress_profiles(WORKFLOW)?;
    let _ = job_lines(WORKFLOW, "publish-signed-test-candidate")?;
    for old in ["publish-build37-download", "publish-signed-motion-test-candidate", "publish-knowledge-save-candidate", "publish-candidate"] {
        if job_lines(WORKFLOW, old).is_ok() {
            return Err(format!("Obsolete publisher {old}; use only publish-signed-test-candidate and tools/android_candidate_publish.rs"));
        }
    }
    Ok(())
}

#[cfg(not(test))]
fn main() {
    if let Err(error) = check_repository() {
        eprintln!("Android repository contract failed: {error}");
        std::process::exit(1);
    }
    println!("Android repository contract passed");
}

#[cfg(test)]
mod tests {
    use super::*;
    use android_release_identity::parse_android_release;
    use android_directory_evidence::prove_directory;

    #[test]
    fn historical_stale_release_index_is_rejected() {
        let stale = "[Android 3.0.2.15 知识模块连续动效修订](android/release_notes_v3.0.2.15.md)。";
        let error = require_release_index_head(stale, ANDROID_APP_VERSION).unwrap_err();
        assert!(error.contains("stale"));
        assert!(require_release_index_head(RELEASE_INDEX, ANDROID_APP_VERSION).is_ok());
    }

    #[test]
    fn stale_branch_label_cannot_choose_the_candidate_version() {
        let historical_branch = "fix/android-knowledge-motion-v3.0.2.15";
        let tag = ANDROID_RELEASE.candidate_tag("0cbb8089c76df2431b504f5af063a101af273bdd").unwrap();
        assert!(historical_branch.ends_with("3.0.2.15"));
        assert!(tag.starts_with(&format!("v{}-candidate-", ANDROID_APP_VERSION)));
        assert!(!tag.contains("3.0.2.15"));
        require_candidate_publisher_identity(CANDIDATE_PUBLISHER).unwrap();
    }

    fn motion_probe_has_unrelated_settings_gate(source: &str) -> bool {
        [
            "排除法律风险分析",
            "Legal analysis exclusion",
            "知识设置",
            "文件夹设置",
            "管理当前页面或文件夹",
        ].iter().any(|needle| source.contains(needle))
    }

    #[test]
    fn historical_motion_probe_legal_settings_gate_is_rejected() {
        // Replays the actual 3.0.2.49 API 35 failure before any motion frames were measured.
        let historical = r#"
            if !tap_any(&["知识设置"])? { return Err("Root knowledge settings entry is not visible".into()); }
            if !legal_menu.contains("排除法律风险分析") {
                return Err("Legal analysis exclusion is not reachable from knowledge settings".into());
            }
            if !settings_menu.contains("排除法律风险分析") {
                return Err("Folder legal exclusion is not reachable from its settings".into());
            }
        "#;
        assert!(motion_probe_has_unrelated_settings_gate(historical));
        assert!(!motion_probe_has_unrelated_settings_gate(MOTION_PROBE));
        assert!(MOTION_PROBE.contains("open_folder_with_motion(root)?"));
    }

    #[test]
    fn historical_english_ai_composer_fixture_is_recognized() {
        // Exact visible strings from the 3.0.2.49 API 35 screenshot and accessibility tree.
        let fixture = r#"<node text="Enter a question"/><node text="+ Image"/>"#;
        assert!(!fixture.contains("输入问题") && !fixture.contains("继续追问"));
        assert!(android_ai_probe_text::composer_visible(fixture));
        assert!(android_ai_probe_text::COMPOSER_LABELS.contains(&"Enter a question"));
        assert!(android_ai_probe_text::image_action_visible(fixture));
    }

    #[test]
    fn historical_japanese_knowledge_navigation_alias_is_recognized() {
        // Exact visible label from the 3.0.2.49 API 35/36 screenshots and accessibility trees.
        let fixture = r#"<node text="知識"/>"#;
        assert_eq!(
            android_locale_probe_text::matched_navigation_label("ja-JP", "ナレッジ", fixture),
            Some("知識")
        );
        assert_eq!(
            android_locale_probe_text::matched_navigation_label("en-US", "Knowledge", r#"<node text="Knowledge"/>"#),
            Some("Knowledge")
        );
        assert_eq!(
            android_locale_probe_text::matched_navigation_label("fr-FR", "Connaissances", fixture),
            None
        );
    }

    #[test]
    fn nested_editor_exit_sends_second_android_system_back() {
        assert!(android_save_exit_policy::should_send_editor_exit_back(false, true, false));
        assert!(!android_save_exit_policy::should_send_editor_exit_back(false, true, true));
        assert!(!android_save_exit_policy::should_send_editor_exit_back(true, false, false));
        assert!(SAVE_PROBE.contains("should_send_editor_exit_back(is_parent, editor, back_sent)"));
        assert!(SAVE_PROBE.contains(r#"cmd(&["shell", "input", "keyevent", "4"])?;"#));
    }

    #[test]
    fn standalone_probes_use_role_scoped_text_helpers() {
        assert!(AI_PROBE_SOURCE.contains("mod android_ai_probe_text;"));
        assert!(!AI_PROBE_SOURCE.contains("mod android_locale_probe_text;"));
        assert!(LOCALE_PROBE_SOURCE.contains("mod android_locale_probe_text;"));
        assert!(!LOCALE_PROBE_SOURCE.contains("mod android_ai_probe_text;"));
    }

    #[test]
    fn document_typing_uses_compile_time_ime_stable_markers() {
        for historical in ["StableHeading", "Stable heading", "StableParagraph", "NoJump", "HotSave", "ExitSave"] {
            assert!(!android_ime_marker::is_ime_stable_marker(historical));
            assert!(!DOCUMENT_TYPING_PROBE.contains(historical));
            assert!(!SHARE_PROBE.contains(historical));
        }
        for (name, call) in [
            ("HEADING_TEXT", "write_marker(HEADING_TEXT)"),
            ("PARAGRAPH_TEXT", "write_marker(PARAGRAPH_TEXT)"),
            ("NO_JUMP_TEXT", "write_marker(NO_JUMP_TEXT)"),
        ] {
            assert!(DOCUMENT_TYPING_PROBE.contains(&format!(
                "const {name}: ImeStableMarker = ImeStableMarker::new("
            )));
            assert!(DOCUMENT_TYPING_PROBE.contains(call));
        }
        assert!(DOCUMENT_TYPING_PROBE.contains("HOT_SAVE_TEXT.as_str()"));
        assert!(DOCUMENT_TYPING_PROBE.contains("EXIT_SAVE_TEXT.as_str()"));
        assert!(SHARE_PROBE.contains("PARAGRAPH_TEXT.as_str()"));
        assert!(SHARE_PROBE.contains("NO_JUMP_TEXT.as_str()"));
    }

    #[test]
    fn historical_wrapped_share_paragraph_does_not_shift_heading_detection() {
        // API 35/36 artifacts 15234674079 and 15235331537 contain the same
        // 12 ink bands. The original paragraph wraps to two lines, so the old
        // fixed get(4..10) offset selected the paragraph tail as the first H1.
        let bands = [58, 25, 37, 24, 24, 37, 30, 24, 37, 30, 24, 22];
        let sample = android_heading_ink::select_heading_ink_sequence(&bands).unwrap();
        assert_eq!((sample.first_h1, sample.first_h2, sample.first_body), (37, 30, 24));
        assert_eq!((sample.second_h1, sample.second_h2, sample.second_body), (37, 30, 24));
        assert!(SHARE_PROBE.contains("select_heading_ink_sequence(&heights)"));
        assert!(!SHARE_PROBE.contains("heights.get(4..10)"));
    }

    #[test]
    fn sticky_long_input_is_paced_and_verified_before_version_creation() {
        // API 35 artifact 15234674079 observed 247/249 bytes when a 40-digit
        // row was sent in one shell command; API 36 artifact 15235331537 passed.
        // Pace the same exact marker in chunks and verify each authored row.
        assert!(SAVE_PROBE.contains("ImeStableText::new(row)"));
        assert!(SAVE_PROBE.contains("stable.chunks(10)"));
        assert!(SAVE_PROBE.contains("verify_sticky_body_progress(&evidence, index, &expected)?;"));
        assert!(!SAVE_PROBE.contains("cmd(&[\"shell\",\"input\",\"text\",&sticky_authored_row(index)])?;"));
    }

    #[test]
    fn current_repository_contract() { check_repository().unwrap(); }

    #[test]
    fn historical_single_workload_is_rejected() {
        // 3.0.2.34 ran only one 500,000-row workload on both APIs.
        let previous = "  knowledge-save-emulator:\n    needs: release\n    strategy:\n      matrix:\n        api: [35, 36]\n";
        assert!(require_stress_profiles(previous).unwrap_err().contains("baseline and extreme"));
        assert!(require_stress_profiles(WORKFLOW).is_ok());
    }

    #[test]
    fn historical_8f4b42d54bd3_stale_native_identity_is_rejected() {
        // Exact packaging and native values from commit 8f4b42d54bd395b1838e5fc29b2d1f7b30d850ce.
        let release = parse_android_release("        versionCode 30018\n        versionName '3.0.2.16'\n");
        let error = release.require_observed("3.0.2.15", 30017).unwrap_err();
        assert!(error.contains("identity mismatch"));
        assert!(release.require_observed("3.0.2.16", 30018).is_ok());
        println!("Historical 8f4b42d54bd3 rejected: {error}");
    }

    #[test]
    fn historical_3e9f16081e1e_stale_history_is_rejected() {
        // Exact declarations and first two entries from 3e9f16081e1eff2bcacb73190f1dcb826065b728.
        let release = parse_android_release("        versionCode 30020\n        versionName '3.0.2.18'\n");
        let old = "        version = \"3.0.2.15\",\n        version = \"3.0.2.14\",\n";
        let error = release.require_history_head(old).unwrap_err();
        assert!(error.contains("history is stale"));
        assert!(release.require_history_head("        version = \"3.0.2.18\",\n        version = \"3.0.2.17\",\n").is_ok());
        println!("Historical 3e9f16081e1e rejected: {error}");
    }

    #[test]
    fn historical_branch_only_gates_are_rejected() {
        // Actual job fields from d964144d3ab8aacad7c9be2e8fa241e625ecc1c4.
        let old_save = "  knowledge-save-emulator:\n    if: github.ref == 'refs/heads/main' || startsWith(github.ref, 'refs/heads/perf/android-knowledge-commit-') || github.ref == 'refs/heads/perf/android-save-preflight-current-v3.0.2.4' || github.ref == 'refs/heads/fix/android-knowledge-motion-v3.0.2.15'\n    needs: release\n    strategy:\n      matrix:\n        api: ${{ fromJSON(github.ref == 'refs/heads/fix/android-knowledge-motion-v3.0.2.15' && '[35,36]' || '[35]') }}\n";
        let old_motion = "  knowledge-motion-emulator:\n    name: Android signed APK continuous knowledge motion on API 35\n    if: github.ref == 'refs/heads/fix/android-knowledge-motion-v3.0.2.15'\n    needs: release\n";
        for (old, job, matrix) in [(old_save, "knowledge-save-emulator", true), (old_motion, "knowledge-motion-emulator", false)] {
            let err = require_core_job(old, job, matrix).unwrap_err();
            println!("Historical d964144d3ab8 rejected: {err}");
        }
        let without_if = old_save.lines().filter(|line| !line.trim_start().starts_with("if:")).collect::<Vec<_>>().join("\n");
        assert!(require_core_job(&without_if, "knowledge-save-emulator", true).is_err());
    }

    #[test]
    fn same_version_candidates_have_distinct_immutable_tags() {
        // Both commits are in the original 3.0.2.15 revision sequence.
        let release = parse_android_release("versionCode 30017\nversionName '3.0.2.15'\n");
        let a = release.candidate_tag("e2e3b7ac410daaed4b3ebecb2446b764428c22d0").unwrap();
        let b = release.candidate_tag("6d7629db6d5c562d83c324966a6a9c81c2582c34").unwrap();
        // The retired unscoped publisher used 'v' + version for both.
        assert_ne!(a, b);
        assert_ne!(a, "v3.0.2.15");
        assert_ne!(b, "v3.0.2.15");
    }

    #[test]
    fn breadcrumb_and_inline_editor_controls_are_not_directory_evidence() {
        // Reconstruct the false positive corrected by 6d7629db6d5c562d83c324966a6a9c81c2582c34.
        // Old predicate: ui.contains(NESTED_FOLDER) && ui.contains("新建子页面").
        // The fixture models that documented state; it is not an archived screenshot.
        fn node(text: &str, desc: &str, class: &str) -> String {
            format!("<node text=\"{text}\" content-desc=\"{desc}\" class=\"{class}\" package=\"com.ofairyo.gridtimer\" bounds=\"[0,0][100,50]\"/>")
        }
        let title = "7391804025006005";
        let folder = [node(title,"","android.widget.TextView"), node("新建子页面","","android.widget.Button"),
            node("新建子文件夹","","android.widget.Button"),node("","文件夹设置","android.widget.Button")].concat();
        let editor = folder.clone() + &node("正文","","android.widget.EditText");
        assert!(editor.contains(title) && editor.contains("新建子页面"));
        assert!(!prove_directory(&editor, Some(title)));
        assert!(prove_directory(&folder, Some(title)));
        assert!(!prove_directory(&folder.replace("[0,0][100,50]", "[0,0][0,0]"), Some(title)));
    }
}
