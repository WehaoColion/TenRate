// Keep runtime-generated home summaries localized without translating user-authored data.
// This is intentionally scoped to the timer dashboard, not a global text replacement.
pub const PATH: &str = "com/ofairyo/gridtimer/ui/GridTimerScreen.kt";

const HELPERS: &str = r####"
@Composable
private fun isEnglishTimerUi(): Boolean =
    LocalConfiguration.current.locales[0]?.language.equals("en", ignoreCase = true)

private fun englishTimerDuration(durationMillis: Long): String {
    val totalMinutes = (durationMillis / 60_000L).coerceAtLeast(0L)
    val hours = totalMinutes / 60L
    val minutes = totalMinutes % 60L
    return when {
        hours > 0 && minutes > 0 -> hours.toString() + "h " + minutes + "m"
        hours > 0 -> hours.toString() + "h"
        else -> minutes.toString() + "m"
    }
}

private fun englishHomeTitle(slot: TimerSlot): String =
    slot.title.ifBlank { "Task " + slot.id.toString().padStart(2, '0') }

@Composable
private fun localizedHomeSlotTitle(slot: TimerSlot): String =
    if (isEnglishTimerUi() && slot.title.isBlank()) englishHomeTitle(slot)
    else TimerRepository.slotDisplayTitle(slot)

private fun englishHomeRhythm(
    activeCount: Int,
    todaySessionCount: Int,
    todayTotal: Long,
    latestSession: TimerSession?,
    peakSession: TimerSession?,
    focusSuggestion: HomeFocusSuggestion,
    focusSlot: TimerSlot?,
    busiestTodaySlot: TimerSlot?,
    busiestTodayDuration: Long,
    now: Long
): String = when {
    activeCount > 0 && focusSlot != null -> {
        val finished = if (todaySessionCount > 0) {
            todaySessionCount.toString() + " completed sessions today (" +
                englishTimerDuration(todayTotal) + ")."
        } else {
            "No completed sessions today."
        }
        finished + " Running: " + englishHomeTitle(focusSlot) + "."
    }
    busiestTodaySlot != null && busiestTodayDuration > 0L ->
        "Most focused today: " + englishHomeTitle(busiestTodaySlot) +
            " (" + englishTimerDuration(busiestTodayDuration) + ")."
    peakSession != null ->
        "Longest focus session: " + englishTimerDuration(peakSession.durationMillis) + "."
    latestSession != null ->
        "Last session ended at " + formatMomentLabel(latestSession.endedAtEpochMillis, now) +
            " (" + englishTimerDuration(latestSession.durationMillis) + ")."
    focusSuggestion.kind == HomeFocusKind.START_FRESH -> "No sessions today."
    else -> "No timer slots are running."
}

private fun englishFocusGuidance(
    suggestion: HomeFocusSuggestion,
    slot: TimerSlot,
    elapsed: Long
): String {
    val slotLabel = "Slot " + slot.id.toString().padStart(2, '0')
    return when (suggestion.kind) {
        HomeFocusKind.RUNNING -> slotLabel + " has logged " + englishTimerDuration(elapsed) + "."
        HomeFocusKind.RESUME -> if (elapsed > 0L) {
            slotLabel + " has logged " + englishTimerDuration(elapsed) + ". Ready to resume."
        } else {
            slotLabel + " is ready to resume."
        }
        HomeFocusKind.START_FRESH -> slotLabel + " is ready to start."
    }
}

// This function is only called for generated dashboard metrics, never note, category,
// task or canvas content. The source data remains stored in its original language.
@Composable
private fun localizedHomeMetricText(value: String): String {
    val translated = localizedUiText(value, sourceLiteralOnly = false)
    if (!isEnglishTimerUi() || translated != value) return translated
    val duration = Regex("""(\d+)小时""").replace(value) { it.groupValues[1] + "h" }
    val normalized = Regex("""(\d+)分""").replace(duration) { it.groupValues[1] + "m" }
    return when (normalized) {
        "当前没有格子在计时" -> "No timers are running"
        "第一条完成记录会出现在这里" -> "Your first completed session will appear here"
        "目前看板还是空的" -> "The board is empty"
        "现在有 1 个格子正在计时" -> "1 slot is running"
        "现在有多个格子在并行计时" -> "Multiple slots are running"
        "归档会保存在这里，随时都能恢复" -> "Archived tasks can be restored here"
        else -> normalized
    }
}

