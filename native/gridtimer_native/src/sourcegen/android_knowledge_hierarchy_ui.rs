// v2.23.4 - Diagnose hierarchy save stages without changing durable write barriers.
// v2.23.3 - Show complete, compact inline child lists in the mobile knowledge page.
// Workspace-local sibling dragging in the full directory; page dates and shared data stay intact.
// Compact page controls while preserving hierarchy navigation and durable handoff.
// v2.23.2.32 - Use the knowledge hierarchy as the single page-location source.
// v2.23.2.30 - Preserve hierarchy handoff and blank-child guards on untouched exits.
// v2.23.2.29 - Navigate and edit knowledge containers without replacing page content.
const STUDIO: &str = "com/ofairyo/gridtimer/ui/NoteStudioSheet.kt";
const EDITOR: &str = "com/ofairyo/gridtimer/ui/NoteDocumentEditor.kt";
pub const PATH: &str = "com/ofairyo/gridtimer/ui/KnowledgeHierarchyUi.kt";

fn replace(source: &mut String, old: &str, new: &str) -> Result<(), String> {
    if source.matches(old).count() != 1 {
        return Err(format!(
            "hierarchy anchor is missing or repeated: {}",
            old.lines().next().unwrap_or("")
        ));
    }
    *source = source.replacen(old, new, 1);
    Ok(())
}
fn imports(source: &mut String) {
    let package = source
        .lines()
        .find(|line| line.starts_with("package "))
        .unwrap_or("")
        .to_owned();
    *source = source.replacen(&package, &format!("{package}\nimport com.ofairyo.gridtimer.data.KnowledgeHierarchyIssue\nimport com.ofairyo.gridtimer.data.KnowledgeLocation\nimport com.ofairyo.gridtimer.data.knowledgeHierarchy\nimport com.ofairyo.gridtimer.data.isKnowledgeFolder\nimport com.ofairyo.gridtimer.data.requiresStructuredKnowledgeReader"),1);
}
pub fn render(path: &str, base: &str) -> Result<String, String> {
    let mut source = base.to_owned();
    if path.ends_with("/NativeOptimizerBridge.kt") {
        replace(
            &mut source,
            "    fun createNoteFolderAppDataJson(",
            &format!("{BRIDGE}\n    fun createNoteFolderAppDataJson("),
        )?;
    } else if path.ends_with("/TimerRepository.kt") {
        replace(
            &mut source,
            "    suspend fun createNoteFolder(",
            &format!("{REPOSITORY}\n    suspend fun createNoteFolder("),
        )?;
    } else if path.ends_with("/TimerViewModel.kt") {
        replace(
            &mut source,
            "    fun createNoteFolder(",
            &format!("{VIEW_MODEL}\n    fun createNoteFolder("),
        )?;
    } else if path == STUDIO {
        imports(&mut source);
        let from = source
            .find("internal fun NoteStudioSheet(")
            .ok_or("hierarchy studio missing")?;
        let to = source[from..]
            .find("internal fun StickyNoteStudioSheet(")
            .map(|i| i + from)
            .ok_or("hierarchy studio boundary missing")?;
        let mut studio = source[from..to].to_owned();
        replace(
            &mut studio,
            "    val scope = rememberCoroutineScope()",
            &format!("    val scope = rememberCoroutineScope()\n{STUDIO_STATE}"),
        )?;
        replace(&mut studio, "            if (frame == null) {\n                NoteCollectionContent(", &format!("            if (frame == null) {{\n                if (hierarchyVisible) {{\n                    androidx.compose.runtime.CompositionLocalProvider(LocalDocumentTransitionActive provides interactiveFrame) {{\n{HIERARCHY_BROWSER}\n                    }}\n                }} else NoteCollectionContent("))?;
        replace(&mut studio, "                    onManageFolders = { if (isCurrentFrame()) folderManagerVisible = true },", "                    onManageFolders = { if (isCurrentFrame()) { hierarchyVisible = true; hierarchyParentId = null } },")?;
        replace(&mut studio,"                    appData = appData,", "                    appData = appData,\n                    onOpenHierarchy = { if (isCurrentFrame()) { hierarchyVisible = true; hierarchyParentId = null } },")?;
        replace(&mut studio,"                    note = documentNote,", "                    note = documentNote,\n                    hierarchyLocation = hierarchyIndex.location(documentNote.id),\n                    hasHierarchyChildren = hierarchyIndex.children(documentNote.id).isNotEmpty(),\n                    hierarchyContent = { rawNavigate ->\n                        val navigate: (() -> Unit) -> Unit = { action ->\n                            if (isCurrentFrame() && frameAccess.mayInteract()) rawNavigate {\n                                if (isCurrentFrame() && frameAccess.mayInteract()) action()\n                            }\n                        }\n                        KnowledgeHierarchyPanel(index = hierarchyIndex, parentId = documentNote.id,\n                            busy = hierarchyBusy, onBrowse = { parent -> navigate {\n                                hierarchyParentId = parent; hierarchyVisible = true; selectedNoteId = null; pendingCreatedNote = null\n                            } }, onOpen = { node -> navigate {\n                                if (node.isFolder) { hierarchyParentId = node.id; hierarchyVisible = true; selectedNoteId = null }\n                                else node.note?.let { child ->\n                                    if (child.isEncryptionLocked()) { hierarchyParentId = documentNote.id; hierarchyVisible = true; selectedNoteId = null; pendingCreatedNote = null; unlockedNote = null; unlockTarget = child; unlockError = \"\" }\n                                    else { hierarchyParentId = hierarchyIndex.parentOf(child.id); hierarchyVisible = true; selectedNoteId = child.id; pendingCreatedNote = null; unlockedNote = null }\n                                }\n                            } }, onRequest = { request -> navigate { hierarchyRequest = request; hierarchyParentId = documentNote.id; hierarchyVisible = true; selectedNoteId = null; pendingCreatedNote = null } })\n                    },\n                    onHierarchyLocationChosen = { _ ->\n                        if (isCurrentFrame() && frameAccess.mayInteract()) {\n                            val currentLocation = viewModel.appData.value.knowledgeHierarchy().location(documentNote.id)\n                            hierarchyRequest = KnowledgeHierarchyRequest(\"move\", documentNote.id, currentLocation.parentId); hierarchyParentId = documentNote.id; hierarchyVisible = true; selectedNoteId = null; pendingCreatedNote = null\n                        }\n                    },")?;
        // The request owns a workspace-scoped callback. A stale callback cannot select another account's page.
        let end = "    unlockTarget?.let { target ->";
        replace(&mut studio, end, &format!("{STUDIO_DIALOG}\n{end}"))?;
        source.replace_range(from..to, &studio);
        replace(&mut source,"private fun NoteCollectionContent(\n    accent: Color,", "private fun NoteCollectionContent(\n    onOpenHierarchy: () -> Unit,\n    accent: Color,")?;
        replace(&mut source,"            item(key = \"knowledge-navigation-entry\") {", "            item(key = \"knowledge-hierarchy-entry\") {\n                androidx.compose.material3.OutlinedButton(onClick = onOpenHierarchy, modifier = Modifier.fillMaxWidth().heightIn(min = 48.dp)) {\n                    Text(\"目录\"); Spacer(Modifier.weight(1f)); Text(\"页面与文件夹\")\n                }\n            }\n            item(key = \"knowledge-navigation-entry\") {")?;
    } else if path == EDITOR {
        imports(&mut source);
        replace(
            &mut source,
            "    if (note.hasStructuredKnowledge()) {",
            "    if (note.requiresStructuredKnowledgeReader()) {",
        )?;
        replace(&mut source,"    onAskKnowledge: (NoteEntry) -> Unit = {}", "    onAskKnowledge: (NoteEntry) -> Unit = {},\n    hierarchyContent: @Composable ((() -> Unit) -> Unit) -> Unit = {},\n    onHierarchyLocationChosen: (String?) -> Unit = {},\n    hierarchyLocation: KnowledgeLocation? = null,\n    hasHierarchyChildren: Boolean = false")?;
        replace(&mut source,"        }, onPreviewImage = onRequestPreviewAttachment)", "        }, onPreviewImage = onRequestPreviewAttachment, hierarchyContent = hierarchyContent)")?;
        replace(&mut source, "    val documentTransitionActive = LocalDocumentTransitionActive.current", "    val documentTransitionActive = LocalDocumentTransitionActive.current\n    var hierarchyHandedOff by remember(note.id, LocalNoteMediaWorkspaceKey.current) { mutableStateOf(false) }")?;
        replace(&mut source, "        if (isSavingDraft || pendingRevisionRestore != null) return@LaunchedEffect", "        if (hierarchyHandedOff || isSavingDraft || pendingRevisionRestore != null) return@LaunchedEffect")?;
        replace(&mut source, "    ) {\n        // An unchanged visit owns no new write.", "    ) {\n        if (hierarchyHandedOff) { onComplete(true); return }\n        // An unchanged visit owns no new write.")?;
        replace(&mut source, "        val visitNeedsBlankDeletion = deleteBlankDraft && blankDraftDeletionEligibleAtEntry &&", "        val visitNeedsBlankDeletion = deleteBlankDraft && blankDraftDeletionEligibleAtEntry && !hasHierarchyChildren && latestNote.document.knowledge == null &&")?;
        replace(&mut source, "            blankDraftDeletionEligibleAtEntry = note.isBlankDraft()", "            blankDraftDeletionEligibleAtEntry = note.isBlankDraft() && note.document.knowledge == null && !hasHierarchyChildren")?;
        replace(&mut source, "        val needsBlankDeletion = deleteBlankDraft &&", "        val needsBlankDeletion = deleteBlankDraft && !hasHierarchyChildren && candidate.document.knowledge == null &&")?;
        replace(&mut source, "            deleteBlankDraft = deleteBlankDraft && blankDraftDeletionEligibleAtEntry", "            deleteBlankDraft = deleteBlankDraft && blankDraftDeletionEligibleAtEntry && !hasHierarchyChildren && candidate.document.knowledge == null")?;
        // Every regular-editor snapshot retains rich/knowledge metadata from the authoritative record.
        // This covers autosave, cancelled imports, preview and durable navigation.
        source = source.replace(
            "val document = NoteDocument(",
            "val document = latestNote.document.copy(",
        );
        replace(
            &mut source,
            "    fun undoDraft() {",
            &format!("{EDITOR_NAVIGATION}\n    fun undoDraft() {{"),
        )?;
        replace(&mut source,"        item(key = \"document-editor-properties\") {", "        item(key = \"document-editor-hierarchy\") { hierarchyContent(::navigateHierarchy) }\n        item(key = \"document-editor-properties\") {")?;
        replace(
            &mut source,
            "                if (previewNote.encryption == null) encryptDialogVisible = true",
            r####"                if (previewNote.encryption == null && folders.any { it.id == previewNote.id }) {
                    localizedToast(context, "从旧文件夹转换的记录暂不支持加密，原内容已保留。", Toast.LENGTH_LONG).show()
                } else if (previewNote.encryption == null && (previewNote.isKnowledgeFolder() || hasHierarchyChildren ||
                    (previewNote.document.knowledge?.get("parentId") as? kotlinx.serialization.json.JsonPrimitive)?.content.orEmpty().isNotBlank())) {
                    localizedToast(context, "先将子项移出、转为页面并移到知识目录根级，再加密。", Toast.LENGTH_LONG).show()
                } else if (previewNote.encryption == null) encryptDialogVisible = true"####,
        )?;
        // Folder selection now commits through the hierarchy action. Text edits must
        // retain the current repository location if another device moved the page.
        let location_anchor =
            "                pinned = pinned,\n                folderId = folderId";
        let other_location_anchor = "            pinned = pinned,\n            folderId = folderId";
        if source.matches(location_anchor).count() + source.matches(other_location_anchor).count()
            != 4
        {
            return Err("hierarchy editor snapshot locations missing".into());
        }
        source = source
            .replace(
                location_anchor,
                "                pinned = pinned,\n                folderId = latestNote.folderId",
            )
            .replace(
                other_location_anchor,
                "            pinned = pinned,\n            folderId = latestNote.folderId",
            );
        // The breadcrumb is the sole location summary. The properties dialog opens
        // the complete hierarchy picker instead of maintaining a second legacy list.
        replace(&mut source,
            "                folderLabel = folders.firstOrNull { it.id == folderId }?.name ?: \"不放文件夹\",\n",
            "                enabled = documentTransitionActive && !isSavingDraft && pendingRevisionRestore == null,\n")?;
        replace(
            &mut source,
            "private fun DocumentPropertyStrip(\n    folderLabel: String,",
            "private fun DocumentPropertyStrip(\n    enabled: Boolean,",
        )?;
        replace(
            &mut source,
            "        DocumentMetaChip(folderLabel, accent, onOpenProperties)\n",
            "",
        )?;
        replace(&mut source,
            "        DocumentMetaChip(\"＋ 属性\", MaterialTheme.colorScheme.onSurface.copy(alpha = 0.68f), onOpenProperties)",
            "        androidx.compose.material3.OutlinedButton(enabled = enabled, onClick = onOpenProperties, modifier = Modifier.heightIn(min = 48.dp)) { Text(\"属性\") }")?;
        replace(&mut source,
            "            folders = folders,\n            folderId = folderId,\n            accentSeed = accentSeed,",
            "            location = hierarchyLocation,\n            canChangeLocation = documentTransitionActive && !isSavingDraft && pendingRevisionRestore == null && hierarchyLocation?.canMove == true,\n            accentSeed = accentSeed,")?;
        replace(&mut source,
            "            onFolderChange = { recordChange(currentDraftState().copy(folderId = it)) },",
            "            onChangeLocation = {\n                val location = hierarchyLocation\n                if (location?.canMove == true && documentTransitionActive && !isSavingDraft && pendingRevisionRestore == null) {\n                    documentPropertiesVisible = false\n                    navigateHierarchy { onHierarchyLocationChosen(location.parentId) }\n                }\n            },")?;
        replace(&mut source,
            "private fun DocumentPropertyDialog(\n    folders: List<NoteFolder>,\n    folderId: String?,",
            "private fun DocumentPropertyDialog(\n    location: KnowledgeLocation?,\n    canChangeLocation: Boolean,")?;
        replace(&mut source,
            "    onFolderChange: (String?) -> Unit,\n    onAccentChange: (String) -> Unit\n) {\n    Dialog(onDismissRequest = onDismiss)",
            "    onChangeLocation: () -> Unit,\n    onAccentChange: (String) -> Unit\n) {\n    Dialog(onDismissRequest = onDismiss)")?;
        replace(&mut source, LEGACY_LOCATION_PROPERTY, LOCATION_PROPERTY)?;
        replace(
            &mut source,
            "\"属性\", \"文件夹与页面色调\"",
            "\"属性\", \"位置与页面色调\"",
        )?;
        source.push_str(LOCATION_PRESENTATION);
        compact_editor_toolbar(&mut source)?;
    } else if path.ends_with("/StructuredKnowledgeReader.kt") {
        replace(&mut source,"    onPreviewImage: (String) -> Unit\n", "    hierarchyContent: @Composable ((() -> Unit) -> Unit) -> Unit = {},\n    onPreviewImage: (String) -> Unit\n")?;
        replace(&mut source,"            item(key = \"structured_header\") {", "            item(key = \"structured_hierarchy\") { hierarchyContent { action ->\n                if (transitionActive && !saving && draft == null && !insertOpen && latestWorkspace == workspace) action()\n            } }\n            item(key = \"structured_header\") {")?;
        replace(&mut source,"                    (page?.get(\"properties\") as? JsonObject)?.forEach { (key, value) ->", "                    (page?.get(\"properties\") as? JsonObject)?.filterKeys { it !in setOf(\"__tenrate_container_kind\", \"__tenrate_legacy_folder\") }?.forEach { (key, value) ->")?;
    }
    Ok(source)
}

