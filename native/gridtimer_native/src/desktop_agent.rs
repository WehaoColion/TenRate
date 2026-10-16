//! Desktop handle for the shared, authenticated source-grounded Agent.
//! A handle owns cancellation and observable execution state for exactly one run.
use crate::android_agent_progress::AgentProgress;
use serde_json::{json, Value};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

#[derive(Clone)]
pub struct DesktopAgentRun {
    task_id: String,
    cancelled: Arc<AtomicBool>,
    started: Arc<AtomicBool>,
    progress: Arc<AgentProgress>,
}

impl DesktopAgentRun {
    pub fn new(task_id: String) -> Self {
        let cancelled = Arc::new(AtomicBool::new(false));
        let progress = Arc::new(AgentProgress::new(&task_id, Arc::clone(&cancelled)));
        Self {
            task_id,
            cancelled,
            started: Arc::new(AtomicBool::new(false)),
            progress,
        }
    }

    pub fn cancel(&self) {
        self.progress.request_cancel();
    }

    /// Progress contains stage, elapsed time and counters only, never source data.
    pub fn progress(&self) -> Value {
        serde_json::to_value(self.progress.snapshot())
            .unwrap_or_else(|_| json!({"terminal":"failed"}))
    }

    /// Uses the same bounded provider tool loop, independent second stage and
    /// local evidence verification as Android. No local answer or write fallback.
    pub fn run(&self, key: &str, endpoint: &str, model: &str, scope: &str) -> Value {
        if self.started.swap(true, Ordering::AcqRel) {
            return json!({"ok":false,"message":"该 Agent 任务已经启动，不能重复发送", "requests":0,
                "verification":{"passed":false}});
        }
        crate::android_agent_upgrade::run_enhanced_android_knowledge_agent_with_progress(
            key,
            endpoint,
            model,
            scope,
            &self.task_id,
            &self.cancelled,
            Some(&self.progress),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn desktop_cancel_before_start_denies_every_model_request_and_save_verification() {
        let run = DesktopAgentRun::new("cancelled-desktop-test".into());
        run.cancel();
        let result = run.run(
            "never-sent",
            "https://api.example.test",
            "model",
            &json!({
                "question":"整理待办", "deepReview":true,
                "documents":[{"id":"a","title":"记录","folder":"","content":"需要测试"}]
            })
            .to_string(),
        );
        assert_eq!(result["requests"], 0);
        assert_eq!(result["verification"]["passed"], false);
        assert_eq!(run.progress()["terminal"], "cancelled");
    }
    #[test]
    fn desktop_missing_configuration_does_not_fake_a_successful_agent_result() {
        let run = DesktopAgentRun::new("unconfigured-desktop-test".into());
        let result = run.run("", "https://api.example.test", "model", "{}");
        assert_eq!(result["ok"], false);
        assert_eq!(result["requests"], 0);
        assert_eq!(result["verification"]["passed"], false);
        assert_eq!(run.progress()["terminal"], "failed");
    }
}
