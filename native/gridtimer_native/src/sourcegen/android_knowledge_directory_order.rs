//! Workspace-local directory presentation order. Never writes shared page data.

pub const PATH: &str = "com/ofairyo/gridtimer/data/KnowledgeDirectoryOrder.kt";
pub const TEST_PATH: &str = "com/ofairyo/gridtimer/data/KnowledgeDirectoryOrderTest.kt";

pub const CONTENTS: &str = r####"package com.ofairyo.gridtimer.data

import android.content.Context
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.currentCoroutineContext
import kotlinx.coroutines.ensureActive
import kotlinx.coroutines.sync.Mutex
import kotlinx.coroutines.sync.withLock
import kotlinx.coroutines.withContext
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.JsonArray
import kotlinx.serialization.json.JsonPrimitive
import java.security.MessageDigest

internal const val KNOWLEDGE_DIRECTORY_DRAG_LABEL = "长按拖动排序"

/** Directory frames have no active editor record; their access is a collection boundary. */
internal fun knowledgeDirectoryCollectionMayInteract(
    currentFrame: Boolean,
    collectionFrame: Boolean,
    visible: Boolean,
    busy: Boolean
): Boolean = currentFrame && collectionFrame && visible && !busy

internal data class KnowledgeDirectoryOrderItem(val id: String, val canDrag: Boolean)

internal data class KnowledgeDirectoryOrderScope(
    val workspaceKey: String,
    val parentId: String?,
    val parentPathIds: List<String>,
    val canReorder: Boolean,
    val items: List<KnowledgeDirectoryOrderItem>
) {
    /** Default date order may change after a content edit; membership and permissions may not. */
    fun isSameSnapshot(other: KnowledgeDirectoryOrderScope): Boolean =
        workspaceKey == other.workspaceKey && parentId == other.parentId &&
            parentPathIds == other.parentPathIds && canReorder == other.canReorder &&
            items.size == other.items.size &&
            items.associate { it.id to it.canDrag } == other.items.associate { it.id to it.canDrag }

    internal fun frozen(): KnowledgeDirectoryOrderScope = copy(
        parentPathIds = parentPathIds.toList(), items = items.toList())

    internal fun valid(): Boolean = workspaceKey.isNotBlank() &&
        (parentId == null || parentId.isNotBlank()) && items.all { it.id.isNotBlank() } &&
        items.map { it.id }.distinct().size == items.size &&
        parentPathIds.all { it.isNotBlank() } && parentPathIds.distinct().size == parentPathIds.size
}

internal fun knowledgeDirectoryOrderScope(
    index: KnowledgeHierarchy,
    workspaceKey: String,
    parentId: String?,
    canInteract: Boolean = true
): KnowledgeDirectoryOrderScope {
    val location = parentId?.let(index::location)
    val validParent = parentId == null || (location?.available == true && location.issue == null)
    return KnowledgeDirectoryOrderScope(
        workspaceKey = workspaceKey,
        parentId = parentId,
        parentPathIds = if (parentId == null) emptyList() else index.breadcrumbs(parentId).map { it.id },
        canReorder = canInteract && validParent && index.mayCreateInside(parentId),
        items = index.children(parentId).map { node ->
            KnowledgeDirectoryOrderItem(node.id, node.writable && index.location(node.id).issue == null)
        }
    )
}

/** Deleted or moved IDs disappear from this projection; new siblings are appended. */
internal fun knowledgeDirectoryOrderedIds(scope: KnowledgeDirectoryOrderScope, savedIds: List<String>): List<String> {
    val available = scope.items.map { it.id }.toSet()
    val ordered = LinkedHashSet<String>()
    savedIds.forEach { if (it in available) ordered.add(it) }
    scope.items.forEach { ordered.add(it.id) }
    return ordered.toList()
}

private fun directoryOrderIsPermutation(scope: KnowledgeDirectoryOrderScope, ids: List<String>): Boolean =
    scope.valid() && ids.size == scope.items.size && ids.distinct().size == ids.size &&
        ids.toSet() == scope.items.map { it.id }.toSet()