fn compact_editor_toolbar(source: &mut String) -> Result<(), String> {
    replace(source,
        "                enabled = documentTransitionActive && !isSavingDraft && pendingRevisionRestore == null,\n                pinned = pinned,",
        "                enabled = documentTransitionActive && !isSavingDraft && pendingRevisionRestore == null,\n                hasPreviewSwitch = markdownPreviewAvailable,\n                showingPreview = showingMarkdownPreview,\n                onEdit = { if (documentTransitionActive && !isSavingDraft && pendingRevisionRestore == null) beginEditing(requestBodyFocus = true) },\n                onPreview = { if (documentTransitionActive && !isSavingDraft && pendingRevisionRestore == null) beginMarkdownPreview() },\n                pinned = pinned,")?;
    let preview_start = source.find("        if (markdownPreviewAvailable) {\n            item(key = \"document-editor-preview-switch\") {")
        .ok_or("compact editor preview switch missing")?;
    let preview_end = source[preview_start..]
        .find("        if (showingMarkdownPreview) {")
        .map(|offset| preview_start + offset)
        .ok_or("compact editor preview boundary missing")?;
    source.replace_range(preview_start..preview_end, "");
    let strip_start = source
        .find("@Composable\nprivate fun DocumentPropertyStrip(")
        .ok_or("compact property strip missing")?;
    let strip_end = source[strip_start..]
        .find("@Composable\nprivate fun DocumentMetaChip(")
        .map(|offset| strip_start + offset)
        .ok_or("compact property strip boundary missing")?;
    source.replace_range(strip_start..strip_end, COMPACT_PROPERTY_STRIP);
    Ok(())
}

