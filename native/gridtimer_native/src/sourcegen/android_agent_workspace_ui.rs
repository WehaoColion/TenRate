// Runs after the existing request/verification transformations. The task workspace
// owns presentation only; native authorization, cancellation and save guards stay authoritative.
fn replace(source: &mut String, before: &str, after: &str) -> Result<(), String> {
    if source.matches(before).count() != 1 {
        return Err(format!("Agent workspace anchor: {before}"));
    }
    *source = source.replacen(before, after, 1);
    Ok(())
}
fn cut(source: &mut String, from: &str, to: &str, replacement: &str) -> Result<(), String> {
    let start = source
        .find(from)
        .ok_or_else(|| format!("Agent workspace start: {from}"))?;
    let end = source[start..]
        .find(to)
        .map(|n| start + n)
        .ok_or_else(|| format!("Agent workspace end: {to}"))?;
    source.replace_range(start..end, replacement);
    Ok(())
}
pub fn render(path: &str, source: &str) -> Result<String, String> {
    if path != "com/ofairyo/gridtimer/ui/NoteStudioSheet.kt" {
        return Ok(source.to_owned());
    }
    let mut source = source.to_owned();
    replace(&mut source, "import androidx.compose.material.icons.rounded.Title", "import androidx.compose.material.icons.rounded.PlayArrow\nimport androidx.compose.material.icons.rounded.Title")?;
    replace(&mut source,
        "    val agentBoundary = remember(workspaceKey, identity, priorityNoteId) { KnowledgeAgentRequestBoundary() }",
        "    var agentWorkspaceEpoch by remember(workspaceKey, identity, priorityNoteId) { mutableStateOf(0) }\n    val agentBoundary = remember(workspaceKey, identity, priorityNoteId, agentWorkspaceEpoch) { KnowledgeAgentRequestBoundary() }")?;
    replace(
        &mut source,
        "    var agentProgress by remember(agentBoundary)",
        STATES,
    )?;
    replace(
        &mut source,
        "        agentBusy = true; agentCancelling = false;",
        "        agentWorkspaceStep = 3\n        agentBusy = true; agentCancelling = false;",
    )?;
    let start = source
        .find("    Dialog(onDismissRequest = { if (agentBusy) stopAgent(\"已关闭 Agent 面板\")")
        .ok_or("Agent workspace dialog missing")?;
    let end = source[start..]
        .find("@OptIn(androidx.compose.foundation.ExperimentalFoundationApi::class")
        .map(|n| start + n)
        .ok_or("Agent workspace dialog end missing")?;
    let mut dialog = source[start..end].to_owned();
    // Remove the old interleaved Agent controls from the ordinary chat list.
    cut(
        &mut dialog,
        "                if (agentMode) item {",
        "                if (question.isBlank() && conversation.isEmpty()) item {",
        "",
    )?;
    cut(
        &mut dialog,
        "                    if (agentMode) {\n                    val launchAction",
        "                if (busy) item {",
        "                }\n",
    )?;
    cut(
        &mut dialog,
        "                if (agentBusy) item {",
        "                if (statusText.isNotBlank()) item {",
        "",
    )?;
    cut(
        &mut dialog,
        "                if (agentMode && (agentBusy",
        "                if (showSendingContent) {",
        "",
    )?;
    cut(
        &mut dialog,
        "                if (showSendingContent) {",
        "            if (!agentMode) {",
        CHAT_PREVIEW,
    )?;
    replace(
        &mut dialog,
        "            LazyColumn(state = messageListState",
        "            if (!agentMode) {\n            LazyColumn(state = messageListState",
    )?;
    replace(
        &mut dialog,
        "            if (!agentMode) {\n                if (busy)",
        "            }\n            if (!agentMode) {\n                if (busy)",
    )?;
    replace(
        &mut dialog,
        "            if (!agentMode) {\n            LazyColumn",
        &format!("{WORKSPACE}\n            if (!agentMode) {{\n            LazyColumn"),
    )?;
    dialog = dialog.replace(
        "Text(if (agentMode) \"AI Agent\" else \"AI 问答\"",
        "Text(if (agentMode) \"任务助手\" else \"AI 问答\"",
    );
    dialog = dialog.replace("if (agentBusy) stopAgent(\"已关闭 Agent 面板\"); onDismiss()", "if (!agentSaving) { if (agentMode && !agentBusy && !busy) requestAgentWorkspaceReset(false, \"close\") else { if (agentBusy) stopAgent(\"已关闭 Agent 面板\"); onDismiss() } }");
    source.replace_range(start..end, &format!("{RESET}\n{dialog}"));
    replace(&mut source, "private fun KnowledgeQuestionField(question: String, hint: String, onQuestionChange: (String) -> Unit)",
        "private fun KnowledgeQuestionField(question: String, hint: String, onQuestionChange: (String) -> Unit, fieldLabel: String = \"消息\")")?;
    replace(
        &mut source,
        "onValueChange = onQuestionChange, label = \"消息\"",
        "onValueChange = onQuestionChange, label = fieldLabel",
    )?;
    source = source
        .replace("关闭面板后可新建任务。", "可直接开始新任务。")
        .replace(
            "onClick = { setAgentMode(true) })",
            "onClick = { if (!busy && !agentBusy && !agentSaving) setAgentMode(true) })",
        );
    Ok(source)
}
const STATES: &str = r####"    var agentWorkspaceStep by remember(agentBoundary) { mutableStateOf(0) }
    var agentSourceSearch by remember(agentBoundary) { mutableStateOf("") }
    var agentEditingDraft by remember(agentBoundary) { mutableStateOf(false) }
    var agentShowDetails by remember(agentBoundary) { mutableStateOf(false) }
    var agentResetConfirmation by remember(agentBoundary) { mutableStateOf(false) }
    var agentResetKeepInputs by remember(agentBoundary) { mutableStateOf(false) }
    var agentResetAfter by remember(agentBoundary) { mutableStateOf("") }
    val agentWorkspaceList = rememberLazyListState()
    LaunchedEffect(agentBoundary, agentWorkspaceStep) { agentWorkspaceList.scrollToItem(0) }
    var agentProgress by remember(agentBoundary)"####;
