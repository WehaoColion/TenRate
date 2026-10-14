// v2.23.2.32 - Resolve document location from the same hierarchy as navigation.
//! Android knowledge hierarchy projection. Domain writes remain in shared Rust.
//! Read only metadata so browsing containers cannot flatten or inspect page bodies.

pub const PATH: &str = "com/ofairyo/gridtimer/data/KnowledgeHierarchy.kt";
pub const TEST_PATH: &str = "com/ofairyo/gridtimer/data/KnowledgeHierarchyTest.kt";

pub const CONTENTS: &str = r####"package com.ofairyo.gridtimer.data

import kotlinx.serialization.json.*

internal const val KNOWLEDGE_CONTAINER_KIND_PROPERTY = "__tenrate_container_kind"
internal const val KNOWLEDGE_LEGACY_FOLDER_PROPERTY = "__tenrate_legacy_folder"

internal fun NoteEntry.isKnowledgeFolder(): Boolean {
    if (kind != NoteEntryKind.DOCUMENT || encryption != null) return false
    val properties = document.knowledge?.get("properties") as? JsonObject ?: return false
    val marker = properties[KNOWLEDGE_CONTAINER_KIND_PROPERTY] as? JsonObject ?: return false
    return (marker["kind"] as? JsonPrimitive)?.contentOrNull == "text" &&
        (marker["value"] as? JsonPrimitive)?.contentOrNull == "folder"
}

/** Parent/container metadata alone does not turn an ordinary page into a complex editor. */
internal fun NoteEntry.requiresStructuredKnowledgeReader(): Boolean {
    if (document.blocks.any { it.knowledge != null }) return true
    val page = document.knowledge ?: return false
    return page.any { (key, value) -> when (key) {
        "version" -> (value as? JsonPrimitive)?.intOrNull != 1
        "parentId" -> value != JsonNull && (value !is JsonPrimitive || !value.isString)
        "order" -> (value as? JsonPrimitive)?.longOrNull == null
        "icon", "cover" -> value != JsonNull && (value as? JsonPrimitive)?.contentOrNull.orEmpty().isNotEmpty()
        "fullWidth", "smallText", "locked", "template" -> (value as? JsonPrimitive)?.booleanOrNull != false
        "tags", "comments" -> value !is JsonArray || value.isNotEmpty()
        "database" -> value != JsonNull
        "properties" -> value !is JsonObject || value.any { (property, cell) ->
            when (property) {
                KNOWLEDGE_CONTAINER_KIND_PROPERTY -> cell !is JsonObject ||
                    (cell["kind"] as? JsonPrimitive)?.contentOrNull != "text" ||
                    (cell["value"] as? JsonPrimitive)?.contentOrNull != "folder"
                KNOWLEDGE_LEGACY_FOLDER_PROPERTY -> cell !is JsonObject ||
                    (cell["kind"] as? JsonPrimitive)?.contentOrNull != "checkbox" ||
                    (cell["value"] as? JsonPrimitive)?.booleanOrNull != true
                else -> true
            }
        }
        else -> true
    } }
}

internal data class KnowledgeHierarchyNode(
    val id: String,
    val title: String,
    val isFolder: Boolean,
    val isLegacyFolder: Boolean,
    val note: NoteEntry?,
    val parentId: String?,
    val writable: Boolean,
    val legacyFolder: NoteFolder? = null
)

internal enum class KnowledgeHierarchyIssue { MISSING_PARENT, INACCESSIBLE_PARENT, CYCLE, IDENTITY_CONFLICT }

internal data class KnowledgeLocation(
    val parentId: String?,
    val ancestors: List<KnowledgeHierarchyNode>,
    val issue: KnowledgeHierarchyIssue?,
    val available: Boolean,
    val canMove: Boolean
)