const COMPACT_PROPERTY_STRIP: &str = r####"@Composable
private fun DocumentPropertyStrip(
    enabled: Boolean,
    hasPreviewSwitch: Boolean,
    showingPreview: Boolean,
    onEdit: () -> Unit,
    onPreview: () -> Unit,
    pinned: Boolean,
    encrypted: Boolean,
    accent: Color,
    onOpenProperties: () -> Unit
) {
    Row(
        modifier = Modifier.fillMaxWidth().horizontalScroll(rememberScrollState()),
        horizontalArrangement = Arrangement.spacedBy(6.dp),
        verticalAlignment = Alignment.CenterVertically
    ) {
        androidx.compose.material3.TextButton(enabled = enabled, onClick = onOpenProperties,
            modifier = Modifier.heightIn(min = 48.dp)) { Text("属性", maxLines = 1, softWrap = false) }
        if (hasPreviewSwitch) {
            androidx.compose.material3.FilterChip(selected = !showingPreview, enabled = enabled,
                onClick = onEdit, modifier = Modifier.heightIn(min = 48.dp),
                colors = androidx.compose.material3.FilterChipDefaults.filterChipColors(
                    selectedContainerColor = accent.copy(alpha = 0.12f), selectedLabelColor = accent),
                label = { Text("编辑原文", maxLines = 1, softWrap = false) })
            androidx.compose.material3.FilterChip(selected = showingPreview, enabled = enabled,
                onClick = onPreview, modifier = Modifier.heightIn(min = 48.dp),
                colors = androidx.compose.material3.FilterChipDefaults.filterChipColors(
                    selectedContainerColor = accent.copy(alpha = 0.12f), selectedLabelColor = accent),
                label = { Text("公式预览", maxLines = 1, softWrap = false) })
        }
        if (pinned) androidx.compose.material3.TextButton(enabled = enabled, onClick = onOpenProperties,
            modifier = Modifier.heightIn(min = 48.dp)) { Text("已置顶", maxLines = 1, softWrap = false) }
        if (encrypted) androidx.compose.material3.TextButton(enabled = enabled, onClick = onOpenProperties,
            modifier = Modifier.heightIn(min = 48.dp)) { Text("已加密", maxLines = 1, softWrap = false) }
    }
}

"####;

const BRIDGE: &str = r####"    fun knowledgeContainerActionAppDataJson(raw: String, action: String, id: String,
        parentId: String, title: String, now: Long): String? {
        if (!nativeAvailable) return null
        return runCatching { nativeKnowledgeContainerActionAppDataJson(raw, action, id, parentId, title, now) }.getOrNull()
    }
    private external fun nativeKnowledgeContainerActionAppDataJson(raw: String, action: String,
        id: String, parentId: String, title: String, now: Long): String?
"####;
const REPOSITORY: &str = r####"    internal suspend fun knowledgeContainerAction(action: String, id: String, parentId: String,
        title: String, expectedWorkspaceKey: String): Boolean {
        awaitInitialized()
        var applied = false
        val operationStarted = System.nanoTime()
        fun recordHierarchyStage(phase: String, started: Long) {
            val elapsedMs = (System.nanoTime() - started).coerceAtLeast(0L) / 1_000_000L
            if (elapsedMs >= 1_000L) {
                // Fixed phase names only. Never write a title, note ID or workspace identity.
                logDiagnosticEvent(
                    category = "notes.local_save.latency",
                    message = "phase=$phase durationMs=$elapsedMs"
                )
            }
        }
        try {
            val attempt = withContext(NonCancellable) {
                updateDataDetailed(expectedWorkspaceKey = expectedWorkspaceKey,
                    persistImmediately = true, publishAfterDurable = true) { data ->
                    val encodeStarted = System.nanoTime()
                    val originalJson = json.encodeToString(data)
                    recordHierarchyStage("hierarchy_encode", encodeStarted)
                    val nativeStarted = System.nanoTime()
                    val mutatedJson = NativeOptimizerBridge.knowledgeContainerActionAppDataJson(
                        originalJson, action, id, parentId, title, now())
                    recordHierarchyStage("hierarchy_rust_mutation", nativeStarted)
                    val decodeStarted = System.nanoTime()
                    val decoded = mutatedJson?.let { decodeNativeAppData(it) }
                    recordHierarchyStage("hierarchy_decode", decodeStarted)
                    decoded?.also { applied = true } ?: data
                }
            }
            return applied && attempt.succeeded
        } finally {
            recordHierarchyStage("hierarchy_action_total", operationStarted)
        }
    }
"####;
const VIEW_MODEL: &str = r####"    internal fun knowledgeContainerAction(action: String, id: String, parentId: String,
        title: String, expectedWorkspaceKey: String = currentWorkspaceKey(),
        onComplete: (Boolean, String) -> Unit) {
        enqueueNoteMutation(onComplete = { ok -> onComplete(ok,
            if (ok) "本地已保存" else "操作未完成。请核对当前位置、页面权限及存储后重试。") }) {
            repository.knowledgeContainerAction(action, id, parentId, title, expectedWorkspaceKey)
        }
    }
"####;
const STUDIO_STATE: &str = r####"    val hierarchyIndex = remember(appData.notes, appData.noteFolders) { appData.knowledgeHierarchy() }
    var hierarchyVisible by rememberSaveable(workspaceKey) { mutableStateOf(false) }
    var hierarchyParentId by rememberSaveable(workspaceKey) { mutableStateOf<String?>(null) }
    var hierarchyRequest by remember(workspaceKey) { mutableStateOf<KnowledgeHierarchyRequest?>(null) }
    var hierarchyBusy by remember(workspaceKey) { mutableStateOf(false) }
    var hierarchyMessage by remember(workspaceKey) { mutableStateOf("") }
    val hierarchyCallbacksAlive = remember(workspaceKey) { java.util.concurrent.atomic.AtomicBoolean(true) }
    DisposableEffect(hierarchyCallbacksAlive) { onDispose { hierarchyCallbacksAlive.set(false) } }
    LaunchedEffect(hierarchyIndex, hierarchyParentId) {
        if (hierarchyParentId != null && !hierarchyIndex.nodes.containsKey(hierarchyParentId)) hierarchyParentId = null
    }
"####;
const HIERARCHY_BROWSER: &str = r####"                    KnowledgeHierarchyBrowser(index = hierarchyIndex, parentId = hierarchyParentId,
                        currentScope = {
                            if (!hierarchyCallbacksAlive.get() || !isCurrentFrame() || !hierarchyVisible) null
                            else com.ofairyo.gridtimer.data.knowledgeDirectoryOrderScope(
                                viewModel.appData.value.knowledgeHierarchy(), viewModel.currentWorkspaceKey(), hierarchyParentId,
                                canInteract = com.ofairyo.gridtimer.data.knowledgeDirectoryCollectionMayInteract(
                                    currentFrame = isCurrentFrame(), collectionFrame = frame == null,
                                    visible = hierarchyVisible, busy = hierarchyBusy || hierarchyRequest != null))
                        },
                        busy = hierarchyBusy, message = hierarchyMessage, onBack = {
                            if (isCurrentFrame() && !hierarchyBusy) {
                                if (hierarchyParentId == null) hierarchyVisible = false
                                else hierarchyParentId = hierarchyIndex.parentOf(hierarchyParentId!!)
                            }
                        }, onBrowse = { if (isCurrentFrame() && !hierarchyBusy) hierarchyParentId = it }, onOpen = { node ->
                            if (isCurrentFrame() && !hierarchyBusy && hierarchyIndex.issues[node.id] != KnowledgeHierarchyIssue.IDENTITY_CONFLICT) node.note?.let { note ->
                                if (note.isEncryptionLocked()) { unlockTarget = note; unlockError = "" }
                                else { pendingCreatedNote = null; unlockedNote = null; selectedNoteId = note.id; searchLocateQuery = null }
                            }
                        }, onRequest = { if (isCurrentFrame() && !hierarchyBusy) hierarchyRequest = it })
