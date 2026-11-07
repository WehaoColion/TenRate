// v2.23.4.14 - Compact read-only startup shell without faux interactions.
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
    // Draw a lightweight read-only shell immediately while verified local state
    // is restored. No AppData or mutation-capable controls are composed here.
    androidx.activity.compose.ReportDrawnWhen { true }
    if (!initialized) {
        val startupSections = listOf("计时", "便签", "知识", "风控")
        SmartisanTheme(preference = ThemeMode.SYSTEM) {
            Surface(modifier = Modifier.fillMaxSize()) {
                Column(
                    modifier = Modifier.fillMaxSize().padding(horizontal = 20.dp, vertical = 22.dp),
                    verticalArrangement = Arrangement.spacedBy(22.dp)
                ) {
                    Column(verticalArrangement = Arrangement.spacedBy(4.dp)) {
                        Text(
                            "十倍率",
                            style = MaterialTheme.typography.titleSmall,
                            color = MaterialTheme.colorScheme.primary
                        )
                        Text("工作台", style = MaterialTheme.typography.headlineMedium)
                    }
                    // Labels preview the real navigation but deliberately have
                    // no click handlers or synthetic module state before restore.
                    Row(
                        modifier = Modifier.fillMaxWidth(),
                        horizontalArrangement = Arrangement.spacedBy(4.dp)
                    ) {
                        startupSections.forEachIndexed { index, title ->
                            Column(
                                modifier = Modifier.weight(1f),
                                horizontalAlignment = Alignment.CenterHorizontally,
                                verticalArrangement = Arrangement.spacedBy(8.dp)
                            ) {
                                Text(
                                    title,
                                    style = MaterialTheme.typography.bodyMedium,
                                    color = if (index == 0) MaterialTheme.colorScheme.primary
                                        else MaterialTheme.colorScheme.onSurfaceVariant
                                )
                                if (index == 0) {
                                    Surface(
                                        modifier = Modifier.width(24.dp).height(3.dp),
                                        shape = RoundedCornerShape(3.dp),
                                        color = MaterialTheme.colorScheme.primary
                                    ) {}
                                } else {
                                    Spacer(modifier = Modifier.height(3.dp))
                                }
                            }
                        }
                    }
                    // Content-sized status only; no full-height loading card or
                    // false progress percentage. Completion replaces this shell.
                    Surface(
                        modifier = Modifier.fillMaxWidth(),
                        shape = RoundedCornerShape(16.dp),
                        color = MaterialTheme.colorScheme.surfaceVariant.copy(alpha = 0.38f)
                    ) {
                        Column(
                            modifier = Modifier.fillMaxWidth().padding(18.dp),
                            verticalArrangement = Arrangement.spacedBy(10.dp)
                        ) {
                            Row(
                                horizontalArrangement = Arrangement.spacedBy(12.dp),
                                verticalAlignment = Alignment.CenterVertically
                            ) {
                                androidx.compose.material3.CircularProgressIndicator(
                                    modifier = Modifier.size(18.dp),
                                    strokeWidth = 2.dp,
                                    color = MaterialTheme.colorScheme.primary
                                )
                                Text(
                                    "正在恢复本机数据",
                                    style = MaterialTheme.typography.titleSmall
                                )
                            }
                            Text(
                                "数据准备完成后自动进入工作台",
                                style = MaterialTheme.typography.bodySmall,
                                color = MaterialTheme.colorScheme.onSurfaceVariant
                            )
                        }
                    }
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
        assert!(screen.contains("CircularProgressIndicator("));
        assert!(!screen.contains("LinearProgressIndicator("));
        assert!(screen.contains("listOf(\"计时\", \"便签\", \"知识\", \"风控\")"));
        assert!(!screen.contains("记账"));
        assert!(!screen.contains("TextButton("));
        assert!(!screen.contains("startupSection = remember"));
        assert!(!screen.contains("androidx.compose.material3.Card("));
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
    fn startup_shell_has_only_content_sized_loading_and_static_navigation() {
        let screen = render(SCREEN, &old_screen()).unwrap();
        let loading = screen.split("    if (!initialized) {").nth(1).unwrap()
            .split("        return\n    }").next().unwrap();
        assert!(loading.contains("Modifier.fillMaxWidth().padding(18.dp)"));
        assert!(loading.contains("Modifier.size(18.dp)"));
        assert!(loading.contains("正在恢复本机数据"));
        assert!(!loading.contains("Modifier.fillMaxWidth().weight(1f)"));
        assert!(!loading.contains("onClick ="));
        assert!(!loading.contains("viewModel."));
        assert!(!loading.contains("AppData.default()"));
    }

    #[test]
    fn changed_gate_fails_closed_and_unrelated_sources_remain_exact() {
        assert!(render(SCREEN, "unexpected screen").is_err());
        assert_eq!(render("other.kt", "unaltered").unwrap(), "unaltered");
    }
}
