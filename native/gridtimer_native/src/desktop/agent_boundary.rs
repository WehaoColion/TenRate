//! Domain rules for desktop Agent source consent and saving a new document.
use serde::Serialize;
use serde_json::{json, Value};
use std::collections::BTreeSet;

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub(super) struct AgentSource {
    pub id: String,
    pub kind: String,
    pub title: String,
    pub folder: String,
    pub content: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct AgentContext {
    pub workspace: String,
    pub configuration: String,
    pub question: String,
    pub deep_review: bool,
    pub documents: Vec<AgentSource>,
    pub source_fingerprint: String,
}
impl AgentContext {
    pub fn scope_json(&self) -> Result<String, String> {
        if self.question.trim().is_empty() || self.question.chars().count() > 1000 {
            return Err("任务目标需要 1 至 1000 个字符".into());
        }
        if self.documents.is_empty() || self.documents.len() > 30 {
            return Err("请选择 1 至 30 条未加密便签或知识页".into());
        }
        let mut ids = BTreeSet::new();
        let mut total = 0usize;
        for source in &self.documents {
            let length = source.content.chars().count();
            if source.id.trim().is_empty()
                || source.id.chars().count() > 160
                || !ids.insert(&source.id)
                || source.title.chars().count() > 300
                || source.folder.chars().count() > 300
                || source.content.trim().is_empty()
                || length > 100_000
            {
                return Err(
                    "所选资料无效或单页超过 10 万字符，请减少范围；本次没有截断资料".into(),
                );
            }
            total = total.saturating_add(length);
        }
        let scope = json!({"question":self.question.trim(), "deepReview":self.deep_review, "documents":self.documents}).to_string();
        if total > 1_000_000 || scope.len() > 1_500_000 {
            return Err("所选资料超过本次 Agent 处理上限，请减少范围；本次没有截断资料".into());
        }
        Ok(scope)
    }
}

pub(super) fn agent_send_allowed(
    acknowledged: bool,
    workspace_ready: bool,
    can_start: bool,
    configuration_valid: bool,
    draft_saved: bool,
    context_current: bool,
) -> bool {
    acknowledged
        && workspace_ready
        && can_start
        && configuration_valid
        && draft_saved
        && context_current
}

pub(super) fn agent_result_verified(result: &Value, context: &AgentContext) -> bool {
    let checks = result
        .get("verification")
        .and_then(|v| v.get("checks"))
        .and_then(Value::as_array);
    let required = [
        "run_complete",
        "draft_complete",
        "tool_chain",
        "sources",
        "tasks",
        "deep_review",
        "write_boundary",
    ];
    let evidence = checks.is_some_and(|checks| {
        required.iter().all(|id| {
            checks
                .iter()
                .filter(|check| check["id"].as_str() == Some(*id))
                .count()
                == 1
                && checks
                    .iter()
                    .any(|check| check["id"].as_str() == Some(*id) && check["passed"] == true)
        })
    });
    let ids = result["draft"]["sourceIds"].as_array();
    let source_ids = ids
        .map(|ids| ids.iter().filter_map(Value::as_str).collect::<Vec<_>>())
        .unwrap_or_default();
    let unique = source_ids.iter().copied().collect::<BTreeSet<_>>();
    let authorized = !source_ids.is_empty()
        && ids.is_some_and(|ids| ids.len() == source_ids.len())
        && unique.len() == source_ids.len()
        && source_ids
            .iter()
            .all(|id| context.documents.iter().any(|source| source.id == *id));
    result["ok"] == true
        && result["verification"]["passed"] == true
        && evidence
        && authorized
        && result["requests"].as_u64().is_some_and(|v| v > 0)
        && result["toolCalls"].as_u64().is_some_and(|v| v > 0)
        && result["answer"]
            .as_str()
            .is_some_and(|v| !v.trim().is_empty())
        && result["draft"]["title"]
            .as_str()
            .is_some_and(|v| !v.trim().is_empty())
        && result["draft"]["content"]
            .as_str()
            .is_some_and(|v| !v.trim().is_empty())
        && (!context.deep_review || result["reviewCompleted"] == true)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum AgentSaveState {
    Running,
    Saved,
    ScopeChanged,
    NotReady,
    Ready,
}
pub(super) fn agent_save_state(
    running: bool,
    saved: bool,
    context_current: bool,
    workspace_ready: bool,
    verified: bool,
    title: &str,
    body: &str,
) -> AgentSaveState {
    if running {
        AgentSaveState::Running
    } else if saved {
        AgentSaveState::Saved
    } else if !context_current || !workspace_ready {
        AgentSaveState::ScopeChanged
    } else if !verified
        || title.trim().is_empty()
        || body.trim().is_empty()
        || title.chars().count() > 300
        || body.chars().count() > 120_000
    {
        AgentSaveState::NotReady
    } else {
        AgentSaveState::Ready
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn context() -> AgentContext {
        AgentContext {
            workspace: "a".into(),
            configuration: "service-model-key".into(),
            question: "整理待办".into(),
            deep_review: true,
            source_fingerprint: "synthetic-source-fingerprint".into(),
            documents: vec![AgentSource {
                id: "a".into(),
                kind: "知识页".into(),
                title: "记录".into(),
                folder: "".into(),
                content: "需要补齐测试".into(),
            }],
        }
    }
    fn result() -> Value {
        json!({"ok":true,"requests":4,"toolCalls":6,"answer":"整理完成", "reviewCompleted":true,
        "draft":{"title":"行动计划","content":"需要补齐测试", "sourceIds":["a"]},
        "verification":{"passed":true,"checks": (["run_complete","draft_complete","tool_chain","sources","tasks","deep_review","write_boundary"].map(|id| json!({"id":id,"passed":true})))}})
    }
    #[test]
    fn authorization_cannot_be_reused_after_the_scope_or_readiness_changes() {
        assert!(agent_send_allowed(true, true, true, true, true, true));
        assert!(!agent_send_allowed(false, true, true, true, true, true));
        assert!(!agent_send_allowed(true, false, true, true, true, true));
        assert!(!agent_send_allowed(true, true, false, true, true, true));
        assert!(!agent_send_allowed(true, true, true, false, true, true));
        assert!(!agent_send_allowed(true, true, true, true, false, true));
        assert!(!agent_send_allowed(true, true, true, true, true, false));
    }

    #[test]
    fn unknown_duplicate_missing_or_unreviewed_evidence_cannot_be_saved() {
        let scope = context();
        let good = result();
        assert!(agent_result_verified(&good, &scope));
        for change in [
            json!(["unknown"]),
            json!(["a", "a"]),
            json!([]),
            json!([null]),
        ] {
            let mut bad = good.clone();
            bad["draft"]["sourceIds"] = change;
            assert!(!agent_result_verified(&bad, &scope));
        }
        let mut bad = good.clone();
        bad["reviewCompleted"] = json!(false);
        assert!(!agent_result_verified(&bad, &scope));
        let mut bad = good.clone();
        bad["verification"]["checks"][0]["passed"] = json!(false);
        assert!(!agent_result_verified(&bad, &scope));
        let mut bad = good;
        bad["requests"] = json!(0);
        assert!(!agent_result_verified(&bad, &scope));
    }
    #[test]
    fn no_scope_overflow_or_duplicate_sources_are_silently_truncated() {
        let mut c = context();
        assert!(c.scope_json().is_ok());
        c.documents[0].content = "字".repeat(100_001);
        assert!(c.scope_json().is_err());
        let mut c = context();
        c.documents.push(c.documents[0].clone());
        assert!(c.scope_json().is_err());
        let mut c = context();
        c.documents.clear();
        assert!(c.scope_json().is_err());
    }
    #[test]
    fn save_gate_requires_completed_verified_current_context_and_a_complete_draft() {
        assert_eq!(
            agent_save_state(false, false, true, true, true, "标题", "正文"),
            AgentSaveState::Ready
        );
        for (running, saved, current, ready, verified, title, body) in [
            (true, false, true, true, true, "标题", "正文"),
            (false, true, true, true, true, "标题", "正文"),
            (false, false, false, true, true, "标题", "正文"),
            (false, false, true, false, true, "标题", "正文"),
            (false, false, true, true, false, "标题", "正文"),
            (false, false, true, true, true, "", "正文"),
            (false, false, true, true, true, "标题", ""),
        ] {
            assert_ne!(
                agent_save_state(running, saved, current, ready, verified, title, body),
                AgentSaveState::Ready
            );
        }
    }
}
