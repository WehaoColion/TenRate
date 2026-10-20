//! Keep everyday bookkeeping in front of analysis. Run after the finance workspace.
const GRID: &str = "com/ofairyo/gridtimer/ui/GridTimerScreen.kt";
const LEDGER: &str = "com/ofairyo/gridtimer/ui/FinanceLedgerSection.kt";
pub const TEST_PATH: &str = "com/ofairyo/gridtimer/ui/FinanceQuickEntryTest.kt";

fn index(source: &str, anchor: &str) -> Result<usize, String> {
    if source.matches(anchor).count() != 1 {
        return Err(format!(
            "finance navigation anchor missing or duplicated: {anchor}"
        ));
    }
    Ok(source.find(anchor).unwrap())
}

fn replace(source: &mut String, before: &str, after: &str) -> Result<(), String> {
    index(source, before)?;
    *source = source.replacen(before, after, 1);
    Ok(())
}

fn item_body(item: &str) -> Result<&str, String> {
    item.trim_end()
        .strip_prefix("            item {\n")
        .and_then(|s| s.strip_suffix("            }"))
        .ok_or_else(|| "finance navigation item boundary changed".into())
}

pub fn render(path: &str, source: &str) -> Result<String, String> {
    if path != GRID && path != LEDGER {
        return Ok(source.to_owned());
    }
    let mut output = source.to_owned();
    if path == GRID {
        if !output.contains("import androidx.compose.material.icons.rounded.Remove\n") {
            output = output.replacen("package com.ofairyo.gridtimer.ui\n", "package com.ofairyo.gridtimer.ui\nimport androidx.compose.material.icons.rounded.Remove\n", 1);
        }
        let start = index(
            &output,
            "    val financeContent: @Composable () -> Unit = {",
        )?;
        let end = output[start..]
            .find("\n    if (showLegalRisk) {")
            .map(|n| start + n)
            .ok_or("finance content end missing")?;
        let content = &output[start..end];
        let positions: Vec<_> = content
            .match_indices("            item {\n")
            .map(|(n, _)| n)
            .collect();
        if positions.len() != 7 {
            return Err("finance navigation expects seven original sections".into());
        }
        let bodies = positions
            .windows(2)
            .map(|p| item_body(&content[p[0]..p[1]]))
            .collect::<Result<Vec<_>, _>>()?;
        let legal_start = index(
            bodies[1],
            "                Surface(shape = RoundedCornerShape(20.dp)",
        )?;
        let period_start = index(bodies[1], "                Text(\"账目浏览周期\"")?;
        let legal = &bodies[1][legal_start..period_start];
        let period = bodies[1][period_start..].replace("                Text(\"账目浏览周期\", style = MaterialTheme.typography.labelMedium)\n", "")
            .replace("FinanceReportPeriodCard(", "FinanceLedgerPeriodSelector(");
        // Move whole original bodies. Review writes, backup/restore and plan callbacks stay intact.
        let new_content = HOME
            .replace("__PERIOD__", &period)
            .replace("__LEDGER__", bodies[3])
            .replace("__RISK__", bodies[2])
            .replace("__LEGAL__", legal)
            .replace("__BACKUP__", bodies[4])
            .replace("__PLAN__", bodies[5]);
        output.replace_range(start..end, &new_content);
        replace(&mut output, "                                    restoreFinanceLauncher.launch(arrayOf(\"application/json\", \"text/plain\"))",
            "                                    showFinanceTools = false\n                                    restoreFinanceLauncher.launch(arrayOf(\"application/json\", \"text/plain\"))")?;
        replace(
            &mut output,
            "    val financeListState = rememberLazyListState()",
            STATE,
        )?;
        // Legal evidence and legacy risk actions now land on the ledger, index 1.
        let start = index(&output, "private fun FinanceSheet(")?;
        let end = output[start..]
            .find("\n@Composable\nprivate fun FinanceAlertCard(")
            .map(|n| start + n)
            .ok_or("finance sheet end missing")?;
        let sheet = output[start..end].replace(
            "financeListState.animateScrollToItem(3)",
            "financeListState.animateScrollToItem(1)",
        );
        output.replace_range(start..end, &sheet);
        replace(&mut output, "    val periodSelection = rememberFinancePeriodSelectionState()",
            "    val periodSelection = androidx.compose.runtime.key(workspaceKey) { rememberFinancePeriodSelectionState() }")?;
        output = output.replace(
            "onClick = { showLegalRisk = true }",
            "onClick = { showFinanceRisk = false; showLegalRisk = true }",
        );
    } else {
        replace(
            &mut output,
            "private fun FinanceLedgerPeriodSelector(",
            "internal fun FinanceLedgerPeriodSelector(",
        )?;
        replace(
            &mut output,
            "    var selectedPeriod by mutableStateOf(FinanceReportPeriod.MONTH)",
            "    var selectedPeriod by mutableStateOf(FinanceReportPeriod.DAY)",
        )?;
        let start = index(&output, "            Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {\n                Text(\n                    text = TITLE_LEDGER,")?;
        let end = output[start..]
            .find("            when (selectedPeriod) {")
            .map(|n| start + n)
            .ok_or("ledger header end missing")?;
        output.replace_range(start..end, "            Text(\"收支与快照\", style = MaterialTheme.typography.titleMedium, fontWeight = FontWeight.SemiBold)\n\n");
        let start = index(
            &output,
            "        FinanceInsightBanner(title = \"\\u4eca\\u65e5\\u7ed3\\u8bba\"",
        )?;
        let end = output[start..]
            .find("        FinanceEditorSectionCard(")
            .map(|n| start + n)
            .ok_or("daily statistics end missing")?;
        let stats = output[start..end].to_owned();
        let summary = format!("{DAILY_SUMMARY}\n        androidx.compose.animation.AnimatedVisibility(visible = statisticsExpanded) {{\n            Column(verticalArrangement = Arrangement.spacedBy(14.dp)) {{\n{stats}\n            }}\n        }}\n\n");
        output.replace_range(start..end, &summary);
        replace(&mut output, "            FinanceCategorySettingsCard(\n", "            var showCategorySettings by remember { mutableStateOf(false) }\n            TextButton(onClick = { showCategorySettings = !showCategorySettings }) {\n                Text(if (showCategorySettings) \"收起分类与目标\" else \"分类与目标设置\")\n            }\n            androidx.compose.animation.AnimatedVisibility(visible = showCategorySettings) {\n            FinanceCategorySettingsCard(\n")?;
        replace(&mut output, "                onProfileChange = onProfileChange\n            )\n        }\n    }\n}\n\nprivate data class FinanceMetricItem(",
            "                onProfileChange = onProfileChange\n            )\n            }\n        }\n    }\n}\n\nprivate data class FinanceMetricItem(")?;
        output.push_str(QUICK_ENTRY);
    }
    for name in [
        "fillMaxSize",
        "heightIn",
        "imePadding",
        "windowInsetsPadding",
        "WindowInsets",
        "safeDrawing",
    ] {
        let import = format!("import androidx.compose.foundation.layout.{name}\n");
        if !output.contains(&import) {
            output = output.replacen(
                "package com.ofairyo.gridtimer.ui\n",
                &format!("package com.ofairyo.gridtimer.ui\n{import}"),
                1,
            );
        }
    }
    if path == LEDGER {
        for import in [
            "androidx.compose.foundation.rememberScrollState",
            "androidx.compose.foundation.verticalScroll",
            "androidx.compose.ui.platform.testTag",
        ] {
            let line = format!("import {import}\n");
            if !output.contains(&line) {
                output = output.replacen(
                    "package com.ofairyo.gridtimer.ui\n",
                    &format!("package com.ofairyo.gridtimer.ui\n{line}"),
                    1,
                );
            }
        }
    }
    Ok(output)
}

