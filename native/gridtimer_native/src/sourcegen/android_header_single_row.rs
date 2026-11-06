// v2.23.4.10 - Refine balanced groups and compact squircle actions.
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

const HEADER_ROW: &str = r####"    // Prefer content-sized labels, a quiet gap and a paired action cluster.
    // The version label is the only shrinkable item, so both actions stay visible.
    BoxWithConstraints(modifier = Modifier.fillMaxWidth()) {
        val compactHeader = maxWidth < 430.dp
        val tightHeader = maxWidth < 300.dp
        Row(
            modifier = Modifier.fillMaxWidth(),
            horizontalArrangement = Arrangement.spacedBy(if (tightHeader) 5.dp else 8.dp),
            verticalAlignment = Alignment.CenterVertically
        ) {
            Row(
                modifier = Modifier.weight(1f),
                horizontalArrangement = Arrangement.spacedBy(if (tightHeader) 4.dp else 7.dp),
                verticalAlignment = Alignment.CenterVertically
            ) {
                HomeHeaderMetaBadge(
                    label = if (compactHeader) "v${BuildConfig.VERSION_NAME}" else buildVersionBadgeLabel(),
                    accent = MaterialTheme.colorScheme.primary,
                    prominent = true,
                    modifier = Modifier.weight(1f, fill = false)
                )
                HomeHeaderMetaBadge(
                    label = if (todayTotal > 0L && englishUi) "Today " + englishTimerDuration(todayTotal)
                        else if (todayTotal > 0L) "今日 ${formatCompactDuration(todayTotal)}"
                        else thirdBadgeLabel,
                    accent = neutralAccent
                )
            }
            Row(
                horizontalArrangement = Arrangement.spacedBy(if (tightHeader) 4.dp else 6.dp),
                verticalAlignment = Alignment.CenterVertically
            ) {
                MiniChromeIconBadge(
                    icon = Icons.Rounded.History,
                    accent = MaterialTheme.colorScheme.secondary,
                    modifier = Modifier.size(40.dp),
                    shape = RoundedCornerShape(14.dp),
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
    }

"####;

const META_BADGE: &str = r####"@Composable
private fun HomeHeaderMetaBadge(
    label: String,
    accent: Color,
    modifier: Modifier = Modifier,
    prominent: Boolean = false
) {
    Surface(
        modifier = modifier,
        shape = RoundedCornerShape(14.dp),
        color = accent.copy(alpha = if (prominent) 0.09f else 0.055f),
        border = BorderStroke(1.dp, accent.copy(alpha = if (prominent) 0.17f else 0.11f))
    ) {
        Text(
            text = label,
            modifier = Modifier.padding(horizontal = 10.dp, vertical = 8.dp),
            maxLines = 1,
            softWrap = false,
            overflow = TextOverflow.Ellipsis,
            style = MaterialTheme.typography.labelMedium.copy(
                color = accent,
                fontWeight = if (prominent) FontWeight.SemiBold else FontWeight.Medium
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

fn replace_in_section(
    source: &mut String,
    start_marker: &str,
    end_marker: &str,
    before: &str,
    after: &str,
) -> Result<(), String> {
    if source.matches(start_marker).count() != 1 || source.matches(end_marker).count() != 1 {
        return Err(format!("home header icon helper boundary changed: {start_marker}"));
    }
    let start = source.find(start_marker).unwrap();
    let end = source.find(end_marker).unwrap();
    if end <= start {
        return Err("home header icon helper boundaries reversed".into());
    }
    let section = &source[start..end];
    if section.matches(before).count() != 1 {
        return Err(format!("home header icon helper changed: {before}"));
    }
    let edited = section.replacen(before, after, 1);
    source.replace_range(start..end, &edited);
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
    // Optional shape only: all other MiniChromeIconBadge call sites retain
    // their original round surfaces, while this pair gets softer 14dp corners.
    replace_in_section(
        &mut output,
        "@Composable\ninternal fun MiniChromeIconBadge(",
        "internal fun shouldDismissTimerDetailDrag(",
        "    testTag: String? = null,\n    onClick: () -> Unit",
        "    testTag: String? = null,\n    shape: RoundedCornerShape = RoundedCornerShape(999.dp),\n    onClick: () -> Unit",
    )?;
    replace_in_section(
        &mut output,
        "@Composable\ninternal fun MiniChromeIconBadge(",
        "internal fun shouldDismissTimerDetailDrag(",
        "        shape = RoundedCornerShape(999.dp),",
        "        shape = shape,",
    )?;
    replace_once(
        &mut output,
        "            modifier = Modifier.fillMaxSize(),\n            icon = themeIcon(themeMode),",
        "            modifier = Modifier.fillMaxSize(),\n            shape = RoundedCornerShape(14.dp),\n            icon = themeIcon(themeMode),",
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
        assert!(rendered.contains("val compactHeader = maxWidth < 430.dp"));
        assert!(rendered.contains("val tightHeader = maxWidth < 300.dp"));
        let header = rendered
            .split("    // Prefer content-sized labels, a quiet gap")
            .nth(1)
            .expect("single-row header inserted")
            .split("    Column(verticalArrangement = Arrangement.spacedBy(if (compactLayout)")
            .next()
            .unwrap();
        assert!(header.contains("modifier = Modifier.weight(1f, fill = false)"));
        assert!(header.contains("prominent = true"));
        assert!(!header.contains("Modifier.weight(2f)"));
        assert!(header.contains("shape = RoundedCornerShape(14.dp)"));
        assert!(header.contains("testTag = \"home_header_history\""));
        assert!(header.contains("onClick = onOpenHistory"));
        assert!(header.contains("onThemeModeSelected = onThemeModeSelected"));
        assert!(header.contains("if (todayTotal > 0L && englishUi)"));
        assert!(rendered.contains("maxLines = 1,\n            softWrap = false,\n            overflow = TextOverflow.Ellipsis"));
        assert!(rendered.contains("testTag = \"home_header_theme\""));
        assert_eq!(rendered.matches("private fun HomeHeaderMetaBadge(").count(), 1);
        assert!(rendered.contains("shape: RoundedCornerShape = RoundedCornerShape(999.dp)"));
        assert!(rendered.contains("modifier = Modifier.fillMaxSize(),\n            shape = RoundedCornerShape(14.dp),\n            icon = themeIcon(themeMode),"));
    }

    #[test]
    fn other_icon_badges_remain_round_by_default() {
        let rendered = render(SCREEN, &english_home_source()).expect("home header patch");
        let icon_helper = rendered
            .split("@Composable\ninternal fun MiniChromeIconBadge(")
            .nth(1)
            .unwrap()
            .split("internal fun shouldDismissTimerDetailDrag(")
            .next()
            .unwrap();
        assert!(icon_helper.contains("shape: RoundedCornerShape = RoundedCornerShape(999.dp)"));
        assert!(icon_helper.contains("        shape = shape,"));
        assert_eq!(icon_helper.matches("shape = shape,").count(), 1);
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