"####;
const STUDIO_DIALOG: &str = r####"    hierarchyRequest?.let { request ->
        val requestLocation = request.id?.let(hierarchyIndex::location)
        KnowledgeHierarchyActionDialog(request = request, index = hierarchyIndex, busy = hierarchyBusy,
            error = hierarchyMessage, onDismiss = { if (!hierarchyBusy) { hierarchyRequest = null; hierarchyMessage = "" } },
            onSubmit = submit@ { parent, title ->
                if (!hierarchyBusy && hierarchyCallbacksAlive.get() && viewModel.currentWorkspaceKey() == workspaceKey) {
                    if (request.action == "move") {
                        val currentIndex = viewModel.appData.value.knowledgeHierarchy()
                        val currentLocation = currentIndex.location(request.id.orEmpty())
                        if (currentLocation.parentId != requestLocation?.parentId || currentLocation.issue != requestLocation?.issue ||
                            currentLocation.ancestors.map { it.id } != requestLocation?.ancestors?.map { it.id } ||
                            !currentIndex.canMoveTo(request.id.orEmpty(), parent)) {
                            hierarchyMessage = "页面位置或权限已变化，请重新选择位置。"
                            return@submit
                        }
                    }
                    hierarchyBusy = true; hierarchyMessage = ""
                    val id = request.id ?: java.util.UUID.randomUUID().toString()
                    viewModel.knowledgeContainerAction(request.action, id, parent.orEmpty(), title, workspaceKey) { ok, detail ->
                        if (hierarchyCallbacksAlive.get() && viewModel.currentWorkspaceKey() == workspaceKey) {
                            hierarchyBusy = false
                            if (ok) {
                                hierarchyRequest = null
                                if (request.action == "create_page") { hierarchyParentId = parent; hierarchyVisible = true; selectedNoteId = id }
                                if (request.action == "create_folder" || request.action == "to_folder") {
                                    hierarchyVisible = true; hierarchyParentId = id; selectedNoteId = null; pendingCreatedNote = null
                                }
                                if (request.action == "to_page") { hierarchyVisible = true; hierarchyParentId = parent; selectedNoteId = id }
                            } else hierarchyMessage = detail
                        }
                    }
                }
            })
    }
"####;
const EDITOR_NAVIGATION: &str = r####"    fun navigateHierarchy(action: () -> Unit) {
        if (!documentTransitionActive || isSavingDraft || pendingRevisionRestore != null) return
        focusManager.clearFocus(force = true); exitKeyboard?.hide()
        isSavingDraft = true
        persistLatestDraft("knowledge_hierarchy_navigation", deleteBlankDraft = false) { saved ->
            isSavingDraft = false
            if (!editorSaveCallbackActive.get() || viewModel.currentWorkspaceKey() != editorWorkspaceKey) return@persistLatestDraft
            if (saved) {
                hierarchyHandedOff = true
                editorSessionViewModel.clear(note.id, editorWorkspaceKey, editorSession)
                action()
            }
            else localizedToast(context, "尚未保存成功，保留当前页面，请重试。", Toast.LENGTH_LONG).show()
        }
    }
"####;

const LEGACY_LOCATION_PROPERTY: &str = r####"                Text("文件夹", style = MaterialTheme.typography.titleSmall.copy(fontWeight = FontWeight.SemiBold))
                FlowRow(
                    horizontalArrangement = Arrangement.spacedBy(8.dp),
                    verticalArrangement = Arrangement.spacedBy(8.dp)
                ) {
                    FlowusDocChip("不放文件夹", folderId == null, accent) { onFolderChange(null) }
                    folders.forEach { folder ->
                        FlowusDocChip(folder.name, folderId == folder.id, accentFor("blue")) {
                            onFolderChange(folder.id)
                        }
                    }
                }"####;

const LOCATION_PROPERTY: &str = r####"                Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                    Text("当前位置", style = MaterialTheme.typography.titleSmall.copy(fontWeight = FontWeight.SemiBold))
                    Text(knowledgeLocationPath(location), style = MaterialTheme.typography.bodyMedium)
                    knowledgeLocationMessage(location)?.let { detail ->
                        Text(detail, style = MaterialTheme.typography.bodySmall,
                            color = if (location?.issue != null) MaterialTheme.colorScheme.error else MaterialTheme.colorScheme.onSurfaceVariant)
                    }
                    if (!canChangeLocation && location?.canMove == true) {
                        Text("正在处理当前页面，请稍后更改位置。", style = MaterialTheme.typography.bodySmall)
                    }
                    androidx.compose.material3.OutlinedButton(
                        enabled = canChangeLocation,
                        onClick = onChangeLocation,
                        modifier = Modifier.fillMaxWidth().heightIn(min = 48.dp)
                    ) { Text("更改位置") }
                }"####;

const LOCATION_PRESENTATION: &str = r####"

internal fun knowledgeLocationPath(location: KnowledgeLocation?): String = when {
    location == null || !location.available -> "位置尚未载入"
    location.issue != null -> "上级位置待核对"
    else -> (listOf("知识目录") + location.ancestors.map { it.title.ifBlank { "无标题" } }).joinToString(" / ")
}

internal fun knowledgeLocationMessage(location: KnowledgeLocation?): String? = when {
    location == null || !location.available -> "页面位置暂不可用，请返回目录后重新打开。"
    location.issue == KnowledgeHierarchyIssue.MISSING_PARENT -> if (location.canMove) "原上级已不存在，请重新选择位置。" else "原上级已不存在；当前页面只读，暂不能更改位置。"
    location.issue == KnowledgeHierarchyIssue.INACCESSIBLE_PARENT -> if (location.canMove) "原上级当前不可访问，请重新选择位置。" else "原上级不可访问；当前页面只读，暂不能更改位置。"
    location.issue == KnowledgeHierarchyIssue.CYCLE -> if (location.canMove) "目录关系存在循环，请重新选择位置。" else "目录关系存在循环；当前页面只读，暂不能更改位置。"
    location.issue == KnowledgeHierarchyIssue.IDENTITY_CONFLICT -> if (location.canMove) "上级目录存在编号冲突，请重新选择位置。" else "记录编号冲突，暂不能更改位置。"
    !location.canMove -> "当前页面为只读、已加密或暂不支持修改，不能更改位置。"
    else -> null
}
"####;

pub const CONTENTS: &str = r####"package com.ofairyo.gridtimer.ui

import androidx.activity.compose.BackHandler
import androidx.compose.foundation.ExperimentalFoundationApi
import androidx.compose.foundation.clickable
import androidx.compose.foundation.gestures.awaitEachGesture
import androidx.compose.foundation.gestures.awaitFirstDown
import androidx.compose.foundation.gestures.awaitLongPressOrCancellation
import androidx.compose.foundation.gestures.drag
import androidx.compose.foundation.gestures.scrollBy
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.animation.core.tween
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.rememberScrollState
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.ui.Modifier
import androidx.compose.ui.Alignment
import androidx.compose.ui.draw.clipToBounds
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Rect
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.hapticfeedback.HapticFeedbackType
import androidx.compose.ui.input.pointer.changedToUp
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.layout.LayoutCoordinates
import androidx.compose.ui.layout.onGloballyPositioned
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.platform.LocalHapticFeedback
import androidx.compose.ui.semantics.CustomAccessibilityAction
import androidx.compose.ui.semantics.clearAndSetSemantics
import androidx.compose.ui.semantics.customActions
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.stateDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.IntOffset
import androidx.compose.ui.zIndex
import com.ofairyo.gridtimer.data.*
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.Job
import kotlinx.coroutines.delay
import kotlinx.coroutines.isActive
import kotlinx.coroutines.launch
import kotlin.math.abs
import kotlin.math.roundToInt

internal data class KnowledgeHierarchyRequest(val action: String, val id: String? = null, val parentId: String? = null)

@Composable
private fun KnowledgeBreadcrumbs(index: KnowledgeHierarchy, id: String?, onBrowse: (String?) -> Unit) {
    Row(Modifier.fillMaxWidth().horizontalScroll(rememberScrollState()), horizontalArrangement = Arrangement.spacedBy(4.dp)) {
        TextButton(onClick = { onBrowse(null) }) { Text("知识目录") }
        id?.let { index.breadcrumbs(it).forEach { node ->
            TextButton(onClick = { onBrowse(node.id) }) { Text("› " + node.title.ifBlank { "无标题" }) }
        } }
    }
}

