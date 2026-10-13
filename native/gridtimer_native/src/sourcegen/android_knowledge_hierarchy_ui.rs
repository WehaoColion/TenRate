// v2.23.2.30 - Preserve hierarchy handoff and blank-child guards on untouched exits.
// v2.23.2.29 - Navigate and edit knowledge containers without replacing page content.
const STUDIO: &str = "com/ofairyo/gridtimer/ui/NoteStudioSheet.kt";
const EDITOR: &str = "com/ofairyo/gridtimer/ui/NoteDocumentEditor.kt";
pub const PATH: &str = "com/ofairyo/gridtimer/ui/KnowledgeHierarchyUi.kt";

fn replace(source: &mut String, old: &str, new: &str) -> Result<(), String> {
    if source.matches(old).count() != 1 {
        return Err(format!(
            "hierarchy anchor is missing or repeated: {}",
            old.lines().next().unwrap_or("")
        ));
    }
    *source = source.replacen(old, new, 1);
    Ok(())
}
fn imports(source: &mut String) {
    let package = source
        .lines()
        .find(|line| line.starts_with("package "))
        .unwrap_or("")
        .to_owned();
    *source = source.replacen(&package, &format!("{package}\nimport com.ofairyo.gridtimer.data.KnowledgeHierarchyIssue\nimport com.ofairyo.gridtimer.data.knowledgeHierarchy\nimport com.ofairyo.gridtimer.data.isKnowledgeFolder\nimport com.ofairyo.gridtimer.data.requiresStructuredKnowledgeReader"),1);
}
pub fn render(path: &str, base: &str) -> Result<String, String> {
    let mut source = base.to_owned();
    if path.ends_with("/NativeOptimizerBridge.kt") {
        replace(
            &mut source,
            "    fun createNoteFolderAppDataJson(",
            &format!("{BRIDGE}\n    fun createNoteFolderAppDataJson("),
        )?;
    } else if path.ends_with("/TimerRepository.kt") {
        replace(
            &mut source,
            "    suspend fun createNoteFolder(",
            &format!("{REPOSITORY}\n    suspend fun createNoteFolder("),
        )?;
    } else if path.ends_with("/TimerViewModel.kt") {
        replace(
            &mut source,
            "    fun createNoteFolder(",
            &format!("{VIEW_MODEL}\n    fun createNoteFolder("),
        )?;
    } else if path == STUDIO {
        imports(&mut source);
        let from = source
            .find("internal fun NoteStudioSheet(")
            .ok_or("hierarchy studio missing")?;
        let to = source[from..]
            .find("internal fun StickyNoteStudioSheet(")
            .map(|i| i + from)
            .ok_or("hierarchy studio boundary missing")?;
        let mut studio = source[from..to].to_owned();
        replace(
            &mut studio,
            "    val scope = rememberCoroutineScope()",
            &format!("    val scope = rememberCoroutineScope()\n{STUDIO_STATE}"),
        )?;
        replace(&mut studio, "            if (frame == null) {\n                NoteCollectionContent(", &format!("            if (frame == null) {{\n                if (hierarchyVisible) {{\n                    androidx.compose.runtime.CompositionLocalProvider(LocalDocumentTransitionActive provides interactiveFrame) {{\n{HIERARCHY_BROWSER}\n                    }}\n                }} else NoteCollectionContent("))?;
        replace(&mut studio, "                    onManageFolders = { if (isCurrentFrame()) folderManagerVisible = true },", "                    onManageFolders = { if (isCurrentFrame()) { hierarchyVisible = true; hierarchyParentId = null } },")?;
        replace(&mut studio,"                    appData = appData,", "                    appData = appData,\n                    onOpenHierarchy = { if (isCurrentFrame()) { hierarchyVisible = true; hierarchyParentId = null } },")?;
        replace(&mut studio,"                    note = documentNote,", "                    note = documentNote,\n                    hasHierarchyChildren = hierarchyIndex.children(documentNote.id).isNotEmpty(),\n                    hierarchyContent = { rawNavigate ->\n                        val navigate: (() -> Unit) -> Unit = { action ->\n                            if (isCurrentFrame() && frameAccess.mayInteract()) rawNavigate {\n                                if (isCurrentFrame() && frameAccess.mayInteract()) action()\n                            }\n                        }\n                        KnowledgeHierarchyPanel(index = hierarchyIndex, parentId = documentNote.id,\n                            busy = hierarchyBusy, onBrowse = { parent -> navigate {\n                                hierarchyParentId = parent; hierarchyVisible = true; selectedNoteId = null; pendingCreatedNote = null\n                            } }, onOpen = { node -> navigate {\n                                if (node.isFolder) { hierarchyParentId = node.id; hierarchyVisible = true; selectedNoteId = null }\n                                else node.note?.let { child ->\n                                    if (child.isEncryptionLocked()) { hierarchyParentId = documentNote.id; hierarchyVisible = true; selectedNoteId = null; pendingCreatedNote = null; unlockedNote = null; unlockTarget = child; unlockError = \"\" }\n                                    else { hierarchyParentId = hierarchyIndex.parentOf(child.id); hierarchyVisible = true; selectedNoteId = child.id; pendingCreatedNote = null; unlockedNote = null }\n                                }\n                            } }, onRequest = { request -> navigate { hierarchyRequest = request; hierarchyParentId = documentNote.id; hierarchyVisible = true; selectedNoteId = null; pendingCreatedNote = null } })\n                    },\n                    onHierarchyLocationChosen = { parent ->\n                        if (isCurrentFrame() && frameAccess.mayInteract()) {\n                            hierarchyRequest = KnowledgeHierarchyRequest(\"move\", documentNote.id, parent); hierarchyParentId = documentNote.id; hierarchyVisible = true; selectedNoteId = null; pendingCreatedNote = null\n                        }\n                    },")?;
        // The request owns a workspace-scoped callback. A stale callback cannot select another account's page.
        let end = "    unlockTarget?.let { target ->";
        replace(&mut studio, end, &format!("{STUDIO_DIALOG}\n{end}"))?;
        source.replace_range(from..to, &studio);
        replace(&mut source,"private fun NoteCollectionContent(\n    accent: Color,", "private fun NoteCollectionContent(\n    onOpenHierarchy: () -> Unit,\n    accent: Color,")?;
        replace(&mut source,"            item(key = \"knowledge-navigation-entry\") {", "            item(key = \"knowledge-hierarchy-entry\") {\n                androidx.compose.material3.OutlinedButton(onClick = onOpenHierarchy, modifier = Modifier.fillMaxWidth().heightIn(min = 48.dp)) {\n                    Text(\"目录\"); Spacer(Modifier.weight(1f)); Text(\"页面与文件夹\")\n                }\n            }\n            item(key = \"knowledge-navigation-entry\") {")?;
    } else if path == EDITOR {
        imports(&mut source);
        replace(
            &mut source,
            "    if (note.hasStructuredKnowledge()) {",
            "    if (note.requiresStructuredKnowledgeReader()) {",
        )?;
        replace(&mut source,"    onAskKnowledge: (NoteEntry) -> Unit = {}", "    onAskKnowledge: (NoteEntry) -> Unit = {},\n    hierarchyContent: @Composable ((() -> Unit) -> Unit) -> Unit = {},\n    onHierarchyLocationChosen: (String?) -> Unit = {},\n    hasHierarchyChildren: Boolean = false")?;
        replace(&mut source,"        }, onPreviewImage = onRequestPreviewAttachment)", "        }, onPreviewImage = onRequestPreviewAttachment, hierarchyContent = hierarchyContent)")?;
        replace(&mut source, "    val documentTransitionActive = LocalDocumentTransitionActive.current", "    val documentTransitionActive = LocalDocumentTransitionActive.current\n    var hierarchyHandedOff by remember(note.id, LocalNoteMediaWorkspaceKey.current) { mutableStateOf(false) }")?;
        replace(&mut source, "        if (isSavingDraft || pendingRevisionRestore != null) return@LaunchedEffect", "        if (hierarchyHandedOff || isSavingDraft || pendingRevisionRestore != null) return@LaunchedEffect")?;
        replace(&mut source, "    ) {\n        // An unchanged visit owns no new write.", "    ) {\n        if (hierarchyHandedOff) { onComplete(true); return }\n        // An unchanged visit owns no new write.")?;
        replace(&mut source, "        val visitNeedsBlankDeletion = deleteBlankDraft && blankDraftDeletionEligibleAtEntry &&", "        val visitNeedsBlankDeletion = deleteBlankDraft && blankDraftDeletionEligibleAtEntry && !hasHierarchyChildren && latestNote.document.knowledge == null &&")?;
        replace(&mut source, "            blankDraftDeletionEligibleAtEntry = note.isBlankDraft()", "            blankDraftDeletionEligibleAtEntry = note.isBlankDraft() && note.document.knowledge == null && !hasHierarchyChildren")?;
        replace(&mut source, "        val needsBlankDeletion = deleteBlankDraft &&", "        val needsBlankDeletion = deleteBlankDraft && !hasHierarchyChildren && candidate.document.knowledge == null &&")?;
        replace(&mut source, "            deleteBlankDraft = deleteBlankDraft && blankDraftDeletionEligibleAtEntry", "            deleteBlankDraft = deleteBlankDraft && blankDraftDeletionEligibleAtEntry && !hasHierarchyChildren && candidate.document.knowledge == null")?;
        // Every regular-editor snapshot retains rich/knowledge metadata from the authoritative record.
        // This covers autosave, cancelled imports, preview and durable navigation.
        source = source.replace(
            "val document = NoteDocument(",
            "val document = latestNote.document.copy(",
        );
        replace(
            &mut source,
            "    fun undoDraft() {",
            &format!("{EDITOR_NAVIGATION}\n    fun undoDraft() {{"),
        )?;
        replace(&mut source,"        item(key = \"document-editor-properties\") {", "        item(key = \"document-editor-hierarchy\") { hierarchyContent(::navigateHierarchy) }\n        item(key = \"document-editor-properties\") {")?;
        replace(
            &mut source,
            "                if (previewNote.encryption == null) encryptDialogVisible = true",
            r####"                if (previewNote.encryption == null && folders.any { it.id == previewNote.id }) {
                    localizedToast(context, "从旧文件夹转换的记录暂不支持加密，原内容已保留。", Toast.LENGTH_LONG).show()
                } else if (previewNote.encryption == null && (previewNote.isKnowledgeFolder() || hasHierarchyChildren ||
                    (previewNote.document.knowledge?.get("parentId") as? kotlinx.serialization.json.JsonPrimitive)?.content.orEmpty().isNotBlank())) {
                    localizedToast(context, "先将子项移出、转为页面并移到知识目录根级，再加密。", Toast.LENGTH_LONG).show()
                } else if (previewNote.encryption == null) encryptDialogVisible = true"####,
        )?;
        // Folder selection now commits through the hierarchy action. Text edits must
        // retain the current repository location if another device moved the page.
        let location_anchor =
            "                pinned = pinned,\n                folderId = folderId";
        let other_location_anchor = "            pinned = pinned,\n            folderId = folderId";
        if source.matches(location_anchor).count() + source.matches(other_location_anchor).count()
            != 4
        {
            return Err("hierarchy editor snapshot locations missing".into());
        }
        source = source
            .replace(
                location_anchor,
                "                pinned = pinned,\n                folderId = latestNote.folderId",
            )
            .replace(
                other_location_anchor,
                "            pinned = pinned,\n            folderId = latestNote.folderId",
            );
        // Choosing a legacy folder is also a real hierarchy move, after saving the live draft.
        replace(&mut source,"            onFolderChange = { recordChange(currentDraftState().copy(folderId = it)) },", "            onFolderChange = { parent ->\n                documentPropertiesVisible = false\n                navigateHierarchy { onHierarchyLocationChosen(parent) }\n            },")?;
    } else if path.ends_with("/StructuredKnowledgeReader.kt") {
        replace(&mut source,"    onPreviewImage: (String) -> Unit\n", "    hierarchyContent: @Composable ((() -> Unit) -> Unit) -> Unit = {},\n    onPreviewImage: (String) -> Unit\n")?;
        replace(&mut source,"            item(key = \"structured_header\") {", "            item(key = \"structured_hierarchy\") { hierarchyContent { action ->\n                if (transitionActive && !saving && draft == null && !insertOpen && latestWorkspace == workspace) action()\n            } }\n            item(key = \"structured_header\") {")?;
        replace(&mut source,"                    (page?.get(\"properties\") as? JsonObject)?.forEach { (key, value) ->", "                    (page?.get(\"properties\") as? JsonObject)?.filterKeys { it !in setOf(\"__tenrate_container_kind\", \"__tenrate_legacy_folder\") }?.forEach { (key, value) ->")?;
    }
    Ok(source)
}