/** One snapshot index for the current workspace. No static/shared state or document text scan. */
internal class KnowledgeHierarchy internal constructor(
    val nodes: Map<String, KnowledgeHierarchyNode>,
    val issues: Map<String, KnowledgeHierarchyIssue>
) {
    private val byParent = nodes.values.groupBy(KnowledgeHierarchyNode::parentId)
    fun children(parentId: String?): List<KnowledgeHierarchyNode> = byParent[parentId].orEmpty()
    fun parentOf(id: String): String? = nodes[id]?.parentId
    fun location(id: String): KnowledgeLocation {
        val node = nodes[id] ?: return KnowledgeLocation(null, emptyList(), null, false, false)
        val parents = ancestors(id)
        return KnowledgeLocation(
            parentId = node.parentId,
            ancestors = parents.asReversed(),
            issue = issues[id] ?: parents.firstNotNullOfOrNull { issues[it.id] },
            available = true,
            canMove = node.writable
        )
    }

    fun ancestors(id: String): List<KnowledgeHierarchyNode> {
        val result = ArrayList<KnowledgeHierarchyNode>()
        val seen = hashSetOf(id)
        var parent = parentOf(id)
        while (parent != null && seen.add(parent)) {
            val item = nodes[parent] ?: break
            result += item
            parent = item.parentId
        }
        return result
    }
    fun breadcrumbs(id: String): List<KnowledgeHierarchyNode> = ancestors(id).asReversed() + listOfNotNull(nodes[id])
    fun containsInScope(id: String, parentId: String?): Boolean =
        nodes.containsKey(id) && (parentId == null || ancestors(id).any { it.id == parentId })
    fun moveTargets(id: String): List<KnowledgeHierarchyNode> {
        if (nodes[id]?.writable != true) return emptyList()
        val forbidden = descendantIds(id) + id
        return nodes.values.filter { mayCreateInside(it.id) && it.id !in forbidden }
    }
    fun canMoveTo(id: String, parentId: String?): Boolean {
        val node = nodes[id] ?: return false
        if (!node.writable) return false
        val repairRoot = parentId == null && issues[id] in setOf(
            KnowledgeHierarchyIssue.MISSING_PARENT, KnowledgeHierarchyIssue.INACCESSIBLE_PARENT,
            KnowledgeHierarchyIssue.CYCLE)
        if (parentId == node.parentId && !repairRoot) return false
        return parentId == null || moveTargets(id).any { it.id == parentId }
    }

    fun descendantIds(id: String): Set<String> {
        val result = LinkedHashSet<String>()
        val pending = java.util.ArrayDeque<String>()
        pending.add(id)
        while (!pending.isEmpty()) children(pending.removeFirst()).forEach {
            if (it.id != id && result.add(it.id)) pending.add(it.id)
        }
        return result
    }
    fun pathLabel(id: String): String = breadcrumbs(id).joinToString(" / ") { it.title.ifBlank { "无标题" } }
    fun mayCreateInside(id: String?): Boolean = id == null ||
        (nodes[id]?.writable == true && id !in issues &&
            ancestors(id).all { it.writable && it.id !in issues })
}

internal fun AppData.knowledgeHierarchy(): KnowledgeHierarchy = buildKnowledgeHierarchy(notes, noteFolders)

