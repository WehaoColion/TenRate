// v2.23.4.9 - Keep the timer home header controls on one row on phones.
const SCREEN: &str = "com/ofairyo/gridtimer/ui/GridTimerScreen.kt";

const ROW_START: &str = r####"    FlowRow(
        modifier = Modifier.fillMaxWidth(),
        horizontalArrangement = Arrangement.spacedBy(8.dp),
        verticalArrangement = Arrangement.spacedBy(8.dp)
    ) {
        MiniChromeBadge(
            label = buildVersionBadgeLabel(),"####;

const ROW_END: &str =
    "    Column(verticalArrangement = Arrangement.spacedBy(if (compactLayout) 12.dp else 16.dp)) {";

const HEADER_ROW: &str = r####"    // The header is a single line: reserve space for both actions before
    // distributing the remaining width between the two text pills.
    BoxWithConstraints(modifier = Modifier.fillMaxWidth()) {
        val narrowHeader = maxWidth < 320.dp
        Row(
            modifier = Modifier.fillMaxWidth(),
            horizontalArrangement = Arrangement.spacedBy(if (narrowHeader) 4.dp else 6.dp),
            verticalAlignment = Alignment.CenterVertically
        ) {
            HomeHeaderMetaBadge(
                label = if (narrowHeader) "v${BuildConfig.VERSION_NAME}" else buildVersionBadgeLabel(),
                accent = MaterialTheme.colorScheme.primary,
                modifier = Modifier.weight(2f)
            )
            HomeHeaderMetaBadge(
                label = if (todayTotal > 0L && englishUi) "Today " + englishTimerDuration(todayTotal)
                    else if (todayTotal > 0L) "今日 ${formatCompactDuration(todayTotal)}"
                    else thirdBadgeLabel,
                accent = neutralAccent,
                modifier = Modifier.weight(1f)
            )
            MiniChromeIconBadge(
                icon = Icons.Rounded.History,
                accent = MaterialTheme.colorScheme.secondary,
                modifier = Modifier.size(40.dp),
                testTag = "home_header_history",
                onClick = onOpenHistory
            )
            HeaderThemeMenu(
                themeMode = themeMode,
                onThemeModeSelected = onThemeModeSelected,
                modifier = Modifier.size(40.dp)
            )
        }
    }

"####;

const META_BADGE: &str = r####"@Composable
private fun HomeHeaderMetaBadge(label: String, accent: Color, modifier: Modifier = Modifier) {
    Surface(
        modifier = modifier,
        shape = RoundedCornerShape(999.dp),
        color = accent.copy(alpha = 0.08f),
        border = BorderStroke(1.dp, accent.copy(alpha = 0.15f))
    ) {
        Text(
            text = label,
            modifier = Modifier.padding(horizontal = 8.dp, vertical = 8.dp),
            maxLines = 1,
            softWrap = false,
            overflow = TextOverflow.Ellipsis,
            style = MaterialTheme.typography.labelMedium.copy(
                color = accent,
                fontWeight = FontWeight.Medium
            )
        )
    }
}

"####;

fn replace_once(source: &mut String, before: &str, after: &str) -> Result<(), String> {
    let found = source.matches(before).count();
    if found != 1 {
        return Err(format!("home header anchor mismatch (expected 1, found {found}): {before}"));
    }
    *source = source.replacen(before, after, 1);
    Ok(())
}

pub fn render(path: &str, source: &str) -> Result<String, String> {
    if path != SCREEN {
        return Ok(source.to_owned());
    }
    let mut output = source.to_owned();
    replace_once(
        &mut output,
        "thirdBadgeLabel = \"归档状态\",",
        "thirdBadgeLabel = \"归档\",",
    )?;

    if output.matches(ROW_START).count() != 1 {
        return Err("home header row anchor missing or duplicated".into());
    }
    if output.matches(ROW_END).count() != 1 {
        return Err("home header next-section anchor missing or duplicated".into());
    }
    let start = output.find(ROW_START).unwrap();
    let end = output.find(ROW_END).unwrap();
    if start >= end {
        return Err("home header row boundaries are out of order".into());
    }
    let current = &output[start..end];
    // The English-home pass must precede this pass, so that the Today
    // runtime label remains localized when a timer has accumulated time.
    for necessary in [
        "label = if (todayTotal > 0L && englishUi)",
        "testTag = \"home_header_history\"",
        "HeaderThemeMenu(",
    ] {
        if !current.contains(necessary) {
            return Err(format!("home header action changed: {necessary}"));
        }
    }
    output.replace_range(start..end, HEADER_ROW);

    replace_once(
        &mut output,
        "@Composable\nprivate fun HeaderThemeMenu(",
        &format!("{META_BADGE}@Composable\nprivate fun HeaderThemeMenu("),
    )?;
    replace_once(
        &mut output,
        "private fun HeaderThemeMenu(\n    themeMode: ThemeMode,\n    onThemeModeSelected: (ThemeMode) -> Unit\n)",
        "private fun HeaderThemeMenu(\n    themeMode: ThemeMode,\n    onThemeModeSelected: (ThemeMode) -> Unit,\n    modifier: Modifier = Modifier\n)",
    )?;
    replace_once(
        &mut output,
        "    Box {\n        MiniChromeIconBadge(\n            icon = themeIcon(themeMode),",
        "    Box(modifier = modifier) {\n        MiniChromeIconBadge(\n            modifier = Modifier.fillMaxSize(),\n            icon = themeIcon(themeMode),",
    )?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn english_home_source() -> String {
        let source = crate::kotlin_sources::SOURCES
            .iter()
            .find(|source| source.path == SCREEN)
            .expect("home screen Kotlin template")
            .contents;
        crate::android_english_home::render(SCREEN, source)
            .expect("English home sourcegen must render before compact header")
    }

    #[test]
    fn archive_badge_and_actions_share_one_bounded_row() {
        let rendered = render(SCREEN, &english_home_source()).expect("home header patch");
        assert!(rendered.contains("thirdBadgeLabel = \"归档\","));
        assert!(!rendered.contains("thirdBadgeLabel = \"归档状态\","));
        assert!(rendered.contains("val narrowHeader = maxWidth < 320.dp"));
        assert!(rendered.contains("modifier = Modifier.weight(2f)"));
        assert!(rendered.contains("modifier = Modifier.weight(1f)"));
        assert!(rendered.contains("maxLines = 1,\n            softWrap = false,\n            overflow = TextOverflow.Ellipsis"));
        assert!(rendered.contains("testTag = \"home_header_history\""));
        assert!(rendered.contains("testTag = \"home_header_theme\""));
        assert!(rendered.contains("if (todayTotal > 0L && englishUi)"));
        assert_eq!(rendered.matches("private fun HomeHeaderMetaBadge(").count(), 1);
        assert!(rendered.contains("modifier = Modifier.fillMaxSize(),\n            icon = themeIcon(themeMode),"));
    }

    #[test]
    fn refuses_reordered_or_missing_header_anchors() {
        let original = english_home_source();
        assert!(render(SCREEN, &original.replace("thirdBadgeLabel = \"归档状态\",", "thirdBadgeLabel = \"已归档\",")).is_err());
        assert!(render(SCREEN, &original.replace("label = buildVersionBadgeLabel(),", "label = \"version\",")).is_err());
        assert!(render(SCREEN, &original.replace("testTag = \"home_header_history\",", "testTag = \"history\",")).is_err());
    }

    #[test]
    fn does_not_change_other_sources() {
        assert_eq!(render("com/example/Other.kt", "unchanged").unwrap(), "unchanged");
    }
}