const STATE: &str = r####"    val financeListState = androidx.compose.runtime.key(workspaceKey) { rememberLazyListState() }
    var showFinanceRisk by remember(workspaceKey) { mutableStateOf(false) }
    var showFinanceTools by remember(workspaceKey) { mutableStateOf(false) }
    var quickEntryExpense by remember(workspaceKey) { mutableStateOf<Boolean?>(null) }
    var quickEntryDay by remember(workspaceKey) { mutableStateOf(LocalDate.now().toString()) }
    fun openQuickEntry(expense: Boolean) {
        quickEntryDay = LocalDate.now().toString()
        quickEntryExpense = expense
    }"####;

const HOME: &str = r####"    val financeContent: @Composable () -> Unit = {
        Column(modifier = Modifier.fillMaxSize()
            .windowInsetsPadding(WindowInsets.statusBars.only(WindowInsetsSides.Top))) {
            Surface(color = MaterialTheme.colorScheme.background) {
                Column(Modifier.fillMaxWidth().padding(horizontal = 16.dp, vertical = 8.dp),
                    verticalArrangement = Arrangement.spacedBy(8.dp)) {
                    Row(Modifier.fillMaxWidth(), verticalAlignment = Alignment.CenterVertically) {
                        Column(Modifier.weight(1f)) {
                            Text("风控", style = MaterialTheme.typography.headlineSmall, fontWeight = FontWeight.Bold)
            Text("快捷记入今天", style = MaterialTheme.typography.bodySmall,
                                color = MaterialTheme.colorScheme.onSurfaceVariant)
                        }
                        TextButton(onClick = { showFinanceRisk = true }, modifier = Modifier.testTag("finance_open_risk")) { Text("风险分析") }
                        TextButton(onClick = { showFinanceTools = true }, modifier = Modifier.testTag("finance_open_tools")) { Text("更多") }
                    }
                    Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(12.dp)) {
                        PhysicalButton(label = "记支出", icon = Icons.Rounded.Remove, accent = financeAccent,
                            filled = true, modifier = Modifier.weight(1f).heightIn(min = 48.dp).testTag("finance_quick_expense"),
                            onClick = { openQuickEntry(true) })
                        PhysicalButton(label = "记收入", icon = Icons.Rounded.Add, accent = financeAccent,
                            filled = false, modifier = Modifier.weight(1f).heightIn(min = 48.dp).testTag("finance_quick_income"),
                            onClick = { openQuickEntry(false) })
                    }
                }
            }
            LazyColumn(state = financeListState, modifier = Modifier.weight(1f).fillMaxWidth().testTag("finance_sheet"),
                contentPadding = PaddingValues(start = 16.dp, end = 16.dp, top = 6.dp,
                    bottom = if (asPage) bottomContentPadding else 28.dp),
                verticalArrangement = Arrangement.spacedBy(12.dp)) {
                item(key = "finance_period") {
__PERIOD__
                }
                item(key = "finance_ledger") {
__LEDGER__
                }
            }
        }
        if (showFinanceRisk) FinanceNavigationDialog("风险分析", onDismiss = { showFinanceRisk = false }) {
__RISK__
__LEGAL__
        }
        if (showFinanceTools) FinanceNavigationDialog("更多", onDismiss = { showFinanceTools = false }) {
__BACKUP__
__PLAN__
        }
        quickEntryExpense?.let { expense ->
            androidx.compose.runtime.key(workspaceKey, quickEntryDay, expense) {
                FinanceQuickEntryEditor(profile = workingProfile, dateKey = quickEntryDay,
                    expense = expense, accent = financeAccent,
                    onProfileChange = { updated -> commitProfileChange(updated) },
                    onDismiss = { quickEntryExpense = null })
            }
        }
    }