internal fun buildKnowledgeHierarchy(notes: List<NoteEntry>, folders: List<NoteFolder>): KnowledgeHierarchy {
    val live = notes.filter { it.kind == NoteEntryKind.DOCUMENT && it.deletedAtEpochMillis == null }
    val legacy = folders.associateBy(NoteFolder::id)
    val source = LinkedHashMap<String, KnowledgeHierarchyNode>()
    val issues = LinkedHashMap<String, KnowledgeHierarchyIssue>()
    folders.forEach { folder ->
        source[folder.id] = KnowledgeHierarchyNode(folder.id, folder.name, true, true, null, null, true, folder)
    }
    // A promoted legacy folder has the same stable id. Its NoteFolder is kept for old clients.
    live.sortedWith(compareByDescending<NoteEntry> { it.updatedAtEpochMillis }.thenBy { it.id }).forEach { note ->
        val page = note.document.knowledge
        val oldFolder = legacy[note.id]
        val promotion = (page?.get("properties") as? JsonObject)?.get(KNOWLEDGE_LEGACY_FOLDER_PROPERTY) as? JsonObject
        val promoted = (promotion?.get("kind") as? JsonPrimitive)?.contentOrNull == "checkbox" &&
            (promotion?.get("value") as? JsonPrimitive)?.booleanOrNull == true && note.encryption == null
        val conflict = oldFolder != null && !promoted
        if (conflict) issues[note.id] = KnowledgeHierarchyIssue.IDENTITY_CONFLICT
        val parent = (page?.get("parentId") as? JsonPrimitive)?.contentOrNull?.takeIf(String::isNotBlank)
            ?: note.folderId?.takeIf { it in legacy && it != note.id }
        val version = (page?.get("version") as? JsonPrimitive)?.intOrNull
        source[note.id] = KnowledgeHierarchyNode(note.id, note.title, note.isKnowledgeFolder(),
            oldFolder != null, note, if (conflict) null else parent, !conflict && note.encryption == null &&
                (page?.get("locked") as? JsonPrimitive)?.booleanOrNull != true &&
                (version == null || version == 1), oldFolder)
    }
    val parents = source.mapValues { (id, node) ->
        val parent = node.parentId
        when {
            parent == null -> null
            parent == id -> { issues[id] = KnowledgeHierarchyIssue.CYCLE; null }
            source[parent] == null -> { issues[id] = KnowledgeHierarchyIssue.MISSING_PARENT; null }
            source.getValue(parent).note?.encryption != null -> { issues[id] = KnowledgeHierarchyIssue.INACCESSIBLE_PARENT; null }
            else -> parent
        }
    }.toMutableMap()
    // Resolve cycles in linear time. Every record remains reachable; no recursion/depth limit.
    val complete = hashSetOf<String>()
    source.keys.forEach { start ->
        if (start !in complete) {
            val path = ArrayList<String>()
            val local = HashMap<String, Int>()
            var current: String? = start
            while (current != null && current !in complete) {
                val cycle = local[current]
                if (cycle != null) {
                    path.subList(cycle, path.size).forEach { id -> parents[id] = null; issues[id] = KnowledgeHierarchyIssue.CYCLE }
                    break
                }
                local[current] = path.size; path += current
                current = parents[current]
            }
            complete.addAll(path)
        }
    }
    val projected = source.mapValuesTo(LinkedHashMap()) { (id, item) -> item.copy(parentId = parents[id]) }
    return KnowledgeHierarchy(projected, issues)
}
"####;

pub const TEST_CONTENTS: &str = r####"package com.ofairyo.gridtimer.data

import kotlinx.serialization.json.*
import org.junit.Assert.*
import org.junit.Test

class KnowledgeHierarchyTest {
    private fun page(id: String, parent: String? = null, folder: Boolean = false): NoteEntry = NoteEntry(
        id = id, title = id, kind = NoteEntryKind.DOCUMENT,
        document = NoteDocument(knowledge = buildJsonObject {
            put("version", 1)
            if (parent != null) put("parentId", parent)
            if (folder) put("properties", buildJsonObject {
                put(KNOWLEDGE_CONTAINER_KIND_PROPERTY, buildJsonObject { put("kind", "text"); put("value", "folder") })
            })
        }, blocks = listOf(NoteBlock(id = "$id-body", text = "body of $id")))
    )

    @Test fun pageFolderAndMixedNestingExposeOnlyImmediateChildren() {
        val root = page("page")
        val innerFolder = page("folder", "page", folder = true)
        val child = page("child", "folder")
        val innerPage = page("inner", "child")
        val index = buildKnowledgeHierarchy(listOf(root, innerFolder, child, innerPage), emptyList())
        assertEquals(listOf("page"), index.children(null).map { it.id })
        assertEquals(listOf("folder"), index.children("page").map { it.id })
        assertEquals(listOf("child"), index.children("folder").map { it.id })
        assertEquals(listOf("page", "folder", "child", "inner"), index.breadcrumbs("inner").map { it.id })
        assertTrue(index.nodes.getValue("folder").isFolder)
        assertFalse(index.nodes.getValue("page").isFolder)
        assertEquals(setOf("folder", "child", "inner"), index.descendantIds("page"))
    }