const BRIDGE: &str = r####"    fun knowledgeContainerActionAppDataJson(raw: String, action: String, id: String,
        parentId: String, title: String, now: Long): String? {
        if (!nativeAvailable) return null
        return runCatching { nativeKnowledgeContainerActionAppDataJson(raw, action, id, parentId, title, now) }.getOrNull()
    }
    private external fun nativeKnowledgeContainerActionAppDataJson(raw: String, action: String,
        id: String, parentId: String, title: String, now: Long): String?
"####;
const REPOSITORY: &str = r####"    internal suspend fun knowledgeContainerAction(action: String, id: String, parentId: String,
        title: String, expectedWorkspaceKey: String): Boolean {
        awaitInitialized()
        var applied = false
        val attempt = withContext(NonCancellable) { updateDataDetailed(expectedWorkspaceKey = expectedWorkspaceKey,
            persistImmediately = true, publishAfterDurable = true) { data ->
            decodeNativeAppData(NativeOptimizerBridge.knowledgeContainerActionAppDataJson(
                json.encodeToString(data), action, id, parentId, title, now()))?.also { applied = true } ?: data
        }
        }
        return applied && attempt.succeeded
    }
"####;
const VIEW_MODEL: &str = r####"    internal fun knowledgeContainerAction(action: String, id: String, parentId: String,
        title: String, expectedWorkspaceKey: String = currentWorkspaceKey(),
        onComplete: (Boolean, String) -> Unit) {
        enqueueNoteMutation(onComplete = { ok -> onComplete(ok,
            if (ok) "本地已保存" else "操作未完成。请核对当前位置、页面权限及存储后重试。") }) {
            repository.knowledgeContainerAction(action, id, parentId, title, expectedWorkspaceKey)
        }
    }
