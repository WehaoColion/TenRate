// Windows-local legal scan exclusions. This is an explicit local privacy policy,
// NOT a synced knowledge document and never a server-side authority.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default, rename_all = "camelCase", deny_unknown_fields)]
struct DesktopLegalKnowledgeExclusions {
    root_excluded: bool,
    node_ids: Vec<String>,
}

impl DesktopLegalKnowledgeExclusions {
    fn validate(&self) -> Result<(), String> {
        if self.node_ids.len() > 1_000 {
            return Err("法律排除规则过多，已拒绝扫描".into());
        }
        let mut seen = HashSet::new();
        for id in &self.node_ids {
            if id.trim().is_empty() || id.len() > 512 || !seen.insert(id) {
                return Err("法律排除规则包含无效或重复的目录编号".into());
            }
        }
        Ok(())
    }
    fn digest(&self) -> String {
        let bytes = serde_json::to_vec(self).expect("validated exclusion policy is serializable");
        format!("{:x}", Sha256::digest(&bytes))
    }
}

fn desktop_legal_read_exclusions(path: &Path) -> Result<DesktopLegalKnowledgeExclusions, String> {
    const MAX_BYTES: u64 = 32 * 1024;
    let file = match std::fs::File::open(path) {
        Ok(file) => file,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            return Ok(DesktopLegalKnowledgeExclusions::default());
        }
        Err(error) => return Err(format!("法律排除规则无法读取，已停止扫描：{error}")),
    };
    let mut bytes = Vec::new();
    std::io::Read::take(file, MAX_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| format!("法律排除规则读取失败，已停止扫描：{error}"))?;
    if bytes.len() as u64 > MAX_BYTES {
        return Err("法律排除规则文件超过安全上限，已停止扫描".into());
    }
    let policy: DesktopLegalKnowledgeExclusions = serde_json::from_slice(&bytes)
        .map_err(|_| "法律排除规则损坏或版本未知，已停止扫描".to_string())?;
    policy.validate()?;
    Ok(policy)
}

fn desktop_legal_excluded_note_ids(
    index: &KnowledgeDirectoryIndex,
    policy: &DesktopLegalKnowledgeExclusions,
) -> Result<HashSet<String>, String> {
    policy.validate()?;
    if !policy.root_excluded && policy.node_ids.is_empty() {
        return Ok(HashSet::new());
    }
    let directly_excluded = policy.node_ids.iter().map(String::as_str)
        .collect::<HashSet<_>>();
    let mut excluded = HashSet::new();
    for (id, node) in &index.nodes {
        if !node.has_note { continue; }
        if policy.root_excluded {
            excluded.insert(id.clone());
            continue;
        }
        let mut cursor = Some(id.as_str());
        let mut visited = HashSet::new();
        while let Some(next) = cursor {
            if !visited.insert(next) {
                return Err("知识目录存在循环，无法保证法律排除范围，已停止扫描".into());
            }
            if directly_excluded.contains(next) {
                excluded.insert(id.clone());
                break;
            }
            cursor = index.nodes.get(next)
                .ok_or("知识目录存在缺失上级，无法保证法律排除范围，已停止扫描")?
                .parent.as_deref();
        }
    }
    Ok(excluded)
}

impl TimerWindowsClient {
    fn desktop_legal_exclusion_path(&self) -> PathBuf {
        // Workspace and local account are bound using the same identity family
        // as the directory preferences. Raw IDs are never used as file names.
        self.settings_path.parent().unwrap_or(Path::new(".")).join(format!(
            "knowledge_legal_exclusions_{}.json",
            sync_core::token_identifier(&format!("{}\n{}",
                self.state_path.display(),
                workspace_note_media_identity(&self.ai_workspace_identity()),
            )),
        ))
    }