"####;

const DAILY_SUMMARY: &str = r####"        var statisticsExpanded by remember(selectedDayKey) { mutableStateOf(false) }
        Column(verticalArrangement = Arrangement.spacedBy(4.dp)) {
            Text("收入 ${formatFinanceCurrency(aggregate.incomeTotal)} · 支出 ${formatFinanceCurrency(aggregate.expenseTotal)}",
                style = MaterialTheme.typography.titleSmall)
            TextButton(onClick = { statisticsExpanded = !statisticsExpanded }) {
                Text(if (statisticsExpanded) "收起当日统计" else "查看当日统计与预算")
            }
        }"####;

const QUICK_ENTRY: &str = r####"

@OptIn(ExperimentalLayoutApi::class)
@Composable
internal fun FinanceNavigationDialog(title: String, onDismiss: () -> Unit, content: @Composable () -> Unit) {
    androidx.compose.ui.window.Dialog(onDismissRequest = onDismiss,
        properties = androidx.compose.ui.window.DialogProperties(usePlatformDefaultWidth = false,
            securePolicy = androidx.compose.ui.window.SecureFlagPolicy.SecureOn)) {
        Surface(Modifier.fillMaxSize(), color = MaterialTheme.colorScheme.background) {
            Column(Modifier.fillMaxSize().windowInsetsPadding(androidx.compose.foundation.layout.WindowInsets.safeDrawing).imePadding()) {
                Row(Modifier.fillMaxWidth().padding(horizontal = 16.dp), verticalAlignment = Alignment.CenterVertically) {
                    Text(title, modifier = Modifier.weight(1f), style = MaterialTheme.typography.titleLarge, fontWeight = FontWeight.SemiBold)
                    TextButton(onClick = onDismiss, modifier = Modifier.heightIn(min = 48.dp)) { Text("返回") }
                }
                Column(Modifier.weight(1f).fillMaxWidth().verticalScroll(rememberScrollState()).padding(16.dp),
                    verticalArrangement = Arrangement.spacedBy(12.dp)) { content() }
            }
        }
    }
}

// Reuse append helpers, exact minor amounts, tombstones and source invalidation.
internal fun financeQuickIncome(profile: FinanceProfile, day: String, entry: FinanceIncomeEntry): FinanceProfile {
    val ledger = profile.dayLedgerOrDefault(day)
    return profile.withDayLedger(day, ledger.copy(incomes = ledger.incomes.appendIncomeRow(entry), confirmedAtEpochMillis = 0L))
}

internal fun financeQuickExpense(profile: FinanceProfile, day: String, entry: FinanceExpenseEntry): FinanceProfile {
    val ledger = profile.dayLedgerOrDefault(day)
    return profile.withDayLedger(day, ledger.copy(expenses = ledger.expenses.appendExpenseRow(entry), confirmedAtEpochMillis = 0L))
}