    @Test fun promotedLegacyFolderKeepsOneIdentityAndExistingFolderAssignments() {
        val legacy = NoteFolder(id = "legacy", name = "old folder")
        val promoted = page("legacy", "parent", folder = true).let { note ->
            val page = note.document.knowledge!!
            val properties = page["properties"] as JsonObject
            note.copy(document = note.document.copy(knowledge = JsonObject(page + ("properties" to JsonObject(properties +
                (KNOWLEDGE_LEGACY_FOLDER_PROPERTY to buildJsonObject { put("kind", "checkbox"); put("value", true) }))))))
        }
        val existing = page("existing").copy(folderId = "legacy")
        val index = buildKnowledgeHierarchy(listOf(page("parent"), promoted, existing), listOf(legacy))
        assertEquals(3, index.nodes.size)
        assertSame(promoted, index.nodes.getValue("legacy").note)
        assertTrue(index.nodes.getValue("legacy").isLegacyFolder)
        assertSame(legacy, index.nodes.getValue("legacy").legacyFolder)
        assertTrue(index.nodes.getValue("legacy").writable)
        assertFalse(index.issues.containsKey("legacy"))
        assertEquals("legacy", index.parentOf("existing"))
        assertEquals("parent", index.parentOf("legacy"))
        assertEquals(listOf("existing"), index.children("legacy").map { it.id })
    }

    @Test fun unconfirmedLegacyIdentityCollisionCannotBeMutatedOrUsedAsAParent() {
        val legacy = NoteFolder(id = "same", name = "Legacy original")
        val collision = page("same", "safe", folder = true).copy(title = "Document original", content = "Original body")
        fun withMarker(kind: String, value: JsonPrimitive) = collision.let { note ->
            val page = note.document.knowledge!!
            val properties = page["properties"] as JsonObject
            note.copy(document = note.document.copy(knowledge = JsonObject(page + ("properties" to JsonObject(properties +
                (KNOWLEDGE_LEGACY_FOLDER_PROPERTY to buildJsonObject { put("kind", kind); put("value", value) }))))))
        }
        for (unconfirmed in listOf(collision, withMarker("checkbox", JsonPrimitive(false)), withMarker("text", JsonPrimitive("true")))) {
            val index = buildKnowledgeHierarchy(listOf(page("safe"), unconfirmed, page("child", "same")), listOf(legacy))
            val node = index.nodes.getValue("same")
            assertEquals(KnowledgeHierarchyIssue.IDENTITY_CONFLICT, index.issues["same"])
            assertFalse(node.writable)
            assertNull(node.parentId)
            assertTrue(index.moveTargets("same").isEmpty())
            assertFalse(index.moveTargets("safe").any { it.id == "same" || it.id == "child" })
            assertFalse(index.mayCreateInside("same"))
            assertFalse(index.mayCreateInside("child"))
            assertSame(unconfirmed, node.note)
            assertSame(legacy, node.legacyFolder)
            assertEquals("Original body", node.note!!.content)
            assertEquals("Legacy original", node.legacyFolder!!.name)
            assertEquals(listOf("child"), index.children("same").map { it.id })
        }
    }

    @Test fun moveTargetsRejectSelfDescendantsAndProtectedParents() {
        val locked = page("locked").let { it.copy(document = it.document.copy(knowledge = JsonObject(it.document.knowledge!! + ("locked" to JsonPrimitive(true))))) }
        val encrypted = page("encrypted").copy(encryption = NoteEncryptionEnvelope(keyId = "private"))
        val index = buildKnowledgeHierarchy(listOf(page("a"), page("b", "a"), page("c", "b"), page("safe"), locked, page("locked-child", "locked"), encrypted), emptyList())
        assertEquals(setOf("safe"), index.moveTargets("a").map { it.id }.toSet())
        assertTrue(index.moveTargets("locked").isEmpty())
        assertTrue(index.moveTargets("encrypted").isEmpty())
        assertEquals("locked", index.parentOf("locked-child"))
        assertFalse(index.mayCreateInside("locked-child"))
        assertFalse(index.mayCreateInside("locked"))
        assertFalse(index.mayCreateInside("encrypted"))
        assertFalse(index.mayCreateInside("missing"))
        assertTrue(index.mayCreateInside(null))
        assertTrue(index.mayCreateInside("safe"))
    }

