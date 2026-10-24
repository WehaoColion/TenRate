// Windows Agent UI implementation. This module is application source only.
#[path = "agent_boundary.rs"]
mod desktop_agent_boundary;
use desktop_agent_boundary::{
    agent_result_verified, agent_save_state, agent_send_allowed, AgentContext, AgentSaveState,
    AgentSource,
};

struct DesktopAgentPreview {
    context: AgentContext,
    scope_json: String,
    acknowledged: bool,
}
struct DesktopAgentCompletion {
    task_id: String,
    context: AgentContext,
    result: Value,
}
struct DesktopAgentActive {
    task_id: String,
    context: AgentContext,
    cancelled: bool,
    handle: gridtimer_native::desktop_agent::DesktopAgentRun,
    rx: mpsc::Receiver<DesktopAgentCompletion>,
}
struct DesktopAgentDraft {
    context: AgentContext,
    result: Value,
    title: String,
    body: String,
    generated_title: String,
    generated_body: String,
    verified: bool,
    saved_id: Option<String>,
}
#[derive(Default)]
struct DesktopAgentSession {
    open: bool,
    step: u8,
    confirm_discard: bool,
    question: String,
    deep_review: bool,
    selected_ids: std::collections::BTreeSet<String>,
    priority_note_id: Option<String>,
    workspace: String,
    configuration: String,
    cached_version: Option<u64>,
    sources: Vec<AgentSource>,
    source_fingerprints: BTreeMap<String, String>,
    source_lengths: BTreeMap<String, usize>,
    preview: Option<DesktopAgentPreview>,
    active: Option<DesktopAgentActive>,
    draft: Option<DesktopAgentDraft>,
    message: String,
    filter: String,
}