internal class KnowledgeDirectoryDragSession private constructor(
    val expectedScope: KnowledgeDirectoryOrderScope,
    private val initialIds: List<String>,
    val draggedId: String
) {
    private var pendingIds = initialIds
    var isActive: Boolean = true
        private set
    val previewIds: List<String> get() = pendingIds.toList()

    /** True only when the target position changed. Invalid snapshots consume the gesture. */
    fun move(currentScope: KnowledgeDirectoryOrderScope, targetId: String): Boolean {
        if (!isActive) return false
        if (!expectedScope.isSameSnapshot(currentScope) || !currentScope.valid()) {
            cancel()
            return false
        }
        val from = pendingIds.indexOf(draggedId)
        val to = pendingIds.indexOf(targetId)
        if (from < 0 || to < 0) {
            cancel()
            return false
        }
        if (from == to) return false
        pendingIds = pendingIds.toMutableList().apply { add(to, removeAt(from)) }.toList()
        return true
    }

    /** A release can be consumed once; navigation, cancellation and no-op drops never commit. */
    fun finish(currentScope: KnowledgeDirectoryOrderScope): List<String>? {
        if (!isActive) return null
        isActive = false
        if (!expectedScope.isSameSnapshot(currentScope) || !currentScope.valid()) return null
        return pendingIds.takeIf { it != initialIds }?.toList()
    }

    fun cancel() {
        isActive = false
        pendingIds = initialIds
    }

    companion object {
        fun start(scope: KnowledgeDirectoryOrderScope, shownIds: List<String>, draggedId: String): KnowledgeDirectoryDragSession? {
            if (!scope.canReorder || scope.items.size < 2 ||
                scope.items.none { it.id == draggedId && it.canDrag } ||
                !directoryOrderIsPermutation(scope, shownIds)) return null
            return KnowledgeDirectoryDragSession(scope.frozen(), shownIds.toList(), draggedId)
        }
    }
}

/** Publish only the exact order whose durable writer succeeded under the still-current boundary. */
internal fun persistKnowledgeDirectoryOrder(
    expectedScope: KnowledgeDirectoryOrderScope,
    currentScope: KnowledgeDirectoryOrderScope?,
    ids: List<String>,
    write: (List<String>) -> Boolean
): List<String>? {
    if (currentScope == null || !expectedScope.isSameSnapshot(currentScope) ||
        !currentScope.canReorder || currentScope.items.none { it.canDrag } ||
        !directoryOrderIsPermutation(currentScope, ids)) return null
    val proposed = ids.toList()
    return if (write(proposed)) proposed else null
}

/** Local layout only: no AppData, note save, history revision or synchronization call. */
internal object KnowledgeDirectoryOrderStore {
    private const val PREFERENCES = "knowledge_directory_order_v1"
    private val access = Mutex()

    private fun key(workspaceKey: String, parentId: String?): String {
        require(workspaceKey.isNotBlank() && (parentId == null || parentId.isNotBlank()))
        val identity = JsonArray(listOf(JsonPrimitive(workspaceKey),
            if (parentId == null) kotlinx.serialization.json.JsonNull else JsonPrimitive(parentId))).toString()
        return MessageDigest.getInstance("SHA-256").digest(identity.toByteArray(Charsets.UTF_8))
            .joinToString("") { "%02x".format(it.toInt() and 0xff) }
    }

    /** Corruption and I/O errors propagate so the caller disables sorting until a successful retry. */
    suspend fun load(context: Context, workspaceKey: String, parentId: String?): List<String> =
        withContext(Dispatchers.IO) {
            access.withLock {
                currentCoroutineContext().ensureActive()
                val preferences = context.applicationContext.getSharedPreferences(PREFERENCES, Context.MODE_PRIVATE)
                val raw = preferences.getString(key(workspaceKey, parentId), null) ?: return@withLock emptyList()
                val array = Json.parseToJsonElement(raw) as? JsonArray
                    ?: throw IllegalStateException("Directory order is not an array.")
                val ids = array.map { value ->
                    val primitive = value as? JsonPrimitive
                        ?: throw IllegalStateException("Directory order contains a non-string identity.")
                    require(primitive.isString && primitive.content.isNotBlank()) {
                        "Directory order contains an invalid identity."
                    }
                    primitive.content
                }
                require(ids.distinct().size == ids.size) { "Directory order contains duplicate identities." }
                ids
            }
        }