    @Test fun deletedMissingOrEncryptedParentCannotHideChildren() {
        val index = buildKnowledgeHierarchy(listOf(page("gone").copy(deletedAtEpochMillis = 1), page("orphan", "gone"),
            page("missing", "nowhere"), page("secret").copy(encryption = NoteEncryptionEnvelope(keyId = "private")),
            page("visible", "secret"), page("sticky").copy(kind = NoteEntryKind.STICKY)), emptyList())
        assertEquals(setOf("orphan", "missing", "secret", "visible"), index.children(null).map { it.id }.toSet())
        assertEquals(KnowledgeHierarchyIssue.MISSING_PARENT, index.issues["orphan"])
        assertEquals(KnowledgeHierarchyIssue.INACCESSIBLE_PARENT, index.issues["visible"])
        assertFalse(index.nodes.containsKey("sticky"))
        assertFalse(index.nodes.containsKey("gone"))
        assertFalse(index.mayCreateInside("missing"))
        assertFalse(index.mayCreateInside("visible"))
    }

    @Test fun corruptCyclesRemainReachableWithoutDiscardingRecords() {
        val index = buildKnowledgeHierarchy(listOf(page("a", "b"), page("b", "a"), page("child", "a"), page("self", "self")), emptyList())
        assertEquals(setOf("a", "b", "self"), index.children(null).map { it.id }.toSet())
        assertEquals(listOf("child"), index.children("a").map { it.id })
        assertEquals(setOf("a", "b", "self"), index.issues.keys)
        assertEquals(setOf("child"), index.descendantIds("a"))
        assertEquals(listOf("a", "child"), index.breadcrumbs("child").map { it.id })
    }

    @Test fun deepHierarchyUsesIterativeTraversalAndPreservesScope() {
        val notes = (0 until 3000).map { page("p$it", if (it == 0) null else "p${it - 1}") }
        val index = buildKnowledgeHierarchy(notes, emptyList())
        assertEquals(2999, index.ancestors("p2999").size)
        assertEquals(2999, index.descendantIds("p0").size)
        assertTrue(index.containsInScope("p2999", "p0"))
        assertFalse(index.containsInScope("p0", "p2999"))
        assertFalse(index.containsInScope("missing", null))
    }

    @Test fun hierarchyProjectionDoesNotRewritePageBodiesAttachmentsHistoryOrMetadata() {
        val original = page("a").copy(content = "source", revisions = listOf(NoteRevisionSnapshot(id = "revision")),
            versions = listOf(NoteVersionSnapshot(id = "version")), attachments = listOf(NoteAttachment(id = "asset")))
        val index = buildKnowledgeHierarchy(listOf(original, page("b", "a")), emptyList())
        assertSame(original, index.nodes.getValue("a").note)
        assertEquals("source", original.content)
        assertEquals("body of a", original.document.blocks.single().text)
        assertEquals("asset", original.attachments.single().id)
        assertEquals("revision", original.revisions.single().id)
        assertEquals("version", original.versions.single().id)
    }

