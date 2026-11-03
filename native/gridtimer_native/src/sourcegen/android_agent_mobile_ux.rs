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

    // Opening Agent from a knowledge page suggests that exact page as the initial
    // selection. It remains a local selection until the normal explicit send gate.
    // Preserve the original saveable keys to avoid resetting selections unexpectedly.
    replace_once(&mut source,
        "    val agentSelectedIds = rememberSaveable(workspaceKey, identity, selectedFolderId) { mutableStateListOf<String>() }",
        "    val agentSelectedIds = rememberSaveable(workspaceKey, identity, selectedFolderId) { mutableStateListOf<String>().apply { priorityNoteId?.let { add(it) } } }",
    )?;

    replace_once(&mut source,
        "    var agentResetConfirmation by remember(agentBoundary) { mutableStateOf(false) }",
        "    var agentResetConfirmation by remember(agentBoundary) { mutableStateOf(false) }\n    var agentExitConfirmation by remember(agentBoundary) { mutableStateOf(false) }\n    val agentExpandedPreviewIds = remember(agentBoundary) { mutableStateListOf<String>() }",
    )?;

    replace_once(&mut source,
        "    LaunchedEffect(agentMode) { if (agentMode && !agentBusy && !agentSaving) agentWorkspaceStep = 0 }",
        r####"    fun requestAgentExit() {
        if (agentSaving) return
        if (agentMode && agentBusy) agentExitConfirmation = true
        else if (agentMode && !busy) requestAgentWorkspaceReset(false, "close")
        else onDismiss()
    }
    LaunchedEffect(agentMode) { if (agentMode && !agentBusy && !agentSaving) agentWorkspaceStep = 0 }"####,
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

    Ok(source)
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
