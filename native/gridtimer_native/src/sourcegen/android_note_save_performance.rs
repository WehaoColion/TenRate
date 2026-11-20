// v3.0.2.6 - Queue final sticky draft checkpoints before the SQLite commit.
// v2.22.49.11 - Speed up Android note draft and local snapshot saves.
const REPOSITORY: &str = "com/ofairyo/gridtimer/data/TimerRepository.kt";
const EDITOR: &str = "com/ofairyo/gridtimer/ui/SmartisanNoteUi.kt";

pub fn render(path: &str, base: &str) -> Result<String, String> {
    let mut source = base.to_owned();
    match path {
        REPOSITORY => {
            replace_once(
                &mut source,
                "            val encoded = strictPersistedJson.encodeToString(\n                appData.copy(schemaVersion = APP_DATA_SCHEMA_VERSION)\n            )\n            val encodedCandidate = checkNotNull(decodePersistedCandidate(\"pending-write\", encoded)) {\n                \"Refusing to persist an app-data snapshot that cannot be decoded.\"\n            }",
                "            // The value is already a typed AppData. Decoding the complete JSON\n            // again on every save only repeats work done by the serializer.\n            val snapshot = appData.copy(schemaVersion = APP_DATA_SCHEMA_VERSION)\n            val encoded = strictPersistedJson.encodeToString(snapshot)\n            val snapshotRevision = appDataRevisionMillis(snapshot)\n            val snapshotItemCount = persistedItemCount(snapshot)",
            )?;
            replace_once(
                &mut source,
                "revision = encodedCandidate.revision,",
                "revision = snapshotRevision,",
            )?;
            replace_once(
                &mut source,
                "itemCount = encodedCandidate.itemCount,",
                "itemCount = snapshotItemCount,",
            )?;
            replace_once(
                &mut source,
                "    private var verifiedWritePreflight: Pair<String, PersistPreflightStamp>? = null",
                MIRROR_CACHE_FIELD,
            )?;
            replace_once(&mut source, MIRROR_REFRESH_OLD, MIRROR_REFRESH_NEW)?;
        }
        EDITOR => {
            replace_once(
                &mut source,
                "import kotlinx.coroutines.launch\nimport kotlinx.coroutines.withContext",
                "import kotlinx.coroutines.launch\nimport kotlinx.coroutines.sync.Mutex\nimport kotlinx.coroutines.sync.withLock\nimport kotlinx.coroutines.withContext",
            )?;
            replace_once(
                &mut source,
                "    var journalWriteFailed by remember(note.id) { mutableStateOf(false) }",
                "    var journalWriteFailed by remember(note.id) { mutableStateOf(false) }\n    val journalWriteMutex = remember(note.id) { Mutex() }\n    val pendingJournalWrite = remember(note.id) { java.util.concurrent.atomic.AtomicReference<kotlinx.coroutines.Job?>(null) }",
            )?;
            replace_once(
                &mut source,
                "    var latestJournalCheckpoint by remember(note.id) {\n        mutableStateOf<NoteDraftJournalCheckpoint?>(null)\n    }\n",
                "",
            )?;
            let start = source
                .find("    fun journalLatestDraft(\n")
                .ok_or("missing note journal helper")?;
            let end = source[start..]
                .find("    fun currentEditSnapshot():")
                .map(|offset| start + offset)
                .ok_or("missing note journal helper end")?;
            source.replace_range(start..end, JOURNAL_HELPER);
            let checkpoint = source
                .find("        val checkpointForAttempt = journalLatestDraft(")
                .ok_or("missing sticky exit checkpoint")?;
            let completion = checkpoint + source[checkpoint..]
                .find("        val completePersistence:")
                .ok_or("missing sticky exit completion")?;
            source.replace_range(checkpoint..completion, "");
            replace_once(&mut source, STICKY_SAVE_OLD, STICKY_SAVE)?;
            replace_once(
                &mut source,
                "                viewModel.upsertNoteAndFlush(\n                    note = previewNote,\n                    expectedWorkspaceKey = editorWorkspaceKey,\n                    reason = \"encrypted_sticky_lock\",\n                    draftCheckpoint = latestJournalCheckpoint\n                ) { persisted ->",
                "                saveCapturedSticky(previewNote, \"encrypted_sticky_lock\", requireCheckpoint = false) { result ->\n                    val persisted = result.committed",
            )?;
        }
        _ => {}
    }
    Ok(source)
}

