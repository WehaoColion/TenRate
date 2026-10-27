// v2.23.2.58 - Admit timer controls once verified startup data is durably restored.
const REPOSITORY: &str = "com/ofairyo/gridtimer/data/TimerRepository.kt";
const VIEW_MODEL: &str = "com/ofairyo/gridtimer/ui/TimerViewModel.kt";
const SCREEN: &str = "com/ofairyo/gridtimer/ui/GridTimerScreen.kt";

fn replace(source: &mut String, old: &str, new: &str) -> Result<(), String> {
    if source.matches(old).count() != 1 {
        return Err(format!("progressive startup anchor is not unique: {old}"));
    }
    *source = source.replacen(old, new, 1);
    Ok(())
}
pub fn render(path: &str, base: &str) -> Result<String, String> {
    let mut source = base.to_owned();
    match path {
        REPOSITORY => {
            replace(&mut source,
                "    val isInitialized: StateFlow<Boolean> = _isInitialized.asStateFlow()",
                r####"    val isInitialized: StateFlow<Boolean> = _isInitialized.asStateFlow()
    private val _timerStartupReady = MutableStateFlow(false)
    val timerStartupReady: StateFlow<Boolean> = _timerStartupReady.asStateFlow()"####)?;
            replace(&mut source,
                r####"            if (!readOnlyFuture && !loadedFromRecoveryCandidate) {
                durableForMediaPrune = flushPersistLocked(resolved)
            }"####,
                r####"            if (!readOnlyFuture && !loadedFromRecoveryCandidate) {
                durableForMediaPrune = flushPersistLocked(resolved)
            }
            // The account-bound snapshot, note draft/media repairs and initial
            // durable commit must complete before timer controls are exposed.
            // The writer mutex stays held until this block exits.
            if (durableForMediaPrune && futureSchemaVersion == null &&
                persistenceWriteBlockedReason == null) {
                _timerStartupReady.value = true
                android.util.Log.i("StartupLatency", "timerReady after durable hydrate")
            }"####)?;
            for old in [
                "    suspend fun startSlot(slotId: Int) {\n        awaitInitialized()",
                "    suspend fun pauseSlots(slotIds: Collection<Int>) {\n        awaitInitialized()",
                "    suspend fun resetSlot(slotId: Int) {\n        awaitInitialized()",
            ] {
                replace(&mut source, old, &old.replace(
                    "        awaitInitialized()",
                    "        if (!_timerStartupReady.value) awaitInitialized()",
                ))?;
            }
        }
        VIEW_MODEL => {
            replace(&mut source,
                "    val isInitialized = repository.isInitialized",
                "    val isInitialized = repository.isInitialized\n    val timerStartupReady = repository.timerStartupReady")?;
        }
        SCREEN => {
            replace(&mut source,
                "        val startupSection = remember { mutableStateOf(0) }",
                "        val startupTimerReady by viewModel.timerStartupReady.collectAsState()\n        val startupSection = remember { mutableStateOf(0) }")?;
            replace(&mut source,
                r####"                            Text(
                                "正在恢复本机内容。可以先进入工作台，数据验证完成后自动显示完整页面。",
                                style = MaterialTheme.typography.bodyMedium
                            )
                            androidx.compose.material3.LinearProgressIndicator(
                                modifier = Modifier.fillMaxWidth()
                            )
                            Text(
                                "后台读取和恢复不会因为进入此界面而暂停。",
                                style = MaterialTheme.typography.bodySmall
                            )"####,
                r####"                            if (startupSection.value == 0 && startupTimerReady) {
                                // No provisional/default AppData is displayed or acted on:
                                // this branch exists only after durable timer readiness.
                                val startupTimerData by viewModel.appData.collectAsState()
                                Text(
                                    "计时数据已安全恢复，可以立即开始或暂停。其他模块继续准备中。",
                                    style = MaterialTheme.typography.bodyMedium
                                )
                                if (startupTimerData.slots.isEmpty()) {
                                    Text("当前没有计时格子", style = MaterialTheme.typography.bodySmall)
                                } else {
                                    LazyColumn(
                                        modifier = Modifier.fillMaxWidth().heightIn(max = 440.dp),
                                        verticalArrangement = Arrangement.spacedBy(6.dp)
                                    ) {
                                        items(startupTimerData.slots, key = { slot -> slot.id }) { slot ->
                                            Row(
                                                modifier = Modifier.fillMaxWidth(),
                                                horizontalArrangement = Arrangement.spacedBy(8.dp),
                                                verticalAlignment = Alignment.CenterVertically
                                            ) {
                                                Column(modifier = Modifier.weight(1f)) {
                                                    Text(
                                                        slot.title.ifBlank { "计时格子" },
                                                        style = MaterialTheme.typography.bodyMedium
                                                    )
                                                    Text(
                                                        if (slot.runningSinceEpochMillis != null) "运行中" else "已暂停",
                                                        style = MaterialTheme.typography.bodySmall
                                                    )
                                                }
                                                androidx.compose.material3.TextButton(
                                                    onClick = {
                                                        viewModel.toggleSlotRunning(
                                                            slot.id,
                                                            slot.runningSinceEpochMillis != null
                                                        )
                                                    }
                                                ) {
                                                    Text(if (slot.runningSinceEpochMillis != null) "暂停" else "开始")
                                                }
                                            }
                                        }
                                    }
                                }
                            } else {
                                Text(
                                    if (startupSection.value == 0)
                                        "正在恢复计时数据，完成安全落盘后即可使用。"
                                    else
                                        "模块正在准备中，完整数据和编辑功能将在安全恢复后开放。",
                                    style = MaterialTheme.typography.bodyMedium
                                )
                                androidx.compose.material3.LinearProgressIndicator(
                                    modifier = Modifier.fillMaxWidth()
                                )
                                Text(
                                    "后台的数据校验、草稿修复和历史维护持续进行。",
                                    style = MaterialTheme.typography.bodySmall
                                )
                            }"####)?;
            replace(&mut source,
                "为保护现有记录，涉及数据的操作将在读取完成后开放。",
                "已恢复的计时可操作；其余数据操作在验证完成后开放。")?;
        }
        _ => {}
    }
    Ok(source)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn missing_anchors_fail_closed() {
        assert_eq!(render("other.kt", "unchanged").unwrap(), "unchanged");
        for path in [REPOSITORY, VIEW_MODEL, SCREEN] {
            assert!(render(path, "unrecognized source").is_err());
        }
    }
    #[test]
    fn ready_needs_authenticated_durable_publication() {
        let mut repo = String::from("    val isInitialized: StateFlow<Boolean> = _isInitialized.asStateFlow()\n");
        repo.push_str(r####"            if (!readOnlyFuture && !loadedFromRecoveryCandidate) {
                durableForMediaPrune = flushPersistLocked(resolved)
            }"####);
        for action in [
            "    suspend fun startSlot(slotId: Int) {\n        awaitInitialized()",
            "    suspend fun pauseSlots(slotIds: Collection<Int>) {\n        awaitInitialized()",
            "    suspend fun resetSlot(slotId: Int) {\n        awaitInitialized()",
        ] {
            repo.push('\n');
            repo.push_str(action);
        }
        let source = render(REPOSITORY, &repo).unwrap();
        assert!(source.find("flushPersistLocked(resolved)").unwrap() <
            source.find("_timerStartupReady.value = true").unwrap());
        assert!(source.contains("futureSchemaVersion == null"));
        assert!(source.contains("persistenceWriteBlockedReason == null"));
        assert_eq!(source.matches("if (!_timerStartupReady.value) awaitInitialized()").count(), 3);
        assert!(render(REPOSITORY, &repo.replace(
            "flushPersistLocked(resolved)", "false")).is_err());
    }
    #[test]
    fn viewmodel_exposes_only_repository_readiness() {
        let source = render(VIEW_MODEL,
            "    val isInitialized = repository.isInitialized").unwrap();
        assert!(source.contains("val timerStartupReady = repository.timerStartupReady"));
    }
}