@Composable
private fun KnowledgeHierarchyRow(node: KnowledgeHierarchyNode, index: KnowledgeHierarchy,
    modifier: Modifier = Modifier, enabled: Boolean = true,
    trailing: (@Composable () -> Unit)? = null, onOpen: () -> Unit) {
    ListItem(modifier = modifier.fillMaxWidth().clickable(enabled = enabled && index.issues[node.id] != KnowledgeHierarchyIssue.IDENTITY_CONFLICT, onClick = onOpen),
        headlineContent = { Text((if (node.isFolder) "▣ " else "▤ ") + node.title.ifBlank { "无标题" }) },
        supportingContent = { Text(when {
            index.issues[node.id] == KnowledgeHierarchyIssue.IDENTITY_CONFLICT -> "记录编号冲突，已保护原页面和旧文件夹，暂停修改"
            node.note?.encryption != null -> "加密页面"
            index.issues.containsKey(node.id) -> "上级不可用，已放在目录中"
            !node.writable -> "只读 · ${index.children(node.id).size} 个子项"
            else -> "${if (node.isFolder) "文件夹" else "页面"} · ${index.children(node.id).size} 个子项"
        }) }, trailingContent = trailing ?: { Text("›") })
}

// The inline list belongs to the document's own scroll surface. Keep entries
// compact and avoid an independently scrolling list inside the document editor.
@Composable
private fun KnowledgeHierarchyInlineRow(
    node: KnowledgeHierarchyNode,
    index: KnowledgeHierarchy,
    enabled: Boolean,
    onOpen: () -> Unit
) {
    val issue = index.issues[node.id]
    val descendantCount = index.children(node.id).size
    val supporting = when {
        issue == KnowledgeHierarchyIssue.IDENTITY_CONFLICT -> "记录编号冲突，暂不能打开"
        node.note?.encryption != null -> "加密页面"
        issue != null -> "上级不可用，已放在目录中"
        !node.writable -> if (descendantCount > 0) "只读 · ${descendantCount} 个子项" else "只读"
        descendantCount > 0 -> "${descendantCount} 个子项"
        else -> null
    }
    Row(
        modifier = Modifier.fillMaxWidth()
            .heightIn(min = 52.dp)
            .clickable(enabled = enabled && issue != KnowledgeHierarchyIssue.IDENTITY_CONFLICT, onClick = onOpen)
            .padding(horizontal = 12.dp, vertical = 6.dp),
        verticalAlignment = Alignment.CenterVertically
    ) {
        Text(if (node.isFolder) "▣" else "▤", style = MaterialTheme.typography.titleMedium,
            color = MaterialTheme.colorScheme.onSurfaceVariant)
        Spacer(Modifier.width(12.dp))
        Column(Modifier.weight(1f)) {
            Text(node.title.ifBlank { "无标题" }, maxLines = 2,
                overflow = androidx.compose.ui.text.style.TextOverflow.Ellipsis,
                style = MaterialTheme.typography.bodyMedium)
            if (supporting != null) {
                Text(supporting, style = MaterialTheme.typography.labelSmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant)
            }
        }
        Spacer(Modifier.width(8.dp))
        Text("›", color = MaterialTheme.colorScheme.onSurfaceVariant)
    }
}

@OptIn(ExperimentalLayoutApi::class)
@Composable
private fun KnowledgeHierarchyActions(index: KnowledgeHierarchy, parentId: String?, busy: Boolean,
    onRequest: (KnowledgeHierarchyRequest) -> Unit, onOpenBody: (() -> Unit)? = null) {
    val node = parentId?.let(index.nodes::get)
    FlowRow(horizontalArrangement = Arrangement.spacedBy(6.dp), verticalArrangement = Arrangement.spacedBy(2.dp)) {
        OutlinedButton(enabled = !busy && index.mayCreateInside(parentId), onClick = { onRequest(KnowledgeHierarchyRequest("create_page", parentId = parentId)) }) { Text("新建子页面") }
        OutlinedButton(enabled = !busy && index.mayCreateInside(parentId), onClick = { onRequest(KnowledgeHierarchyRequest("create_folder", parentId = parentId)) }) { Text("新建子文件夹") }
        if (node != null) {
            TextButton(enabled = !busy && node.writable, onClick = { onRequest(KnowledgeHierarchyRequest("move", node.id, node.parentId)) }) { Text("移动") }
            TextButton(enabled = !busy && node.writable, onClick = { onRequest(KnowledgeHierarchyRequest("rename", node.id, node.parentId)) }) { Text("重命名") }
            TextButton(enabled = !busy && node.writable && node.note?.document?.knowledge?.get("database")?.let { it != kotlinx.serialization.json.JsonNull } != true,
                onClick = { onRequest(KnowledgeHierarchyRequest(if (node.isFolder) "to_page" else "to_folder", node.id, node.parentId)) }) {
                Text(if (node.isFolder) "转为页面" else "转为文件夹")
            }
            if (onOpenBody != null && node.note != null) TextButton(enabled = !busy && index.issues[node.id] != KnowledgeHierarchyIssue.IDENTITY_CONFLICT, onClick = onOpenBody) { Text("查看与编辑正文") }
        }
    }
}

@Composable
internal fun KnowledgeHierarchyPanel(index: KnowledgeHierarchy, parentId: String, busy: Boolean,
    onBrowse: (String?) -> Unit, onOpen: (KnowledgeHierarchyNode) -> Unit, onRequest: (KnowledgeHierarchyRequest) -> Unit) {
    val workspace = LocalNoteMediaWorkspaceKey.current
    val active = LocalDocumentTransitionActive.current
    val canInteract = active && !busy
    val children = index.children(parentId)
    val node = index.nodes[parentId]
    val canCreate = index.mayCreateInside(parentId)
    val canEdit = node?.writable == true
    val canConvert = canEdit && node?.note?.document?.knowledge?.get("database")?.let { it != kotlinx.serialization.json.JsonNull } != true
    var menu by remember(workspace, parentId) { mutableStateOf<String?>(null) }
    var expanded by rememberSaveable(workspace, parentId) { mutableStateOf(true) }
    val callbacksAlive = remember(workspace, parentId) { java.util.concurrent.atomic.AtomicBoolean(true) }
    DisposableEffect(callbacksAlive) { onDispose { callbacksAlive.set(false) } }
    val latestIndex by rememberUpdatedState(index)
    val latestCanInteract by rememberUpdatedState(canInteract)
    val latestParentId by rememberUpdatedState(parentId)
    val latestWorkspace by rememberUpdatedState(workspace)
    LaunchedEffect(canInteract, canCreate, canEdit, canConvert, node?.isFolder) { menu = null }
    fun mayInteract(): Boolean = callbacksAlive.get() && latestCanInteract &&
        latestParentId == parentId && latestWorkspace == workspace
    fun requestAction(action: String) {
        menu = null
        if (!mayInteract()) return
        val currentIndex = latestIndex
        val currentNode = currentIndex.nodes[parentId] ?: return
        val creating = action == "create_page" || action == "create_folder"
        val allowed = when (action) {
            "create_page", "create_folder" -> currentIndex.mayCreateInside(parentId)
            "move", "rename" -> currentNode.writable
            "to_page", "to_folder" -> currentNode.writable &&
                (action == "to_page") == currentNode.isFolder &&
                currentNode.note?.document?.knowledge?.get("database")?.let { it != kotlinx.serialization.json.JsonNull } != true
            else -> false
        }
        if (allowed) onRequest(KnowledgeHierarchyRequest(action,
            id = if (creating) null else currentNode.id,
            parentId = if (creating) parentId else currentNode.parentId))
    }
    Column(Modifier.fillMaxWidth(), verticalArrangement = Arrangement.spacedBy(2.dp)) {
        Row(Modifier.fillMaxWidth().horizontalScroll(rememberScrollState()),
            verticalAlignment = Alignment.CenterVertically) {
            TextButton(enabled = canInteract, onClick = { if (mayInteract()) onBrowse(null) },
                modifier = Modifier.heightIn(min = 48.dp), contentPadding = PaddingValues(horizontal = 4.dp)) {
                Text("知识目录", maxLines = 1, softWrap = false, style = MaterialTheme.typography.labelMedium)
            }
            index.breadcrumbs(parentId).forEach { crumb ->
                Text("›", color = MaterialTheme.colorScheme.onSurfaceVariant)
                TextButton(enabled = canInteract, onClick = { if (mayInteract()) onBrowse(crumb.id) },
                    modifier = Modifier.heightIn(min = 48.dp), contentPadding = PaddingValues(horizontal = 4.dp)) {
                    Text(crumb.title.ifBlank { "无标题" }, maxLines = 1, softWrap = false,
                        style = MaterialTheme.typography.labelMedium)
                }
            }
        }
        Row(Modifier.fillMaxWidth().horizontalScroll(rememberScrollState()),
            verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            TextButton(enabled = canInteract, onClick = {
                if (mayInteract()) {
                    if (children.isEmpty()) onBrowse(parentId) else expanded = !expanded
                }
            }, modifier = Modifier.heightIn(min = 48.dp).semantics {
                contentDescription = "子项，共 ${children.size} 项"
                stateDescription = if (expanded && children.isNotEmpty()) "已展开" else "已收起"
            }) { Text("子项 ${children.size}" + if (children.isEmpty()) "" else if (expanded) " ▴" else " ▾", maxLines = 1, softWrap = false) }
            Box {
                TextButton(enabled = canInteract && canCreate, onClick = {
                    if (mayInteract() && latestIndex.mayCreateInside(parentId)) menu = "create"
                }, modifier = Modifier.heightIn(min = 48.dp).semantics { contentDescription = "新建子页面或子文件夹" }) {
                    Text("新建 ▾", maxLines = 1, softWrap = false)
                }
                DropdownMenu(expanded = menu == "create" && canInteract && canCreate,
                    onDismissRequest = { menu = null }) {
                    DropdownMenuItem(text = { Text("子页面") }, enabled = canInteract && canCreate,
                        onClick = { requestAction("create_page") })
                    DropdownMenuItem(text = { Text("子文件夹") }, enabled = canInteract && canCreate,
                        onClick = { requestAction("create_folder") })
                }
            }
            Box {
                TextButton(enabled = canInteract && canEdit, onClick = {
                    if (mayInteract() && latestIndex.nodes[parentId]?.writable == true) menu = "more"
                }, modifier = Modifier.heightIn(min = 48.dp).semantics { contentDescription = "更多页面操作" }) {
                    Text("更多 ▾", maxLines = 1, softWrap = false)
                }
                DropdownMenu(expanded = menu == "more" && canInteract && canEdit,
                    onDismissRequest = { menu = null }) {
                    DropdownMenuItem(text = { Text("移动") }, enabled = canInteract && canEdit,
                        onClick = { requestAction("move") })
                    DropdownMenuItem(text = { Text("重命名") }, enabled = canInteract && canEdit,
                        onClick = { requestAction("rename") })
                    DropdownMenuItem(text = { Text(if (node?.isFolder == true) "转为页面" else "转为文件夹") },
                        enabled = canInteract && canConvert,
                        onClick = { requestAction(if (node?.isFolder == true) "to_page" else "to_folder") })
                }
            }
        }
        if (expanded && children.isNotEmpty()) {
            // All direct children stay on the parent's scroll surface. No three-item preview
            // and no second "view all" screen for the current hierarchy level.
            children.forEach { child ->
                key(child.id) {
                    KnowledgeHierarchyInlineRow(child, index, enabled = canInteract) {
                        if (mayInteract()) onOpen(child)
                    }
                }
            }
        }
    }
}

