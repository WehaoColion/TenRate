// v2.23.2.52 - Coalesce image-import snapshot writes and hash streamed bytes.
const MEDIA: &str = "com/ofairyo/gridtimer/data/NoteMediaStore.kt";
const REPOSITORY: &str = "com/ofairyo/gridtimer/data/TimerRepository.kt";
const EDITOR: &str = "com/ofairyo/gridtimer/ui/NoteDocumentEditor.kt";

fn replace_once(source: &mut String, before: &str, after: &str) -> Result<(), String> {
    let count = source.matches(before).count();
    if count != 1 {
        return Err(format!("image import performance: expected one anchor (found {count}): {}", before.lines().next().unwrap_or("")));
    }
    *source = source.replacen(before, after, 1);
    Ok(())
}

fn section<'a>(source: &'a str, start: &str, end: &str) -> Result<&'a str, String> {
    let from = source.find(start).ok_or_else(|| format!("missing image-import start: {start}"))?;
    let to = from + source[from..].find(end).ok_or_else(|| format!("missing image-import end: {end}"))?;
    Ok(&source[from..to])
}

pub fn render(path: &str, base: &str) -> Result<String, String> {
    let mut source = base.to_owned();
    match path {
        MEDIA => {
            let old = section(&source, "    internal fun importImageIntoFile(", "    internal fun recoverablePendingImports(")?.to_owned();
            let mut updated = old.clone();
            replace_once(&mut updated,
                "        try {\n            inputStreamProvider()?.use { input ->",
                "        val digestDuringCopy = MessageDigest.getInstance(\"SHA-256\")\n        try {\n            inputStreamProvider()?.use { input ->")?;
            replace_once(&mut updated,
                "                        output.write(buffer, 0, read)",
                "                        digestDuringCopy.update(buffer, 0, read)\n                        output.write(buffer, 0, read)")?;
            replace_once(&mut updated,
                "            val digest = sha256(temporary)",
                "            // Hash the bytes as they are copied; keep the post-publish digest verification.\n            val digest = digestDuringCopy.digest().joinToString(separator = \"\") { byte ->\n                \"%02x\".format(byte.toInt() and 0xff)\n            }")?;
            replace_once(&mut source, &old, &updated)?;
        }
        REPOSITORY => {
            let old = section(&source, "    suspend fun importNoteImage(", "    suspend fun deleteNoteAttachment(")?.to_owned();
            let mut updated = old.clone();
            // Reuse the existing per-note Kotlin fallback without a full AppData
            // JSON/JNI roundtrip for every imported photo.
            let from = updated.find("            val attachmentJson =").ok_or("missing attachment JSON roundtrip")?;
            let to = from + updated[from..].find("            if (!attached) {").ok_or("missing attachment validation")?;
            updated.replace_range(from..to, r####"            val transformed = data.copy(
                notes = data.notes.map { note ->
                    if (note.id != noteId) {
                        note
                    } else {
                        note.withImportedImageAttachment(
                            importedAttachment = importedAttachment,
                            updatedAtEpochMillis = timestamp
                        )?.also { attached = true } ?: note
                    }
                }
            )
"####);
            replace_once(&mut updated,
                "        var durableSnapshot: AppData? = null\n        var persistFailed = false\n",
                "")?;
            replace_once(&mut updated,
                "            if (flushPersistLocked(updated)) {\n                durableSnapshot = updated\n            } else {\n                persistFailed = true\n            }",
                "            // The blob AND its recovery journal were fsynced before publication.\n            // Defer the expensive whole-workspace snapshot until the picker batch settles.\n            scheduleImageImportPersistence(workspaceKey)")?;
            let from = updated.find("        if (persistFailed) {").ok_or("missing import persistence handler")?;
            let to = from + updated[from..].find("        return importedAttachment").ok_or("missing import completion")?;
            updated.replace_range(from..to, r####"        // Journaled imports survive interrupted or failed deferred commits.
"####);
            updated.push_str(IMAGE_PERSIST);
            replace_once(&mut source, &old, &updated)?;
        }
        EDITOR => {
            // The pending attachment renders its own status: do not also place
            // that status inside the editable caption field.
            replace_once(&mut source,
                "                attachmentId = \"pending:$operationId\",\n                caption = \"正在导入图片…\"",
                "                attachmentId = \"pending:$operationId\",\n                caption = \"\"")?;
        }
        _ => {}
    }
    Ok(source)
}

const IMAGE_PERSIST: &str = r####"    // One recovery-journal cleanup per picker batch, only after a successful
    // same-workspace snapshot. Failed/superseded commits retain journals.
    private var pendingImageImportFinalizer: kotlinx.coroutines.Job? = null

    private fun scheduleImageImportPersistence(workspaceKey: String) {
        schedulePersist()
        pendingImageImportFinalizer?.cancel()
        pendingImageImportFinalizer = scope.launch {
            pendingPersistJob?.join()
            val committed = writeMutex.withLock {
                if (activeWorkspaceKey == workspaceKey && dirtySnapshot == null) {
                    lastPersistedSnapshot
                } else {
                    null
                }
            }
            if (committed != null) {
                withContext(Dispatchers.IO) {
                    runCatching {
                        NoteMediaStore.finalizeDurableImports(
                            context = appContext,
                            workspaceKey = workspaceKey,
                            durableData = committed
                        )
                    }.onFailure { failure ->
                        logDiagnosticEvent(
                            category = "notes.attachment.import.journal",
                            message = "Imported metadata committed; journal cleanup deferred.",
                            throwable = failure
                        )
                    }
                }
            }
        }
    }

"####;

#[cfg(test)]
mod tests {
    use super::*;

    fn original(path: &str) -> &'static str {
        super::super::kotlin_sources::SOURCES.iter()
            .find(|entry| entry.path == path).expect("Kotlin source template").contents
    }

    #[test]
    fn streamed_digest_retains_synced_publication_and_readback() {
        let media = render(MEDIA, original(MEDIA)).unwrap();
        let imported = section(&media, "    internal fun importImageIntoFile(", "    internal fun recoverablePendingImports(").unwrap();
        assert!(imported.contains("digestDuringCopy.update(buffer, 0, read)"));
        assert!(!imported.contains("val digest = sha256(temporary)"));
        assert!(imported.contains("output.fd.sync()"));
        assert!(imported.contains("beforePublish(attachment)"));
        assert!(imported.contains("AtomicFileStore.replaceWithSyncedFile(targetFile, temporary)"));
        assert!(imported.contains("sha256(targetFile) == digest"));
        assert!(imported.contains("MAX_IMPORTED_IMAGE_BYTES"));
    }

    #[test]
    fn metadata_is_scoped_debounced_and_crash_recoverable() {
        let repo = render(REPOSITORY, original(REPOSITORY)).unwrap();
        let imported = section(&repo, "    suspend fun importNoteImage(", "    suspend fun deleteNoteAttachment(").unwrap();
        assert!(imported.contains("withImportedImageAttachment("));
        assert!(!imported.contains("importNoteImageAttachmentAppDataJson("));
        assert!(imported.contains("writeMutex.withLock"));
        assert!(imported.contains("scheduleImageImportPersistence(workspaceKey)"));
        assert!(!imported.contains("flushPersistLocked(updated)"));
        assert!(imported.contains("pendingPersistJob?.join()"));
        assert!(imported.contains("dirtySnapshot == null"));
        assert!(imported.contains("lastPersistedSnapshot"));
        assert!(imported.contains("finalizeDurableImports("));
        assert!(imported.contains("activeWorkspaceKey == workspaceKey"));
        assert!(imported.contains("schedulePersistRetry") == false);
    }

    #[test]
    fn document_shows_only_one_import_progress_label() {
        let editor = render(EDITOR, original(EDITOR)).unwrap();
        assert!(editor.contains("attachmentId = \"pending:$operationId\",\n                caption = \"\""));
        assert!(editor.contains("\"正在导入图片…\""));
    }

    #[test]
    fn unrelated_sources_are_unchanged() {
        assert_eq!(render("other.kt", "unchanged").unwrap(), "unchanged");
    }
}