impl TimerWindowsClient {
    fn cancel_desktop_agent(&mut self) {
        self.desktop_agent.preview = None;
        if let Some(active) = self.desktop_agent.active.as_mut() {
            active.cancelled = true;
            active.handle.cancel();
            self.desktop_agent.message =
                "正在停止后续步骤；本次在途请求结束前仍会等待，已发送的请求可能计费".into();
        }
    }
    fn refresh_desktop_agent_sources(&mut self) {
        let workspace = self.background_job_workspace_fingerprint();
        let configuration = desktop_ai_configuration_binding(&self.sync);
        let changed_workspace =
            !self.desktop_agent.workspace.is_empty() && self.desktop_agent.workspace != workspace;
        let changed_config = !self.desktop_agent.configuration.is_empty()
            && self.desktop_agent.configuration != configuration;
        if changed_workspace
            || changed_config
            || self.workspace_edit_locked()
            || !self.background_work_is_allowed()
        {
            self.cancel_desktop_agent();
        }
        if changed_workspace || changed_config {
            self.desktop_agent.draft = None;
        }
        if changed_workspace {
            self.desktop_agent.question.clear();
            self.desktop_agent.selected_ids.clear();
            self.desktop_agent.priority_note_id = None;
            self.desktop_agent.sources.clear();
            self.desktop_agent.filter.clear();
            self.desktop_agent.cached_version = None;
            self.desktop_agent.open = false;
        }
        self.desktop_agent.workspace = workspace;
        self.desktop_agent.configuration = configuration;
        let version = self.notes_cache_version();
        if self.desktop_agent.cached_version == Some(version) {
            self.invalidate_desktop_agent_stale_context();
            return;
        }
        self.desktop_agent.cached_version = Some(version);
        let folders = self
            .data
            .note_folders
            .iter()
            .map(|folder| (folder.id.as_str(), folder.name.as_str()))
            .collect::<BTreeMap<_, _>>();
        self.desktop_agent.sources = self
            .data
            .notes
            .iter()
            .filter(|note| note.deleted_at_epoch_millis.is_none() && note.encryption.is_none())
            .map(|note| AgentSource {
                id: note.id.clone(),
                kind: if desktop_note_kind(note) == DesktopNoteKind::Document {
                    "知识页"
                } else {
                    "便签"
                }
                .into(),
                title: note.title.clone(),
                folder: note
                    .folder_id
                    .as_deref()
                    .and_then(|id| folders.get(id))
                    .copied()
                    .unwrap_or("未入库")
                    .to_owned(),
                content: desktop_ai_current_note_text(&note.content, &note.document),
            })
            .filter(|source| !source.content.trim().is_empty())
            .collect();
        self.desktop_agent.source_lengths = self
            .desktop_agent
            .sources
            .iter()
            .map(|source| (source.id.clone(), source.content.chars().count()))
            .collect();
        self.desktop_agent.source_fingerprints = self
            .desktop_agent
            .sources
            .iter()
            .map(|source| {
                (
                    source.id.clone(),
                    format!("{:x}", Sha256::digest(json!(source).to_string().as_bytes())),
                )
            })
            .collect();
        self.desktop_agent.selected_ids.retain(|id| {
            self.desktop_agent
                .sources
                .iter()
                .any(|source| &source.id == id)
        });
        self.invalidate_desktop_agent_stale_context();
    }
    fn invalidate_desktop_agent_stale_context(&mut self) {
        if self
            .desktop_agent
            .active
            .as_ref()
            .is_some_and(|active| !self.desktop_agent_context_current(&active.context))
        {
            self.cancel_desktop_agent();
        }
        if self
            .desktop_agent
            .preview
            .as_ref()
            .is_some_and(|preview| !self.desktop_agent_context_current(&preview.context))
        {
            self.desktop_agent.preview = None;
            self.desktop_agent.message = "资料已变化，请重新核对本次发送范围".into();
        }
    }
    fn desktop_agent_source_fingerprint(&self) -> String {
        let selected = self
            .desktop_agent
            .selected_ids
            .iter()
            .map(|id| (id, self.desktop_agent.source_fingerprints.get(id)))
            .collect::<Vec<_>>();
        format!(
            "{:x}",
            Sha256::digest(json!(selected).to_string().as_bytes())
        )
    }
    fn desktop_agent_context_current(&self, context: &AgentContext) -> bool {
        !(self.note_dirty
            && context
                .documents
                .iter()
                .any(|source| source.id == self.selected_note_id))
            && context.workspace == self.background_job_workspace_fingerprint()
            && context.configuration == desktop_ai_configuration_binding(&self.sync)
            && context.question == self.desktop_agent.question.trim()
            && context.deep_review == self.desktop_agent.deep_review
            && context.source_fingerprint == self.desktop_agent_source_fingerprint()
            && (self.desktop_agent.priority_note_id.is_none()
                || context.documents.len() == 1
                    && Some(&context.documents[0].id)
                        == self.desktop_agent.priority_note_id.as_ref())
    }