@Composable
private fun localizedTileTodayStats(
    todayDuration: Long,
    todayTotalDuration: Long,
    fallback: String
): String {
    if (!isEnglishTimerUi()) return fallback
    val ratio = if (todayTotalDuration > 0L) {
        (todayDuration.toFloat() / todayTotalDuration.toFloat()).coerceIn(0f, 1f)
    } else 0f
    return "Today " + englishTimerDuration(todayDuration) + " · " + formatRatioPercent(ratio)
}

@Composable
private fun localizedTileStatus(
    isRunning: Boolean,
    isOnBreak: Boolean,
    phaseRemainingMillis: Long,
    runningSinceLabel: String,
    isBlankSlate: Boolean,
    todayDuration: Long,
    latestFinishLabel: String,
    fallback: String
): String {
    if (!isEnglishTimerUi()) return fallback
    return when {
        isRunning && isOnBreak -> "Short break: " + formatCountdownDuration(phaseRemainingMillis) + " remaining"
        isRunning -> "Started at " + runningSinceLabel.ifBlank { "--" }
        isBlankSlate -> ""
        todayDuration > 0L -> "Focused today: " + englishTimerDuration(todayDuration)
        latestFinishLabel.isNotBlank() -> "Last ended at " + latestFinishLabel
        else -> ""
    }
}

"####;

fn replace_exact(source: &mut String, needle: &str, replacement: &str, expected: usize) -> Result<(), String> {
    let found = source.matches(needle).count();
    if found != expected {
        return Err(format!("Android English home anchor mismatch (expected {expected}, found {found}): {needle}"));
    }
    *source = source.replace(needle, replacement);
    Ok(())
}

