// Windows - Lossless nested knowledge containers and workspace-local directory order.
// Shared documents, revisions and date-based home lists are never changed by a drag.

#[derive(Clone, Debug, PartialEq, Eq)]
enum KnowledgeDirectoryIssue {
    MissingParent,
    InaccessibleParent,
    Cycle,
    IdentityConflict,
}
impl KnowledgeDirectoryIssue {
    fn label(&self) -> &'static str {
        match self {
            Self::MissingParent => "原上级已不存在，可重新选择位置",
            Self::InaccessibleParent => "原上级不可访问，可重新选择位置",
            Self::Cycle => "目录关系存在循环，可重新选择位置",
            Self::IdentityConflict => "编号冲突，原页面与文件夹均已保留，暂停修改",
        }
    }
}

#[derive(Clone, Debug)]
struct KnowledgeDirectoryNode {
    id: String,
    title: String,
    parent: Option<String>,
    folder: bool,
    legacy: bool,
    has_note: bool,
    writable: bool,
    encrypted: bool,
    database: bool,
    updated: i64,
    issue: Option<KnowledgeDirectoryIssue>,
}
#[derive(Default)]
struct KnowledgeDirectoryIndex {
    nodes: BTreeMap<String, KnowledgeDirectoryNode>,
    children: BTreeMap<Option<String>, Vec<String>>,
}
impl KnowledgeDirectoryIndex {
    fn build(notes: &[DesktopNote], folders: &[DesktopNoteFolder]) -> Self {
        let legacy = folders
            .iter()
            .map(|f| f.id.as_str())
            .collect::<HashSet<_>>();
        let mut nodes = BTreeMap::new();
        for folder in folders {
            nodes
                .entry(folder.id.clone())
                .or_insert_with(|| KnowledgeDirectoryNode {
                    id: folder.id.clone(),
                    title: folder.name.clone(),
                    parent: None,
                    folder: true,
                    legacy: true,
                    has_note: false,
                    writable: true,
                    encrypted: false,
                    database: false,
                    updated: folder.updated_at_epoch_millis,
                    issue: None,
                });
        }
        // Deleted promoted records must not resurrect their compatibility folder.
        for note in notes {
            let promoted = note.document.knowledge.as_ref().is_some_and(|p| {
                p.properties.get(knowledge::LEGACY_FOLDER_PROPERTY)
                    == Some(&knowledge::CellValue::Checkbox(true))
            });
            if promoted && note.deleted_at_epoch_millis.is_some() {
                nodes.remove(&note.id);
            } else if !promoted {
                if let Some(folder) = nodes.get_mut(&note.id) {
                    folder.writable = false;
                    folder.issue = Some(KnowledgeDirectoryIssue::IdentityConflict);
                }
            }
        }
        for note in notes.iter().filter(|n| {
            desktop_note_kind(n) == DesktopNoteKind::Document && n.deleted_at_epoch_millis.is_none()
        }) {
            let meta = note.document.knowledge.as_ref();
            let promoted = meta.is_some_and(|p| {
                p.properties.get(knowledge::LEGACY_FOLDER_PROPERTY)
                    == Some(&knowledge::CellValue::Checkbox(true))
            }) && note.encryption.is_none();
            let conflict = legacy.contains(note.id.as_str()) && !promoted;
            let parent = meta
                .and_then(|p| p.parent_id.clone())
                .filter(|p| !p.is_empty())
                .or_else(|| {
                    note.folder_id
                        .clone()
                        .filter(|p| p != &note.id && legacy.contains(p.as_str()))
                });
            let folder = meta.is_some_and(|p| matches!(p.properties.get(knowledge::FOLDER_KIND_PROPERTY), Some(knowledge::CellValue::Text(kind)) if kind == "folder"));
            let title = if conflict {
                format!(
                    "{} · 旧文件夹：{}",
                    note.title,
                    folders
                        .iter()
                        .find(|f| f.id == note.id)
                        .map_or("", |f| f.name.as_str())
                )
            } else {
                note.title.clone()
            };
            nodes.insert(
                note.id.clone(),
                KnowledgeDirectoryNode {
                    id: note.id.clone(),
                    title,
                    parent: if conflict { None } else { parent },
                    folder,
                    legacy: legacy.contains(note.id.as_str()),
                    has_note: true,
                    writable: !conflict
                        && note.encryption.is_none()
                        && !meta.is_some_and(|p| {
                            p.locked || p.version != knowledge::KNOWLEDGE_FORMAT_VERSION
                        }),
                    encrypted: note.encryption.is_some(),
                    database: meta.is_some_and(|p| p.database.is_some()),
                    updated: note.updated_at_epoch_millis,
                    issue: conflict.then_some(KnowledgeDirectoryIssue::IdentityConflict),
                },
            );
        }
        let mut invalid = Vec::new();
        for (id, node) in &nodes {
            if let Some(parent) = node.parent.as_ref() {
                let issue = if parent == id {
                    Some(KnowledgeDirectoryIssue::Cycle)
                } else if !nodes.contains_key(parent) {
                    Some(KnowledgeDirectoryIssue::MissingParent)
                } else if nodes[parent].encrypted {
                    Some(KnowledgeDirectoryIssue::InaccessibleParent)
                } else {
                    None
                };
                if let Some(issue) = issue {
                    invalid.push((id.clone(), issue));
                }
            }
        }
        for (id, issue) in invalid {
            let node = nodes.get_mut(&id).unwrap();
            node.parent = None;
            node.issue = Some(issue);
        }
        let mut complete = HashSet::new();
        for start in nodes.keys().cloned().collect::<Vec<_>>() {
            let mut path = Vec::new();
            let mut positions = BTreeMap::new();
            let mut current = Some(start);
            while let Some(id) = current {
                if complete.contains(&id) {
                    break;
                }
                if let Some(&cycle) = positions.get(&id) {
                    for bad in &path[cycle..] {
                        let node = nodes.get_mut(bad).unwrap();
                        node.parent = None;
                        node.issue = Some(KnowledgeDirectoryIssue::Cycle);
                    }
                    break;
                }
                positions.insert(id.clone(), path.len());
                path.push(id.clone());
                current = nodes.get(&id).and_then(|n| n.parent.clone());
            }
            complete.extend(path);
        }
        let mut children = BTreeMap::<Option<String>, Vec<String>>::new();
        for node in nodes.values() {
            children
                .entry(node.parent.clone())
                .or_default()
                .push(node.id.clone());
        }
        for items in children.values_mut() {
            items.sort_by(|a, b| {
                nodes[b]
                    .updated
                    .cmp(&nodes[a].updated)
                    .then_with(|| a.cmp(b))
            });
        }
        Self { nodes, children }
    }
    fn children(&self, parent: Option<&str>) -> &[String] {
        self.children
            .get(&parent.map(str::to_string))
            .map(Vec::as_slice)
            .unwrap_or_default()
    }
    fn path(&self, id: &str) -> Vec<String> {
        let mut result = Vec::new();
        let mut seen = HashSet::new();
        let mut current = Some(id);
        while let Some(id) = current {
            if !seen.insert(id) {
                break;
            }
            let Some(node) = self.nodes.get(id) else {
                break;
            };
            result.push(id.to_string());
            current = node.parent.as_deref();
        }
        result.reverse();
        result
    }
    fn may_create(&self, parent: Option<&str>) -> bool {
        parent.is_none_or(|id| {
            self.nodes.contains_key(id)
                && self
                    .path(id)
                    .iter()
                    .all(|id| self.nodes[id].writable && self.nodes[id].issue.is_none())
        })
    }
    fn may_move(&self, id: &str, parent: Option<&str>) -> bool {
        self.nodes.get(id).is_some_and(|n| n.writable)
            && self.may_create(parent)
            && parent.is_none_or(|parent| !self.path(parent).iter().any(|ancestor| ancestor == id))
    }
    fn scope(
        &self,
        workspace: &str,
        parent: Option<&str>,
        interactive: bool,
    ) -> KnowledgeDirectoryScope {
        KnowledgeDirectoryScope {
            workspace: workspace.into(),
            parent: parent.map(str::to_string),
            path: parent.map(|id| self.path(id)).unwrap_or_default(),
            can_reorder: interactive
                && self.may_create(parent)
                && self
                    .children(parent)
                    .iter()
                    .any(|id| self.nodes[id].writable && self.nodes[id].issue.is_none()),
            items: self
                .children(parent)
                .iter()
                .map(|id| {
                    (
                        id.clone(),
                        self.nodes[id].writable && self.nodes[id].issue.is_none(),
                    )
                })
                .collect(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct KnowledgeDirectoryScope {
    workspace: String,
    parent: Option<String>,
    path: Vec<String>,
    can_reorder: bool,
    items: BTreeMap<String, bool>,
}
impl KnowledgeDirectoryScope {
    fn valid(&self) -> bool {
        !self.workspace.is_empty()
            && self.parent.as_ref().is_none_or(|id| !id.is_empty())
            && self.items.keys().all(|id| !id.is_empty())
            && self.path.iter().collect::<HashSet<_>>().len() == self.path.len()
    }
    fn permutation(&self, ids: &[String]) -> bool {
        self.valid()
            && ids.len() == self.items.len()
            && ids.iter().collect::<HashSet<_>>().len() == ids.len()
            && ids.iter().all(|id| self.items.contains_key(id))
    }
}
#[derive(Clone)]
struct KnowledgeDirectoryDrag {
    scope: KnowledgeDirectoryScope,
    initial: Vec<String>,
    dragged: String,
}
fn knowledge_directory_reordered(
    drag: &KnowledgeDirectoryDrag,
    current: &KnowledgeDirectoryScope,
    target: &str,
) -> Option<Vec<String>> {
    if drag.scope != *current
        || !current.can_reorder
        || !current.permutation(&drag.initial)
        || current.items.get(&drag.dragged) != Some(&true)
    {
        return None;
    }
    let mut ids = drag.initial.clone();
    let from = ids.iter().position(|id| id == &drag.dragged)?;
    let to = ids.iter().position(|id| id == target)?;
    if from == to {
        return None;
    }
    let moved = ids.remove(from);
    ids.insert(to, moved);
    Some(ids)
}
fn knowledge_directory_ordered(available: &[String], saved: &[String]) -> Vec<String> {
    let allowed = available.iter().collect::<HashSet<_>>();
    let mut seen = HashSet::new();
    saved
        .iter()
        .chain(available)
        .filter(|id| allowed.contains(id) && seen.insert((*id).clone()))
        .cloned()
        .collect()
}
#[derive(Clone, Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct KnowledgeDirectoryPreferences {
    parents: BTreeMap<String, Vec<String>>,
}
impl KnowledgeDirectoryPreferences {
    fn validate(&self) -> bool {
        self.parents.values().all(|ids| {
            ids.iter().all(|id| !id.is_empty())
                && ids.iter().collect::<HashSet<_>>().len() == ids.len()
        })
    }
    fn parent_key(parent: Option<&str>) -> String {
        serde_json::to_string(&parent).unwrap_or_default()
    }
}
#[derive(Clone)]
struct KnowledgeDirectoryRequest {
    workspace: String,
    action: String,
    id: String,
    parent: Option<String>,
    title: String,
}
#[derive(Default)]
struct DesktopKnowledgeDirectory {
    workspace: String,
    open: bool,
    parent: Option<String>,
    query: String,
    preferences: KnowledgeDirectoryPreferences,
    order_error: String,
    request: Option<KnowledgeDirectoryRequest>,
    cache: Option<(u64, Arc<KnowledgeDirectoryIndex>)>,
}
impl TimerWindowsClient {
    fn close_knowledge_directory(&mut self) {
        self.desktop_ui.knowledge.directory.open = false;
        self.desktop_ui.knowledge.directory.request = None;
    }
    fn knowledge_directory_index(&mut self) -> Arc<KnowledgeDirectoryIndex> {
        let workspace = self.background_job_workspace_fingerprint();
        if self.desktop_ui.knowledge.directory.workspace != workspace {
            self.desktop_ui.knowledge.directory = DesktopKnowledgeDirectory {
                workspace: workspace.clone(),
                ..Default::default()
            };
            let path = self.knowledge_directory_preferences_path();
            if path.exists() || backup_path(&path).exists() {
                let loaded = fs::read_to_string(&path)
                    .ok()
                    .and_then(|raw| {
                        serde_json::from_str::<KnowledgeDirectoryPreferences>(&raw).ok()
                    })
                    .filter(|value| value.validate())
                    .or_else(|| {
                        fs::read_to_string(backup_path(&path))
                            .ok()
                            .and_then(|raw| {
                                serde_json::from_str::<KnowledgeDirectoryPreferences>(&raw).ok()
                            })
                            .filter(|value| value.validate())
                    });
                match loaded {
                    Some(value) => self.desktop_ui.knowledge.directory.preferences = value,
                    None => {
                        self.desktop_ui.knowledge.directory.order_error =
                            "目录排序文件无法读取，已保留原文件；可重试读取。".into()
                    }
                }
            }
        }
        let version = self.notes_cache_version();
        let directory = &mut self.desktop_ui.knowledge.directory;
        if !directory
            .cache
            .as_ref()
            .is_some_and(|(cached, _)| *cached == version)
        {
            directory.cache = Some((
                version,
                Arc::new(KnowledgeDirectoryIndex::build(
                    &self.data.notes,
                    &self.data.note_folders,
                )),
            ));
        }
        Arc::clone(&directory.cache.as_ref().unwrap().1)
    }
    fn knowledge_directory_preferences_path(&self) -> PathBuf {
        self.settings_path
            .parent()
            .unwrap_or(Path::new("."))
            .join(format!(
                "knowledge_directory_{}.json",
                sync_core::token_identifier(&format!(
                    "{}\n{}",
                    self.state_path.display(),
                    workspace_note_media_identity(&self.ai_workspace_identity())
                ))
            ))
    }
    fn open_knowledge_directory(&mut self, parent: Option<String>) -> bool {
        if !matches!(self.shutdown_state, ClientShutdownState::Running) {
            return false;
        }
        if self.flush_note_draft().is_err() {
            return false;
        }
        let index = self.knowledge_directory_index();
        if parent
            .as_ref()
            .is_some_and(|id| !index.nodes.contains_key(id) || index.nodes[id].encrypted)
        {
            self.status = "目录已删除或尚未解锁".into();
            return false;
        }
        self.desktop_ui.knowledge.directory.parent = parent;
        self.desktop_ui.knowledge.directory.query.clear();
        self.desktop_ui.knowledge.directory.open = true;
        true
    }
    fn commit_knowledge_directory_order(
        &mut self,
        expected: &KnowledgeDirectoryScope,
        ids: &[String],
    ) -> bool {
        self.update_knowledge_directory_order(expected, Some(ids))
    }
    fn reset_knowledge_directory_order(&mut self, expected: &KnowledgeDirectoryScope) -> bool {
        self.update_knowledge_directory_order(expected, None)
    }
    fn update_knowledge_directory_order(
        &mut self,
        expected: &KnowledgeDirectoryScope,
        ids: Option<&[String]>,
    ) -> bool {
        let index = self.knowledge_directory_index();
        let actual = index.scope(
            &self.background_job_workspace_fingerprint(),
            self.desktop_ui.knowledge.directory.parent.as_deref(),
            self.desktop_ui.knowledge.directory.open
                && self.desktop_ui.knowledge.directory.query.trim().is_empty()
                && !self.workspace_edit_locked()
                && self.workspace_persistence_ready
                && matches!(self.shutdown_state, ClientShutdownState::Running),
        );
        if *expected != actual
            || !actual.can_reorder
            || ids.is_some_and(|ids| !actual.permutation(ids))
            || !self.desktop_ui.knowledge.directory.order_error.is_empty()
        {
            self.status = "目录已变化，本次排序未保存".into();
            return false;
        }
        let mut proposed = self.desktop_ui.knowledge.directory.preferences.clone();
        let key = KnowledgeDirectoryPreferences::parent_key(actual.parent.as_deref());
        if let Some(ids) = ids {
            proposed.parents.insert(key, ids.to_vec());
        } else {
            proposed.parents.remove(&key);
        }
        // The single UI owner publishes only after the local atomic writer succeeds.
        match save_json_config(
            &self.knowledge_directory_preferences_path(),
            &proposed,
            "knowledge_directory",
        ) {
            Ok(()) => {
                self.desktop_ui.knowledge.directory.preferences = proposed;
                self.status = "目录顺序已保存".into();
                true
            }
            Err(error) => {
                self.status = format!("排序未保存，仍保留原顺序：{error}");
                false
            }
        }
    }
    fn apply_knowledge_directory_action(&mut self, request: &KnowledgeDirectoryRequest) -> bool {
        if request.workspace != self.background_job_workspace_fingerprint()
            || self.workspace_edit_locked()
            || !self.workspace_persistence_ready
            || !matches!(self.shutdown_state, ClientShutdownState::Running)
        {
            self.status = "当前状态不允许执行目录修改".into();
            return false;
        }
        if self.flush_note_draft().is_err() {
            return false;
        }
        let selected = self.selected_note_id.clone();
        let next = app_data::knowledge_container_action_app_data_json(
            &self.state_json,
            &request.action,
            &request.id,
            request.parent.as_deref().unwrap_or(""),
            &request.title,
            now_millis(),
        );
        if next.is_none() {
            self.status = "操作未执行：请核对父目录、锁定状态、编号冲突或循环关系".into();
            return false;
        }
        if !self.replace_state(next, "目录已更新") {
            return false;
        }
        self.desktop_ui.knowledge.directory.cache = None;
        if selected == request.id {
            if let Some(note) = self.data.notes.iter().find(|n| n.id == selected).cloned() {
                self.load_note_draft_without_flush(&note);
            }
        }
        if request.action == "create_page" {
            self.close_knowledge_directory();
            self.select_note_by_id(&request.id);
        } else if request.action == "create_folder" {
            self.desktop_ui.knowledge.directory.parent = Some(request.id.clone());
        }
        true
    }
    fn request_knowledge_directory_action(
        &mut self,
        action: &str,
        id: &str,
        parent: Option<String>,
        title: &str,
    ) {
        self.desktop_ui.knowledge.directory.request = Some(KnowledgeDirectoryRequest {
            workspace: self.background_job_workspace_fingerprint(),
            action: action.into(),
            id: id.into(),
            parent,
            title: title.into(),
        });
    }
    fn ui_knowledge_directory_actions(
        &mut self,
        ui: &mut egui::Ui,
        index: &KnowledgeDirectoryIndex,
        parent: Option<&str>,
    ) {
        let enabled = self.workspace_persistence_ready
            && !self.workspace_edit_locked()
            && matches!(self.shutdown_state, ClientShutdownState::Running);
        ui.horizontal_wrapped(|ui| {
            ui.add_enabled_ui(enabled && index.may_create(parent), |ui| {
                ui.menu_button("＋ 新建", |ui| {
                    for (label, action) in
                        [("子页面", "create_page"), ("子文件夹", "create_folder")]
                    {
                        if ui.button(label).clicked() {
                            self.request_knowledge_directory_action(
                                action,
                                &random_hex_identifier(),
                                parent.map(str::to_string),
                                "",
                            );
                            ui.close_menu();
                        }
                    }
                });
            });
            if let Some(node) = parent.and_then(|id| index.nodes.get(id)) {
                ui.add_enabled_ui(enabled && node.writable, |ui| {
                    ui.menu_button("管理", |ui| {
                        for (label, action) in [
                            ("移动", "move"),
                            ("重命名", "rename"),
                            (
                                if node.folder {
                                    "转为页面"
                                } else {
                                    "转为文件夹"
                                },
                                if node.folder { "to_page" } else { "to_folder" },
                            ),
                        ] {
                            if ui
                                .add_enabled(
                                    !(action == "to_folder" && node.database),
                                    egui::Button::new(label),
                                )
                                .clicked()
                            {
                                self.request_knowledge_directory_action(
                                    action,
                                    &node.id,
                                    node.parent.clone(),
                                    &node.title,
                                );
                                ui.close_menu();
                            }
                        }
                    });
                });
                if node.has_note
                    && node.issue != Some(KnowledgeDirectoryIssue::IdentityConflict)
                    && ui.button("编辑正文").clicked()
                {
                    self.close_knowledge_directory();
                    self.select_note_by_id(&node.id);
                }
            }
        });
    }
    fn ui_knowledge_directory(&mut self, ctx: &egui::Context) {
        let index = self.knowledge_directory_index();
        if egui::DragAndDrop::has_payload_of_type::<KnowledgeDirectoryDrag>(ctx)
            && (!self.desktop_ui.knowledge.directory.open
                || ctx.input(|input| input.key_pressed(egui::Key::Escape)))
        {
            egui::DragAndDrop::clear_payload(ctx);
        }
        if !self.desktop_ui.knowledge.directory.open {
            return;
        }
        let parent = self.desktop_ui.knowledge.directory.parent.clone();
        if parent
            .as_ref()
            .is_some_and(|id| !index.nodes.contains_key(id) || index.nodes[id].encrypted)
        {
            self.desktop_ui.knowledge.directory.parent = None;
            self.desktop_ui.knowledge.directory.request = None;
            self.status = "原目录已删除或不可访问，已返回目录根层".into();
            return;
        }
        let workspace = self.background_job_workspace_fingerprint();
        let mut open = true;
        let width = (ctx.screen_rect().width() - 80.0).clamp(280.0, 740.0);
        egui::Window::new("知识目录")
            .id(egui::Id::new("knowledge_directory_browser"))
            .open(&mut open)
            .default_width(width)
            .default_height(520.0)
            .resizable(true)
            .collapsible(false)
            .show(ctx, |ui| {
                ui.set_enabled(
                    !self.workspace_edit_locked()
                        && self.workspace_persistence_ready
                        && matches!(self.shutdown_state, ClientShutdownState::Running),
                );
                ui.horizontal_wrapped(|ui| {
                    if ui.small_button("知识目录").clicked() {
                        self.desktop_ui.knowledge.directory.parent = None;
                        self.desktop_ui.knowledge.directory.query.clear();
                    }
                    for id in parent.as_ref().map(|id| index.path(id)).unwrap_or_default() {
                        let node = &index.nodes[&id];
                        ui.label("›");
                        if ui
                            .small_button(if node.title.is_empty() {
                                "未命名"
                            } else {
                                &node.title
                            })
                            .clicked()
                        {
                            self.desktop_ui.knowledge.directory.parent = Some(id);
                            self.desktop_ui.knowledge.directory.query.clear();
                        }
                    }
                });
                self.ui_knowledge_directory_actions(ui, &index, parent.as_deref());
                if let Some(issue) = parent
                    .as_ref()
                    .and_then(|id| index.nodes[id].issue.as_ref())
                {
                    ui.colored_label(palette().danger, issue.label());
                }
                ui.add(
                    egui::TextEdit::singleline(&mut self.desktop_ui.knowledge.directory.query)
                        .desired_width(f32::INFINITY)
                        .hint_text("在当前目录查找"),
                );
                if !self.desktop_ui.knowledge.directory.order_error.is_empty() {
                    ui.colored_label(
                        palette().danger,
                        &self.desktop_ui.knowledge.directory.order_error,
                    );
                    if ui.button("重试读取排序").clicked() {
                        self.desktop_ui.knowledge.directory.workspace.clear();
                    }
                }
                let query = self
                    .desktop_ui
                    .knowledge
                    .directory
                    .query
                    .trim()
                    .to_lowercase();
                let scope = index.scope(
                    &workspace,
                    parent.as_deref(),
                    query.is_empty()
                        && self.workspace_persistence_ready
                        && !self.workspace_edit_locked()
                        && matches!(self.shutdown_state, ClientShutdownState::Running),
                );
                if egui::DragAndDrop::payload::<KnowledgeDirectoryDrag>(ctx)
                    .is_some_and(|drag| drag.scope != scope || !scope.can_reorder)
                {
                    egui::DragAndDrop::clear_payload(ctx);
                }
                let saved = self
                    .desktop_ui
                    .knowledge
                    .directory
                    .preferences
                    .parents
                    .get(&KnowledgeDirectoryPreferences::parent_key(
                        parent.as_deref(),
                    ))
                    .cloned()
                    .unwrap_or_default();
                let ordered =
                    knowledge_directory_ordered(index.children(parent.as_deref()), &saved);
                let shown = ordered
                    .iter()
                    .filter(|id| {
                        query.is_empty()
                            || (!index.nodes[*id].encrypted
                                && index.nodes[*id].title.to_lowercase().contains(&query))
                    })
                    .cloned()
                    .collect::<Vec<_>>();
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new(format!("{} 项", shown.len()))
                            .small()
                            .color(palette().muted),
                    );
                    ui.label(
                        egui::RichText::new("拖动右侧手柄排序")
                            .small()
                            .color(palette().muted),
                    );
                    if ui
                        .add_enabled(
                            scope.can_reorder && !saved.is_empty(),
                            egui::Button::new("恢复默认顺序").small(),
                        )
                        .clicked()
                    {
                        self.reset_knowledge_directory_order(&scope);
                    }
                });
                egui::ScrollArea::vertical()
                    .id_source(("knowledge_directory_rows", parent.clone()))
                    .max_height((ui.available_height() - 35.0).max(150.0))
                    .show_rows(ui, 44.0, shown.len(), |ui, range| {
                        if let Some(drag) =
                            egui::DragAndDrop::payload::<KnowledgeDirectoryDrag>(ui.ctx())
                        {
                            if drag.scope == scope && scope.can_reorder && ui.is_enabled() {
                                let clip = ui.clip_rect();
                                let delta = ui.input(|input| {
                                    let Some(pointer) = input
                                        .pointer
                                        .hover_pos()
                                        .filter(|point| clip.contains(*point))
                                    else {
                                        return 0.0;
                                    };
                                    if !input.pointer.primary_down() {
                                        return 0.0;
                                    }
                                    let edge = 36.0_f32.min(clip.height() * 0.25);
                                    let direction = if pointer.y < clip.top() + edge {
                                        ((clip.top() + edge - pointer.y) / edge).clamp(0.0, 1.0)
                                    } else if pointer.y > clip.bottom() - edge {
                                        -((pointer.y - clip.bottom() + edge) / edge).clamp(0.0, 1.0)
                                    } else {
                                        0.0
                                    };
                                    direction * 420.0 * input.stable_dt.min(0.05)
                                });
                                if delta != 0.0 {
                                    ui.scroll_with_delta(egui::vec2(0.0, delta));
                                    ui.ctx().request_repaint_after(Duration::from_millis(16));
                                }
                            }
                        }
                        for row in range {
                            let node = &index.nodes[&shown[row]];
                            ui.push_id(&node.id, |ui| {
                                ui.horizontal(|ui| {
                                    let title = if node.encrypted {
                                        "加密页面"
                                    } else if node.title.is_empty() {
                                        "未命名"
                                    } else {
                                        &node.title
                                    };
                                    let label = format!(
                                        "{} {}",
                                        if node.folder {
                                            "▣"
                                        } else if node.database {
                                            "▦"
                                        } else {
                                            "▤"
                                        },
                                        title
                                    );
                                    let response = ui.add_sized(
                                        [ui.available_width() - 82.0, 38.0],
                                        egui::SelectableLabel::new(false, &label),
                                    );
                                    if response.clicked()
                                        && node.issue
                                            != Some(KnowledgeDirectoryIssue::IdentityConflict)
                                    {
                                        if node.folder {
                                            if self.flush_note_draft().is_ok() {
                                                self.desktop_ui.knowledge.directory.parent =
                                                    Some(node.id.clone());
                                                self.desktop_ui.knowledge.directory.query.clear();
                                            }
                                        } else {
                                            self.close_knowledge_directory();
                                            self.select_note_by_id(&node.id);
                                        }
                                    }
                                    if let Some(payload) =
                                        response.dnd_hover_payload::<KnowledgeDirectoryDrag>()
                                    {
                                        if payload.scope == scope
                                            && scope.can_reorder
                                            && scope.items.get(&payload.dragged) == Some(&true)
                                            && payload.dragged != node.id
                                            && scope.permutation(&payload.initial)
                                        {
                                            let from = payload
                                                .initial
                                                .iter()
                                                .position(|id| id == &payload.dragged);
                                            let to = payload
                                                .initial
                                                .iter()
                                                .position(|id| id == &node.id);
                                            if let (Some(from), Some(to)) = (from, to) {
                                                let y = if from < to {
                                                    response.rect.bottom()
                                                } else {
                                                    response.rect.top()
                                                };
                                                ui.painter().line_segment(
                                                    [
                                                        egui::pos2(response.rect.left() + 4.0, y),
                                                        egui::pos2(response.rect.right() - 4.0, y),
                                                    ],
                                                    egui::Stroke::new(2.0, palette().accent),
                                                );
                                            }
                                        }
                                    }
                                    if let Some(payload) =
                                        response.dnd_release_payload::<KnowledgeDirectoryDrag>()
                                    {
                                        if let Some(ids) = knowledge_directory_reordered(
                                            &payload, &scope, &node.id,
                                        ) {
                                            self.commit_knowledge_directory_order(&scope, &ids);
                                        }
                                    }
                                    if let Some(issue) = &node.issue {
                                        response.on_hover_text(issue.label());
                                    }
                                    let drag = ui.add_enabled(
                                        scope.can_reorder
                                            && node.writable
                                            && node.issue.is_none()
                                            && self
                                                .desktop_ui
                                                .knowledge
                                                .directory
                                                .order_error
                                                .is_empty(),
                                        egui::Label::new("⠿").sense(egui::Sense::drag()),
                                    );
                                    if drag.drag_started() {
                                        drag.dnd_set_drag_payload(KnowledgeDirectoryDrag {
                                            scope: scope.clone(),
                                            initial: ordered.clone(),
                                            dragged: node.id.clone(),
                                        });
                                    }
                                    ui.menu_button("⋯", |ui| {
                                        if ui
                                            .add_enabled(
                                                node.issue
                                                    != Some(
                                                        KnowledgeDirectoryIssue::IdentityConflict,
                                                    )
                                                    && !node.encrypted,
                                                egui::Button::new("进入子项"),
                                            )
                                            .clicked()
                                        {
                                            self.desktop_ui.knowledge.directory.parent =
                                                Some(node.id.clone());
                                            self.desktop_ui.knowledge.directory.query.clear();
                                            ui.close_menu();
                                        }
                                        for (label, action) in [
                                            ("重命名", "rename"),
                                            ("移动", "move"),
                                            (
                                                if node.folder {
                                                    "转为页面"
                                                } else {
                                                    "转为文件夹"
                                                },
                                                if node.folder { "to_page" } else { "to_folder" },
                                            ),
                                        ] {
                                            if ui
                                                .add_enabled(
                                                    node.writable
                                                        && !(action == "to_folder"
                                                            && node.database),
                                                    egui::Button::new(label),
                                                )
                                                .clicked()
                                            {
                                                self.request_knowledge_directory_action(
                                                    action,
                                                    &node.id,
                                                    node.parent.clone(),
                                                    &node.title,
                                                );
                                                ui.close_menu();
                                            }
                                        }
                                    });
                                });
                            });
                        }
                    });
                if shown.is_empty() {
                    ui.label(if query.is_empty() {
                        "还没有子项"
                    } else {
                        "没有匹配的子项"
                    });
                }
            });
        if !open {
            self.close_knowledge_directory();
        }
        self.ui_knowledge_directory_request(ctx, &index);
    }
    fn ui_knowledge_directory_request(
        &mut self,
        ctx: &egui::Context,
        index: &KnowledgeDirectoryIndex,
    ) {
        let Some(mut request) = self.desktop_ui.knowledge.directory.request.clone() else {
            return;
        };
        if request.workspace != self.background_job_workspace_fingerprint() {
            self.desktop_ui.knowledge.directory.request = None;
            return;
        }
        let mut open = true;
        let mut confirm = false;
        let mut cancelled = false;
        let label = match request.action.as_str() {
            "create_page" => "新建页面",
            "create_folder" => "新建文件夹",
            "rename" => "重命名",
            "move" => "移动到",
            "to_folder" => "转为文件夹",
            _ => "转为页面",
        };
        egui::Window::new(label)
            .id(egui::Id::new("knowledge_directory_action"))
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .default_width(340.0)
            .show(ctx, |ui| {
                ui.set_enabled(
                    !self.workspace_edit_locked()
                        && self.workspace_persistence_ready
                        && matches!(self.shutdown_state, ClientShutdownState::Running),
                );
                if matches!(
                    request.action.as_str(),
                    "rename" | "create_folder" | "create_page"
                ) {
                    ui.add(
                        egui::TextEdit::singleline(&mut request.title)
                            .char_limit(240)
                            .desired_width(310.0)
                            .hint_text("名称"),
                    );
                }
                if request.action == "move" {
                    egui::ScrollArea::vertical()
                        .max_height(280.0)
                        .show(ui, |ui| {
                            ui.selectable_value(&mut request.parent, None, "知识目录根层");
                            for node in index
                                .nodes
                                .values()
                                .filter(|node| index.may_move(&request.id, Some(&node.id)))
                            {
                                let title = index
                                    .path(&node.id)
                                    .iter()
                                    .map(|id| {
                                        if index.nodes[id].title.is_empty() {
                                            "未命名"
                                        } else {
                                            &index.nodes[id].title
                                        }
                                    })
                                    .collect::<Vec<_>>()
                                    .join(" / ");
                                ui.selectable_value(
                                    &mut request.parent,
                                    Some(node.id.clone()),
                                    title,
                                );
                            }
                        });
                }
                if matches!(request.action.as_str(), "to_folder" | "to_page") {
                    ui.label("保留正文、附件、历史版本及全部子项。");
                }
                let valid = match request.action.as_str() {
                    "create_folder" | "rename" => !request.title.trim().is_empty(),
                    "move" => index.may_move(&request.id, request.parent.as_deref()),
                    "create_page" => index.may_create(request.parent.as_deref()),
                    _ => index.nodes.get(&request.id).is_some_and(|node| {
                        node.writable && !(request.action == "to_folder" && node.database)
                    }),
                };
                ui.horizontal(|ui| {
                    if ui.add_enabled(valid, egui::Button::new("确定")).clicked() {
                        confirm = true;
                    }
                    if ui.button("取消").clicked() {
                        cancelled = true;
                    }
                });
            });
        if confirm {
            if self.apply_knowledge_directory_action(&request) {
                self.desktop_ui.knowledge.directory.request = None;
            } else {
                self.desktop_ui.knowledge.directory.request = Some(request);
            }
        } else {
            self.desktop_ui.knowledge.directory.request = (open && !cancelled).then_some(request);
        }
    }
    fn ui_knowledge_compact_hierarchy(&mut self, ui: &mut egui::Ui) {
        let id = self.selected_note_id.clone();
        let index = self.knowledge_directory_index();
        let Some(node) = index.nodes.get(&id) else {
            return;
        };
        if node.encrypted {
            return;
        }
        ui.horizontal_wrapped(|ui| {
            let parent = node.parent.as_ref().and_then(|id| index.nodes.get(id));
            let label = parent.map_or("知识目录", |node| {
                if node.title.is_empty() {
                    "未命名"
                } else {
                    &node.title
                }
            });
            if ui
                .small_button(label)
                .on_hover_text("查看所属目录")
                .clicked()
            {
                self.open_knowledge_directory(node.parent.clone());
            }
            if ui
                .small_button(format!("子项 {}", index.children(Some(&id)).len()))
                .clicked()
            {
                self.open_knowledge_directory(Some(id.clone()));
            }
            ui.menu_button("⋯", |ui| {
                self.ui_knowledge_directory_actions(ui, &index, Some(&id));
                ui.separator();
                if ui.button("问这篇文档").clicked() {
                    self.open_desktop_document_ai(&id);
                    ui.close_menu();
                }
                if ui.button("Agent 处理这篇文档").clicked() {
                    self.open_desktop_agent_entry(Some(id.clone()));
                    ui.close_menu();
                }
            });
            if let Some(issue) = &node.issue {
                ui.colored_label(palette().danger, issue.label());
            }
        });
    }
}