"####;
const STUDIO_STATE: &str = r####"    val hierarchyIndex = remember(appData.notes, appData.noteFolders) { appData.knowledgeHierarchy() }
    var hierarchyVisible by rememberSaveable(workspaceKey) { mutableStateOf(false) }
    var hierarchyParentId by rememberSaveable(workspaceKey) { mutableStateOf<String?>(null) }
    var hierarchyRequest by remember(workspaceKey) { mutableStateOf<KnowledgeHierarchyRequest?>(null) }
    var hierarchyBusy by remember(workspaceKey) { mutableStateOf(false) }
    var hierarchyMessage by remember(workspaceKey) { mutableStateOf("") }
    val hierarchyCallbacksAlive = remember(workspaceKey) { java.util.concurrent.atomic.AtomicBoolean(true) }
    DisposableEffect(hierarchyCallbacksAlive) { onDispose { hierarchyCallbacksAlive.set(false) } }
    LaunchedEffect(hierarchyIndex, hierarchyParentId) {
        if (hierarchyParentId != null && !hierarchyIndex.nodes.containsKey(hierarchyParentId)) hierarchyParentId = null
    }
"####;
const HIERARCHY_BROWSER: &str = r####"                    KnowledgeHierarchyBrowser(index = hierarchyIndex, parentId = hierarchyParentId,
                        busy = hierarchyBusy, message = hierarchyMessage, onBack = {
                            if (isCurrentFrame() && !hierarchyBusy) {
                                if (hierarchyParentId == null) hierarchyVisible = false
                                else hierarchyParentId = hierarchyIndex.parentOf(hierarchyParentId!!)
                            }
                        }, onBrowse = { if (isCurrentFrame() && !hierarchyBusy) hierarchyParentId = it }, onOpen = { node ->
                            if (isCurrentFrame() && !hierarchyBusy && hierarchyIndex.issues[node.id] != KnowledgeHierarchyIssue.IDENTITY_CONFLICT) node.note?.let { note ->
                                if (note.isEncryptionLocked()) { unlockTarget = note; unlockError = "" }
                                else { pendingCreatedNote = null; unlockedNote = null; selectedNoteId = note.id; searchLocateQuery = null }
                            }
                        }, onRequest = { if (isCurrentFrame() && !hierarchyBusy) hierarchyRequest = it })