pub fn render(path: &str, source: &str) -> Result<String, String> {
    if path != PATH {
        return Ok(source.to_owned());
    }
    let mut output = source.to_owned();
    replace_exact(
        &mut output,
        "@OptIn(ExperimentalLayoutApi::class)\n@Composable\nprivate fun HeaderBlock(",
        &(HELPERS.to_owned() + "@OptIn(ExperimentalLayoutApi::class)\n@Composable\nprivate fun HeaderBlock("),
        1,
    )?;
    replace_exact(
        &mut output,
        "    val heroDescription = if (compactLayout) rhythmNarrative else description",
        r####"    val englishUi = isEnglishTimerUi()
    val shownRhythm = if (englishUi) {
        englishHomeRhythm(
            activeCount, todaySessionCount, todayTotal, latestSession, peakSession,
            focusSuggestion, focusSlot, busiestTodaySlot, busiestTodayDuration, now
        )
    } else rhythmNarrative
    val heroDescription = if (englishUi) {
        if (compactLayout) shownRhythm
        else "Choose one slot and make your next step clear."
    } else if (compactLayout) shownRhythm else description"####,
        1,
    )?;
    replace_exact(&mut output, "text = rhythmNarrative,", "text = shownRhythm,", 1)?;
    replace_exact(
        &mut output,
        r####"label = if (todayTotal > 0L) "今日 ${formatCompactDuration(todayTotal)}" else thirdBadgeLabel,"####,
        r####"label = if (todayTotal > 0L && englishUi) "Today " + englishTimerDuration(todayTotal)
                    else if (todayTotal > 0L) "今日 ${formatCompactDuration(todayTotal)}"
                    else thirdBadgeLabel,"####,
        1,
    )?;
    replace_exact(
        &mut output,
        r####"label = "进行中 ${activeCount.toString().padStart(2, '0')}", "####.trim_end(),
        r####"label = if (englishUi) "Running " + activeCount.toString().padStart(2, '0')
                    else "进行中 ${activeCount.toString().padStart(2, '0')}", "####.trim_end(),
        1,
    )?;
    replace_exact(
        &mut output,
        r####"label = "今日 ${formatCompactDuration(todayTotal)}","####,
        r####"label = if (englishUi) "Today " + englishTimerDuration(todayTotal)
                    else "今日 ${formatCompactDuration(todayTotal)}","####,
        1,
    )?;
    replace_exact(
        &mut output,
        r####"label = "归档 ${archivedTaskCount.toString().padStart(2, '0')}", "####.trim_end(),
        r####"label = if (englishUi) "Archived " + archivedTaskCount.toString().padStart(2, '0')
                    else "归档 ${archivedTaskCount.toString().padStart(2, '0')}", "####.trim_end(),
        1,
    )?;
    replace_exact(
        &mut output,
        "    val contentPadding = if (compactLayout) 16.dp else 20.dp",
        "    val englishUi = isEnglishTimerUi()\n    val contentPadding = if (compactLayout) 16.dp else 20.dp",
        1,
    )?;
    replace_exact(
        &mut output,
        "text = guidance,\n                maxLines = if (compactLayout) 3",
        "text = if (englishUi) englishFocusGuidance(suggestion, slot, elapsed) else guidance,\n                maxLines = if (compactLayout) 3",
        1,
    )?;
    replace_exact(&mut output, "text = TimerRepository.slotDisplayTitle(slot),", "text = localizedHomeSlotTitle(slot),", 3)?;
    replace_exact(
        &mut output,
        "val timerDialTitle = TimerRepository.slotDisplayTitle(slot.copy(id = displayIndex))",
        "val timerDialTitle = localizedHomeSlotTitle(slot.copy(id = displayIndex))",
        1,
    )?;
    replace_exact(
        &mut output,
        "text = todayStatsLabel,",
        "text = localizedTileTodayStats(todayDuration, todayTotalDuration, todayStatsLabel),",
        1,
    )?;
    replace_exact(
        &mut output,
        "text = slotStatusLabel,",
        "text = localizedTileStatus(isRunning, isOnBreak, phaseRemainingMillis, slot.runningSinceEpochMillis?.let { formatMomentLabel(it, elapsedNow) }.orEmpty(), isBlankSlate, todayDuration, latestSession?.let { formatMomentLabel(it.endedAtEpochMillis, elapsedNow) }.orEmpty(), slotStatusLabel),",
        1,
    )?;
    replace_exact(
        &mut output,
        "text = currentValue,\n                style = MaterialTheme.typography.titleLarge",
        "text = localizedHomeMetricText(currentValue),\n                style = MaterialTheme.typography.titleLarge",
        1,
    )?;
    replace_exact(
        &mut output,
        "text = note,\n            style = MaterialTheme.typography.bodySmall.copy(\n                color = MaterialTheme.colorScheme.onSurface.copy(alpha = 0.54f)",
        "text = localizedHomeMetricText(note),\n            style = MaterialTheme.typography.bodySmall.copy(\n                color = MaterialTheme.colorScheme.onSurface.copy(alpha = 0.54f)",
        1,
    )?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::{render, PATH};

    #[test]
    fn english_home_rewrites_only_dashboard_display_paths() {
        let source = crate::kotlin_sources::SOURCES
            .iter()
            .find(|file| file.path == PATH)
            .expect("GridTimerScreen source");
        let actual = render(PATH, source.contents).expect("strict anchors match");
        for expected in [
            "englishHomeRhythm(", "localizedTileTodayStats(", "localizedHomeSlotTitle(slot)",
            "localizedHomeMetricText(note)", "englishFocusGuidance(suggestion, slot, elapsed)",
            "Archived ", "No sessions today."
        ] {
            assert!(actual.contains(expected), "missing {expected}");
        }
        assert!(actual.contains("text = footerLabel,"), "user-authored note content must remain untouched");
        assert!(actual.contains("note = slot.note"), "do not translate private note data");
        assert!(!actual.contains("text = rhythmNarrative,"), "native narrative must not bypass the locale");
    }

    #[test]
    fn unrelated_generated_sources_are_unchanged() {
        let value = "User-created task 任务 01";
        assert_eq!(render("com/ofairyo/gridtimer/ui/NoteDocumentEditor.kt", value).unwrap(), value);
    }
}