    fn desktop_legal_update_exclusion(&mut self, target: Option<&str>) -> Result<bool, String> {
        if self.workspace_edit_locked() || !self.workspace_persistence_ready {
            return Err("当前工作区不可修改法律排除规则".into());
        }
        let path = self.desktop_legal_exclusion_path();
        let mut policy = desktop_legal_read_exclusions(&path)?;
        let excluded = if let Some(id) = target {
            let index = self.knowledge_directory_index();
            let node = index.nodes.get(id).ok_or("知识页面已经不存在")?;
            if node.issue.is_some() || node.encrypted {
                return Err("此知识页面存在编号冲突或加密状态，不允许修改排除规则".into());
            }
            // An inherited rule cannot be reversed inside an excluded parent.
            if desktop_legal_excluded_note_ids(&index, &policy)?.contains(id)
                && !policy.node_ids.iter().any(|saved| saved == id)
            {
                return Err("仍受上级排除规则约束，请到上级修改".into());
            }
            if policy.node_ids.iter().any(|saved| saved == id) {
                policy.node_ids.retain(|saved| saved != id);
                false
            } else {
                policy.node_ids.push(id.to_owned());
                true
            }
        } else {
            policy.root_excluded = !policy.root_excluded;
            policy.root_excluded
        };
        policy.node_ids.sort_unstable();
        policy.validate()?;
        let json = serde_json::to_string_pretty(&policy)
            .map_err(|error| format!("排除规则编码失败：{error}"))?;
        atomic_save_text(&path, &json, |raw| {
            let candidate = serde_json::from_str::<DesktopLegalKnowledgeExclusions>(raw).ok()?;
            candidate.validate().ok()?;
            serde_json::to_string_pretty(&candidate).ok()
        }).map_err(|error| format!("法律排除规则未保存：{error}"))?;
        if desktop_legal_read_exclusions(&path)? != policy {
            return Err("排除规则保存校验失败，已停止使用旧授权".into());
        }
        // Any previously prepared evidence and one-shot send authorization are
        // immediately stale, regardless of whether they were already displayed.
        self.desktop_ui.legal_risk.preview = None;
        self.desktop_ui.legal_risk.send_consent = None;
        self.desktop_ui.legal_risk.cancel.store(true, AtomicOrdering::Release);
        if let Some(cancel) = &self.desktop_ui.legal_risk.running_cancel {
            cancel.store(true, AtomicOrdering::Release);
        }
        Ok(excluded)
    }
}

#[cfg(test)]
mod desktop_legal_exclusion_tests {
    use super::*;
    #[test]
    fn invalid_policy_is_rejected_instead_of_defaulting_to_all_knowledge() {
        let p = std::env::temp_dir().join(format!("legal_policy_test_{}", random_hex_identifier()));
        std::fs::write(&p, b"{invalid").unwrap();
        assert!(desktop_legal_read_exclusions(&p).is_err());
        std::fs::remove_file(&p).unwrap();
        assert_eq!(desktop_legal_read_exclusions(&p).unwrap(),
            DesktopLegalKnowledgeExclusions::default());
    }
    #[test]
    fn inherited_exclusion_covers_children_without_affecting_siblings() {
        let mut index = KnowledgeDirectoryIndex::default();
        for (id, parent, has_note) in [
            ("folder", None, false),
            ("child", Some("folder"), true),
            ("sibling", None, true),
        ] {
            index.nodes.insert(id.into(), KnowledgeDirectoryNode {
                id: id.into(), title: id.into(),
                parent: parent.map(str::to_string), folder: !has_note,
                legacy: false, has_note, writable: true, encrypted: false,
                database: false, updated: 0, issue: None,
            });
        }
        let policy = DesktopLegalKnowledgeExclusions {
            root_excluded: false, node_ids: vec!["folder".into()],
        };
        let excluded = desktop_legal_excluded_note_ids(&index, &policy).unwrap();
        assert!(excluded.contains("child"));
        assert!(!excluded.contains("sibling"));
        assert!(!excluded.contains("folder"));
        let all = DesktopLegalKnowledgeExclusions { root_excluded: true, node_ids: Vec::new() };
        let excluded = desktop_legal_excluded_note_ids(&index, &all).unwrap();
        assert_eq!(excluded.len(), 2);
    }
}