    /** The provider reads current workspace/StateFlow state, not a captured UI hierarchy snapshot. */
    suspend fun commit(
        context: Context,
        expectedScope: KnowledgeDirectoryOrderScope,
        currentScopeProvider: () -> KnowledgeDirectoryOrderScope?,
        ids: List<String>
    ): Boolean = withContext(Dispatchers.IO) {
        access.withLock {
            currentCoroutineContext().ensureActive()
            val preferences = context.applicationContext.getSharedPreferences(PREFERENCES, Context.MODE_PRIVATE)
            val storageKey = key(expectedScope.workspaceKey, expectedScope.parentId)
            val previous = preferences.getString(storageKey, null)
            val encoded = JsonArray(ids.map { JsonPrimitive(it) }).toString()
            currentCoroutineContext().ensureActive()
            persistKnowledgeDirectoryOrder(expectedScope, currentScopeProvider(), ids) {
                var committed = false
                try {
                    committed = preferences.edit().putString(storageKey, encoded).commit()
                } finally {
                    if (!committed) {
                        // SharedPreferences updates memory before disk. Restore the last durable view,
                        // even if disk remains unavailable; other store reads hold the same mutex.
                        val restore = preferences.edit()
                        if (previous == null) restore.remove(storageKey) else restore.putString(storageKey, previous)
                        restore.commit()
                    }
                }
                committed
            } != null
        }
    }
}
"####;

pub const TEST_CONTENTS: &str = r####"package com.ofairyo.gridtimer.data

import kotlinx.serialization.json.*
import org.junit.Assert.*
import org.junit.Test

class KnowledgeDirectoryOrderTest {
    private fun scope() = KnowledgeDirectoryOrderScope("workspace-a", "parent", listOf("ancestor", "parent"), true,
        listOf("page", "folder", "legacy").map { KnowledgeDirectoryOrderItem(it, true) })

    private fun session(source: KnowledgeDirectoryOrderScope = scope()) =
        requireNotNull(KnowledgeDirectoryDragSession.start(source, source.items.map { it.id }, "page"))

    @Test fun collectionInteractionAllowsRecordlessDirectoryAndRejectsStaleHiddenBusyOrEditorFrames() {
        val index = buildKnowledgeHierarchy(listOf(
            NoteEntry(id = "page", kind = NoteEntryKind.DOCUMENT),
            NoteEntry(id = "other", kind = NoteEntryKind.DOCUMENT)), emptyList())
        // The current collection has no editor target or selected editor record.
        val allowed = knowledgeDirectoryCollectionMayInteract(
            currentFrame = true, collectionFrame = true, visible = true, busy = false)
        val current = knowledgeDirectoryOrderScope(index, "workspace-a", null, canInteract = allowed)
        assertNotNull(KnowledgeDirectoryDragSession.start(current, current.items.map { it.id }, "page"))
        val blocked = listOf(
            knowledgeDirectoryCollectionMayInteract(false, true, true, false),
            knowledgeDirectoryCollectionMayInteract(true, false, true, false),
            knowledgeDirectoryCollectionMayInteract(true, true, false, false),
            knowledgeDirectoryCollectionMayInteract(true, true, true, true))
        blocked.forEach { mayInteract ->
            val scope = knowledgeDirectoryOrderScope(index, "workspace-a", null, canInteract = mayInteract)
            assertNull(KnowledgeDirectoryDragSession.start(scope, scope.items.map { it.id }, "page"))
        }
    }

    @Test fun projectionKeepsSavedSiblingsAndAppendsNewItemsWithoutDuplicatesOrCrossLevelIds() {
        val source = scope().copy(items = scope().items + KnowledgeDirectoryOrderItem("new-page", true))
        assertEquals(listOf("legacy", "page", "folder", "new-page"),
            knowledgeDirectoryOrderedIds(source, listOf("removed", "legacy", "legacy", "other-level", "page")))
        assertEquals(source.items.map { it.id }, knowledgeDirectoryOrderedIds(source, emptyList()))
    }