fn replace_once(source: &mut String, old: &str, new: &str) -> Result<(), String> {
    if source.matches(old).count() != 1 {
        return Err(format!("note save anchor missing or repeated: {old}"));
    }
    *source = source.replacen(old, new, 1);
    Ok(())
}

const STICKY_SAVE_OLD: &str = r####"            SmartisanNoteExitAction.SaveAndFlush -> viewModel.upsertNoteAndFlushResult(
                note = candidate,
                expectedWorkspaceKey = editorWorkspaceKey,
                reason = reason,
                draftCheckpoint = checkpointForAttempt,
                onComplete = completeNotePersistence
            )"####;

const STICKY_SAVE: &str = r####"            SmartisanNoteExitAction.SaveAndFlush ->
                saveCapturedSticky(candidate, reason, onComplete = completeNotePersistence)"####;

const JOURNAL_HELPER: &str = r####"    fun saveCapturedSticky(
        draft: NoteEntry,
        reason: String,
        requireCheckpoint: Boolean = true,
        onComplete: (com.ofairyo.gridtimer.data.NoteSaveResult) -> Unit
    ) {
        val sequence = ++nextJournalSequence
        pendingJournalWrite.getAndSet(null)?.cancel()
        viewModel.upsertCapturedNoteAndFlushResult(
            note = draft,
            baseNote = editorBaseNote,
            expectedWorkspaceKey = editorWorkspaceKey,
            reason = reason,
            editorSessionId = editorSessionId,
            sequence = sequence,
            requireCheckpoint = requireCheckpoint,
            draftWriteMutex = journalWriteMutex,
            onComplete = onComplete
        )
    }

    fun journalLatestDraft(
        titleOverride: String = titleText,
        plainBodyOverride: String = if (richTextEnabled) richPlainBody else bodyFieldValue.text,
        richBodyHtmlOverride: String = richBodyHtml,
        richTextEnabledOverride: Boolean = richTextEnabled
    ) {
        nextJournalSequence += 1L
        val sequence = nextJournalSequence
        val base = editorBaseNote
        val draft = base.toSmartisanDraftNote(
            title = titleOverride,
            plainBody = plainBodyOverride,
            richBodyHtml = richBodyHtmlOverride,
            richTextEnabled = richTextEnabledOverride,
            accentSeed = accentSeed,
            folderId = folderId,
            pinned = pinned,
            attachments = visibleAttachments
        )
        val write = {
            viewModel.journalNoteDraft(
                workspaceKey = editorWorkspaceKey,
                baseNote = base,
                note = draft,
                editorSessionId = editorSessionId,
                sequence = sequence
            )
        }
        fun publish(checkpoint: NoteDraftJournalCheckpoint?) {
            if (checkpoint == null) {
                if (!journalWriteFailed) {
                    localizedToast(
                        context,
                        "本地草稿暂未落盘，退出时会再次保存。",
                        Toast.LENGTH_LONG
                    ).show()
                }
                journalWriteFailed = true
            } else {
                journalWriteFailed = false
            }
        }
        // An edit never performs a file read, fsync, and readback on the IME thread.
        // Cancellation removes queued older edits; the journal's sequence guard also
        // rejects an older write that finishes after a newer one.
        pendingJournalWrite.getAndSet(null)?.cancel()
        pendingJournalWrite.set(scope.launch(Dispatchers.IO) {
            val checkpoint = journalWriteMutex.withLock { write() }
            withContext(Dispatchers.Main.immediate) {
                if (nextJournalSequence == sequence) publish(checkpoint)
            }
        })
    }

