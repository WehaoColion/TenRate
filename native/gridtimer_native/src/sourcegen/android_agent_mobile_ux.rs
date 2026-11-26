// v3.0.2.40 - Ensure compact AI composer focus extension is imported by the Rust generator.
// v3.0.2.39 - Compact AI conversation layout, independent history and real emulator gate.
// Android Agent mobile UX: task-first flow and honest foreground-only execution.
// This layer runs after android_agent_workspace_ui and keeps Rust authorization,
// cancellation and save boundaries unchanged.
fn replace_once(source: &mut String, before: &str, after: &str) -> Result<(), String> {
    let count = source.matches(before).count();
    if count != 1 {
        return Err(format!("Agent mobile UX anchor count {count}: {before}"));
    }
    *source = source.replacen(before, after, 1);
    Ok(())
}

fn replace_count(source: &mut String, before: &str, after: &str, expected: usize) -> Result<(), String> {
    let count = source.matches(before).count();
    if count != expected {
        return Err(format!("Agent mobile UX anchor count {count}, wanted {expected}: {before}"));
    }
    *source = source.replace(before, after);
    Ok(())
}

pub fn render(path: &str, source: &str) -> Result<String, String> {
    if path != "com/ofairyo/gridtimer/ui/NoteStudioSheet.kt" {
        return Ok(source.to_owned());
    }
    let mut source = source.to_owned();

    // The existing selection state is owned by the knowledge UI. A Compose
    // effect below safely preselects a currently eligible page once per task;
    // it never bypasses the existing explicit send confirmation.

    replace_once(&mut source,
        "    var agentResetConfirmation by remember(agentBoundary) { mutableStateOf(false) }",
        "    var agentResetConfirmation by remember(agentBoundary) { mutableStateOf(false) }\n    var agentExitConfirmation by remember(agentBoundary) { mutableStateOf(false) }\n    var agentInitialSourcePicked by remember(agentBoundary) { mutableStateOf(false) }\n    val agentExpandedPreviewIds = remember(agentBoundary) { mutableStateListOf<String>() }",
    )?;

    replace_once(&mut source,
        "    LaunchedEffect(agentMode) { if (agentMode && !agentBusy && !agentSaving) agentWorkspaceStep = 0 }",
        r####"    fun requestAgentExit() {
        if (pendingTurn != null || savingConversation) {
            statusText = "当前问答仍未确认保存，请先点击重试保存。"
            return
        }
        if (agentSaving) return
        if (agentMode && agentBusy) agentExitConfirmation = true
        else if (agentMode && !busy) requestAgentWorkspaceReset(false, "close")
        else onDismiss()
    }
    LaunchedEffect(agentMode) { if (agentMode && !agentBusy && !agentSaving) agentWorkspaceStep = 0 }
    LaunchedEffect(agentMode, priorityNoteId, agentAvailableDocuments) {
        if (agentMode && !agentInitialSourcePicked) {
            agentInitialSourcePicked = true
            val currentId = priorityNoteId
            if (currentId != null && agentAvailableDocuments.any { it.id == currentId } &&
                currentId !in agentSelectedIds && agentSelectedIds.size < 30) {
                agentSelectedIds.add(currentId)
            }
        }
    }"####,
    )?;
    replace_once(&mut source,
        "    if (agentResetConfirmation) {",
        r####"    if (agentExitConfirmation) {
        androidx.compose.material3.AlertDialog(
            onDismissRequest = { agentExitConfirmation = false },
            title = { Text("停止正在执行的任务？") },
            text = { Text("当前版本不支持后台持续执行。离开任务面板会取消后续请求，未完成的结果不能保存。可以留在这里等待任务结束。") },
            confirmButton = {
                TextButton(onClick = {
                    agentExitConfirmation = false
                    stopAgent("用户停止并退出任务")
                    onDismiss()
                }) { Text("停止并退出") }
            },
            dismissButton = { TextButton(onClick = { agentExitConfirmation = false }) { Text("继续执行") } }
        )
    }
    if (agentResetConfirmation) {"####,
    )?;
    // Both the system back action and the header close action must confirm.
    replace_count(&mut source,
        "if (!agentSaving) { if (agentMode && !agentBusy && !busy) requestAgentWorkspaceReset(false, \"close\") else { if (agentBusy) stopAgent(\"已关闭 Agent 面板\"); onDismiss() } }",
        "requestAgentExit()", 2,
    )?;

    // Build a small, local suggestion list from titles and folder names only.
    // No contents leave the device until the user explicitly selects and authorizes.
    replace_once(&mut source,
        r####"                Row(modifier = Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.SpaceBetween, verticalAlignment = Alignment.CenterVertically) {
                    TextButton(enabled = !locked, onClick = { requestAgentWorkspaceReset(false, "chat") })"####,
        r####"                val suggestedDocuments = remember(visibleDocuments, question, priorityNoteId, appData.noteFolders) {
                    val terms = question.lowercase().filter { it.isLetterOrDigit() }.windowed(2).distinct().take(36)
                    if (terms.isEmpty()) emptyList() else visibleDocuments.mapNotNull { note ->
                        val title = note.displayTitle().lowercase()
                        val folder = appData.findNoteFolder(note.folderId)?.name.orEmpty().lowercase()
                        val score = terms.count { title.contains(it) } * 2 + terms.count { folder.contains(it) }
                        if (score > 0 || note.id == priorityNoteId) note to (score + if (note.id == priorityNoteId) 100 else 0) else null
                    }.sortedByDescending { it.second }.take(6).map { it.first }
                }
                Row(modifier = Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.SpaceBetween, verticalAlignment = Alignment.CenterVertically) {
                    TextButton(enabled = !locked, onClick = { requestAgentWorkspaceReset(false, "chat") })"####,
    )?;
    replace_once(&mut source,
        r####"listOf("写任务", "选资料", "核对", "结果")"####,
        r####"listOf("任务", "资料", "授权", "成果")"####,
    )?;
    replace_once(&mut source,
        "根据你选择的便签和知识页整理草稿，检查后再保存。",
        "描述目标，选好授权资料后就能开始；Agent 不会自行修改原文。",
    )?;
    replace_once(&mut source,
        r####"                            item {
                                Text("从一个常用任务开始", style = MaterialTheme.typography.labelLarge)"####,
        r####"                            if (selectedAgentDocuments.isNotEmpty()) item {
                                TextButton(enabled = canPrepare, onClick = { agentWorkspaceStep = 1 }) {
                                    Text("调整资料范围 · 已选 ${selectedAgentDocuments.size} 条")
                                }
                            }
                            item {
                                Text("从一个常用任务开始", style = MaterialTheme.typography.labelLarge)"####,
    )?;

    replace_once(&mut source,
        r####"                            if (visibleDocuments.isEmpty()) item { Text(if (agentSourceSearch.isBlank())"####,
        r####"                            if (suggestedDocuments.isNotEmpty()) item {
                                FlowusPanel(accent = notesAccentColor) {
                                    Column(Modifier.fillMaxWidth().padding(14.dp), verticalArrangement = Arrangement.spacedBy(6.dp)) {
                                        Text("与任务相关的资料 · ${suggestedDocuments.size} 条", fontWeight = FontWeight.SemiBold)
                                        Text("仅在本机按标题和文档库匹配；推荐不会自动授权。", style = MaterialTheme.typography.bodySmall)
                                        Text(suggestedDocuments.take(3).joinToString("、") { it.displayTitle() }, maxLines = 2, overflow = TextOverflow.Ellipsis, style = MaterialTheme.typography.bodySmall)
                                        TextButton(enabled = canPrepare && suggestedDocuments.any { it.id !in agentSelectedIds }, onClick = {
                                            stopAgent("资料选择已改变")
                                            val next = addKnowledgeAgentVisibleSources(agentSelectedIds.toSet(), suggestedDocuments.map(NoteEntry::id))
                                            agentSelectedIds.clear(); agentSelectedIds.addAll(next)
                                        }) { Text("加入推荐资料") }
                                    }
                                }
                            }
                            if (visibleDocuments.isEmpty()) item { Text(if (agentSourceSearch.isBlank())"####,
    )?;

    replace_once(&mut source,
        r####"                                        Text(document.content, style = MaterialTheme.typography.bodyMedium)"####,
        r####"                                        val expanded = document.id in agentExpandedPreviewIds
                                        TextButton(onClick = {
                                            if (expanded) agentExpandedPreviewIds.remove(document.id)
                                            else agentExpandedPreviewIds.add(document.id)
                                        }) { Text(if (expanded) "收起正文" else "查看完整正文") }
                                        if (expanded) Text(document.content, style = MaterialTheme.typography.bodyMedium)"####,
    )?;
    replace_once(&mut source,
        "下方是完整资料预览。模型按需检索和读取，每阶段读取累计上限 32,000 字符；每轮携带本阶段工具记录。请求设置 store:false，自定义服务的留存规则由服务商决定。",
        "下方逐条列出本次授权资料，可展开核对完整正文。模型按需读取，每阶段最多累计 32,000 字符；请求设置 store:false，自定义服务的留存规则由服务商决定。",
    )?;
    replace_once(&mut source,
        "我已核对目标、接收方和完整资料，授权本次发送。",
        "我已确认目标、接收方和资料范围，同意按需发送所选资料的正文。",
    )?;
    replace_once(&mut source,
        r####"                                    Text(agentProgress?.let { knowledgeAgentActivityLabel(it, agentCancelling) } ?: "正在准备本次任务…", style = MaterialTheme.typography.bodyMedium)"####,
        r####"                                    Text(agentProgress?.let { knowledgeAgentActivityLabel(it, agentCancelling) } ?: "正在准备本次任务…", style = MaterialTheme.typography.bodyMedium)
                                    Text("请保持应用在前台；切到后台将停止本次任务。", style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurface.copy(alpha = 0.7f))"####,
    )?;

    replace_once(&mut source,
        r####"                        PhysicalButton(label = "下一步：选择资料", icon = Icons.Rounded.Folder, accent = notesAccentColor, filled = true,"####,
        r####"                        PhysicalButton(label = if (selectedAgentDocuments.isNotEmpty()) "核对授权范围 · ${selectedAgentDocuments.size} 条" else "下一步：选资料", icon = Icons.Rounded.Folder, accent = notesAccentColor, filled = true,"####,
    )?;
    replace_once(&mut source,
        "onClick = { agentPreviewAcknowledged = false; showSendingContent = false; agentWorkspaceStep = 1 })",
        "onClick = { agentPreviewAcknowledged = false; showSendingContent = selectedAgentDocuments.isNotEmpty(); agentWorkspaceStep = if (selectedAgentDocuments.isNotEmpty()) 2 else 1 })",
    )?;

    compact_chat_surface(&mut source)?;
    Ok(source)
}