    @Test fun sameLevelReleaseProducesOnePermutationAndConsumesTheSession() {
        val source = scope()
        val drag = session(source)
        assertTrue(drag.move(source, "legacy"))
        assertEquals(listOf("folder", "legacy", "page"), drag.previewIds)
        assertEquals(listOf("folder", "legacy", "page"), drag.finish(source))
        assertNull(drag.finish(source))
        assertFalse(drag.move(source, "folder"))
        assertEquals(listOf("page", "folder", "legacy"), source.items.map { it.id })
    }

    @Test fun cancelAfterMovingNeverReturnsACommitAndRestoresThePreview() {
        val drag = session()
        drag.move(scope(), "legacy")
        drag.cancel()
        assertFalse(drag.isActive)
        assertEquals(listOf("page", "folder", "legacy"), drag.previewIds)
        assertNull(drag.finish(scope()))
        assertFalse(drag.move(scope(), "folder"))
    }

    @Test fun unchangedAndReturnedToOriginalPositionsDoNotCommit() {
        assertNull(session().finish(scope()))
        val drag = session()
        assertFalse(drag.move(scope(), "page"))
        assertTrue(drag.move(scope(), "legacy"))
        assertTrue(drag.move(scope(), "folder"))
        assertNull(drag.finish(scope()))
    }

    @Test fun staleWorkspaceParentPathMembershipAndPermissionsRejectReleaseAndWrite() {
        val original = scope()
        val changed = listOf(original.copy(workspaceKey = "workspace-b"), original.copy(parentId = "other"),
            original.copy(parentPathIds = listOf("other-ancestor", "parent")), original.copy(canReorder = false),
            original.copy(items = original.items.dropLast(1)),
            original.copy(items = original.items + KnowledgeDirectoryOrderItem("new", true)),
            original.copy(items = original.items.map { if (it.id == "page") it.copy(canDrag = false) else it }))
        changed.forEach { current ->
            val drag = session(original)
            drag.move(original, "legacy")
            assertNull(drag.finish(current))
            var writes = 0
            assertNull(persistKnowledgeDirectoryOrder(original, current, original.items.map { it.id }.reversed()) { writes++; true })
            assertEquals(0, writes)
            val moving = session(original)
            assertFalse(moving.move(current, "legacy"))
            assertFalse(moving.isActive)
            assertNull(moving.finish(original))
        }
    }

    @Test fun readOnlyEmptySingleUnknownAndMalformedItemsCannotStartDragging() {
        val source = scope()
        val ids = source.items.map { it.id }
        assertNull(KnowledgeDirectoryDragSession.start(source.copy(canReorder = false), ids, "page"))
        assertNull(KnowledgeDirectoryDragSession.start(source.copy(items = source.items.map { it.copy(canDrag = false) }), ids, "page"))
        assertNull(KnowledgeDirectoryDragSession.start(source.copy(items = emptyList()), emptyList(), "page"))
        assertNull(KnowledgeDirectoryDragSession.start(source.copy(items = source.items.take(1)), listOf("page"), "page"))
        assertNull(KnowledgeDirectoryDragSession.start(source, ids, "unknown"))
        assertNull(KnowledgeDirectoryDragSession.start(source, listOf("page", "page", "legacy"), "page"))
        assertNull(KnowledgeDirectoryDragSession.start(source.copy(items = source.items + source.items.first()), ids, "page"))
        listOf(source.copy(canReorder = false), source.copy(items = source.items.map { it.copy(canDrag = false) })).forEach { protected ->
            assertNull(persistKnowledgeDirectoryOrder(protected, protected, ids.reversed()) {
                fail("Protected scopes must never reach storage."); true
            })
        }
    }