const RESET: &str = r####"    fun resetAgentWorkspace(keepInputs: Boolean) {
        if (!canResetKnowledgeAgentWorkspace(agentBusy, agentSaving, busy)) return
        agentBoundary.close()
        agentTaskId = null
        agentDraft = null; agentDraftContext = null; agentDraftTitle = ""; agentDraftBody = ""
        agentResultSources = emptyList(); agentTrace = emptyList(); agentPlan = emptyList()
        agentVerification = emptyList(); agentVerificationPassed = false; agentHumanEdited = false
        agentPreviewAcknowledged = false; showSendingContent = false; statusText = ""
        if (!keepInputs) { question = ""; agentSelectedIds.clear() }
        agentWorkspaceEpoch++
    }
    fun requestAgentWorkspaceReset(keepInputs: Boolean, after: String = "") {
        if (!canResetKnowledgeAgentWorkspace(agentBusy, agentSaving, busy)) return
        if (agentDraft != null && !agentSaved) {
            agentResetKeepInputs = keepInputs; agentResetAfter = after; agentResetConfirmation = true
        } else {
            resetAgentWorkspace(keepInputs)
            if (after == "chat") setAgentMode(false)
            if (after == "close") onDismiss()
        }
    }
    LaunchedEffect(agentMode) { if (agentMode && !agentBusy && !agentSaving) agentWorkspaceStep = 0 }
    LaunchedEffect(agentWorkspaceStep, showSendingContent) {
        if (agentMode && agentWorkspaceStep == 2 && !showSendingContent) agentWorkspaceStep = 1
    }
    if (agentResetConfirmation) {
        androidx.compose.material3.AlertDialog(onDismissRequest = { agentResetConfirmation = false },
            title = { Text("放弃当前草稿？") },
            text = { Text(if (agentResetKeepInputs) "保留任务目标与资料选择，重新执行后生成新草稿。当前未保存的内容将被放弃。" else "当前未保存的内容将被放弃，已保存的知识页不受影响。") },
            confirmButton = { TextButton(onClick = { val keep = agentResetKeepInputs; val after = agentResetAfter; agentResetConfirmation = false; resetAgentWorkspace(keep); if (after == "chat") setAgentMode(false); if (after == "close") onDismiss() }) { Text("继续") } },
            dismissButton = { TextButton(onClick = { agentResetConfirmation = false }) { Text("保留草稿") } })
    }