private data class KnowledgeDirectoryDragVisual(
    val session: KnowledgeDirectoryDragSession,
    val node: KnowledgeHierarchyNode,
    val pointerY: Float,
    val pointerOffsetY: Float,
    val heightPx: Int
)

@OptIn(ExperimentalFoundationApi::class)
@Composable
internal fun KnowledgeHierarchyBrowser(index: KnowledgeHierarchy, parentId: String?, busy: Boolean,
    message: String, currentScope: () -> KnowledgeDirectoryOrderScope?,
    onBack: () -> Unit, onBrowse: (String?) -> Unit,
    onOpen: (KnowledgeHierarchyNode) -> Unit, onRequest: (KnowledgeHierarchyRequest) -> Unit) {
    val workspace = LocalNoteMediaWorkspaceKey.current
    val context = LocalContext.current
    val active = LocalDocumentTransitionActive.current
    val density = LocalDensity.current
    val haptic = LocalHapticFeedback.current
    val workerScope = rememberCoroutineScope()
    val listState = key(workspace, parentId) { rememberLazyListState() }
    val directoryScope = remember(index, workspace, parentId, active, busy) {
        knowledgeDirectoryOrderScope(index, workspace, parentId, active && !busy)
    }
    val latestIndex by rememberUpdatedState(index)
    val latestScope by rememberUpdatedState(directoryScope)
    val latestFreshScope by rememberUpdatedState(currentScope)
    val latestInteractive by rememberUpdatedState(active && !busy)
    val callbacksAlive = remember(workspace, parentId) { java.util.concurrent.atomic.AtomicBoolean(true) }
    var savedIds by remember(workspace, parentId) { mutableStateOf<List<String>>(emptyList()) }
    var loaded by remember(workspace, parentId) { mutableStateOf(false) }
    var loading by remember(workspace, parentId) { mutableStateOf(true) }
    var saving by remember(workspace, parentId) { mutableStateOf(false) }
    var reload by remember(workspace, parentId) { mutableIntStateOf(0) }
    var orderMessage by remember(workspace, parentId) { mutableStateOf("") }
    var pendingIds by remember(workspace, parentId) { mutableStateOf<List<String>?>(null) }
    var previewIds by remember(workspace, parentId) { mutableStateOf<List<String>?>(null) }
    var dragging by remember(workspace, parentId) { mutableStateOf<KnowledgeDirectoryDragVisual?>(null) }
    var autoScrollJob by remember(workspace, parentId) { mutableStateOf<Job?>(null) }
    var autoScrollGeneration by remember(workspace, parentId) { mutableIntStateOf(0) }
    var viewport by remember(workspace, parentId) { mutableStateOf<LayoutCoordinates?>(null) }
    val handleBounds = remember(workspace, parentId) { mutableMapOf<String, Rect>() }
    val normalIds = knowledgeDirectoryOrderedIds(directoryScope, savedIds)
    val displayedIds = pendingIds ?: previewIds ?: normalIds
    val latestDisplayedIds by rememberUpdatedState(displayedIds)
    val edgePx = with(density) { 64.dp.toPx() }
    val maxScrollPx = with(density) { 18.dp.toPx() }
    val hysteresisPx = with(density) { 5.dp.toPx() }

    fun freshScope(): KnowledgeDirectoryOrderScope? = latestFreshScope()?.takeIf {
        callbacksAlive.get() && latestInteractive && it.workspaceKey == workspace && it.parentId == parentId
    }
    fun stopAutoScroll() {
        autoScrollGeneration++
        autoScrollJob?.cancel()
        autoScrollJob = null
    }
    fun cancelDrag() {
        stopAutoScroll()
        dragging?.session?.cancel()
        dragging = null
        previewIds = null
    }
    fun navigation(action: () -> Unit) {
        if (!callbacksAlive.get() || !latestInteractive || saving || freshScope() == null) return
        cancelDrag()
        action()
    }
    fun submitOrder(expected: KnowledgeDirectoryOrderScope, proposed: List<String>) {
        val current = freshScope()
        if (!loaded || loading || saving || current == null || !expected.isSameSnapshot(current)) return
        saving = true
        pendingIds = proposed
        orderMessage = ""
        workerScope.launch {
            val committed = try {
                KnowledgeDirectoryOrderStore.commit(context, expected, { freshScope() }, proposed)
            } catch (cancelled: CancellationException) {
                throw cancelled
            } catch (_: Exception) { false }
            if (!callbacksAlive.get()) return@launch
            saving = false
            pendingIds = null
            val stillHere = freshScope()
            if (stillHere == null) return@launch
            if (committed) savedIds = proposed
            else {
                orderMessage = "排序未保存，已恢复原顺序。请核对目录和存储后重试。"
                loaded = false
                reload++
            }
        }
    }
    fun finishDrag() {
        val visual = dragging ?: return
        val current = freshScope()
        stopAutoScroll()
        dragging = null
        previewIds = null
        if (current == null) { visual.session.cancel(); return }
        val proposed = visual.session.finish(current) ?: return
        submitOrder(visual.session.expectedScope, proposed)
    }
    fun updateDrag(pointerY: Float) {
        val visual = dragging ?: return
        // Preview uses the current projection; begin/drop/write independently read authoritative state.
        val current = latestScope
        if (!callbacksAlive.get() || !latestInteractive || !visual.session.expectedScope.isSameSnapshot(current)) {
            cancelDrag()
            if (callbacksAlive.get()) orderMessage = "目录内容或权限已变化，请重新拖动。"
            return
        }
        val ids = visual.session.previewIds
        val positions = ids.withIndex().associate { it.value to it.index }
        val sourceIndex = positions[visual.session.draggedId] ?: return
        val center = pointerY - visual.pointerOffsetY + visual.heightPx / 2f
        val target = listState.layoutInfo.visibleItemsInfo.filter { item ->
            val id = item.key as? String
            val position = positions[id] ?: -1
            val midpoint = item.offset + item.size / 2f
            position >= 0 && id != visual.session.draggedId &&
                ((position > sourceIndex && center > midpoint + hysteresisPx) ||
                    (position < sourceIndex && center < midpoint - hysteresisPx))
        }.minByOrNull { abs(center - (it.offset + it.size / 2f)) }
        if (target != null) visual.session.move(current, target.key as String)
        if (!visual.session.isActive) { cancelDrag(); return }
        previewIds = visual.session.previewIds
        dragging = visual.copy(pointerY = pointerY)
    }
    fun autoScrollStep(pointerY: Float): Float {
        val layout = listState.layoutInfo
        val start = layout.viewportStartOffset.toFloat()
        val end = (layout.viewportEndOffset - layout.afterContentPadding).toFloat()
        if (end <= start) return 0f
        val edge = edgePx.coerceAtMost((end - start) / 3f).coerceAtLeast(1f)
        val strength = when {
            pointerY < start + edge -> ((pointerY - start - edge) / edge).coerceIn(-1f, 0f)
            pointerY > end - edge -> ((pointerY - end + edge) / edge).coerceIn(0f, 1f)
            else -> 0f
        }
        return strength * abs(strength) * maxScrollPx
    }
    fun updateAutoScroll() {
        val visual = dragging ?: return
        if (autoScrollStep(visual.pointerY) == 0f) { stopAutoScroll(); return }
        if (autoScrollJob?.isActive == true) return
        val generation = ++autoScrollGeneration
        autoScrollJob = workerScope.launch {
            try {
                while (isActive) {
                    val current = dragging ?: break
                    if (!callbacksAlive.get() || !latestInteractive ||
                        !current.session.expectedScope.isSameSnapshot(latestScope)) { cancelDrag(); break }
                    val step = autoScrollStep(current.pointerY)
                    if (step == 0f) break
                    val consumed = listState.scrollBy(step)
                    // scrollBy may suspend while a newer pointer event updates the drag.
                    val latest = dragging ?: break
                    if (generation != autoScrollGeneration || latest.session !== current.session) break
                    updateDrag(latest.pointerY)
                    if (consumed == 0f) break
                    delay(16)
                }
            } finally {
                if (generation == autoScrollGeneration) autoScrollJob = null
            }
        }
    }
    fun startDrag(id: String, pointer: Offset): Boolean {
        if (!loaded || loading || saving || dragging != null) return false
        val current = freshScope() ?: return false
        if (!latestScope.isSameSnapshot(current)) return false
        val session = KnowledgeDirectoryDragSession.start(current, latestDisplayedIds, id) ?: return false
        val item = listState.layoutInfo.visibleItemsInfo.firstOrNull { it.key == id } ?: return false
        val node = latestIndex.nodes[id] ?: return false
        dragging = KnowledgeDirectoryDragVisual(session, node, pointer.y,
            (pointer.y - item.offset).coerceIn(0f, item.size.toFloat()), item.size)
        previewIds = session.previewIds
        orderMessage = ""
        haptic.performHapticFeedback(HapticFeedbackType.LongPress)
        updateAutoScroll()
        return true
    }
    fun moveOnePosition(id: String, direction: Int): Boolean {
        if (!loaded || loading || saving || dragging != null) return false
        val current = freshScope() ?: return false
        if (!latestScope.isSameSnapshot(current)) return false
        val ids = latestDisplayedIds
        val sourceIndex = ids.indexOf(id)
        val target = ids.getOrNull(sourceIndex + direction) ?: return false
        val session = KnowledgeDirectoryDragSession.start(current, ids, id) ?: return false
        if (!session.move(current, target)) return false
        val proposed = session.finish(current) ?: return false
        submitOrder(current, proposed)
        return true
    }

    DisposableEffect(callbacksAlive) {
        onDispose {
            callbacksAlive.set(false)
            dragging?.session?.cancel()
            autoScrollJob?.cancel()
            handleBounds.clear()
        }
    }
    LaunchedEffect(workspace, parentId, reload) {
        loading = true
        try {
            val stored = KnowledgeDirectoryOrderStore.load(context, workspace, parentId)
            if (callbacksAlive.get()) { savedIds = stored; loaded = true }
        } catch (cancelled: CancellationException) {
            throw cancelled
        } catch (_: Exception) {
            if (callbacksAlive.get()) {
                loaded = false
                orderMessage = "无法读取本机目录顺序，请重试。"
            }
        } finally {
            if (callbacksAlive.get()) loading = false
        }
    }
    LaunchedEffect(directoryScope, active, busy) {
        val session = dragging?.session
        if (session != null && (!active || busy || !session.expectedScope.isSameSnapshot(directoryScope))) {
            cancelDrag()
        }
    }
    BackHandler(enabled = active) { navigation(onBack) }
    val node = parentId?.let(index.nodes::get)
    val canSort = active && !busy && !saving && loaded && !loading && directoryScope.canReorder && displayedIds.size > 1
    val navigationEnabled = active && !busy && !saving && dragging == null
    Box(Modifier.fillMaxSize().windowInsetsPadding(WindowInsets.safeDrawing).padding(horizontal = 14.dp)
        .clipToBounds().onGloballyPositioned { viewport = it }
        .pointerInput(workspace, parentId, callbacksAlive) {
            // The viewport owns the pointer: recycling an offscreen dragged row cannot end the gesture.
            awaitEachGesture {
                val down = awaitFirstDown(requireUnconsumed = false)
                val id = listState.layoutInfo.visibleItemsInfo.firstNotNullOfOrNull { item ->
                    (item.key as? String)?.takeIf { handleBounds[it]?.contains(down.position) == true }
                } ?: return@awaitEachGesture
                val pressed = awaitLongPressOrCancellation(down.id) ?: return@awaitEachGesture
                if (!startDrag(id, pressed.position)) return@awaitEachGesture
                pressed.consume()
                try {
                    val released = drag(pressed.id) { change ->
                        if (dragging != null) {
                            change.consume()
                            updateDrag(change.position.y)
                            updateAutoScroll()
                        }
                    }
                    if (released) {
                        currentEvent.changes.forEach { if (it.changedToUp()) it.consume() }
                        finishDrag()
                    } else cancelDrag()
                } finally {
                    // Coroutine cancellation is a cancellation, never a drop/commit.
                    if (dragging != null) cancelDrag()
                }
            }
        }) {
        LazyColumn(Modifier.fillMaxSize(), state = listState, userScrollEnabled = dragging == null && !saving,
            contentPadding = PaddingValues(bottom = 100.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
            item(key = 0L) {
                Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.SpaceBetween) {
                    TextButton(enabled = active && !busy && !saving, onClick = { navigation(onBack) }) { Text("返回") }
                    Text("目录", style = MaterialTheme.typography.titleLarge)
                    if (busy || saving || loading) CircularProgressIndicator(Modifier.size(22.dp))
                }
                KnowledgeBreadcrumbs(index, parentId) { navigation { onBrowse(it) } }
                Text(node?.title?.ifBlank { "无标题" } ?: "页面与文件夹", style = MaterialTheme.typography.headlineSmall.copy(fontWeight = FontWeight.SemiBold))
                KnowledgeHierarchyActions(index, parentId, !navigationEnabled,
                    onRequest = { request -> navigation { onRequest(request) } },
                    onOpenBody = node?.note?.let { { navigation { onOpen(node) } } })
                if (message.isNotBlank()) Text(message, color = MaterialTheme.colorScheme.error)
                if (orderMessage.isNotBlank()) Row(verticalAlignment = Alignment.CenterVertically) {
                    Text(orderMessage, modifier = Modifier.weight(1f), color = MaterialTheme.colorScheme.error,
                        style = MaterialTheme.typography.bodySmall)
                    if (!loaded && !loading) TextButton(onClick = { orderMessage = ""; reload++ }) { Text("重试") }
                }
                Text("${displayedIds.size} 个子项", style = MaterialTheme.typography.labelLarge)
            }
            items(displayedIds, key = { it }) { id ->
                val child = index.nodes[id]
                if (child != null) {
                    val placeholder = dragging?.session?.draggedId == id
                    val draggable = canSort && directoryScope.items.any { it.id == id && it.canDrag }
                    val position = displayedIds.indexOf(id)
                    DisposableEffect(id, handleBounds) { onDispose { handleBounds.remove(id) } }
                    var rowModifier = if (dragging != null || pendingIds != null) Modifier.animateItemPlacement(tween(140)) else Modifier
                    if (placeholder) rowModifier = rowModifier.graphicsLayer { alpha = 0f }.clearAndSetSemantics { }
                    else if (draggable) rowModifier = rowModifier.semantics {
                        customActions = buildList {
                            if (position > 0) add(CustomAccessibilityAction("上移") { moveOnePosition(id, -1) })
                            if (position < displayedIds.lastIndex) add(CustomAccessibilityAction("下移") { moveOnePosition(id, 1) })
                        }
                    }
                    KnowledgeHierarchyRow(child, index, modifier = rowModifier, enabled = navigationEnabled,
                        trailing = {
                            Row(verticalAlignment = Alignment.CenterVertically) {
                                if (draggable) Box(Modifier.size(48.dp).semantics {
                                    contentDescription = "$KNOWLEDGE_DIRECTORY_DRAG_LABEL，${child.title.ifBlank { "无标题" }}"
                                    stateDescription = "第 ${position + 1} 项，共 ${displayedIds.size} 项"
                                }.onGloballyPositioned { handle ->
                                    val surface = viewport
                                    if (surface?.isAttached == true && handle.isAttached) {
                                        handleBounds[id] = surface.localBoundingBoxOf(handle, clipBounds = false)
                                    }
                                }, contentAlignment = Alignment.Center) { Text("⠿", color = MaterialTheme.colorScheme.onSurfaceVariant) }
                                Text("›")
                            }
                        }) {
                        navigation { if (child.isFolder) onBrowse(child.id) else onOpen(child) }
                    }
                }
            }
            if (displayedIds.isEmpty()) item(key = 1L) { Text("此处还没有页面或文件夹") }
        }
        dragging?.let { visual ->
            Surface(modifier = Modifier.fillMaxWidth().height(with(density) { visual.heightPx.toDp() })
                .offset { IntOffset(0, (visual.pointerY - visual.pointerOffsetY).roundToInt()) }
                .zIndex(1f).clearAndSetSemantics { }, shape = RoundedCornerShape(12.dp),
                tonalElevation = 3.dp, shadowElevation = 6.dp) {
                KnowledgeHierarchyRow(visual.node, index, enabled = false,
                    trailing = { Row(verticalAlignment = Alignment.CenterVertically) {
                        Box(Modifier.size(48.dp), contentAlignment = Alignment.Center) { Text("⠿") }; Text("›")
                    } }) { }
            }
        }
    }
}