"####;
const STUDIO_DIALOG: &str = r####"    hierarchyRequest?.let { request ->
        KnowledgeHierarchyActionDialog(request = request, index = hierarchyIndex, busy = hierarchyBusy,
            error = hierarchyMessage, onDismiss = { if (!hierarchyBusy) { hierarchyRequest = null; hierarchyMessage = "" } },
            onSubmit = { parent, title ->
                if (!hierarchyBusy && hierarchyCallbacksAlive.get() && viewModel.currentWorkspaceKey() == workspaceKey) {
                    hierarchyBusy = true; hierarchyMessage = ""
                    val id = request.id ?: java.util.UUID.randomUUID().toString()
                    viewModel.knowledgeContainerAction(request.action, id, parent.orEmpty(), title, workspaceKey) { ok, detail ->
                        if (hierarchyCallbacksAlive.get() && viewModel.currentWorkspaceKey() == workspaceKey) {
                            hierarchyBusy = false
                            if (ok) {
                                hierarchyRequest = null
                                if (request.action == "create_page") { hierarchyParentId = parent; hierarchyVisible = true; selectedNoteId = id }
                                if (request.action == "create_folder" || request.action == "to_folder") {
                                    hierarchyVisible = true; hierarchyParentId = id; selectedNoteId = null; pendingCreatedNote = null
                                }
                                if (request.action == "to_page") { hierarchyVisible = true; hierarchyParentId = parent; selectedNoteId = id }
                            } else hierarchyMessage = detail
                        }
                    }
                }
            })
    }