"####;
const CHAT_PREVIEW: &str = r####"                if (showSendingContent) {
                    item { Text("待发送的问题", fontWeight = FontWeight.SemiBold); Text(question.ifBlank { "尚未输入问题" }) }
                    items(knowledgeConversationContext(conversation.map { it.turn })) { turn ->
                        Text("你：${turn.question}\nAI：${turn.answer}", style = MaterialTheme.typography.bodySmall)
                    }
                    if (queryMode == KnowledgeAiMode.KNOWLEDGE) item { Text("待发送的来源节选 ${sourceCandidates.size}", fontWeight = FontWeight.SemiBold) }
                    items(sourceCandidates, key = { "preview-${it.note.id}" }) { source -> KnowledgeSourceCard(source, sourceCandidates.indexOf(source) + 1) { onOpenSource(source) } }
                }
            }
"####;
const WORKSPACE: &str = r####"            if (agentMode) {
                val locked = agentBusy || agentSaving || busy
                val canPrepare = !locked && !agentSaved
                val visibleDocuments = remember(agentSelectableDocuments, agentSourceSearch, appData.noteFolders) {
                    agentSelectableDocuments.filter { knowledgeAgentSourceMatches(agentSourceSearch, it.displayTitle(), appData.findNoteFolder(it.folderId)?.name.orEmpty()) }
                }
                Row(modifier = Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.SpaceBetween, verticalAlignment = Alignment.CenterVertically) {
                    TextButton(enabled = !locked, onClick = { requestAgentWorkspaceReset(false, "chat") }) { Text("返回问答") }
                    TextButton(enabled = !locked, onClick = { requestAgentWorkspaceReset(false) }) { Text("新任务") }
                }
                Row(modifier = Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    listOf("写任务", "选资料", "核对", "结果").forEachIndexed { index, title ->
                        Text("${index + 1} $title", modifier = Modifier.weight(1f), style = MaterialTheme.typography.labelMedium,
                            color = if (index == agentWorkspaceStep) notesAccentColor else MaterialTheme.colorScheme.onSurface.copy(alpha = 0.45f),
                            fontWeight = if (index == agentWorkspaceStep) FontWeight.Bold else FontWeight.Normal)
                    }
                }
                LazyColumn(state = agentWorkspaceList, modifier = Modifier.weight(1f).fillMaxWidth(), verticalArrangement = Arrangement.spacedBy(14.dp)) {
                    when (agentWorkspaceStep) {
                        0 -> {
                            item {
                                Text("这次想完成什么？", style = MaterialTheme.typography.titleLarge, fontWeight = FontWeight.SemiBold)
                                Text("根据你选择的便签和知识页整理草稿，检查后再保存。", style = MaterialTheme.typography.bodyMedium)
                            }
                            item {
                                KnowledgeQuestionField(question, "说清目标、希望的结构和需要保留的细节", { if (canPrepare) { stopAgent("任务目标已改变"); question = it } }, fieldLabel = "任务目标")
                            }
                            item {
                                Text("从一个常用任务开始", style = MaterialTheme.typography.labelLarge)
                                listOf("整理项目进展，列出待办与未解决的问题", "归纳资料中的主要观点，标明来源和分歧", "整理成一份复盘，包含结论与下一步行动").forEach { template ->
                                    TextButton(enabled = canPrepare, onClick = { stopAgent("任务目标已改变"); question = template }) { Text(template) }
                                }
                            }
                            item {
                                FlowusPanel(accent = notesAccentColor) {
                                    Column(Modifier.fillMaxWidth().padding(14.dp), verticalArrangement = Arrangement.spacedBy(6.dp)) {
                                        Text(if (configuration.testable) configuration.model else "先配置 AI", fontWeight = FontWeight.Medium)
                                        Text(configuration.recipientHost.ifBlank { "尚未设置接口" }, style = MaterialTheme.typography.bodySmall)
                                        TextButton(enabled = !locked, onClick = { boundary.invalidate(); onDismiss(); onOpenAiSettings() }) { Text("AI 设置") }
                                    }
                                }
                                Row(verticalAlignment = Alignment.CenterVertically) {
                                    Checkbox(checked = agentDeepReview, enabled = canPrepare, onCheckedChange = { checked -> stopAgent("核验模式已改变"); agentDeepReview = checked })
                                    Column {
                                        Text("增加一次深度复核", fontWeight = FontWeight.Medium)
                                        Text("再次核对来源、遗漏与冲突，会增加模型请求。", style = MaterialTheme.typography.bodySmall)
                                    }
                                }
                            }
                        }
                        1 -> {
                            item {
                                Text("选择本次可读取的资料", style = MaterialTheme.typography.titleMedium, fontWeight = FontWeight.SemiBold)
                                Text("已选 ${agentSelectedIds.size}/30 · 只包含未删除、未加密的便签与知识页。", style = MaterialTheme.typography.bodySmall)
                                FlowRow(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                                    if (selectedFolderId != null) ChoicePill(text = selectedFolderName ?: "当前文档库", selected = searchScope == KnowledgeSearchScope.CURRENT_FOLDER, accent = notesAccentColor,
                                        onClick = { if (canPrepare) { stopAgent("资料范围已改变"); agentSelectedIds.clear(); searchScope = KnowledgeSearchScope.CURRENT_FOLDER } })
                                    ChoicePill(text = "全部资料", selected = searchScope == KnowledgeSearchScope.ALL, accent = notesAccentColor,
                                        onClick = { if (canPrepare) { stopAgent("资料范围已改变"); agentSelectedIds.clear(); searchScope = KnowledgeSearchScope.ALL } })
                                }
                                OutlinedTextField(value = agentSourceSearch, onValueChange = { agentSourceSearch = it.take(100) }, label = { Text("搜索标题或文档库") }, singleLine = true, modifier = Modifier.fillMaxWidth())
                                Row {
                                    TextButton(enabled = canPrepare && visibleDocuments.isNotEmpty(), onClick = {
                                        stopAgent("资料选择已改变")
                                        val next = addKnowledgeAgentVisibleSources(agentSelectedIds.toSet(), visibleDocuments.map(NoteEntry::id))
                                        agentSelectedIds.clear(); agentSelectedIds.addAll(next)
                                    }) { Text("选入当前结果") }
                                    TextButton(enabled = canPrepare && agentSelectedIds.isNotEmpty(), onClick = { stopAgent("资料选择已改变"); agentSelectedIds.clear() }) { Text("清空已选") }
                                }
                            }
                            if (visibleDocuments.isEmpty()) item { Text(if (agentSourceSearch.isBlank()) "当前范围没有可用资料。可切换到全部资料，或先创建便签与知识页。" else "没有匹配的标题或文档库，试试其他关键词。") }
                            items(visibleDocuments, key = { "workspace-source-${it.id}" }) { note ->
                                val checked = note.id in agentSelectedIds
                                val selectable = canPrepare && (checked || agentSelectedIds.size < 30)
                                Row(modifier = Modifier.fillMaxWidth().clickable(enabled = selectable) {
                                    stopAgent("资料选择已改变")
                                    if (checked) agentSelectedIds.remove(note.id) else agentSelectedIds.add(note.id)
                                }.padding(vertical = 6.dp), verticalAlignment = Alignment.CenterVertically) {
                                    Checkbox(checked = checked, enabled = selectable, onCheckedChange = { value ->
                                        stopAgent("资料选择已改变")
                                        if (!value) agentSelectedIds.remove(note.id) else if (note.id !in agentSelectedIds && agentSelectedIds.size < 30) agentSelectedIds.add(note.id)
                                    })
                                    Column(Modifier.weight(1f)) {
                                        Text(note.displayTitle(), maxLines = 2, overflow = TextOverflow.Ellipsis, fontWeight = FontWeight.Medium)
                                        Text("${if (note.kind == NoteEntryKind.STICKY) "便签" else "知识页"} · ${appData.findNoteFolder(note.folderId)?.name ?: "未归类"}", style = MaterialTheme.typography.bodySmall)
                                    }
                                }
                            }
                        }
                        2 -> {
                            item {
                                Text("核对本次发送范围", style = MaterialTheme.typography.titleMedium, fontWeight = FontWeight.SemiBold)
                                Text("目标：$question")
                                Text("接收方：${configuration.recipientHost}\n模型：${configuration.model}", style = MaterialTheme.typography.bodySmall)
                                Text("已选 ${selectedAgentDocuments.size} 条资料，共 ${selectedAgentDocuments.sumOf { it.content.length }} 字符。${if (agentDeepReview) "包含深度复核，每阶段最多 6 次请求。" else "首轮最多 6 次请求。"}", style = MaterialTheme.typography.bodySmall)
                                Text("下方是完整资料预览。模型按需检索和读取，每阶段读取累计上限 32,000 字符；每轮携带本阶段工具记录。请求设置 store:false，自定义服务的留存规则由服务商决定。", style = MaterialTheme.typography.bodySmall)
                                Text("附件、财务、计时记录和密钥不在本次授权范围。", style = MaterialTheme.typography.bodySmall)
                            }
                            items(selectedAgentDocuments, key = { "workspace-preview-${it.id}" }) { document ->
                                FlowusPanel(accent = notesAccentColor) {
                                    Column(Modifier.fillMaxWidth().padding(14.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
                                        Text(document.title, fontWeight = FontWeight.SemiBold)
                                        Text("${document.folder.ifBlank { "未归类" }} · ${document.content.length} 字符", style = MaterialTheme.typography.bodySmall)
                                        Text(document.content, style = MaterialTheme.typography.bodyMedium)
                                    }
                                }
                            }
                        }
                        else -> {
                            item {
                                Text(knowledgeAgentTerminalLabel(agentTerminal, agentSaved, agentCancelling), style = MaterialTheme.typography.titleMedium, fontWeight = FontWeight.SemiBold)
                                if (agentBusy) {
                                    androidx.compose.material3.LinearProgressIndicator(modifier = Modifier.fillMaxWidth())
                                    Text(agentProgress?.let { knowledgeAgentActivityLabel(it, agentCancelling) } ?: "正在准备本次任务…", style = MaterialTheme.typography.bodyMedium)
                                    agentProgress?.let { Text("已等待 ${it.elapsedSeconds} 秒 · 已读取 ${it.documentsRead} 条资料", style = MaterialTheme.typography.bodySmall) }
                                }
                                Text(question, style = MaterialTheme.typography.bodySmall, maxLines = 3, overflow = TextOverflow.Ellipsis)
                                if (statusText.isNotBlank()) Text(statusText, style = MaterialTheme.typography.bodySmall)
                            }
                            if (agentDraft != null) item {
                                Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.SpaceBetween, verticalAlignment = Alignment.CenterVertically) {
                                    Text(if (agentSaved) "已保存的知识页" else "草稿预览", fontWeight = FontWeight.SemiBold)
                                    TextButton(enabled = !locked && !agentSaved, onClick = { agentEditingDraft = !agentEditingDraft }) { Text(if (agentEditingDraft) "查看预览" else "编辑草稿") }
                                }
                                if (agentEditingDraft && !agentSaved) {
                                    OutlinedTextField(value = agentDraftTitle, enabled = !locked, onValueChange = { if (!locked) { agentDraftTitle = it; agentHumanEdited = true } }, label = { Text("标题") }, modifier = Modifier.fillMaxWidth())
                                    OutlinedTextField(value = agentDraftBody, enabled = !locked, onValueChange = { if (!locked) { agentDraftBody = it; agentHumanEdited = true } }, label = { Text("正文与待办") }, modifier = Modifier.fillMaxWidth(), minLines = 6, maxLines = 18)
                                } else {
                                    Text(agentDraftTitle, style = MaterialTheme.typography.titleLarge, fontWeight = FontWeight.SemiBold)
                                    AndroidRenderedMarkdown(agentDraftBody, modifier = Modifier.fillMaxWidth())
                                }
                                if (agentHumanEdited) Text("你已编辑草稿；核验记录对应模型生成版本，保存以当前内容为准。", style = MaterialTheme.typography.bodySmall)
                            }
                            item {
                                TextButton(onClick = { agentShowDetails = !agentShowDetails }) { Text(if (agentShowDetails) "收起执行详情" else "查看执行与核验详情") }
                                if (agentShowDetails) {
                                    Text("模型请求 $agentRequests 次 · 工具调用 $agentToolCalls 次", style = MaterialTheme.typography.bodySmall)
                                    agentPlan.forEachIndexed { index, step -> Text("${index + 1}. $step", style = MaterialTheme.typography.bodySmall) }
                                    agentTrace.forEach { Text(it, style = MaterialTheme.typography.bodySmall) }
                                    agentVerification.forEach { Text(it, style = MaterialTheme.typography.bodySmall) }
                                }
                            }
                            items(agentResultSources, key = { "workspace-result-${it.note.id}" }) { source -> KnowledgeSourceCard(source, agentResultSources.indexOf(source) + 1) { onOpenSource(source) } }
                        }
                    }
                }
                if (agentWorkspaceStep == 2) {
                    Row(verticalAlignment = Alignment.CenterVertically) {
                        Checkbox(checked = agentPreviewAcknowledged, enabled = canPrepare && showSendingContent,
                            onCheckedChange = { agentPreviewAcknowledged = it && canPrepare && showSendingContent })
                        Text("我已核对目标、接收方和完整资料，授权本次发送。", style = MaterialTheme.typography.bodySmall,
                            modifier = Modifier.weight(1f).clickable(enabled = canPrepare && showSendingContent) { agentPreviewAcknowledged = !agentPreviewAcknowledged })
                    }
                }
                if (agentWorkspaceStep in 1..2) Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.SpaceBetween) {
                    TextButton(enabled = canPrepare, onClick = { agentPreviewAcknowledged = false; showSendingContent = false; agentWorkspaceStep-- }) { Text("上一步") }
                    Text("可发送 ${selectedAgentDocuments.size} 条", style = MaterialTheme.typography.bodySmall)
                }
                when (agentWorkspaceStep) {
                    0 -> {
                        if (!configuration.testable) Text("先配置 AI，再选择资料。", style = MaterialTheme.typography.bodySmall)
                        PhysicalButton(label = "下一步：选择资料", icon = Icons.Rounded.Folder, accent = notesAccentColor, filled = true,
                            enabled = canPrepare && configuration.testable && question.isNotBlank(), modifier = Modifier.fillMaxWidth(),
                            onClick = { agentPreviewAcknowledged = false; showSendingContent = false; agentWorkspaceStep = 1 })
                    }
                    1 -> {
                        if (agentSelectedIds.size > selectedAgentDocuments.size) Text("${agentSelectedIds.size - selectedAgentDocuments.size} 条所选资料正文不可用，不会发送。", style = MaterialTheme.typography.bodySmall)
                        PhysicalButton(label = "下一步：核对发送内容", icon = Icons.Rounded.CheckBox, accent = notesAccentColor, filled = true,
                        enabled = canPrepare && selectedAgentDocuments.isNotEmpty() && question.isNotBlank() && configuration.testable,
                        modifier = Modifier.fillMaxWidth(), onClick = { showSendingContent = true; agentPreviewAcknowledged = false; agentWorkspaceStep = 2 })
                    }
                    2 -> PhysicalButton(label = "授权并开始执行", icon = Icons.Rounded.PlayArrow, accent = notesAccentColor, filled = true,
                        enabled = knowledgeAgentLaunchAction(locked, agentSaving, !agentSaved && configuration.testable && question.isNotBlank() && selectedAgentDocuments.isNotEmpty(), showSendingContent, agentPreviewAcknowledged) == KnowledgeAgentLaunchAction.START,
                        modifier = Modifier.fillMaxWidth(), onClick = ::requestAgent)
                    else -> when {
                        agentBusy -> PhysicalButton(label = if (agentCancelling) "正在停止后续步骤" else "停止本次任务", icon = Icons.Rounded.Close, accent = notesAccentColor, filled = false,
                            enabled = !agentCancelling, modifier = Modifier.fillMaxWidth(), onClick = { stopAgent("已取消任务") })
                        agentSaving -> PhysicalButton(label = "正在确认保存…", icon = Icons.Rounded.SaveAlt, accent = notesAccentColor, filled = true, enabled = false, modifier = Modifier.fillMaxWidth(), onClick = {})
                        agentSaved -> PhysicalButton(label = "开始新任务", icon = Icons.Rounded.Add, accent = notesAccentColor, filled = true,
                            modifier = Modifier.fillMaxWidth(), onClick = { requestAgentWorkspaceReset(false) })
                        agentDraft != null -> {
                            val saveState = currentAgentSaveState()
                            Text(knowledgeAgentSaveReason(saveState), style = MaterialTheme.typography.bodySmall)
                            PhysicalButton(label = "保存为知识页", icon = Icons.Rounded.SaveAlt, accent = notesAccentColor, filled = true,
                                enabled = saveState == KnowledgeAgentSaveState.READY && !locked, modifier = Modifier.fillMaxWidth(), onClick = ::saveAgentDraft)
                            TextButton(enabled = !locked, onClick = { requestAgentWorkspaceReset(true) }) { Text("调整任务后重新执行") }
                        }
                        else -> PhysicalButton(label = "调整任务并重试", icon = Icons.AutoMirrored.Rounded.Redo, accent = notesAccentColor, filled = true,
                            enabled = !locked, modifier = Modifier.fillMaxWidth(), onClick = { requestAgentWorkspaceReset(true) })
                    }
                }
            }
"####;