    fn desktop_agent_context(&self) -> Result<AgentContext, String> {
        let documents = self
            .desktop_agent
            .selected_ids
            .iter()
            .map(|id| {
                self.desktop_agent
                    .sources
                    .iter()
                    .find(|source| &source.id == id)
                    .cloned()
                    .ok_or_else(|| "已选资料无法读取，请重新选择".to_owned())
            })
            .collect::<Result<Vec<_>, _>>()?;
        if let Some(priority) = self.desktop_agent.priority_note_id.as_ref() {
            if documents.len() != 1 || &documents[0].id != priority {
                return Err("当前文档已经不可读取，没有改用其他页面".into());
            }
        }
        let context = AgentContext {
            workspace: self.background_job_workspace_fingerprint(),
            configuration: desktop_ai_configuration_binding(&self.sync),
            question: self.desktop_agent.question.trim().into(),
            deep_review: self.desktop_agent.deep_review,
            source_fingerprint: self.desktop_agent_source_fingerprint(),
            documents,
        };
        context.scope_json()?;
        Ok(context)
    }
    fn open_desktop_agent_entry(&mut self, priority_note_id: Option<String>) {
        if self.flush_document_operation_drafts().is_err() {
            return;
        }
        self.refresh_desktop_agent_sources();
        if self.desktop_agent.active.is_some() {
            self.desktop_agent.open = true;
            self.desktop_agent.step = 3;
            return;
        }
        if self.desktop_agent.draft.as_ref().is_some_and(|draft| draft.saved_id.is_none()) {
            self.desktop_agent.open = true;
            self.desktop_agent.step = 4;
            self.desktop_agent.message = "已有未保存草稿，请先保存或明确放弃。".into();
            return;
        }
        self.desktop_agent.step = 0;
        self.desktop_agent.confirm_discard = false;
        self.desktop_agent.priority_note_id = priority_note_id.clone();
        self.desktop_agent.selected_ids.clear();
        if let Some(id) = priority_note_id {
            if self
                .desktop_agent
                .sources
                .iter()
                .any(|source| source.id == id)
            {
                self.desktop_agent.selected_ids.insert(id);
            } else {
                self.status = "当前文档不可读取；加密或已删除内容不纳入 Agent".into();
                return;
            }
        }
        self.desktop_agent.deep_review = true;
        self.desktop_agent.preview = None;
        self.desktop_agent.draft = None;
        self.desktop_agent.message.clear();
        self.desktop_agent.open = true;
    }
    fn prepare_desktop_agent(&mut self) {
        self.desktop_agent.preview = None;
        if !self.background_work_is_allowed()
            || self.workspace_edit_locked()
            || !self.workspace_persistence_ready
            || self.knowledge_ai_pending
            || self.desktop_agent.active.is_some()
        {
            self.desktop_agent.message = "请等待当前任务或账户切换结束，并确认本地资料可写".into();
            return;
        }
        if let Err(error) = self.flush_document_operation_drafts() {
            self.desktop_agent.message = error.to_string();
            return;
        }
        self.refresh_desktop_agent_sources();
        let preview = (|| -> Result<_, String> {
            desktop_ai_configured_recipient(&self.sync)?;
            let context = self.desktop_agent_context()?;
            let scope_json = context.scope_json()?;
            Ok(DesktopAgentPreview {
                context,
                scope_json,
                acknowledged: false,
            })
        })();
        match preview {
            Ok(preview) => {
                self.desktop_agent.preview = Some(preview);
                self.desktop_agent.step = 2;
            },
            Err(error) => self.desktop_agent.message = error,
        }
    }
    fn send_desktop_agent(&mut self) {
        self.refresh_desktop_agent_sources();
        let Some(preview) = self.desktop_agent.preview.take() else {
            return;
        };
        let ready = agent_send_allowed(
            preview.acknowledged,
            self.workspace_persistence_ready && !self.workspace_edit_locked(),
            self.background_work_is_allowed()
                && !self.knowledge_ai_pending
                && self.desktop_agent.active.is_none(),
            desktop_ai_configured_recipient(&self.sync).is_ok(),
            !self.note_dirty,
            self.desktop_agent_context_current(&preview.context),
        );
        if !ready {
            self.desktop_agent.message = "发送条件或资料已变化，请重新预览后授权".into();
            return;
        }
        let task_id = random_desktop_identifier("desktop-agent");
        let handle = gridtimer_native::desktop_agent::DesktopAgentRun::new(task_id.clone());
        let worker_handle = handle.clone();
        let context = preview.context;
        let worker_context = context.clone();
        let worker_id = task_id.clone();
        let key = self.sync.ai_api_key.clone();
        let base = self.sync.ai_base_url.clone();
        let model = self.sync.ai_model.clone();
        let (tx, rx) = mpsc::channel();
        self.desktop_agent.draft = None;
        match self.task_supervisor.spawn(
            RuntimeTaskKind::KnowledgeAiRequest,
            TaskDurability::Ephemeral,
            move |cancellation| {
                if cancellation.is_cancelled() {
                    worker_handle.cancel();
                }
                let result = worker_handle.run(&key, &base, &model, &preview.scope_json);
                let _ = tx.send(DesktopAgentCompletion {
                    task_id: worker_id,
                    context: worker_context,
                    result,
                });
            },
        ) {
            Ok(_) => {
                self.desktop_agent.active = Some(DesktopAgentActive {
                    task_id,
                    context,
                    cancelled: false,
                    handle,
                    rx,
                });
                self.desktop_agent.step = 3;
                self.desktop_agent.message = "正在连接模型；本次任务只生成可编辑草稿".into();
            }
            Err(error) => self.desktop_agent.message = format!("Agent 无法启动：{error}"),
        }
    }
    fn poll_desktop_agent(&mut self) {
        if self.desktop_agent.open
            || self.desktop_agent.active.is_some()
            || self.desktop_agent.draft.is_some()
        {
            self.refresh_desktop_agent_sources();
        }
        let Some(active) = self.desktop_agent.active.take() else {
            return;
        };
        match active.rx.try_recv() {
            Ok(completion) => {
                let cancelled =
                    active.cancelled || active.handle.progress()["cancelRequested"] == true;
                let current = !self.workspace_edit_locked()
                    && self.background_work_is_allowed()
                    && completion.task_id == active.task_id
                    && completion.context == active.context
                    && self.desktop_agent_context_current(&completion.context);
                if cancelled || !current {
                    self.desktop_agent.step = 1;
                    self.desktop_agent.message =
                        "任务已取消或资料、账户、配置已变化，本次结果不能保存；可重新核对后运行"
                            .into();
                    return;
                }
                let result = completion.result;
                let verified = agent_result_verified(&result, &completion.context);
                let draft = result.get("draft");
                let title = draft
                    .and_then(|d| d["title"].as_str())
                    .unwrap_or("")
                    .to_owned();
                let mut body = draft
                    .and_then(|d| d["content"].as_str())
                    .unwrap_or("")
                    .to_owned();
                if let Some(items) = draft
                    .and_then(|d| d["actionItems"].as_array())
                    .filter(|items| !items.is_empty())
                {
                    body.push_str("\n\n## 待办\n");
                    for item in items.iter().filter_map(Value::as_str) {
                        body.push_str(&format!("- [ ] {item}\n"));
                    }
                }
                self.desktop_agent.message = result["message"]
                    .as_str()
                    .unwrap_or("任务结束，没有返回可保存草稿")
                    .into();
                self.desktop_agent.step = 4;
                self.desktop_agent.draft = Some(DesktopAgentDraft {
                    context: completion.context,
                    result,
                    generated_title: title.clone(),
                    generated_body: body.clone(),
                    title,
                    body,
                    verified,
                    saved_id: None,
                });
            }
            Err(mpsc::TryRecvError::Empty) => self.desktop_agent.active = Some(active),
            Err(mpsc::TryRecvError::Disconnected) => {
                self.desktop_agent.step = 1;
                self.desktop_agent.message = "Agent 执行已中断，没有写入知识库；请重新运行".into()
            }
        }
    }
    fn desktop_agent_save_state(&self) -> AgentSaveState {
        let Some(draft) = self.desktop_agent.draft.as_ref() else {
            return AgentSaveState::NotReady;
        };
        agent_save_state(
            self.desktop_agent.active.is_some(),
            draft.saved_id.is_some(),
            self.desktop_agent_context_current(&draft.context),
            self.workspace_persistence_ready
                && !self.workspace_edit_locked()
                && self.background_work_is_allowed(),
            draft.verified && agent_result_verified(&draft.result, &draft.context),
            &draft.title,
            &draft.body,
        )
    }
    fn save_desktop_agent_draft(&mut self) {
        if self.flush_document_operation_drafts().is_err() {
            self.desktop_agent.message = self.status.clone();
            return;
        }
        self.refresh_desktop_agent_sources();
        if self.desktop_agent_save_state() != AgentSaveState::Ready {
            self.desktop_agent.message =
                "当前草稿不能保存，请检查核验结果、来源变化和本地保存状态".into();
            return;
        }
        let draft = self.desktop_agent.draft.as_ref().unwrap();
        let id = random_desktop_identifier("page");
        let mut body = draft.body.clone();
        body.push_str("\n\n## 来源\n");
        for source_id in draft.result["draft"]["sourceIds"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
        {
            let Some(source) = draft
                .context
                .documents
                .iter()
                .find(|source| source.id == source_id)
            else {
                return;
            };
            body.push_str(&format!(
                "- {}（{}，记录 {}）\n",
                source.title, source.folder, source.id
            ));
        }
        let blocks = knowledge::markdown_blocks(&body, new_desktop_note_block_id);
        let pages = json!([{"id":id,"kind":"DOCUMENT","title":draft.title.trim(),"document":{"knowledge":knowledge::KnowledgePage::default(),"blocks":blocks}}]);
        let next = app_data::upsert_knowledge_pages_app_data_json(
            &self.state_json,
            &pages.to_string(),
            now_millis(),
        );
        let outcome = self.replace_state_outcome(next, "Agent 草稿已保存为新的知识页");
        if outcome.is_committed() {
            self.desktop_agent.draft.as_mut().unwrap().saved_id = Some(id);
            self.desktop_agent.message = if outcome == StateReplaceOutcome::CommittedWithWarning {
                format!("新知识页已提交，本地恢复副本仍需处理：{}", self.status)
            } else {
                "已保存为新的知识页；本任务不能重复保存".into()
            };
        } else {
            self.desktop_agent.message = format!("保存失败，草稿仍保留：{}", self.status);
        }
    }
    fn open_desktop_agent_source(&mut self, note_id: &str) {
        if self.flush_document_operation_drafts().is_err() {
            return;
        }
        let Some(note) = self
            .data
            .notes
            .iter()
            .find(|note| {
                note.id == note_id
                    && note.encryption.is_none()
                    && note.deleted_at_epoch_millis.is_none()
            })
            .cloned()
        else {
            self.desktop_agent.message = "来源已删除或加密，无法打开".into();
            return;
        };
        if desktop_note_kind(&note) == DesktopNoteKind::Document {
            self.switch_tab(AppTab::Knowledge);
            self.open_knowledge_source(note_id);
        } else {
            self.switch_tab(AppTab::Notes);
            if self.tab != AppTab::Notes {
                return;
            }
            self.note_search_draft.clear();
            self.note_trash_mode = false;
            self.select_note(&note);
            self.rebuild_view_cache();
        }
        self.desktop_agent.open = false;
    }

    fn ui_desktop_agent_window(&mut self, ctx: &egui::Context) {
        if !self.desktop_agent.open {
            return;
        }
        let mut open = true;
        let mut preview = false;
        let mut send = false;
        let mut cancel = false;
        let mut save = false;
        let mut source_to_open = None;
        let mut saved_to_open = None;
        let mut start_new_task = false;
        let running = self.desktop_agent.active.is_some();
        egui::Window::new("AI Agent").id(egui::Id::new("desktop_agent")).default_width(760.0).resizable(true).vscroll(true).open(&mut open).show(ctx,|ui| {
            ui.label(format!("{} · {}",self.sync.ai_model,desktop_ai_configured_recipient(&self.sync).unwrap_or_else(|e| e)));
            ui.horizontal_wrapped(|ui| {
                let steps = ["任务目标", "选择资料", "核对授权", "执行任务", "审阅结果"];
                for (index, title) in steps.iter().enumerate() {
                    ui.label(if self.desktop_agent.step as usize == index { format!("● {} {}", index + 1, title) } else { format!("{} {}", index + 1, title) });
                }
            });
            ui.label("仅处理本次授权资料，来源页不会被 Agent 修改。");
            ui.add_enabled_ui(!running,|ui| {
                if self.desktop_agent.step == 0 {
                ui.label(format!("任务目标 · {}/1000",self.desktop_agent.question.chars().count()));
                if ui.add(egui::TextEdit::multiline(&mut self.desktop_agent.question).desired_rows(3).desired_width(f32::INFINITY).hint_text("例如：整理这些记录的待办、冲突和需要补充的事实")).changed() { self.desktop_agent.preview=None; }
                if ui.checkbox(&mut self.desktop_agent.deep_review,"第二阶段独立深度核验").changed() { self.desktop_agent.preview=None; }
                ui.horizontal_wrapped(|ui| {
                    for (name, goal) in [
                        ("整理待办", "提取所选资料的待办、依赖和待确认事项，列出来源。"),
                        ("核对矛盾", "核对所选资料的事实与时间线，指出不一致和证据缺口。"),
                        ("阶段复盘", "总结所选资料的进展、问题、经验与下一步行动。"),
                    ] {
                        if ui.small_button(name).clicked() { self.desktop_agent.question = goal.into(); self.desktop_agent.preview = None; }
                    }
                });
                if ui.add_enabled(!self.desktop_agent.question.trim().is_empty() && self.desktop_agent.question.chars().count() <= 1000, egui::Button::new("下一步：选择资料")).clicked() {
                    self.desktop_agent.step = 1;
                }
                }
                if self.desktop_agent.step == 1 {
                if ui.small_button("返回任务目标").clicked() { self.desktop_agent.step = 0; self.desktop_agent.preview = None; }
                ui.horizontal(|ui| {
                    ui.label(format!("已选 {}/30 条资料",self.desktop_agent.selected_ids.len()));
                    if self.desktop_agent.priority_note_id.is_some() { ui.label("仅当前文档"); }
                    else {
                        if ui.button("清空选择").clicked() { self.desktop_agent.selected_ids.clear(); self.desktop_agent.preview=None; }
                        if ui.button("选择筛选结果（最多 30 条）").clicked() {
                            let filter=self.desktop_agent.filter.trim().to_lowercase();
                            self.desktop_agent.selected_ids=self.desktop_agent.sources.iter()
                                .filter(|source| filter.is_empty() || format!("{} {}",source.title,source.folder).to_lowercase().contains(&filter))
                                .take(30).map(|source|source.id.clone()).collect();
                            self.desktop_agent.preview=None;
                        }
                    }
                });
                if self.desktop_agent.priority_note_id.is_none() { ui.add(egui::TextEdit::singleline(&mut self.desktop_agent.filter).hint_text("按标题或文件夹筛选")); }
                let filter=self.desktop_agent.filter.trim().to_lowercase();
                egui::CollapsingHeader::new("选择便签与知识页 / 查看原文").default_open(self.desktop_agent.draft.is_none()).show(ui,|ui| {
                    egui::ScrollArea::vertical().id_source("desktop_agent_sources").max_height(210.0).show(ui,|ui| {
                        for source in &self.desktop_agent.sources {
                            if self.desktop_agent.priority_note_id.as_ref().is_some_and(|id| id!=&source.id) { continue; }
                            if !filter.is_empty() && !format!("{} {}",source.title,source.folder).to_lowercase().contains(&filter) { continue; }
                            let mut selected=self.desktop_agent.selected_ids.contains(&source.id);
                            let enabled=self.desktop_agent.priority_note_id.is_none() && (selected || self.desktop_agent.selected_ids.len()<30);
                            if ui.add_enabled(enabled,egui::Checkbox::new(&mut selected,format!("{} · {} · {} · {} 字",source.kind,source.title,source.folder,self.desktop_agent.source_lengths.get(&source.id).copied().unwrap_or(0)))).changed() {
                                if selected { self.desktop_agent.selected_ids.insert(source.id.clone()); } else { self.desktop_agent.selected_ids.remove(&source.id); }
                                self.desktop_agent.preview=None;
                            }
                            egui::CollapsingHeader::new(format!("原文 · {}",source.title)).id_source(("agent_source_body",&source.id)).show(ui,|ui| {ui.label(&source.content);});
                        }
                    });
                });
                ui.label("只提供当前可读的最新内容；加密、已删除内容及历史版本不纳入。超限会提示调整范围。");
                }
            });
            if let Some(active)=self.desktop_agent.active.as_ref() {
                let progress=active.handle.progress();
                let stage=match progress["stage"].as_str() {Some("review")=>"第二阶段深度核验",Some("verifying")=>"本机核验",_=>"首轮整理"};
                let activity=match progress["activity"].as_str() {Some("model")=>"等待模型返回",Some("tool")=>"读取授权资料",Some("verifying")=>"检查证据与草稿",_=>"准备下一步"};
                ui.separator();ui.label(format!("{stage} · {activity} · 已用 {} 秒",progress["elapsedSeconds"].as_u64().unwrap_or(0)));
                ui.label(format!("实际模型请求 {} 次 · 本机工具 {} 次 · 本阶段读取 {} 条资料",progress["requests"],progress["toolCalls"],progress["documentsRead"]));
                if progress["cancelRequested"]==true { ui.label("正在停止后续步骤，等待在途请求结束"); } else { cancel=ui.button("停止本次任务").clicked(); }
                ctx.request_repaint_after(Duration::from_millis(250));
            } else if self.desktop_agent.step == 1 {
                preview=ui.add_enabled(self.workspace_persistence_ready && !self.workspace_edit_locked() && !self.knowledge_ai_pending,egui::Button::new("核对资料并继续")).clicked();
            } else if self.desktop_agent.step == 3 {
                if ui.button("返回选择资料").clicked() { self.desktop_agent.step = 1; }
            }
            if self.desktop_agent.step == 2 {
            if ui.small_button("返回选择资料").clicked() { self.desktop_agent.step = 1; self.desktop_agent.preview = None; }
            if let Some(preview)=self.desktop_agent.preview.as_mut() {
                ui.separator();let recipient=desktop_ai_configured_recipient(&self.sync).unwrap_or_else(|e|e);
                ui.label(egui::RichText::new("本次发送确认").strong());ui.label(format!("接收域名：{recipient} · 模型：{}",self.sync.ai_model));
                ui.label(format!("授权 {} 条资料、{} 字节；每阶段最多 6 次模型请求、16 次工具调用。{}",preview.context.documents.len(),preview.scope_json.len(),if preview.context.deep_review {"本次最多 12 次模型请求。"}else{"本次最多 6 次模型请求。"}));
                ui.label("请求设置 store: false；自定义服务仍按自身规则保存。所选资料可能包含他人个人信息，请核对范围。");
                egui::CollapsingHeader::new("查看本次完整授权内容").show(ui,|ui| {
                    ui.label(&preview.context.question);for source in &preview.context.documents {ui.label(egui::RichText::new(format!("{} · {} · {}",source.kind,source.title,source.folder)).strong());ui.label(&source.content);}
                });
                ui.checkbox(&mut preview.acknowledged,"我已核对接收方及全部授权资料，允许本次发送");
                send=ui.add_enabled(preview.acknowledged,egui::Button::new("发送并运行 Agent")).clicked();
            }
            }
            if !self.desktop_agent.message.is_empty() { ui.label(&self.desktop_agent.message); }
            let save_state=self.desktop_agent_save_state();
            if self.desktop_agent.step == 4 {
            if let Some(draft)=self.desktop_agent.draft.as_mut() {
                ui.separator();ui.label(if draft.saved_id.is_some() {"任务已保存"} else if draft.verified {"核验通过 · 待你审阅保存"} else {"未完成或核验失败 · 不能保存"});
                ui.label(format!("实际模型请求 {} 次 · 本机工具调用 {} 次",draft.result["requests"],draft.result["toolCalls"]));
                if let Some(plan)=draft.result["plan"].as_array() {egui::CollapsingHeader::new("执行计划").show(ui,|ui| {for line in plan.iter().filter_map(Value::as_str) {ui.label(line);}});}
                egui::CollapsingHeader::new("工具轨迹与本机核验").show(ui,|ui| {
                    if let Some(trace)=draft.result["toolTrace"].as_array() {for item in trace {ui.label(format!("{} · {} 项",item["summary"].as_str().unwrap_or("工具"),item["resultCount"]));}}
                    if let Some(checks)=draft.result["verification"]["checks"].as_array() {for check in checks {ui.label(format!("{} {}：{}",if check["passed"]==true {"✓"}else{"!"},check["label"].as_str().unwrap_or("检查"),check["detail"].as_str().unwrap_or("")));}}
                });
                ui.add_enabled_ui(draft.saved_id.is_none(),|ui| {
                    ui.label("新知识页标题");ui.add(egui::TextEdit::singleline(&mut draft.title).desired_width(f32::INFINITY));
                    ui.label("草稿正文与待办");ui.add(egui::TextEdit::multiline(&mut draft.body).desired_width(f32::INFINITY).desired_rows(9));
                });
                if draft.title!=draft.generated_title || draft.body!=draft.generated_body {ui.label("你已编辑草稿；核验记录覆盖模型生成版本，保存内容以当前编辑框为准。");}
                if let Some(ids)=draft.result["draft"]["sourceIds"].as_array() {ui.horizontal_wrapped(|ui| {for id in ids.iter().filter_map(Value::as_str) {if let Some(source)=draft.context.documents.iter().find(|source|source.id==id) {
                    if ui.button(format!("打开来源：{}",source.title)).clicked() {source_to_open=Some(id.to_owned());}
                }}});}
                ui.horizontal(|ui| {
                    if ui.button("复制草稿").clicked() {ui.output_mut(|o|o.copied_text=draft.body.clone());}
                    save=ui.add_enabled(save_state==AgentSaveState::Ready,egui::Button::new("保存为新的知识页")).clicked();
                    if let Some(id)=draft.saved_id.as_ref() {if ui.button("打开已保存知识页").clicked() {saved_to_open=Some(id.clone());}}
                });
                if save_state==AgentSaveState::ScopeChanged {ui.label("资料、账户、配置或任务目标已经变化，请重新运行后保存。");}
                if draft.saved_id.is_some() && ui.button("开始新任务").clicked() { start_new_task = true; }
            }
            }
        });
        if !open {
            if self.desktop_agent.draft.as_ref().is_some_and(|draft| draft.saved_id.is_none()) {
                self.desktop_agent.confirm_discard = true;
            } else {
                self.cancel_desktop_agent();
                self.desktop_agent.open = false;
            }
        }
        if self.desktop_agent.confirm_discard {
            let mut abandon = false;
            let mut keep = false;
            egui::Window::new("放弃未保存的 Agent 草稿？").collapsible(false).show(ctx, |ui| {
                ui.label("当前草稿尚未保存为知识页。放弃后无法从此窗口恢复。");
                ui.horizontal(|ui| {
                    abandon = ui.button("确认放弃").clicked();
                    keep = ui.button("继续审阅").clicked();
                });
            });
            if abandon {
                self.cancel_desktop_agent();
                self.desktop_agent.draft = None;
                self.desktop_agent.open = false;
                self.desktop_agent.confirm_discard = false;
            } else if keep {
                self.desktop_agent.confirm_discard = false;
            }
        }
        if cancel {
            self.cancel_desktop_agent();
        }
        if preview {
            self.prepare_desktop_agent();
        }
        if send {
            self.send_desktop_agent();
        }
        if save {
            self.save_desktop_agent_draft();
        }
        if let Some(id) = source_to_open.or(saved_to_open) {
            self.open_desktop_agent_source(&id);
        }
        if start_new_task {
            let priority = self.desktop_agent.priority_note_id.clone();
            self.open_desktop_agent_entry(priority);
        }
    }
}