"####;
const EDITOR_NAVIGATION: &str = r####"    fun navigateHierarchy(action: () -> Unit) {
        if (!documentTransitionActive || isSavingDraft || pendingRevisionRestore != null) return
        focusManager.clearFocus(force = true); exitKeyboard?.hide()
        isSavingDraft = true
        persistLatestDraft("knowledge_hierarchy_navigation", deleteBlankDraft = false) { saved ->
            isSavingDraft = false
            if (!editorSaveCallbackActive.get() || viewModel.currentWorkspaceKey() != editorWorkspaceKey) return@persistLatestDraft
            if (saved) {
                hierarchyHandedOff = true
                editorSessionViewModel.clear(note.id, editorWorkspaceKey, editorSession)
                action()
            }
            else localizedToast(context, "尚未保存成功，保留当前页面，请重试。", Toast.LENGTH_LONG).show()
        }
    }
"####;

pub const CONTENTS: &str = r####"package com.ofairyo.gridtimer.ui

import androidx.activity.compose.BackHandler
import androidx.compose.foundation.clickable
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.rememberScrollState
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import com.ofairyo.gridtimer.data.*

internal data class KnowledgeHierarchyRequest(val action: String, val id: String? = null, val parentId: String? = null)

@Composable
private fun KnowledgeBreadcrumbs(index: KnowledgeHierarchy, id: String?, onBrowse: (String?) -> Unit) {
    Row(Modifier.fillMaxWidth().horizontalScroll(rememberScrollState()), horizontalArrangement = Arrangement.spacedBy(4.dp)) {
        TextButton(onClick = { onBrowse(null) }) { Text("知识目录") }
        id?.let { index.breadcrumbs(it).forEach { node ->
            TextButton(onClick = { onBrowse(node.id) }) { Text("› " + node.title.ifBlank { "无标题" }) }
        } }
    }
}

@Composable
private fun KnowledgeHierarchyRow(node: KnowledgeHierarchyNode, index: KnowledgeHierarchy, onOpen: () -> Unit) {
    ListItem(modifier = Modifier.fillMaxWidth().clickable(enabled = index.issues[node.id] != KnowledgeHierarchyIssue.IDENTITY_CONFLICT, onClick = onOpen),
        headlineContent = { Text((if (node.isFolder) "▣ " else "▤ ") + node.title.ifBlank { "无标题" }) },
        supportingContent = { Text(when {
            index.issues[node.id] == KnowledgeHierarchyIssue.IDENTITY_CONFLICT -> "记录编号冲突，已保护原页面和旧文件夹，暂停修改"
            node.note?.encryption != null -> "加密页面"
            index.issues.containsKey(node.id) -> "上级不可用，已放在目录中"
            !node.writable -> "只读 · ${index.children(node.id).size} 个子项"
            else -> "${if (node.isFolder) "文件夹" else "页面"} · ${index.children(node.id).size} 个子项"
        }) }, trailingContent = { Text("›") })
}