@Composable
internal fun KnowledgeHierarchyActionDialog(request: KnowledgeHierarchyRequest, index: KnowledgeHierarchy,
    busy: Boolean, error: String, onDismiss: () -> Unit, onSubmit: (String?, String) -> Unit) {
    val node = request.id?.let(index.nodes::get)
    val location = request.id?.let(index::location)
    val moving = request.action == "move"
    // A move picker follows the live source location. Changes made elsewhere reset
    // the pending target, so an earlier parent cannot be submitted by accident.
    val sourceLocationKey = listOf(location?.parentId, location?.issue, location?.available,
        location?.canMove, location?.ancestors?.map { it.id })
    var title by rememberSaveable(request) { mutableStateOf(if (request.action == "rename") node?.title.orEmpty() else "") }
    var parent by rememberSaveable(request, sourceLocationKey) { mutableStateOf(if (moving) location?.parentId else request.parentId) }
    var destinationChosen by rememberSaveable(request, sourceLocationKey) { mutableStateOf(false) }
    var query by rememberSaveable(request) { mutableStateOf("") }
    val creating = request.action.startsWith("create_")
    val naming = creating || request.action == "rename"
    val valid = !busy && when {
        naming -> (if (creating) index.mayCreateInside(request.parentId) else node?.writable == true) && title.isNotBlank() && title.length <= 120 && !title.contains('\n')
        moving -> destinationChosen && index.canMoveTo(request.id.orEmpty(), parent)
        else -> node?.writable == true
    }
    AlertDialog(onDismissRequest = { if (!busy) onDismiss() }, title = { Text(when(request.action) {
        "create_folder" -> "新建文件夹"; "create_page" -> "新建页面"; "rename" -> "重命名"
        "move" -> "移动到"; "to_folder" -> "转为文件夹"; else -> "转为页面"
    }) }, text = {
        Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
            if (naming) OutlinedTextField(value = title, onValueChange = { title = it.take(120) }, singleLine = true,
                enabled = !busy, label = { Text("名称") }, modifier = Modifier.fillMaxWidth())
            if (moving) {
                OutlinedTextField(value = query, onValueChange = { query = it.take(120) }, singleLine = true,
                    enabled = !busy, label = { Text("查找位置") }, modifier = Modifier.fillMaxWidth())
                Text("当前位置：" + knowledgeLocationPath(location))
                knowledgeLocationMessage(location)?.let { Text(it, style = MaterialTheme.typography.bodySmall) }
                Text("目标：" + if (!destinationChosen) "请选择位置" else parent?.let { target ->
                    if (index.nodes.containsKey(target)) "知识目录 / " + index.pathLabel(target) else "所选位置已不存在"
                }.orEmpty().ifBlank { "知识目录" })
                val targets = index.moveTargets(request.id.orEmpty()).filter { index.pathLabel(it.id).contains(query.trim(), ignoreCase = true) }
                LazyColumn(Modifier.fillMaxWidth().heightIn(max = 300.dp)) {
                    item { TextButton(enabled = !busy && location?.canMove == true,
                        onClick = { parent = null; destinationChosen = true }) { Text("知识目录（根级）") } }
                    items(targets, key = { it.id }) { target ->
                        TextButton(enabled = !busy && location?.canMove == true,
                            onClick = { parent = target.id; destinationChosen = true }) { Text(index.pathLabel(target.id)) }
                    }
                }
            } else if (!naming) Text("保留正文、附件、页面链接和子项，只更改页面类型。可随时转换回来。")
            if (creating) Text("位置：" + request.parentId?.let(index::pathLabel).orEmpty().ifBlank { "知识目录" })
            if (busy) Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) { CircularProgressIndicator(Modifier.size(22.dp)); Text("正在保存…") }
            if (error.isNotBlank()) Text(error, color = MaterialTheme.colorScheme.error)
        }
    }, confirmButton = { TextButton(enabled = valid, onClick = { onSubmit(parent, title.trim()) }) { Text("确定") } },
        dismissButton = { TextButton(enabled = !busy, onClick = onDismiss) { Text("取消") } })
}
"####;

