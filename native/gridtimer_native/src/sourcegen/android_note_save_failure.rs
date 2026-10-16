// v2.23.2.36 - Retain the exact failure for the document save that owns the exit.
const DOCUMENT: &str = "com/ofairyo/gridtimer/ui/NoteDocumentEditor.kt";

pub fn render(path: &str, base: &str) -> Result<String, String> {
    if path != DOCUMENT {
        return Ok(base.to_owned());
    }
    let mut source = base.to_owned();
    for (old, new) in [
        (
            "    val exitSaveGate = NoteEditorExitSaveGate<NoteEntry>()",
            "    val exitSaveGate = NoteEditorExitSaveGate<NoteEntry>()\n    val lastSaveFailure = mutableStateOf<com.ofairyo.gridtimer.data.NoteSaveFailure?>(null)",
        ),
        (
            "        val completePersistence: (Boolean) -> Unit = { saved ->",
            "        var persistenceFailure: com.ofairyo.gridtimer.data.NoteSaveFailure? = null\n        val completePersistence: (Boolean) -> Unit = { saved ->",
        ),
        (
            "            if (completion.accepted && completion.latest && persistenceGeneration == saveRequestGeneration) {",
            "            if (completion.accepted && completion.latest && persistenceGeneration == saveRequestGeneration) {\n                editorSession.lastSaveFailure.value = persistenceFailure",
        ),
        (
            "        val requestedExitAction = resolveSmartisanNoteExitAction(",
            "        val completeNotePersistence: (com.ofairyo.gridtimer.data.NoteSaveResult) -> Unit = { result ->\n            persistenceFailure = result.failure\n            if (!result.committed) {\n                android.util.Log.w(\"NoteSave\", \"document_exit failure=${result.failure?.name ?: \"UNKNOWN\"}\")\n            }\n            completePersistence(result.committed)\n        }\n        val requestedExitAction = resolveSmartisanNoteExitAction(",
        ),
        (
            "SmartisanNoteExitAction.SaveAndFlush -> viewModel.upsertNoteAndFlush(",
            "SmartisanNoteExitAction.SaveAndFlush -> viewModel.upsertNoteAndFlushResult(",
        ),
        (
            "                reason = reason,\n                onComplete = completePersistence\n            )\n            SmartisanNoteExitAction.DeleteBlankDraft",
            "                reason = reason,\n                onComplete = completeNotePersistence\n            )\n            SmartisanNoteExitAction.DeleteBlankDraft",
        ),
        (
            "localizedToast(context, \"保存失败，笔记仍留在编辑页。\", Toast.LENGTH_LONG).show()",
            "localizedToast(context, documentSaveFailureMessage(editorSession.lastSaveFailure.value), Toast.LENGTH_LONG).show()",
        ),
    ] {
        if source.matches(old).count() != 1 {
            return Err(format!("document save failure anchor is not unique: {old}"));
        }
        source = source.replacen(old, new, 1);
    }
    source.push_str(FAILURE_MESSAGE);
    Ok(source)
}

const FAILURE_MESSAGE: &str = r####"

private fun documentSaveFailureMessage(failure: com.ofairyo.gridtimer.data.NoteSaveFailure?): String {
    val reason = when (failure) {
        com.ofairyo.gridtimer.data.NoteSaveFailure.STORAGE_FULL -> "手机存储空间不足，请腾出空间后重试"
        com.ofairyo.gridtimer.data.NoteSaveFailure.DATABASE_WRITE_FAILED -> "本机数据库写入失败，请重试并导出诊断日志"
        com.ofairyo.gridtimer.data.NoteSaveFailure.SCHEMA_MISMATCH -> "资料格式高于当前版本，请更新应用后重试"
        com.ofairyo.gridtimer.data.NoteSaveFailure.READ_ONLY_PROTECTION -> "本机资料处于恢复保护状态，请导出诊断日志"
        com.ofairyo.gridtimer.data.NoteSaveFailure.PROTECTION_STATE_CONFLICT -> "页面保护状态已变化，请保留草稿并重新核对"
        com.ofairyo.gridtimer.data.NoteSaveFailure.SERIALIZATION_FAILED,
        com.ofairyo.gridtimer.data.NoteSaveFailure.VERSION_REPAIR_FAILED -> "页面内容校验未通过，请导出诊断日志"
        com.ofairyo.gridtimer.data.NoteSaveFailure.WORKSPACE_CHANGED -> "工作区已切换，本次未保存"
        com.ofairyo.gridtimer.data.NoteSaveFailure.QUEUE_UNAVAILABLE -> "保存服务已关闭，请保留草稿后重试"
        com.ofairyo.gridtimer.data.NoteSaveFailure.FLUSH_FAILED -> "本机文件写入失败，请检查存储空间后重试"
        null -> "保存失败，请重试"
    }
    return "$reason；内容仍保留在编辑页。"
}
"####;