@OptIn(ExperimentalLayoutApi::class)
@Composable
private fun KnowledgeHierarchyActions(index: KnowledgeHierarchy, parentId: String?, busy: Boolean,
    onRequest: (KnowledgeHierarchyRequest) -> Unit, onOpenBody: (() -> Unit)? = null) {
    val node = parentId?.let(index.nodes::get)
    FlowRow(horizontalArrangement = Arrangement.spacedBy(6.dp), verticalArrangement = Arrangement.spacedBy(2.dp)) {
        OutlinedButton(enabled = !busy && index.mayCreateInside(parentId), onClick = { onRequest(KnowledgeHierarchyRequest("create_page", parentId = parentId)) }) { Text("新建子页面") }
        OutlinedButton(enabled = !busy && index.mayCreateInside(parentId), onClick = { onRequest(KnowledgeHierarchyRequest("create_folder", parentId = parentId)) }) { Text("新建子文件夹") }
        if (node != null) {
            TextButton(enabled = !busy && node.writable, onClick = { onRequest(KnowledgeHierarchyRequest("move", node.id, node.parentId)) }) { Text("移动") }
            TextButton(enabled = !busy && node.writable, onClick = { onRequest(KnowledgeHierarchyRequest("rename", node.id, node.parentId)) }) { Text("重命名") }
            TextButton(enabled = !busy && node.writable && node.note?.document?.knowledge?.get("database")?.let { it != kotlinx.serialization.json.JsonNull } != true,
                onClick = { onRequest(KnowledgeHierarchyRequest(if (node.isFolder) "to_page" else "to_folder", node.id, node.parentId)) }) {
                Text(if (node.isFolder) "转为页面" else "转为文件夹")
            }
            if (onOpenBody != null && node.note != null) TextButton(enabled = !busy && index.issues[node.id] != KnowledgeHierarchyIssue.IDENTITY_CONFLICT, onClick = onOpenBody) { Text("查看与编辑正文") }
        }
    }
}

@Composable
internal fun KnowledgeHierarchyPanel(index: KnowledgeHierarchy, parentId: String, busy: Boolean,
    onBrowse: (String?) -> Unit, onOpen: (KnowledgeHierarchyNode) -> Unit, onRequest: (KnowledgeHierarchyRequest) -> Unit) {
    val children = index.children(parentId)
    Column(Modifier.fillMaxWidth(), verticalArrangement = Arrangement.spacedBy(4.dp)) {
        KnowledgeBreadcrumbs(index, parentId) { if (!busy) onBrowse(it) }
        KnowledgeHierarchyActions(index, parentId, busy, onRequest)
        Text("子项 ${children.size}", style = MaterialTheme.typography.labelLarge)
        children.take(3).forEach { node -> KnowledgeHierarchyRow(node, index) { if (!busy) onOpen(node) } }
        if (children.isEmpty()) Text("还没有子项", style = MaterialTheme.typography.bodySmall)
        TextButton(enabled = !busy, onClick = { onBrowse(parentId) }) { Text("查看全部子项") }
        HorizontalDivider()
    }
}

@Composable
internal fun KnowledgeHierarchyBrowser(index: KnowledgeHierarchy, parentId: String?, busy: Boolean,
    message: String, onBack: () -> Unit, onBrowse: (String?) -> Unit,
    onOpen: (KnowledgeHierarchyNode) -> Unit, onRequest: (KnowledgeHierarchyRequest) -> Unit) {
    val node = parentId?.let(index.nodes::get)
    val children = index.children(parentId)
    val active = LocalDocumentTransitionActive.current
    BackHandler(enabled = active) { if (!busy) onBack() }
    LazyColumn(Modifier.fillMaxSize().windowInsetsPadding(WindowInsets.safeDrawing).padding(horizontal = 14.dp),
        contentPadding = PaddingValues(bottom = 100.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
        item {
            Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.SpaceBetween) {
                TextButton(enabled = !busy, onClick = onBack) { Text("返回") }
                Text("目录", style = MaterialTheme.typography.titleLarge)
                if (busy) CircularProgressIndicator(Modifier.size(22.dp))
            }
            KnowledgeBreadcrumbs(index, parentId) { if (!busy) onBrowse(it) }
            Text(node?.title?.ifBlank { "无标题" } ?: "页面与文件夹", style = MaterialTheme.typography.headlineSmall.copy(fontWeight = FontWeight.SemiBold))
            KnowledgeHierarchyActions(index, parentId, busy, onRequest,
                onOpenBody = node?.note?.let { { onOpen(node) } })
            if (message.isNotBlank()) Text(message, color = MaterialTheme.colorScheme.error)
            Text("${children.size} 个子项", style = MaterialTheme.typography.labelLarge)
        }
        items(children, key = { it.id }) { child ->
            KnowledgeHierarchyRow(child, index) { if (!busy) { if (child.isFolder) onBrowse(child.id) else onOpen(child) } }
        }
        if (children.isEmpty()) item { Text("此处还没有页面或文件夹") }
    }
}

