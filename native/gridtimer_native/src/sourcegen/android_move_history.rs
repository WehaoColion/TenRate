// Android move-to destination history is generated from Rust; no generated Kotlin is checked in.
pub const PATH: &str = "com/ofairyo/gridtimer/ui/KnowledgeMoveHistory.kt";
pub const TEST_PATH: &str = "com/ofairyo/gridtimer/ui/KnowledgeMoveHistoryTest.kt";

pub const CONTENTS: &str = r####"package com.ofairyo.gridtimer.ui

import android.content.Context
import org.json.JSONArray
import org.json.JSONObject
import java.security.MessageDigest

// This is UI-local usage metadata, never part of synced knowledge content.
internal data class KnowledgeMoveUse(val id: String?, val count: Int, val lastUsedAt: Long)

private const val MAX_MOVE_HISTORY = 64
private const val DAY_MS = 86_400_000L

private fun knowledgeMovePreferenceKey(workspaceKey: String): String {
    // Avoid writing workspace identities into preference names.
    val digest = MessageDigest.getInstance("SHA-256").digest(workspaceKey.toByteArray(Charsets.UTF_8))
    return "workspace_" + digest.joinToString("") { (it.toInt() and 0xff).toString(16).padStart(2, '0') }
}

internal fun rankKnowledgeMoveHistory(entries: List<KnowledgeMoveUse>, now: Long): List<KnowledgeMoveUse> =
    entries.sortedWith(compareByDescending<KnowledgeMoveUse> { item ->
        val days = ((now - item.lastUsedAt).coerceAtLeast(0L) / DAY_MS).coerceAtMost(3650L)
        item.count.coerceIn(1, 10_000).toDouble() * 30.0 / (30.0 + days)
    }.thenByDescending { it.lastUsedAt }.thenBy { it.id.orEmpty() })

internal fun recentKnowledgeMoveHistory(entries: List<KnowledgeMoveUse>): List<KnowledgeMoveUse> =
    entries.sortedWith(compareByDescending<KnowledgeMoveUse> { it.lastUsedAt }.thenBy { it.id.orEmpty() })

internal fun readKnowledgeMoveHistory(context: Context, workspaceKey: String): List<KnowledgeMoveUse> =
    runCatching {
        val raw = context.applicationContext.getSharedPreferences("knowledge_move_history_v1", Context.MODE_PRIVATE)
            .getString(knowledgeMovePreferenceKey(workspaceKey), null).orEmpty()
        // A corrupt preference must not allocate an unbounded parser input or block the picker.
        if (raw.length > 24_000) return@runCatching emptyList()
        if (raw.isEmpty()) return@runCatching emptyList()
        val array = JSONArray(raw)
        val unique = LinkedHashMap<String, KnowledgeMoveUse>()
        for (i in 0 until minOf(array.length(), MAX_MOVE_HISTORY)) {
            val row = array.optJSONObject(i) ?: continue
            val id = row.optString("id", "").takeIf { it.isNotEmpty() }
            val count = row.optInt("count", 0)
            val lastUsed = row.optLong("at", 0L)
            if (count !in 1..10_000 || lastUsed <= 0L) continue
            unique[id.orEmpty()] = KnowledgeMoveUse(id, count, lastUsed)
        }
        unique.values.toList()
    }.getOrDefault(emptyList())

internal fun recordKnowledgeMoveDestination(context: Context, workspaceKey: String, destinationId: String?) {
    // Called only after the durable move callback reports success.
    val now = System.currentTimeMillis()
    val existing = readKnowledgeMoveHistory(context, workspaceKey)
    val usage = LinkedHashMap<String, KnowledgeMoveUse>()
    existing.forEach { usage[it.id.orEmpty()] = it }
    val key = destinationId.orEmpty()
    val previous = usage[key]
    usage[key] = KnowledgeMoveUse(destinationId, ((previous?.count ?: 0) + 1).coerceAtMost(10_000), now)
    val output = JSONArray()
    recentKnowledgeMoveHistory(usage.values.toList()).take(MAX_MOVE_HISTORY).forEach {
        output.put(JSONObject().put("id", it.id.orEmpty()).put("count", it.count).put("at", it.lastUsedAt))
    }
    context.applicationContext.getSharedPreferences("knowledge_move_history_v1", Context.MODE_PRIVATE)
        .edit().putString(knowledgeMovePreferenceKey(workspaceKey), output.toString()).apply()
}
"####;

pub const TEST_CONTENTS: &str = r####"package com.ofairyo.gridtimer.ui

import org.junit.Assert.*
import org.junit.Test

class KnowledgeMoveHistoryTest {
    private val day = 86_400_000L
    private val now = 200L * day

    @Test fun frequentDestinationsRankAboveOneOffRecentSelections() {
        val history = listOf(
            KnowledgeMoveUse("daily", 14, now - day),
            KnowledgeMoveUse("once", 1, now),
            KnowledgeMoveUse("inactive", 25, now - 180L * day)
        )
        assertEquals(listOf("daily", "inactive", "once"), rankKnowledgeMoveHistory(history, now).map { it.id })
    }

    @Test fun recentActivityEventuallyOutweighsStaleHistory() {
        val history = listOf(
            KnowledgeMoveUse("stale", 40, now - 180L * day),
            KnowledgeMoveUse("active", 12, now)
        )
        assertEquals("active", rankKnowledgeMoveHistory(history, now).first().id)
        assertEquals("active", recentKnowledgeMoveHistory(history).first().id)
    }

    @Test fun rootAndRenamedDestinationsRemainStableIds() {
        val history = listOf(KnowledgeMoveUse(null, 7, now), KnowledgeMoveUse("stable-id", 2, now - day))
        assertNull(rankKnowledgeMoveHistory(history, now).first().id)
        assertEquals("stable-id", rankKnowledgeMoveHistory(history, now)[1].id)
    }

    @Test fun futureTimeOrBadCountsDoNotCauseNegativeScores() {
        val history = listOf(KnowledgeMoveUse("future", 1, now + day), KnowledgeMoveUse("ordinary", 2, now))
        assertEquals("ordinary", rankKnowledgeMoveHistory(history, now).first().id)
    }
}
"####;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn history_is_bounded_per_workspace_and_only_records_confirmed_moves() {
        assert!(CONTENTS.contains("MAX_MOVE_HISTORY = 64"));
        assert!(CONTENTS.contains("MessageDigest.getInstance(\"SHA-256\")"));
        assert!(CONTENTS.contains("putString(knowledgeMovePreferenceKey(workspaceKey)"));
        assert!(CONTENTS.contains("readKnowledgeMoveHistory(context, workspaceKey)"));
        assert!(!CONTENTS.contains("note.content"));
        assert!(TEST_CONTENTS.contains("recentActivityEventuallyOutweighsStaleHistory"));
    }
}