// Ordinary chat only. Run after the task workspace has separated its own UI.
// Keep the same request, history, save and consent callbacks.
fn compact_chat_surface(source: &mut String) -> Result<(), String> {
    // The compact composer introduces this extension. Own its import here,
    // instead of relying on a different source generator or another screen.
    const FOCUS_IMPORT: &str = "import androidx.compose.ui.focus.onFocusChanged";
    if !source.lines().any(|line| line.trim() == FOCUS_IMPORT) {
        replace_once(source, "package com.ofairyo.gridtimer.ui\n",
            "package com.ofairyo.gridtimer.ui\n\nimport androidx.compose.ui.focus.onFocusChanged\n")?;
    }
    replace_once(source, "    val messageListState = rememberLazyListState()",
        "    val messageListState = rememberLazyListState()\n    val chatHistoryListState = rememberLazyListState()\n    var showChatOptions by remember(activeChatId) { mutableStateOf(false) }")?;
    replace_once(source, "LaunchedEffect(conversation.size, busy) {",
        "LaunchedEffect(activeChatId, conversation.size, busy, showChatHistory) {")?;
    replace_once(source, "if (!agentMode && last >= 0) messageListState.animateScrollToItem(last)",
        "if (!agentMode && !showChatHistory && last >= 0) messageListState.scrollToItem(last)")?;
    replace_once(source, "Box(modifier = Modifier.fillMaxSize().padding(8.dp).imePadding().navigationBarsPadding()",
        "Box(modifier = Modifier.fillMaxSize().imePadding().navigationBarsPadding()")?;
    replace_once(source, "shape = RoundedCornerShape(24.dp), color = flowusPanelBackground(), border = BorderStroke(1.dp, flowusBorderColor()))",
        "shape = RoundedCornerShape(if (agentMode) 24.dp else 0.dp), color = flowusPanelBackground(), border = BorderStroke(if (agentMode) 1.dp else 0.dp, flowusBorderColor()))")?;
    replace_once(source, "Column(modifier = Modifier.fillMaxSize().padding(16.dp), verticalArrangement = Arrangement.spacedBy(10.dp))",
        "Column(modifier = Modifier.fillMaxSize().padding(horizontal = if (agentMode) 16.dp else 12.dp, vertical = 6.dp), verticalArrangement = Arrangement.spacedBy(4.dp))")?;
    replace_once(source, r####"Text(if (agentMode) "任务助手" else "AI 问答", style = MaterialTheme.typography.titleLarge, fontWeight = FontWeight.SemiBold)"####,
        r####"Text(if (agentMode) "任务助手" else "AI 问答", modifier = Modifier.weight(1f), maxLines = 1, overflow = TextOverflow.Ellipsis, style = MaterialTheme.typography.titleMedium, fontWeight = FontWeight.SemiBold)"####)?;
    replace_once(source, r####"onClick = { showChatHistory = !showChatHistory }) { Text("历史") }"####,
        r####"onClick = { showChatHistory = !showChatHistory; showChatOptions = false }) { Text(if (showChatHistory) "返回对话" else "历史") }"####)?;
    replace_once(source, r####"CapsuleAction(text = "关闭", icon = Icons.Rounded.Close, onClick = { requestAgentExit() })"####,
        r####"if (agentMode) CapsuleAction(text = "关闭", icon = Icons.Rounded.Close, onClick = { requestAgentExit() })
                            else androidx.compose.material3.IconButton(onClick = { requestAgentExit() }) {
                                androidx.compose.material3.Icon(Icons.Rounded.Close, contentDescription = "关闭")
                            }"####)?;
    replace_once(source, "            LazyColumn(state = messageListState, modifier = Modifier.weight(1f).fillMaxWidth(), verticalArrangement = Arrangement.spacedBy(12.dp)) {",
        r####"            if (!showChatHistory && conversation.isNotEmpty()) {
                Row(Modifier.fillMaxWidth(), verticalAlignment = Alignment.CenterVertically) {
                    TextButton(onClick = {
                        showChatOptions = !showChatOptions
                        if (showChatOptions) scope.launch { messageListState.scrollToItem(0) }
                    }) { Text(if (showChatOptions) "收起设置" else if (queryMode == KnowledgeAiMode.DIRECT) "直接问 AI · 设置" else "问知识库 · 设置") }
                    Spacer(Modifier.weight(1f))
                    TextButton(enabled = historyReady && historyError.isBlank() && pendingTurn == null &&
                        !savingConversation && !busy && !agentBusy, onClick = ::saveWholeConversation) {
                        Text(if (savingConversation) "正在保存…" else "保存 ${conversation.size} 轮")
                    }
                }
            }
            androidx.compose.material3.HorizontalDivider(color = MaterialTheme.colorScheme.outlineVariant)
            LazyColumn(state = if (showChatHistory) chatHistoryListState else messageListState,
                modifier = Modifier.weight(1f).fillMaxWidth(),
                verticalArrangement = Arrangement.spacedBy(12.dp),
                contentPadding = PaddingValues(horizontal = 4.dp, vertical = 8.dp)) {"####)?;
    replace_once(source, r####"                    if (chatSessions.isEmpty()) item { Text("暂无历史对话。") }
                }
                item {"####,
        r####"                    if (chatSessions.isEmpty()) item { Text("暂无历史对话。") }
                } else {
                if (conversation.isEmpty() || showChatOptions || showSendingContent || !configuration.testable) {
                item {"####)?;
    // Close the options region immediately before the actual busy/status items.
    replace_once(source, "                if (busy) item {",
        "                }\n                if (busy) item {")?;
    let from = source.find("                if (!agentMode && conversation.isNotEmpty()) item {")
        .ok_or("chat duplicate save block missing")?;
    let to = source[from..].find("                if (!agentMode) {\n                    items(conversation")
        .map(|i| from + i).ok_or("chat turn boundary missing")?;
    source.replace_range(from..to, r####"                if (showChatOptions && currentChat?.pageId != null) item {
                    TextButton(onClick = { currentChat?.pageId?.let(onOpenConversationPage) },
                        enabled = !savingConversation && pendingTurn == null && !busy) { Text("查看关联知识页") }
                }
"####);
    replace_once(source, "            }\n            }\n            if (!agentMode) {\n                if (busy)",
        "                }\n            }\n            }\n            if (!agentMode && !showChatHistory) {\n                androidx.compose.material3.HorizontalDivider(color = MaterialTheme.colorScheme.outlineVariant)\n                if (busy)")?;
    // A bounded input and compact attachment/send row replace the tall stack.
    let from = source.find("                    KnowledgeQuestionField(question = question,\n                        hint = if (conversation.isEmpty())")
        .ok_or("chat composer missing")?;
    let to = source[from..].find("                    if (imageModelUnsupported)")
        .map(|i| from + i).ok_or("chat composer image warning missing")?;
    source.replace_range(from..to, r####"                    KnowledgeQuestionField(question = question,
                        hint = if (conversation.isEmpty()) "输入问题" else "继续追问…",
                        onQuestionChange = { if (!busy) { question = it; statusText = "" } }, compact = true)
                    KnowledgeAiImagePicker(
                        selected = selectedImageUris,
                        enabled = !busy && !agentBusy,
                        onChange = { images ->
                            if (!busy && !agentBusy) {
                                invalidateAnswer()
                                selectedImageUris = images
                                if (question.isBlank() && images.isNotEmpty()) question = "请分析这些图片"
                            }
                        },
                        compact = true,
                        trailingAction = {
                            androidx.compose.material3.Button(
                                enabled = historyReady && historyError.isBlank() && pendingTurn == null &&
                                    !savingConversation && !busy && !agentBusy && configuration.testable && !imageModelUnsupported && question.isNotBlank() &&
                                    (queryMode == KnowledgeAiMode.DIRECT || sourceCandidates.isNotEmpty()),
                                onClick = ::requestAnswer
                            ) { Text(if (conversation.isEmpty()) "发送" else "继续提问") }
                        }
                    )
"####);
    let from = source.find("                PhysicalButton(label = if (busy) \"正在回答\" else if (conversation.isEmpty())")
        .ok_or("duplicate chat send missing")?;
    let to = source[from..].find("\n            }\n          }")
        .map(|i| from + i).ok_or("chat footer boundary missing")?;
    source.replace_range(from..to, r####"                if (showChatOptions || conversation.isEmpty()) {
                    Text("模型最多参考最近 8 轮、24,000 字符；本机保留全部文字问答。图片原件不自动归档，须手动再次引用。",
                        style = MaterialTheme.typography.labelSmall,
                        color = MaterialTheme.colorScheme.onSurface.copy(alpha = 0.70f))
                }"####);
    replace_once(source, "fieldLabel: String = \"消息\") {",
        "fieldLabel: String = \"消息\", compact: Boolean = false) {")?;
    replace_once(source, "    SmartisanTextField(value = question, onValueChange = onQuestionChange, label = fieldLabel,",
        r####"    if (compact) {
        OutlinedTextField(value = question,
            onValueChange = { if (it.length <= 1000) onQuestionChange(it) },
            placeholder = { Text(hint) },
            minLines = 1, maxLines = 3,
            shape = RoundedCornerShape(20.dp),
            interactionSource = interactions,
            modifier = Modifier.fillMaxWidth().focusRequester(focusRequester)
                .bringIntoViewRequester(bringIntoView)
                .onFocusChanged { focused ->
                    fieldFocused = focused.isFocused
                    if (fieldFocused) ticket = request.request() else request.cancel()
                })
        if (question.length >= 900) Text("${question.length}/1000", style = MaterialTheme.typography.labelSmall)
    } else SmartisanTextField(value = question, onValueChange = onQuestionChange, label = fieldLabel,"####)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strict_anchor_replacement_guards_against_duplicate_ui_actions() {
        let mut content = "close close".to_owned();
        assert!(replace_once(&mut content, "close", "confirm").is_err());
        assert!(replace_count(&mut content, "close", "confirm", 2).is_ok());
        assert_eq!(content, "confirm confirm");
    }

    #[test]
    fn does_not_touch_non_agent_android_sources() {
        let source = "original Kotlin output";
        assert_eq!(render("com/ofairyo/gridtimer/ui/OtherScreen.kt", source).unwrap(), source);
    }
}
