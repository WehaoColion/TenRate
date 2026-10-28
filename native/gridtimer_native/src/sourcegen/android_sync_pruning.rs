// v2.23.2.60 - Avoid repeated retained-media scans when synchronization removes nothing.
const REPOSITORY: &str = "com/ofairyo/gridtimer/data/TimerRepository.kt";

fn replace(source: &mut String, old: &str, new: &str) -> Result<(), String> {
    if source.matches(old).count() != 1 {
        return Err(format!("sync media pruning anchor is not unique: {old}"));
    }
    *source = source.replacen(old, new, 1);
    Ok(())
}

fn edit_section<F>(source: &mut String, start: &str, end: &str, edit: F) -> Result<(), String>
where
    F: FnOnce(&mut String) -> Result<(), String>,
{
    let a = source
        .find(start)
        .ok_or_else(|| format!("missing section {start}"))?;
    let b = a + source[a..]
        .find(end)
        .ok_or_else(|| format!("missing section {end}"))?;
    let mut section = source[a..b].to_owned();
    edit(&mut section)?;
    source.replace_range(a..b, &section);
    Ok(())
}

pub fn render(path: &str, base: &str) -> Result<String, String> {
    let mut source = base.to_owned();
    if path != REPOSITORY {
        return Ok(source);
    }
    edit_section(
        &mut source,
        "    private suspend fun syncAccountMediaBestEffort(",
        "    suspend fun syncNow()",
        |section| {
            replace(section,
            "            updateData { data ->\n",
            "            var previousMediaMetadata: AppData? = null\n            val metadataUpdate = updateDataDetailed(expectedWorkspaceKey = workspaceKey) { data ->\n                previousMediaMetadata = data\n")?;
            replace(section,
            "            if (persistNow(\"account_media_metadata\")) {\n                pruneNoteMedia(_appData.value)\n            }",
            r####"            if (!metadataUpdate.succeeded || activeWorkspaceKey != workspaceKey || !persistNow("account_media_metadata")) {
                logDiagnosticEvent(
                    category = "sync.account.media",
                    message = "Media metadata could not be safely persisted for the original workspace."
                )
                return summary.copy(failures = summary.failures + 1)
            }
            val needsPrune = shouldPruneSyncNoteMedia(
                previousMediaMetadata,
                metadataUpdate.snapshot,
                summary.remoteDeletedAtByAttachmentId.isNotEmpty()
            )
            if (needsPrune) {
                pruneNoteMedia(_appData.value)
            } else {
                logDiagnosticEvent(
                    category = "notes.media.prune",
                    message = "Skipped retained-media scan: sync metadata removed no attachment references."
                )
            }"####)
        },
    )?;
    edit_section(
        &mut source,
        "    private suspend fun applyAccountSnapshotIfVisible(",
        "    private fun historyDeletionSummaryText(",
        |section| {
            replace(section,
            "        var missingWorkspaceRecoveryPersistFailure: Throwable? = null\n",
            "        var missingWorkspaceRecoveryPersistFailure: Throwable? = null\n        var needsMediaPrune = true\n")?;
            replace(section,
            "            keptNewerLocalData = !effectiveForceReplace && applied != received\n",
            "            keptNewerLocalData = !effectiveForceReplace && applied != received\n            needsMediaPrune = shouldPruneSyncNoteMedia(_appData.value, applied, recoveringMissingWorkspace)\n")?;
            replace(
                section,
                "        pruneNoteMedia(applied)\n",
                r####"        if (needsMediaPrune) {
            pruneNoteMedia(applied)
        } else {
            logDiagnosticEvent(
                category = "notes.media.prune",
                message = "Skipped retained-media scan: account snapshot removed no attachment references."
            )
        }
"####,
            )
        },
    )?;
    edit_section(
        &mut source,
        "    private suspend fun pruneNoteMedia(appData: AppData) {",
        "    private suspend fun monitorMicroBreaks()",
        |section| {
            replace(section,
            "    private suspend fun pruneNoteMedia(appData: AppData) {\n        runCatching {",
            "    private suspend fun pruneNoteMedia(appData: AppData) {\n        val scanStarted = android.os.SystemClock.elapsedRealtime()\n        logDiagnosticEvent(category = \"notes.media.prune\", message = \"Retained-media scan started.\")\n        runCatching {")?;
            replace(
                section,
                r####"        }.onFailure { throwable ->
            Log.w(TAG, "Failed to prune note media files.", throwable)
            logDiagnosticEvent(
                category = "notes.media.prune",
                message = "Failed to prune note media files.",
                throwable = throwable
            )
        }
    }
"####,
                r####"        }.onFailure { throwable ->
            Log.w(TAG, "Failed to prune note media files.", throwable)
            logDiagnosticEvent(
                category = "notes.media.prune",
                message = "Failed to prune note media files.",
                throwable = throwable
            )
        }
        logDiagnosticEvent(
            category = "notes.media.prune",
            message = "Retained-media scan returned elapsedMs=${android.os.SystemClock.elapsedRealtime() - scanStarted}"
        )
    }
"####,
            )
        },
    )?;
    Ok(source)
}
