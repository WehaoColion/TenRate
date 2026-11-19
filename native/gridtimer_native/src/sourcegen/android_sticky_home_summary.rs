// v3.0.2.3 - Keep the latest sticky-note preview bounded and its actions stable.
// Rust owns this Android Compose UI source transformation; no generated Kotlin is committed.

const PATH: &str = "com/ofairyo/gridtimer/ui/NoteStudioSheet.kt";
const BEGIN: &str = "private fun StickyNoteSummaryCard(\n";
const END: &str = "\n@OptIn(ExperimentalLayoutApi::class)\n@Composable\nprivate fun StickyNotePresetRow(";

pub fn render(path: &str, source: &str) -> Result<String, String> {
    if path != PATH {
        return Ok(source.to_owned());
    }
    let from = source.find(BEGIN)
        .ok_or_else(|| "sticky summary: card declaration not found".to_string())?;
    if source[from + BEGIN.len()..].contains(BEGIN) {
        return Err("sticky summary: ambiguous card declaration".into());
    }
    let to = source[from..].find(END).map(|offset| from + offset)
        .ok_or_else(|| "sticky summary: following preset-row anchor not found".to_string())?;
    let original = &source[from..to];
    if !original.contains("text = latestNote?.let {") ||
        !original.contains("verticalAlignment = Alignment.CenterVertically") ||
        !original.contains("onManageFolders = onManageFolders") {
        return Err("sticky summary: expected original card layout changed".into());
    }
    let mut result = String::with_capacity(source.len());
    result.push_str(&source[..from]);
    result.push_str(CARD);
    result.push_str(&source[to..]);
    Ok(result)
}

const CARD: &str = r###"private fun StickyNoteSummaryCard(
    accent: Color,
    latestNote: NoteEntry?,
    now: Long,
    trashMode: Boolean,
    onCreateNote: () -> Unit,
    onToggleTrash: () -> Unit,
    onManageFolders: () -> Unit
) {
    SmartisanCardSurface(
        accent = accent,
        shape = RoundedCornerShape(30.dp),
        containerColor = MaterialTheme.colorScheme.panel,
        shadowElevation = 10.dp,
        borderColor = accent.copy(alpha = 0.18f)
    ) {
        Column(
            modifier = Modifier
                .fillMaxWidth()
                .padding(20.dp),
            verticalArrangement = Arrangement.spacedBy(10.dp)
        ) {
            Text(
                text = if (trashMode) "回收站" else "最近便签",
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
                style = MaterialTheme.typography.titleLarge.copy(fontWeight = FontWeight.SemiBold)
            )
            if (latestNote != null) {
                Text(
                    text = noteUpdatedLabel(latestNote, now),
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                    style = MaterialTheme.typography.bodySmall.copy(
                        color = MaterialTheme.colorScheme.onSurface.copy(alpha = 0.58f)
                    )
                )
                Text(
                    text = latestNote.previewBody(),
                    modifier = Modifier.fillMaxWidth(),
                    maxLines = 2,
                    overflow = TextOverflow.Ellipsis,
                    style = MaterialTheme.typography.bodyMedium.copy(
                        color = MaterialTheme.colorScheme.onSurface.copy(alpha = 0.72f)
                    )
                )
            } else {
                Text(
                    text = if (trashMode) "恢复后回到便签。" else "暂无内容",
                    maxLines = 2,
                    overflow = TextOverflow.Ellipsis,
                    style = MaterialTheme.typography.bodyMedium.copy(
                        color = MaterialTheme.colorScheme.onSurface.copy(alpha = 0.72f)
                    )
                )
            }
            FlowRow(
                modifier = Modifier.fillMaxWidth(),
                horizontalArrangement = Arrangement.spacedBy(8.dp),
                verticalArrangement = Arrangement.spacedBy(8.dp)
            ) {
                if (!trashMode) {
                    CapsuleAction(
                        text = "新建",
                        icon = Icons.Rounded.Add,
                        accent = accent,
                        containerColor = accent.copy(alpha = 0.10f),
                        borderColor = accent.copy(alpha = 0.18f),
                        onClick = onCreateNote
                    )
                    CapsuleAction(
                        text = "文件夹",
                        icon = Icons.Rounded.Folder,
                        accent = MaterialTheme.colorScheme.secondary,
                        containerColor = MaterialTheme.colorScheme.secondary.copy(alpha = 0.08f),
                        borderColor = MaterialTheme.colorScheme.secondary.copy(alpha = 0.16f),
                        onClick = onManageFolders
                    )
                }
                CapsuleAction(
                    text = if (trashMode) "回到便签" else "回收站",
                    icon = if (trashMode) Icons.Rounded.FolderOpen else Icons.Rounded.RestoreFromTrash,
                    accent = MaterialTheme.colorScheme.secondary,
                    containerColor = MaterialTheme.colorScheme.secondary.copy(alpha = 0.08f),
                    borderColor = MaterialTheme.colorScheme.secondary.copy(alpha = 0.16f),
                    onClick = onToggleTrash
                )
            }
        }
    }
}
"###;