@Composable
internal fun FinanceQuickEntryEditor(profile: FinanceProfile, dateKey: String, expense: Boolean, accent: Color,
    onProfileChange: (FinanceProfile) -> Unit, onDismiss: () -> Unit) {
    val incomeEntry = remember(dateKey, expense) { FinanceIncomeEntry() }
    val expenseEntry = remember(dateKey, expense) { FinanceExpenseEntry() }
    var discardRequested by remember { mutableStateOf(false) }
    FinanceNavigationDialog(if (expense) "记支出" else "记收入", onDismiss = { discardRequested = true }) {
        Text("记入 $dateKey", style = MaterialTheme.typography.titleSmall, modifier = Modifier.testTag("finance_quick_date"))
        Text("其他日期可返回日账选择，保存后自动汇总。", style = MaterialTheme.typography.bodySmall)
        if (expense) FinanceExpenseRow(entry = expenseEntry, accent = accent, settings = profile.settings, isNew = true,
            onSave = { onProfileChange(financeQuickExpense(profile, dateKey, it)); onDismiss() }, onDelete = onDismiss)
        else FinanceIncomeRow(entry = incomeEntry, accent = accent, isNew = true,
            onSave = { onProfileChange(financeQuickIncome(profile, dateKey, it)); onDismiss() }, onDelete = onDismiss)
    }
    if (discardRequested) AlertDialog(onDismissRequest = { discardRequested = false },
        title = { Text("放弃这笔记录？") }, text = { Text("尚未保存的内容将被清除。") },
        confirmButton = { TextButton(onClick = onDismiss) { Text("放弃") } },
        dismissButton = { TextButton(onClick = { discardRequested = false }) { Text("继续填写") } })
}
"####;

pub const TEST_CONTENTS: &str = r####"package com.ofairyo.gridtimer.ui
import com.ofairyo.gridtimer.data.*
import org.junit.Assert.*
import org.junit.Test

class FinanceQuickEntryTest {
    @Test fun quickIncomeKeepsExactCentsAndOtherRowsWhileInvalidatingReview() {
        val old = FinanceDayLedger(note = "keep", confirmedAtEpochMillis = 123L,
            expenses = listOf(FinanceExpenseEntry(id = "expense", amount = 12L, amountMinor = 1234L)),
            incomes = listOf(FinanceIncomeEntry(id = "deleted", deletedAtEpochMillis = 9L, amountMinor = 0L)))
        val profile = FinanceProfile(acquisitionFocus = "keep plan", dailyLedgers = mapOf("2026-10-19" to old, "2026-10-18" to FinanceDayLedger(note = "yesterday")))
        val entry = FinanceIncomeEntry(id = "new", name = "income", amount = 100L, amountMinor = 10001L)
        val updated = financeQuickIncome(profile, "2026-10-19", entry)
        val day = updated.dayLedgerOrDefault("2026-10-19")
        assertEquals(listOf(old.incomes.single(), entry), day.incomes)
        assertEquals(old.expenses, day.expenses)
        assertEquals("keep", day.note)
        assertEquals(0L, day.confirmedAtEpochMillis)
        assertEquals(profile.dailyLedgers["2026-10-18"], updated.dailyLedgers["2026-10-18"])
        assertEquals(profile.acquisitionFocus, updated.acquisitionFocus)
        assertEquals(123L, old.confirmedAtEpochMillis)
    }
    @Test fun quickExpenseAddsToLatestProfileWithoutReplacingIncomeOrSnapshots() {
        val income = FinanceIncomeEntry(id = "synced", amount = 999L, amountMinor = 99999L)
        val snapshot = FinanceMonthSnapshot(note = "snapshot")
        val profile = FinanceProfile(dailyLedgers = mapOf("2026-10-19" to FinanceDayLedger(incomes = listOf(income), confirmedAtEpochMillis = 5L)),
            monthlySnapshots = mapOf("2026-10" to snapshot))
        val expense = FinanceExpenseEntry(id = "new", name = "expense", amountMinor = 1L)
        val updated = financeQuickExpense(profile, "2026-10-19", expense)
        assertEquals(listOf(income), updated.dayLedgerOrDefault("2026-10-19").incomes)
        assertEquals(listOf(expense), updated.dayLedgerOrDefault("2026-10-19").expenses)
        assertEquals(profile.monthlySnapshots, updated.monthlySnapshots)
        assertEquals(0L, updated.dayLedgerOrDefault("2026-10-19").confirmedAtEpochMillis)
    }
    @Test fun bookkeepingStartsAtTheDailyLedger() {
        assertEquals(FinanceReportPeriod.DAY, FinancePeriodSelectionState().selectedPeriod)
    }
}
"####;
