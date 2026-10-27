// v2.23.2.57 - Make first UI frame independent from durable workspace hydration.
//
// The screen still returns before collecting default AppData. Timer effects,
// actions and persistence must not observe the placeholder model.
const SCREEN: &str = "com/ofairyo/gridtimer/ui/GridTimerScreen.kt";
const START: &str = "    val initialized by viewModel.isInitialized.collectAsState()\n    androidx.activity.compose.ReportDrawnWhen { initialized }\n    if (!initialized) {";
const END: &str = "    val appData by viewModel.appData.collectAsState()";

pub fn render(path: &str, base: &str) -> Result<String, String> {
    if path != SCREEN {
        return Ok(base.to_owned());
    }
    if base.matches(START).count() != 1 || base.matches(END).count() != 1 {
        return Err("startup workspace shell requires the verified initialization gate".into());
    }
    let start = base.find(START).unwrap();
    let end = base.find(END).unwrap();
    if end <= start {
        return Err("startup workspace gate must precede AppData consumption".into());
    }
    let old_gate = &base[start..end];
    if !old_gate.contains("        return\n    }") ||
        !old_gate.contains("SmartisanTheme(preference = ThemeMode.SYSTEM)") {
        return Err("startup workspace shell cannot safely replace unknown gate".into());
    }
    Ok(format!("{}{}{}", &base[..start], SHELL, &base[end..]))
}

const SHELL: &str = r####"    val initialized by viewModel.isInitialized.collectAsState()
    // The first frame is the workbench itself; durable initialization remains
    // separately observable in StartupLatency ready/loadingGate.
    androidx.activity.compose.ReportDrawnWhen { true }
    if (!initialized) {
        // This is a UI-only staging surface. Never read AppData.default(), start
        // timers, send notifications or enqueue mutations before verified restore.
        val startupSection = remember { mutableStateOf(0) }
        val startupSections = listOf("计时", "便签", "知识", "记账")
        SmartisanTheme(preference = ThemeMode.SYSTEM) {
            Surface(modifier = Modifier.fillMaxSize()) {
                Column(
                    modifier = Modifier.fillMaxSize().padding(16.dp),
                    verticalArrangement = Arrangement.spacedBy(14.dp)
                ) {
                    Text("十倍率", style = MaterialTheme.typography.headlineMedium)
                    Text("工作台", style = MaterialTheme.typography.titleLarge)
                    Row(
                        modifier = Modifier.fillMaxWidth(),
                        horizontalArrangement = Arrangement.spacedBy(4.dp)
                    ) {
                        startupSections.forEachIndexed { index, title ->
                            androidx.compose.material3.TextButton(
                                modifier = Modifier.weight(1f),
                                onClick = { startupSection.value = index }
                            ) {
                                Text(
                                    title,
                                    color = if (startupSection.value == index)
                                        MaterialTheme.colorScheme.primary
                                    else MaterialTheme.colorScheme.onSurfaceVariant
                                )
                            }
                        }
                    }
                    androidx.compose.material3.Card(
                        modifier = Modifier.fillMaxWidth().weight(1f)
                    ) {
                        Column(
                            modifier = Modifier.fillMaxWidth().padding(20.dp),
                            verticalArrangement = Arrangement.spacedBy(16.dp)
                        ) {
                            Text(
                                startupSections[startupSection.value],
                                style = MaterialTheme.typography.titleLarge
                            )
                            Text(
                                "正在恢复本机内容。可以先进入工作台，数据验证完成后自动显示完整页面。",
                                style = MaterialTheme.typography.bodyMedium
                            )
                            androidx.compose.material3.LinearProgressIndicator(
                                modifier = Modifier.fillMaxWidth()
                            )
                            Text(
                                "后台读取和恢复不会因为进入此界面而暂停。",
                                style = MaterialTheme.typography.bodySmall
                            )
                        }
                    }
                    Text(
                        "为保护现有记录，涉及数据的操作将在读取完成后开放。",
                        style = MaterialTheme.typography.bodySmall
                    )
                }
            }
        }
        return
    }
"####;

#[cfg(test)]
mod tests {
    use super::*;

    fn old_screen() -> String {
        format!(
            "before\n{}\n        SmartisanTheme(preference = ThemeMode.SYSTEM) {{ loader() }}\n        return\n    }}\n{}\n    TimerNotificationEffects(slots = appData.slots)",
            START, END
        )
    }

    #[test]
    fn first_frame_is_not_gated_by_repository_restore() {
        let screen = render(SCREEN, &old_screen()).unwrap();
        assert!(screen.contains("ReportDrawnWhen { true }"));
        assert!(!screen.contains("ReportDrawnWhen { initialized }"));
        assert!(screen.contains("startupSections.forEachIndexed"));
        assert!(screen.contains("LinearProgressIndicator("));
        assert!(!screen.contains("CircularProgressIndicator()"));
    }

    #[test]
    fn no_default_model_or_mutating_effects_run_before_restore() {
        let screen = render(SCREEN, &old_screen()).unwrap();
        let guard = screen.find("    if (!initialized) {").unwrap();
        let early_exit = screen[guard..].find("        return\n    }").unwrap() + guard;
        let app_data = screen.find(END).unwrap();
        let notifications = screen.find("TimerNotificationEffects(slots = appData.slots)").unwrap();
        assert!(guard < early_exit && early_exit < app_data && app_data < notifications);
        assert!(!screen[guard..early_exit].contains("viewModel.appData"));
        assert!(!screen[guard..early_exit].contains("viewModel.start"));
        assert!(!screen[guard..early_exit].contains("viewModel.save"));
    }

    #[test]
    fn changed_gate_fails_closed_and_unrelated_sources_remain_exact() {
        assert!(render(SCREEN, "unexpected screen").is_err());
        assert_eq!(render("other.kt", "unaltered").unwrap(), "unaltered");
    }
}