@Composable
internal fun KnowledgeHierarchyActionDialog(request: KnowledgeHierarchyRequest, index: KnowledgeHierarchy,
    busy: Boolean, error: String, onDismiss: () -> Unit, onSubmit: (String?, String) -> Unit) {
    val node = request.id?.let(index.nodes::get)
    var title by rememberSaveable(request) { mutableStateOf(if (request.action == "rename") node?.title.orEmpty() else "") }
    var parent by rememberSaveable(request) { mutableStateOf(request.parentId) }
    var query by rememberSaveable(request) { mutableStateOf("") }
    val moving = request.action == "move"
    val creating = request.action.startsWith("create_")
    val naming = creating || request.action == "rename"
    val valid = !busy && when {
        naming -> (if (creating) index.mayCreateInside(request.parentId) else node?.writable == true) && title.isNotBlank() && title.length <= 120 && !title.contains('\n')
        moving -> node?.writable == true && (parent == null || index.moveTargets(request.id.orEmpty()).any { it.id == parent }) && parent != node?.parentId
        else -> node?.writable == true
    }
    AlertDialog(onDismissRequest = { if (!busy) onDismiss() }, title = { Text(when(request.action) {
        "create_folder" -> "新建文件夹"; "create_page" -> "新建页面"; "rename" -> "重命名"
        "move" -> "移动到"; "to_folder" -> "转为文件夹"; else -> "转为页面"
    }) }, text = {
        Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
            if (naming) OutlinedTextField(value = title, onValueChange = { title = it.take(120) }, singleLine = true,
                enabled = !busy, label = { Text("名称") }, modifier = Modifier.fillMaxWidth())
            if (moving) {
                OutlinedTextField(value = query, onValueChange = { query = it.take(120) }, singleLine = true,
                    enabled = !busy, label = { Text("查找位置") }, modifier = Modifier.fillMaxWidth())
                Text("当前位置：" + node?.parentId?.let(index::pathLabel).orEmpty().ifBlank { "知识目录" })
                Text("目标：" + parent?.let(index::pathLabel).orEmpty().ifBlank { "知识目录" })
                val targets = index.moveTargets(request.id.orEmpty()).filter { index.pathLabel(it.id).contains(query.trim(), ignoreCase = true) }
                LazyColumn(Modifier.fillMaxWidth().heightIn(max = 300.dp)) {
                    item { TextButton(enabled = !busy, onClick = { parent = null }) { Text("知识目录（根级）") } }
                    items(targets, key = { it.id }) { target ->
                        TextButton(enabled = !busy, onClick = { parent = target.id }) { Text(index.pathLabel(target.id)) }
                    }
                }
            } else if (!naming) Text("保留正文、附件、页面链接和子项，只更改页面类型。可随时转换回来。")
            if (creating) Text("位置：" + request.parentId?.let(index::pathLabel).orEmpty().ifBlank { "知识目录" })
            if (busy) Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) { CircularProgressIndicator(Modifier.size(22.dp)); Text("正在保存…") }
            if (error.isNotBlank()) Text(error, color = MaterialTheme.colorScheme.error)
        }
    }, confirmButton = { TextButton(enabled = valid, onClick = { onSubmit(parent, title.trim()) }) { Text("确定") } },
        dismissButton = { TextButton(enabled = !busy, onClick = onDismiss) { Text("取消") } })
}
"####;