    @Test fun hierarchyMetadataKeepsPlainEditorWhileComplexFieldsKeepStructuredProtection() {
        assertFalse(page("plain", "parent").requiresStructuredKnowledgeReader())
        assertFalse(page("folder", "parent", true).requiresStructuredKnowledgeReader())
        val defaults = buildJsonObject {
            put("version", 1); put("parentId", "parent"); put("icon", ""); put("cover", ""); put("order", 0)
            put("fullWidth", false); put("smallText", false); put("locked", false); put("template", false)
            put("tags", JsonArray(emptyList())); put("comments", JsonArray(emptyList())); put("properties", JsonObject(emptyMap())); put("database", JsonNull)
        }
        val plain = page("plain").let { it.copy(document = it.document.copy(knowledge = defaults)) }
        assertFalse(plain.requiresStructuredKnowledgeReader())
        val legacyPage = plain.copy(document = plain.document.copy(knowledge = JsonObject(defaults + ("properties" to buildJsonObject {
            put(KNOWLEDGE_LEGACY_FOLDER_PROPERTY, buildJsonObject { put("kind", "checkbox"); put("value", true) })
        }))))
        assertFalse(legacyPage.requiresStructuredKnowledgeReader())
        for (field in listOf("database", "properties", "tags", "locked", "version", "unknown")) {
            val value: JsonElement = when (field) {
                "tags" -> JsonArray(listOf(JsonPrimitive("tag")))
                "properties" -> buildJsonObject { put("status", buildJsonObject { put("kind", "text"); put("value", "active") }) }
                "version" -> JsonPrimitive(999)
                else -> JsonPrimitive(true)
            }
            val structured = plain.copy(document = plain.document.copy(knowledge = JsonObject(defaults + (field to value))))
            assertTrue(field, structured.requiresStructuredKnowledgeReader())
        }
        assertTrue(plain.copy(document = plain.document.copy(blocks = listOf(NoteBlock(knowledge = buildJsonObject { put("kind", "todo") })))).requiresStructuredKnowledgeReader())
    }


    @Test fun locationUsesRealHierarchyBeforeLegacyFolderAndTracksCurrentSnapshot() {
        val old = NoteFolder(id = "old", name = "Legacy")
        val child = page("child", "folder").copy(folderId = "old")
        val notes = listOf(page("root"), page("folder", "root", true), child)
        val first = buildKnowledgeHierarchy(notes, listOf(old)).location("child")
        assertTrue(first.available)
        assertTrue(first.canMove)
        assertNull(first.issue)
        assertEquals("folder", first.parentId)
        assertEquals(listOf("root", "folder"), first.ancestors.map { it.id })
        val renamed = notes.map { if (it.id == "folder") it.copy(title = "Renamed") else it }
        assertEquals("Renamed", buildKnowledgeHierarchy(renamed, listOf(old)).location("child").ancestors.last().title)
        val moved = renamed.filterNot { it.id == "child" } + page("child", "root")
        val current = buildKnowledgeHierarchy(moved, listOf(old)).location("child")
        assertEquals("root", current.parentId)
        assertEquals(listOf("root"), current.ancestors.map { it.id })
        assertEquals("folder", first.parentId)
    }

    @Test fun locationDistinguishesRootLegacyAndMissingRecord() {
        val legacy = NoteFolder(id = "legacy", name = "Old folder")
        val index = buildKnowledgeHierarchy(listOf(page("root"), page("child").copy(folderId = "legacy")), listOf(legacy))
        val root = index.location("root")
        assertTrue(root.available)
        assertNull(root.parentId)
        assertTrue(root.ancestors.isEmpty())
        assertNull(root.issue)
        val old = index.location("child")
        assertEquals("legacy", old.parentId)
        assertEquals(listOf("legacy"), old.ancestors.map { it.id })
        val absent = index.location("pending-or-deleted")
        assertFalse(absent.available)
        assertFalse(absent.canMove)
        assertTrue(absent.ancestors.isEmpty())
    }

    @Test fun locationRetainsOwnAndAncestorIssuesInsteadOfClaimingNormalRoot() {
        val secret = page("secret").copy(encryption = NoteEncryptionEnvelope(keyId = "key"))
        val index = buildKnowledgeHierarchy(listOf(page("orphan", "gone"), page("descendant", "orphan"), secret,
            page("hidden-parent", "secret"), page("a", "b"), page("b", "a")), emptyList())
        for (id in listOf("orphan", "descendant")) {
            assertEquals(KnowledgeHierarchyIssue.MISSING_PARENT, index.location(id).issue)
            assertTrue(index.location(id).available)
        }
        assertEquals(listOf("orphan"), index.location("descendant").ancestors.map { it.id })
        assertEquals(KnowledgeHierarchyIssue.INACCESSIBLE_PARENT, index.location("hidden-parent").issue)
        assertEquals(KnowledgeHierarchyIssue.CYCLE, index.location("a").issue)
        assertNull(index.location("a").parentId)
    }

