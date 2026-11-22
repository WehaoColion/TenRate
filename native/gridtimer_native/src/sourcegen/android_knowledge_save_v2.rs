// v3.0.2.9 - Exclude hierarchy containers from scoped normalization.
// v3.0.2.2 - Scope ordinary knowledge-page creation normalization to the inserted
// note. All other paths and any failed safety assertion retain full sanitization.
const REPOSITORY: &str = "com/ofairyo/gridtimer/data/TimerRepository.kt";

fn replace_one(source: &mut String, before: &str, after: &str) -> Result<(), String> {
    let occurrences = source.matches(before).count();
    if occurrences != 1 {
        return Err(format!("knowledge save 3.0.2.2 expected one anchor, found {occurrences}: {before}"));
    }
    *source = source.replacen(before, after, 1);
    Ok(())
}

pub fn render(path: &str, base: &str) -> Result<String, String> {
    if path == "com/ofairyo/gridtimer/core/NativeOptimizerBridge.kt" {
        let mut source = base.to_owned();
        let anchor = "    fun buildNoteDocumentTextDigest(";
        replace_one(&mut source, anchor, &format!(r#"    private external fun nativeTextBlocksStorageContent(texts: Array<String>): String?

    fun textBlocksStorageContent(texts: Array<String>): String? =
        if (nativeAvailable) runCatching {{ nativeTextBlocksStorageContent(texts) }}.getOrNull() else null

{anchor}"#))?;
        return Ok(source);
    }
    if path == "com/ofairyo/gridtimer/data/NoteDocuments.kt" {
        let mut source = base.to_owned();
        let anchor = "internal fun NoteDocument.toStorageContent(): String {";
        replace_one(&mut source, anchor, &format!(r#"{anchor}
    if (!richTextEnabled && blocks.all {{ it.type == NoteBlockType.TEXT }}) {{
        NativeOptimizerBridge.textBlocksStorageContent(Array(blocks.size) {{ blocks[it].text }})?.let {{ return it }}
    }}"#))?;
        return Ok(source);
    }
    if path != REPOSITORY {
        return Ok(base.to_owned());
    }
    let mut source = base.to_owned();
    replace_one(
        &mut source,
        "    private suspend fun updateDataDetailed(\n        noteMutationId: String? = null,",
        "    private suspend fun updateDataDetailed(\n        hierarchyCreateNoteId: String? = null,\n        noteMutationId: String? = null,",
    )?;
    replace_one(
        &mut source,
        "        writeMutex.withLock {\n            if (noteMutationId != null) recordNoteSaveStage(\"draft_write_lock\", writeWaitStarted)",
        "        writeMutex.withLock {\n            if (noteMutationId != null) recordNoteSaveStage(\"draft_write_lock\", writeWaitStarted)\n            if (hierarchyCreateNoteId != null) recordNoteSaveStage(\"hierarchy_write_lock\", writeWaitStarted)",
    )?;

    // The existing Rust sourcegen already narrows ordinary note upserts and
    // timer changes. This new case deliberately receives the FULL hierarchy
    // for parent validation but normalizes only a strictly proven new note.
    let previous = r####"                        val normalized = stampChangedFieldRevisions(
                            previous = mutationBase,
                            next = transformed,
                            mutationAt = now()
                        ).sanitized()
                        when {
                            noteMutationId != null -> current.acceptNoteMutation(normalized, noteMutationId)"####;
    let next = r####"                        val normalizeAt = now()
                        val normalizeStarted = System.nanoTime()
                        val scopedCreated = hierarchyCreateNoteId?.let { createdId ->
                            current.scopedKnowledgeCreateOrNull(transformed, createdId, normalizeAt)
                        }
                        val normalized = scopedCreated ?: stampChangedFieldRevisions(
                            previous = mutationBase,
                            next = transformed,
                            mutationAt = normalizeAt
                        ).sanitized()
                        if (hierarchyCreateNoteId != null) {
                            recordNoteSaveStage("hierarchy_normalize", normalizeStarted)
                            logDiagnosticEvent(
                                category = "notes.local_save.path",
                                message = if (scopedCreated != null) "hierarchy_create_scoped"
                                    else "hierarchy_create_full_fallback"
                            )
                        }
                        when {
                            hierarchyCreateNoteId != null -> normalized.reuseUnchangedDomains(current)
                            noteMutationId != null -> current.acceptNoteMutation(normalized, noteMutationId)"####;
    replace_one(&mut source, previous, next)?;
    replace_one(
        &mut source,
        "    // Sanitization remains authoritative.",
        &format!("{SCOPED_HELPER}    // Sanitization remains authoritative."),
    )?;
    install_commit_receipts(&mut source)?;
    Ok(source)
}

fn install_commit_receipts(source: &mut String) -> Result<(), String> {
    replace_one(source, "            var transformFailure: DataUpdateAttempt? = null",
        "            var deferRecoveryMirrors = false\n            var transformFailure: DataUpdateAttempt? = null")?;
    replace_one(source, "                        val normalized = scopedCreated ?:",
        "                        deferRecoveryMirrors = scopedCreated?.notes?.firstOrNull { it.id == hierarchyCreateNoteId }?.isPlainRecoveryDocument() == true\n                        val normalized = scopedCreated ?:")?;
    replace_one(source, "                            noteMutationId != null -> current.acceptNoteMutation(normalized, noteMutationId)",
        r#"                            noteMutationId != null -> current.acceptNoteMutation(normalized, noteMutationId).also {
                                deferRecoveryMirrors = mutationBase.notes.singleOrNull()?.isPlainRecoveryDocument() == true &&
                                    normalized.notes.singleOrNull()?.isPlainRecoveryDocument() == true
                            }"#)?;
    for (before, after) in [
        ("val persistAttempt = persistSafelyDetailed(updated)", "val persistAttempt = persistSafelyDetailed(updated, deferRecoveryMirrors)"),
        ("fun persistSafelyDetailed(appData: AppData):", "fun persistSafelyDetailed(appData: AppData, deferRecoveryMirrors: Boolean = false):"),
        ("            persist(appData)\n            PersistAttempt", "            persist(appData, deferRecoveryMirrors)\n            PersistAttempt"),
        ("fun persist(appData: AppData) {", "fun persist(appData: AppData, deferRecoveryMirrors: Boolean = false) {"),
        ("        persistToWorkspace(activeWorkspaceKey, appData)", "        persistToWorkspace(activeWorkspaceKey, appData, deferRecoveryMirrors)"),
        ("fun persistToWorkspace(workspaceKey: String, appData: AppData) {", "fun persistToWorkspace(workspaceKey: String, appData: AppData, deferRecoveryMirrors: Boolean = false) {"),
        ("            val committedStamp = try {", "            val receipt = try {"),
        ("                    stateDatabase.snapshotWriteStamp()\n                }\n            } finally", "                    stamp.copy(database = stateDatabase.snapshotWriteStamp())\n                }\n            } finally"),
        ("            // Only publish the cache after commit AND successful recovery mirrors.\n            // Any failure leaves it invalid, so the next attempt checks every surface.\n            runCatching {",
         "            fun refresh(committedStamp: AppStateDatabase.SnapshotWriteStamp): Boolean = runCatching {"),
        ("            }\n            stageFinished(\"recovery_mirror\")", r#"            }.isSuccess
            pendingRecoveryMirrorJob?.cancel()
            pendingRecoveryMirrorJob = null
            if (deferRecoveryMirrors && receipt.primary.size >= 0L && receipt.backup.size >= 0L) {
                scheduleRecoveryMirrors(workspaceKey, receipt, encoded, snapshotRevision, snapshotItemCount, ::refresh)
                Log.i("NoteSaveLatency", "phase=sqlite_receipt mirrors=queued")
            } else {
                refresh(receipt.database)
                stageFinished("recovery_mirror")
            }"#),
        ("            redactEncryptedNoteRecoveryCopiesLocked(workspaceKey, protectedNote, draftCheckpoint)",
         "            pendingRecoveryMirrorJob?.cancel()\n            pendingRecoveryMirrorJob = null\n            redactEncryptedNoteRecoveryCopiesLocked(workspaceKey, protectedNote, draftCheckpoint)"),
    ] {
        replace_one(source, before, after)?;
    }
    let anchor = "    private var verifiedWritePreflight: Pair<String, PersistPreflightStamp>? = null";
    replace_one(source, anchor, &format!("{anchor}\n{RECOVERY_WORKER}"))
}

const RECOVERY_WORKER: &str = r#"
    private var pendingRecoveryMirrorJob: Job? = null

    private fun NoteEntry.isPlainRecoveryDocument(): Boolean =
        kind == NoteEntryKind.DOCUMENT && encryption == null && !isDeleted() && attachments.isEmpty() &&
            !document.richTextEnabled && !isKnowledgeFolder() && !requiresStructuredKnowledgeReader() &&
            document.blocks.all { it.type == NoteBlockType.TEXT }

    // Called under the repository writer. A new commit or privacy redaction cancels
    // the preceding ticket. On process death, verified SQLite startup repairs mirrors.
    private fun scheduleRecoveryMirrors(
        workspaceKey: String, receipt: PersistPreflightStamp, encoded: String,
        revision: Long, itemCount: Int, refresh: (AppStateDatabase.SnapshotWriteStamp) -> Boolean
    ) {
        pendingRecoveryMirrorJob = scope.launch {
            var retryMillis = 250L
            while (true) {
                delay(retryMillis)
                val finished = writeMutex.withLock {
                    if (activeWorkspaceKey != workspaceKey || futureSchemaVersion != null ||
                        persistenceWriteBlockedReason != null || loadedFromRecoveryCandidate) return@withLock true
                    runCatching {
                        stateDatabase.withSnapshotWriteTransaction {
                            val files = workspaceFiles(workspaceKey)
                            // Never overwrite a file changed by an independent recovery or privacy operation.
                            if (persistFileStamp(files.state) != receipt.primary ||
                                persistFileStamp(files.backup) != receipt.backup) return@withSnapshotWriteTransaction true
                            val current = stateDatabase.snapshotWriteStamp()
                            if (current != receipt.database &&
                                (futureSchemaVersionOnAnySurface(workspaceKey) != null ||
                                    !stateDatabase.currentSnapshotMatches(workspaceKey, encoded,
                                        APP_DATA_SCHEMA_VERSION, revision, itemCount))) return@withSnapshotWriteTransaction true
                            val start = System.nanoTime()
                            refresh(current).also { recordNoteSaveStage("recovery_mirror", start) }
                        }
                    }.getOrElse { error ->
                        Log.w(TAG, "Recovery mirror verification will retry.", error)
                        false
                    }
                }
                if (finished) return@launch
                retryMillis = (retryMillis * 2L).coerceAtMost(30_000L)
            }
        }
    }
"#;

const SCOPED_HELPER: &str = r####"    // Never trust a partial hierarchy update unless the entire original
    // workspace (except the one insertion) is byte-for-byte represented by the
    // same immutable objects. A failed guard uses full-domain normalization.
    private fun AppData.scopedKnowledgeCreateOrNull(
        candidate: AppData,
        createdId: String,
        mutationAt: Long
    ): AppData? {
        if (notes.any { it.id == createdId } || candidate.notes.size != notes.size + 1) return null
        val position = candidate.notes.indexOfFirst { it.id == createdId }
        if (position < 0 || candidate.notes.count { it.id == createdId } != 1) return null
        // The single-node path also supports folders: only the new note is
        // normalized, while every pre-existing document remains byte-for-byte intact.
        // A complete AppData comparison catches unexpected folder, privacy,
        // tombstone, ledger, or sync mutation before accepting a narrow path.
        if (candidate.copy(notes = notes) != this) return null
        for (index in notes.indices) {
            val afterIndex = if (index < position) index else index + 1
            if (candidate.notes[afterIndex] !== notes[index]) return null
        }
        val originalSlice = noteMutationData(createdId)
        val scopedInput = originalSlice.copy(notes = listOf(candidate.notes[position]))
        val normalized = stampChangedFieldRevisions(
            previous = originalSlice,
            next = scopedInput,
            mutationAt = mutationAt
        ).sanitized()
        val created = normalized.notes.singleOrNull()
            ?.takeIf { it.id == createdId && !it.isDeleted() } ?: return null
        return candidate.copy(notes = candidate.notes.toMutableList().apply {
            set(position, created)
        })
    }

"####;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scoped_create_is_guarded_and_full_fallback_is_retained() {
        assert!(SCOPED_HELPER.contains("candidate.copy(notes = notes) != this"));
        assert!(SCOPED_HELPER.contains("candidate.notes[afterIndex] !== notes[index]"));
        assert!(SCOPED_HELPER.contains("notes.any { it.id == createdId }"));
        assert!(SCOPED_HELPER.contains("normalized.notes.singleOrNull()"));
        assert!(SCOPED_HELPER.contains("return null"));
        assert!(SCOPED_HELPER.contains(".sanitized()"));
        assert!(SCOPED_HELPER.contains("return candidate.copy(notes ="));
        let previous = r####"                        val normalized = stampChangedFieldRevisions(
                            previous = mutationBase,
                            next = transformed,
                            mutationAt = now()
                        ).sanitized()
                        when {
                            noteMutationId != null -> current.acceptNoteMutation(normalized, noteMutationId)"####;
        assert!(previous.contains("stampChangedFieldRevisions"));
        assert!(!SCOPED_HELPER.contains("persistSafelyDetailed"));
    }
}