    @Test fun failedDurableWriteNeverPublishesAndInvalidOrdersNeverReachTheWriter() {
        val source = scope()
        val proposed = listOf("folder", "legacy", "page")
        var published = source.items.map { it.id }
        var attempts = 0
        persistKnowledgeDirectoryOrder(source, source, proposed) { attempts++; false }?.let { published = it }
        assertEquals(1, attempts)
        assertEquals(listOf("page", "folder", "legacy"), published)
        listOf(listOf("page", "folder"), listOf("page", "page", "legacy"), listOf("page", "folder", "foreign")).forEach { ids ->
            assertNull(persistKnowledgeDirectoryOrder(source, source, ids) { attempts++; true })
        }
        assertNull(persistKnowledgeDirectoryOrder(source, null, proposed) { attempts++; true })
        assertEquals(1, attempts)
        assertEquals(proposed, persistKnowledgeDirectoryOrder(source, source, proposed) { true })
    }

    @Test fun storageExceptionCannotPublishAnOrder() {
        val original = scope().items.map { it.id }
        var published = original
        try {
            persistKnowledgeDirectoryOrder(scope(), scope(), original.reversed()) { throw java.io.IOException("disk unavailable") }
                ?.let { published = it }
            fail("Storage errors must reach the caller.")
        } catch (_: java.io.IOException) { }
        assertEquals(original, published)
    }

    @Test fun contentDrivenDefaultOrderChangeDoesNotInvalidateManualOrderOrGesture() {
        val source = scope()
        val current = source.copy(items = source.items.reversed())
        assertTrue(source.isSameSnapshot(current))
        assertEquals(listOf("legacy", "page", "folder"),
            knowledgeDirectoryOrderedIds(current, listOf("legacy", "page", "folder")))
        val drag = session(source)
        drag.move(source, "legacy")
        assertEquals(listOf("folder", "legacy", "page"), drag.finish(current))
    }

    @Test fun scopeFactoryPreservesContentTimesHistoryAndRejectsProtectedOrDamagedLocations() {
        fun page(id: String, parent: String? = null, locked: Boolean = false, folder: Boolean = false) = NoteEntry(
            id = id, kind = NoteEntryKind.DOCUMENT, title = id, content = "unchanged body",
            createdAtEpochMillis = 100, updatedAtEpochMillis = 200,
            revisions = listOf(NoteRevisionSnapshot(id = "revision-$id")),
            versions = listOf(NoteVersionSnapshot(id = "version-$id")),
            document = NoteDocument(knowledge = buildJsonObject {
                put("version", 1); if (parent != null) put("parentId", parent); put("locked", locked)
                if (folder) put("properties", buildJsonObject {
                    put(KNOWLEDGE_CONTAINER_KIND_PROPERTY, buildJsonObject { put("kind", "text"); put("value", "folder") })
                })
            }))
        val notes = listOf(page("parent"), page("page", "parent"), page("folder", "parent", folder = true), page("locked", locked = true),
            page("orphan", "missing"), page("readonly-child", "locked"))
        val before = notes.toList()
        val index = buildKnowledgeHierarchy(notes, listOf(NoteFolder(id = "legacy", name = "Legacy")))
        val source = knowledgeDirectoryOrderScope(index, "workspace-a", "parent")
        val drag = requireNotNull(KnowledgeDirectoryDragSession.start(source, source.items.map { it.id }, "page"))
        drag.move(source, "folder")
        assertNotNull(drag.finish(source))
        assertEquals(before, notes)
        notes.forEach { assertSame(it, index.nodes.getValue(it.id).note) }
        assertFalse(knowledgeDirectoryOrderScope(index, "workspace-a", "locked").canReorder)
        assertFalse(knowledgeDirectoryOrderScope(index, "workspace-a", "missing").canReorder)
        assertFalse(knowledgeDirectoryOrderScope(index, "workspace-a", "readonly-child").canReorder)
        assertFalse(knowledgeDirectoryOrderScope(index, "workspace-a", "parent", canInteract = false).canReorder)
        val root = knowledgeDirectoryOrderScope(index, "workspace-a", null)
        assertFalse(root.items.first { it.id == "locked" }.canDrag)
        assertFalse(root.items.first { it.id == "orphan" }.canDrag)
        assertTrue(root.items.first { it.id == "legacy" }.canDrag)
    }
}
"####;