    @Test fun locationCannotMoveEncryptedLockedUnsupportedConflictingOrMissingRecords() {
        fun flag(note: NoteEntry, key: String, value: JsonElement) = note.copy(document = note.document.copy(
            knowledge = JsonObject(note.document.knowledge!! + (key to value))))
        val notes = listOf(page("ok"), page("encrypted").copy(encryption = NoteEncryptionEnvelope(keyId = "key")),
            flag(page("locked"), "locked", JsonPrimitive(true)), flag(page("future"), "version", JsonPrimitive(999)), page("conflict"))
        val index = buildKnowledgeHierarchy(notes, listOf(NoteFolder(id = "conflict", name = "original")))
        assertTrue(index.location("ok").canMove)
        for (id in listOf("encrypted", "locked", "future", "conflict", "missing")) assertFalse(id, index.location(id).canMove)
        assertEquals(KnowledgeHierarchyIssue.IDENTITY_CONFLICT, index.location("conflict").issue)
    }

    @Test fun moveSelectionRejectsNoOpDescendantsAndProtectedOrMissingTargets() {
        val locked = page("locked", "missing-parent").let { it.copy(document = it.document.copy(
            knowledge = JsonObject(it.document.knowledge!! + ("locked" to JsonPrimitive(true))))) }
        val encrypted = page("encrypted").copy(encryption = NoteEncryptionEnvelope(keyId = "private"))
        val future = page("future", "missing-parent").let { it.copy(document = it.document.copy(
            knowledge = JsonObject(it.document.knowledge!! + ("version" to JsonPrimitive(999))))) }
        val index = buildKnowledgeHierarchy(listOf(page("root"), page("child", "root"), page("grandchild", "child"),
            page("safe"), locked, page("under-locked", "locked"), encrypted, future, page("conflict")),
            listOf(NoteFolder(id = "conflict", name = "old")))
        assertTrue(index.canMoveTo("child", null))
        assertTrue(index.canMoveTo("child", "safe"))
        for (target in listOf("root", "child", "grandchild", "locked", "under-locked", "encrypted", "missing", "conflict")) {
            assertFalse(target, index.canMoveTo("child", target))
        }
        assertFalse(index.canMoveTo("root", null))
        for (id in listOf("locked", "future")) assertEquals(KnowledgeHierarchyIssue.MISSING_PARENT, index.location(id).issue)
        for (id in listOf("locked", "encrypted", "future", "missing", "conflict")) {
            assertFalse(id, index.canMoveTo(id, null))
            assertFalse(id, index.canMoveTo(id, "safe"))
        }
    }

    @Test fun moveSelectionAllowsExplicitRootRepairForBrokenParentsOnly() {
        val secret = page("secret").copy(encryption = NoteEncryptionEnvelope(keyId = "key"))
        val index = buildKnowledgeHierarchy(listOf(page("orphan", "gone"), secret, page("hidden", "secret"),
            page("a", "b"), page("b", "a"), page("root"), page("child", "root")), emptyList())
        for (id in listOf("orphan", "hidden", "a", "b")) {
            assertNull(index.parentOf(id))
            assertTrue(id, index.canMoveTo(id, null))
        }
        assertFalse(index.canMoveTo("root", null))
        assertFalse(index.canMoveTo("child", "root"))
        val repaired = buildKnowledgeHierarchy(listOf(page("orphan")), emptyList())
        assertFalse(repaired.canMoveTo("orphan", null))
    }

    @Test fun markerCannotMasqueradeAsFolderWhenEncryptedOrWronglyTyped() {
        val folder = page("folder", folder = true)
        assertTrue(folder.isKnowledgeFolder())
        assertFalse(folder.copy(encryption = NoteEncryptionEnvelope(keyId = "private"), encryptionUnlocked = true).isKnowledgeFolder())
        val corrupt = folder.copy(document = folder.document.copy(knowledge = buildJsonObject {
            put("properties", buildJsonObject { put(KNOWLEDGE_CONTAINER_KIND_PROPERTY, buildJsonObject { put("kind", "number"); put("value", "folder") }) })
        }))
        assertFalse(corrupt.isKnowledgeFolder())
        assertTrue(corrupt.requiresStructuredKnowledgeReader())
    }
}
"####;