"####;

const MIRROR_CACHE_FIELD: &str = r####"    private var verifiedWritePreflight: Pair<String, PersistPreflightStamp>? = null
    // A successful mirror write already proved these bytes and revisions. Stamps
    // invalidate the cache whenever another process or recovery path edits a file.
    private data class VerifiedMirrorCache(
        val workspaceKey: String,
        val primaryStamp: PersistFileStamp,
        val backupStamp: PersistFileStamp,
        val primaryRaw: String,
        val primaryRevision: Long,
        val backupRevision: Long?
    )
    private var verifiedMirrorCache: java.lang.ref.WeakReference<VerifiedMirrorCache>? = null
    // Bounded strong retention avoids repeated decode of frequently saved small
    // mirrors. Larger workspaces keep the existing weak-reference memory policy.
    private var retainedMirrorCache: VerifiedMirrorCache? = null"####;

const MIRROR_REFRESH_OLD: &str = r####"                val primary = readPersistedCandidate(files.state)
                val backup = readPersistedCandidate(files.backup)
                var backupReady = backup != null
                if (primary != null) {
                    val backupWinner = chooseSaferPersistedCandidate(primary, backup)
                    if (backupWinner === primary && backup?.raw != primary.raw) {
                        AtomicFileStore.writeText(files.backup, primary.raw)
                        backupReady = true
                    }
                }
                AtomicFileStore.writeText(files.state, encoded)
                if (!backupReady) {
                    AtomicFileStore.writeText(files.backup, encoded)
                }
                verifiedWritePreflight = workspaceKey to PersistPreflightStamp(
                    committedStamp, persistFileStamp(files.state), persistFileStamp(files.backup)
                )"####;

const MIRROR_REFRESH_NEW: &str = r####"                val mirrorCache = (retainedMirrorCache ?: verifiedMirrorCache?.get())?.takeIf { cache ->
                    cache.workspaceKey == workspaceKey &&
                        cache.primaryStamp == persistFileStamp(files.state) &&
                        cache.backupStamp == persistFileStamp(files.backup)
                }
                retainedMirrorCache = null
                verifiedMirrorCache = null
                val primary = if (mirrorCache == null) readPersistedCandidate(files.state) else null
                val backup = if (mirrorCache == null) readPersistedCandidate(files.backup) else null
                val previousRaw = mirrorCache?.primaryRaw ?: primary?.raw
                val previousRevision = mirrorCache?.primaryRevision ?: primary?.revision
                var backupRevision = mirrorCache?.backupRevision ?: backup?.revision
                var backupReady = backupRevision != null
                val primaryWins = if (mirrorCache != null) {
                    previousRevision != null &&
                        (backupRevision == null || previousRevision >= backupRevision)
                } else {
                    primary != null && chooseSaferPersistedCandidate(primary, backup) === primary
                }
                if (primaryWins && previousRaw != null &&
                    (mirrorCache != null || backup?.raw != previousRaw)) {
                    AtomicFileStore.writeText(files.backup, previousRaw)
                    backupRevision = previousRevision
                    backupReady = true
                }
                AtomicFileStore.writeText(files.state, encoded)
                if (!backupReady) {
                    AtomicFileStore.writeText(files.backup, encoded)
                    backupRevision = snapshotRevision
                }
                val primaryStamp = persistFileStamp(files.state)
                val backupStamp = persistFileStamp(files.backup)
                val nextMirrorCache = VerifiedMirrorCache(
                    workspaceKey, primaryStamp, backupStamp, encoded, snapshotRevision, backupRevision
                )
                verifiedMirrorCache = java.lang.ref.WeakReference(nextMirrorCache)
                retainedMirrorCache = nextMirrorCache.takeIf { encoded.length <= 1_000_000 }
                verifiedWritePreflight = workspaceKey to PersistPreflightStamp(
                    committedStamp, primaryStamp, backupStamp
                )"####;