#[cfg(test)]
mod mobile_inline_children_tests {
    use super::CONTENTS;

    #[test]
    fn inline_children_are_complete_and_initially_expanded() {
        assert!(CONTENTS.contains("rememberSaveable(workspace, parentId) { mutableStateOf(true) }"));
        assert!(CONTENTS.contains("children.forEach { child ->"));
        assert!(CONTENTS.contains("KnowledgeHierarchyInlineRow(child, index, enabled = canInteract)"));
        assert!(!CONTENTS.contains("children.take(3)"));
        assert!(!CONTENTS.contains("查看全部 ${children.size} 项"));
    }

    #[test]
    fn inline_list_uses_parent_scroll_and_does_not_persist_expand_state() {
        assert!(CONTENTS.contains("expanded = !expanded"));
        assert!(CONTENTS.contains("KnowledgeHierarchyInlineRow("));
        assert!(CONTENTS.contains("if (supporting != null)"));
        assert!(!CONTENTS.contains("LazyColumn(Modifier.heightIn"));
    }
}

#[cfg(test)]
mod hierarchy_save_latency_tests {
    use super::*;

    #[test]
    fn hierarchy_save_stays_durable_and_logs_only_slow_phase_names() {
        let source = render(
            "com/ofairyo/gridtimer/data/TimerRepository.kt",
            "    suspend fun createNoteFolder(",
        )
        .unwrap();
        assert!(source.contains("persistImmediately = true, publishAfterDurable = true"));
        assert!(source.contains("withContext(NonCancellable)"));
        assert!(source.contains("expectedWorkspaceKey = expectedWorkspaceKey"));
        for phase in [
            "hierarchy_encode",
            "hierarchy_rust_mutation",
            "hierarchy_decode",
            "hierarchy_action_total",
        ] {
            assert!(source.contains(phase), "{phase}");
        }
        assert!(source.contains("elapsedMs >= 1_000L"));
        assert!(!source.contains("title=$title"));
        assert!(!source.contains("workspace=$expectedWorkspaceKey"));
    }
}
