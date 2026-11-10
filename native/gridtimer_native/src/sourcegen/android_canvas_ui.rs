// v2.23.4.12 Android - Stabilize canvas pan and card drag through save acknowledgement.
// v2.22.49.7 Android - Reuse edge geometry and draw buffers; isolate gesture zoom recomposition.
// v2.22.49.6 Android - Keep multi-canvas creation and selection visible and guard capacity.
// v2.22.49.5 Android - Reuse committed graph lists when applying validated viewport replies.
// v2.22.49.4 Android - Touch canvas UI emitted from the Rust source tree.
pub const PATH: &str = "com/ofairyo/gridtimer/ui/KnowledgeCanvasScreen.kt";
pub const TEST_PATH: &str = "com/ofairyo/gridtimer/ui/KnowledgeCanvasTest.kt";
pub const TEST_CONTENTS: &str = r####"package com.ofairyo.gridtimer.ui

import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.unit.IntSize
import com.ofairyo.gridtimer.data.*
import org.junit.Assert.*
import org.junit.Test
import kotlinx.coroutines.Dispatchers
import kotlinx.serialization.json.*
import java.util.concurrent.CountDownLatch
import java.util.concurrent.TimeUnit

class KnowledgeCanvasTest {
    private class Backend : CanvasBackend {
        val entered = CountDownLatch(1); val release = CountDownLatch(1); val closed = CountDownLatch(1)
        var fail = false
        var commandReply: String? = null
        var openedWorkspace = ""
        var commandedToken = ""
        var initial = """{"ok":true,"token":"bound-account","revision":0,"canvas":{"id":"board","title":"Board","centerX":0,"centerY":0,"zoom":1,"nodes":[],"edges":[]},"canvases":[{"id":"board","title":"Board"}]}"""
        override fun open(root: String, workspace: String): String { openedWorkspace=workspace; return initial }
        override fun command(token: String, request: String): String {
            commandedToken=token; entered.countDown(); check(release.await(5,TimeUnit.SECONDS))
            return if(fail) """{"ok":false,"error":"disk full"}""" else commandReply ?: initial.replace("\"revision\":0","\"revision\":1")
        }
        override fun close(token: String) { commandedToken=token; closed.countDown() }
    }
    private fun awaitReady(model: KnowledgeCanvasModel) {
        val deadline=System.nanoTime()+TimeUnit.SECONDS.toNanos(5)
        while(model.busy && System.nanoTime()<deadline) Thread.sleep(1)
        assertFalse("canvas worker timed out",model.busy)
    }
    @Test fun closingTheScreenDrainsAnAcceptedWriteBeforeReleasingItsAccountSession() {
        val backend=Backend(); val model=KnowledgeCanvasModel("root","account-a",backend,Dispatchers.Unconfined,Dispatchers.IO)
        awaitReady(model)
        assertTrue(model.submit(buildJsonObject { put("op","view") }))
        assertTrue(backend.entered.await(5,TimeUnit.SECONDS))
        model.close()
        assertFalse(backend.closed.await(50,TimeUnit.MILLISECONDS))
        backend.release.countDown()
        assertTrue(backend.closed.await(5,TimeUnit.SECONDS))
        assertEquals("account-a",backend.openedWorkspace); assertEquals("bound-account",backend.commandedToken)
        assertEquals(1L,model.reply?.revision)
    }
    @Test fun failedSaveKeepsThePreviousCommittedStateAndDoesNotReportSuccess() {
        val backend=Backend().apply { fail=true; release.countDown() }
        val model=KnowledgeCanvasModel("root","account-a",backend,Dispatchers.Unconfined,Dispatchers.IO)
        awaitReady(model); val committed=model.reply
        assertTrue(model.submit(buildJsonObject { put("op","addNote") }))
        awaitReady(model)
        assertSame(committed,model.reply); assertFalse(model.lastSucceeded); assertTrue(model.error.isNotBlank())
        model.close(); assertTrue(backend.closed.await(5,TimeUnit.SECONDS))
    }
    @Test fun gesturesAreVisuallyQuietWithoutRelaxingTheMutationGate() {
        val backend = Backend()
        val model = KnowledgeCanvasModel("root", "account-a", backend, Dispatchers.Unconfined, Dispatchers.IO)
        awaitReady(model)
        val gesture = buildJsonObject { put("op", "gesture"); put("viewX", 8); put("viewY", 9); put("zoom", 1) }
        assertTrue(model.submit(gesture))
        assertTrue(backend.entered.await(5, TimeUnit.SECONDS))
        assertTrue(model.busy)
        assertTrue(model.state.value.quietGesture)
        assertFalse(model.submit(gesture))
        backend.release.countDown()
        awaitReady(model)
        assertFalse(model.state.value.quietGesture)
        assertTrue(model.lastSucceeded)
        model.close()
    }
    @Test fun regularEditsStillExposeTheSavingState() {
        val backend = Backend()
        val model = KnowledgeCanvasModel("root", "account-a", backend, Dispatchers.Unconfined, Dispatchers.IO)
        awaitReady(model)
        assertTrue(model.submit(buildJsonObject { put("op", "addNote") }))
        assertTrue(backend.entered.await(5, TimeUnit.SECONDS))
        assertTrue(model.busy)
        assertFalse(model.state.value.quietGesture)
        backend.release.countDown()
        awaitReady(model)
        model.close()
    }
    @Test fun dragPreviewKeepsCardAndIncidentEdgesAtTheSamePositionOnAck() {
        val original = node("a", 100f, 250f)
        val acknowledged = original.copy(x = 165f, y = 300f)
        val preview = CanvasDragPreview("a", 165f, 300f)
        assertEquals(Offset(65f, 50f), preview.offset(original))
        assertEquals(Offset.Zero, preview.offset(acknowledged))
        assertEquals(165f, original.x + preview.offset(original).x, 0f)
        assertEquals(165f, acknowledged.x + preview.offset(acknowledged).x, 0f)
    }
    @Test fun busyOrClosedCanvasCannotQueueAnotherMutation() {
        val backend=Backend(); val model=KnowledgeCanvasModel("root","account-a",backend,Dispatchers.Unconfined,Dispatchers.IO)
        awaitReady(model)
        val request=buildJsonObject { put("op","view") }
        assertTrue(model.submit(request)); assertFalse(model.submit(request))
        model.close(); assertFalse(model.submit(request)); backend.release.countDown()
        assertTrue(backend.closed.await(5,TimeUnit.SECONDS))
    }
    private fun node(id: String, x: Float = 0f, y: Float = 0f) = CanvasNode(id,"note","","card",x,y,256f,168f,"blue")
    private fun boardReply(count: Int, active: Int, revision: Long) = Json.encodeToString(CanvasReply.serializer(), CanvasReply(ok=true, token="bound-account", revision=revision,
        canvas=CanvasBoard("b$active","Board $active",0f,0f,1f,emptyList(),emptyList()),
        canvases=List(count) { CanvasSummary("b$it","Board $it") }))
    @Test fun creationDisablesWhileSavingAndAtTheBoardLimit() {
        val backend=Backend().apply { initial=boardReply(127,0,0); commandReply=boardReply(128,127,1) }
        val model=KnowledgeCanvasModel("root","account-a",backend,Dispatchers.Unconfined,Dispatchers.IO)
        awaitReady(model); assertTrue(model.canCreateBoard)
        assertTrue(model.submit(buildJsonObject { put("op","new"); put("title","Next") }))
        assertTrue(backend.entered.await(5,TimeUnit.SECONDS)); assertFalse(model.canCreateBoard)
        backend.release.countDown(); awaitReady(model)
        assertEquals(128,model.reply!!.canvases.size); assertFalse(model.canCreateBoard)
        assertFalse(model.submit(buildJsonObject { put("op","new"); put("title","Overflow") }))
        model.close(); assertTrue(backend.closed.await(5,TimeUnit.SECONDS))
    }
    @Test fun createAndSwitchKeepBothBoardsAndFailedSwitchKeepsTheCommittedBoard() {
        val backend=Backend().apply { initial=boardReply(1,0,0); commandReply=boardReply(2,1,1); release.countDown() }
        val model=KnowledgeCanvasModel("root","account-a",backend,Dispatchers.Unconfined,Dispatchers.IO)
        awaitReady(model); assertTrue(model.canCreateBoard)
        assertTrue(model.submit(buildJsonObject { put("op","new"); put("title","Second") })); awaitReady(model)
        assertEquals("b1",model.reply!!.canvas!!.id); assertEquals(listOf("b0","b1"),model.reply!!.canvases.map { it.id })
        val committed=model.reply; backend.fail=true
        val switch=buildJsonObject { put("op","switch"); put("id","b0") }
        assertTrue(model.submit(switch)); awaitReady(model)
        assertSame(committed,model.reply); assertFalse(model.lastSucceeded)
        backend.fail=false; backend.commandReply=boardReply(2,0,2)
        assertTrue(model.submit(switch)); awaitReady(model)
        assertEquals("b0",model.reply!!.canvas!!.id); assertEquals(2,model.reply!!.canvases.size); assertTrue(model.lastSucceeded)
        model.close(); assertTrue(backend.closed.await(5,TimeUnit.SECONDS))
    }
    private fun committed() = CanvasReply(ok=true, token="account", revision=4, canvas=CanvasBoard("board","Board",0f,0f,1f,listOf(node("a"),node("b")),listOf(CanvasEdge("edge","a","b"))))
    private fun viewportReply() = CanvasReply(ok=true, token="account", revision=5, baseRevision=4, viewport=CanvasViewReply("board",20f,30f,0.8f),canUndo=true)
    @Test fun viewportReplyReusesCommittedCardsAndConnectionsAndUpdatesHistoryState() {
        val previous=committed(); val merged=mergeCanvasReply(previous,viewportReply())!!
        assertSame(previous.canvas!!.nodes,merged.canvas!!.nodes); assertSame(previous.canvas.edges,merged.canvas.edges)
        assertEquals(20f,merged.canvas.centerX,0f); assertEquals(5L,merged.revision); assertTrue(merged.canUndo)
    }
    @Test fun viewportReplyRejectsAnotherAccountBoardOrRevisionAndInvalidCoordinates() {
        val previous=committed(); val next=viewportReply()
        assertNull(mergeCanvasReply(null,next))
        assertNull(mergeCanvasReply(previous,next.copy(token="other")))
        assertNull(mergeCanvasReply(previous,next.copy(baseRevision=3)))
        assertNull(mergeCanvasReply(previous,next.copy(revision=3)))
        assertNull(mergeCanvasReply(previous,next.copy(viewport=next.viewport!!.copy(id="other"))))
        assertNull(mergeCanvasReply(previous,next.copy(viewport=next.viewport!!.copy(zoom=Float.NaN))))
        assertNull(mergeCanvasReply(previous,next.copy(viewport=next.viewport!!.copy(centerX=300000f))))
    }
    @Test fun workerAcceptsCompactSaveAndRejectsUnrelatedCompactAcknowledgement() {
        val backend=Backend().apply { release.countDown(); commandReply="""{"ok":true,"token":"bound-account","revision":1,"baseRevision":0,"viewport":{"id":"board","centerX":10,"centerY":20,"zoom":1}}""" }
        val model=KnowledgeCanvasModel("root","account-a",backend,Dispatchers.Unconfined,Dispatchers.IO)
        awaitReady(model); val initial=model.reply!!
        assertTrue(model.submit(buildJsonObject { put("op","view") })); awaitReady(model)
        assertTrue(model.lastSucceeded); assertSame(initial.canvas!!.nodes,model.reply!!.canvas!!.nodes)
        val committed=model.reply
        backend.commandReply=backend.commandReply!!.replace("\"bound-account\"","\"foreign-account\"")
        assertTrue(model.submit(buildJsonObject { put("op","view") })); awaitReady(model)
        assertFalse(model.lastSucceeded); assertSame(committed,model.reply)
        model.close(); assertTrue(backend.closed.await(5,TimeUnit.SECONDS))
    }
    @Test fun overviewContainsAllCardsAndPreservesLegacyZoomBounds() {
        val nodes = listOf(node("far-left",-9000f,-4000f),node("far-right",12000f,9000f))
        val area = IntSize(400,700)
        val overview = fitCanvasOverview(nodes,area)
        nodes.forEach { n ->
            val a = overview.screenAt(Offset(n.x,n.y))
            val b = overview.screenAt(Offset(n.x+n.width,n.y+n.height))
            assertTrue(a.x >= 23f && a.y >= 23f)
            assertTrue(b.x <= area.width-23f && b.y <= area.height-23f)
            val roundTrip = overview.worldAt(a)
            assertEquals(n.x,roundTrip.x,0.02f)
            assertEquals(n.y,roundTrip.y,0.02f)
        }
        val empty = fitCanvasOverview(emptyList(),area)
        assertTrue(empty.scale > 0f)
    }
    @Test fun zoomKeepsTheTouchedWorldPointUnderTheMovingFingers() {
        val size = IntSize(1080,1800)
        val view = CanvasViewport(300f,120f,0.9f)
        val touch = Offset(220f,460f); val pan = Offset(50f,-30f)
        val before = view.world(touch,size,3f)
        val after = view.transformed(touch,pan,1.8f,size,3f).world(touch+pan,size,3f)
        assertEquals(before.x,after.x,0.001f); assertEquals(before.y,after.y,0.001f)
    }
    @Test fun zoomLimitsCannotProduceInvalidCoordinates() {
        val size=IntSize(600,800); val view=CanvasViewport(0f,0f,1f)
        assertEquals(2.5f,view.transformed(Offset.Zero,Offset.Zero,100f,size,2f).zoom,0f)
        assertEquals(0.35f,view.transformed(Offset.Zero,Offset.Zero,0.01f,size,2f).zoom,0f)
        assertEquals(view,view.transformed(Offset.Zero,Offset.Zero,Float.NaN,size,2f))
        assertEquals(view,view.transformed(Offset.Zero,Offset(Float.NaN,0f),1f,size,2f))
    }
    @Test fun selectionChoosesTheTopCardAndRejectsEmptySpace() {
        val nodes=listOf(node("back"),node("front",20f,20f))
        assertEquals("front",canvasHit(nodes,Offset(50f,50f))?.id)
        assertNull(canvasHit(nodes,Offset(-1f,-1f)))
        assertEquals("back",canvasHit(nodes,Offset(0f,0f))?.id)
    }
    @Test fun viewportCullsOffscreenCardsButKeepsPartiallyVisibleOnes() {
        val view=CanvasViewport(0f,0f,1f); val size=IntSize(400,600)
        assertFalse(canvasVisible(node("far",10000f,10000f),view,size,2f))
        assertTrue(canvasVisible(node("partial",99f,0f),view,size,2f))
        assertFalse(canvasVisible(node("outside",101f,0f),view,size,2f))
    }
    @Test fun visibleConnectionsKeepCrossingEdgesAndRejectMissingOrCoincidentEndpoints() {
        val nodes=listOf(node("left",-1000f,-84f),node("right",1000f,-84f))
        val edge=CanvasEdge("crossing","left","right")
        val renderer=CanvasEdgeRenderer(nodes,listOf(edge))
        assertTrue(renderer.updateVisible(-100f,-100f,100f,100f,null,Offset.Zero)>0)
        assertEquals(0,renderer.updateVisible(-100f,1000f,100f,1200f,null,Offset.Zero))
        assertEquals(0,CanvasEdgeRenderer(nodes,listOf(edge.copy(to="missing"))).updateVisible(-2000f,-2000f,2000f,2000f,null,Offset.Zero))
        assertEquals(0,CanvasEdgeRenderer(listOf(node("left"),node("right")),listOf(edge)).updateVisible(-2000f,-2000f,2000f,2000f,null,Offset.Zero))
    }
    @Test fun draggingUpdatesIncidentConnectionsAndCancellingRestoresTheSavedGeometry() {
        val nodes=listOf(node("a"),node("b",1000f),node("c",0f,1000f),node("d",1000f,1000f))
        val renderer=CanvasEdgeRenderer(nodes,listOf(CanvasEdge("ab","a","b"),CanvasEdge("cd","c","d")))
        val buffer=renderer.visibleLines
        fun frame(drag: String? = null, delta: Offset = Offset.Zero) = renderer.updateVisible(-5000f,-5000f,5000f,5000f,drag,delta)
        val count=frame(); assertTrue(count>0); val saved=buffer.copyOf(count)
        assertEquals(count,frame("a",Offset(300f,250f)))
        assertSame(buffer,renderer.visibleLines)
        assertFalse(saved.take(count/2)==buffer.take(count/2))
        assertArrayEquals(saved.copyOfRange(count/2,count),buffer.copyOfRange(count/2,count),0f)
        assertTrue(buffer.take(count).all { it.isFinite() })
        assertEquals(count,frame()); assertArrayEquals(saved,buffer.copyOf(count),0f)
        // Drag an endpoint into a previously empty viewport, then leave without committing.
        assertEquals(0,renderer.updateVisible(0f,3000f,2000f,3200f,null,Offset.Zero))
        assertTrue(renderer.updateVisible(0f,3000f,2000f,3200f,"a",Offset(0f,3200f))>0)
        assertEquals(0,renderer.updateVisible(0f,3000f,2000f,3200f,null,Offset.Zero))
    }
    @Test fun encryptedPagePreviewsNeverExposeAnUnlockedCopy() {
        val secret = NoteEntry(id="sealed",title="secret title",content="secret body",encryption=NoteEncryptionEnvelope(keyId="key"),encryptionUnlocked=true)
        val preview=canvasPageLabel(secret)
        assertFalse(preview.toString().contains("secret"))
        assertEquals(canvasPageLabel(secret.copy(encryptionUnlocked=false)),preview)
    }
    @Test fun deletedPagesNeverLeakTheirOldPreview() {
        val deleted=NoteEntry(id="deleted",title="private removed title",content="private removed body",deletedAtEpochMillis=1L)
        assertEquals(canvasPageLabel(null),canvasPageLabel(deleted))
        assertFalse(canvasPageLabel(deleted).toString().contains("private"))
    }
}
"####;

pub fn render(path: &str, source: &str) -> Result<String, String> {
    if !path.ends_with("/NoteStudioSheet.kt") {
        return Ok(source.to_owned());
    }
    let replacements = [
        ("    var knowledgeDialogVisible by remember { mutableStateOf(false) }", "    var knowledgeDialogVisible by remember { mutableStateOf(false) }\n    val canvasWorkspaceKey = LocalNoteMediaWorkspaceKey.current\n    var canvasVisible by rememberSaveable(canvasWorkspaceKey) { mutableStateOf(false) }"),
        ("    LaunchedEffect(selectedNoteId != null) {\n        onEditingStateChange(selectedNoteId != null)\n    }", "    LaunchedEffect(selectedNoteId != null, canvasVisible) {\n        onEditingStateChange(selectedNoteId != null || canvasVisible)\n    }"),
        ("    val noteStudioContent: @Composable () -> Unit = {\n        AnimatedContent", "    val noteStudioContent: @Composable () -> Unit = {\n        if (canvasVisible && selectedNoteId == null) {\n            KnowledgeCanvasScreen(appData = appData, onBack = { canvasVisible = false }, onOpenPage = { note ->\n                pendingCreatedNote = null\n                if (note.isEncryptionLocked()) { unlockTarget = note; unlockError = \"\" }\n                else { unlockedNote = null; selectedNoteId = note.id; searchLocateQuery = null }\n            }, onCreatePage = { title, onCreated ->\n                val newPageId = java.util.UUID.randomUUID().toString()\n                viewModel.knowledgeContainerAction(\"create_page\", newPageId, \"\", title, canvasWorkspaceKey) { ok, _ ->\n                    if (viewModel.currentWorkspaceKey() == canvasWorkspaceKey) onCreated(if (ok) newPageId else null)\n                }\n            })\n        } else AnimatedContent"),
        ("                    onSelectFolder = viewModel::selectNoteFolder,", "                    onOpenCanvas = { canvasVisible = true },\n                    onSelectFolder = viewModel::selectNoteFolder,"),
        ("    onAskKnowledge: () -> Unit,\n    onSelectFolder:", "    onAskKnowledge: () -> Unit,\n    onOpenCanvas: () -> Unit,\n    onSelectFolder:"),
        ("        if (!trashMode) {\n            item {\n                KnowledgeAskEntryCard(", "        if (!trashMode) {\n            item {\n                androidx.compose.material3.OutlinedButton(onClick = onOpenCanvas, modifier = Modifier.fillMaxWidth()) {\n                    Text(\"画布\", style = MaterialTheme.typography.titleMedium)\n                    Spacer(Modifier.weight(1f))\n                    Text(\"卡片与连线\")\n                }\n            }\n            item {\n                KnowledgeAskEntryCard("),
    ];
    let mut result = source.to_owned();
    for (needle, replacement) in replacements {
        if !result.contains(needle) {
            return Err(format!("canvas integration anchor missing: {needle}"));
        }
        result = result.replacen(needle, replacement, 1);
    }
    Ok(result)
}

pub const CONTENTS: &str = r####"package com.ofairyo.gridtimer.ui

import android.graphics.Paint
import android.text.StaticLayout
import android.text.TextPaint
import android.text.Layout
import androidx.activity.compose.BackHandler
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.gestures.*
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clipToBounds
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.nativeCanvas
import androidx.compose.ui.graphics.toArgb
import androidx.compose.ui.input.pointer.*
import androidx.compose.ui.layout.onSizeChanged
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.IntSize
import androidx.compose.ui.unit.dp
import androidx.compose.ui.window.Dialog
import androidx.compose.ui.window.DialogProperties
import com.ofairyo.gridtimer.data.*
import kotlinx.coroutines.*
import kotlinx.coroutines.channels.Channel
import kotlinx.coroutines.flow.*
import kotlinx.serialization.Serializable
import kotlinx.serialization.json.*
import kotlin.math.*

internal object KnowledgeCanvasNative {
    init { System.loadLibrary("gridtimer_native") }
    external fun open(root: String, workspace: String): String
    external fun command(token: String, request: String): String
    external fun close(token: String)
}

internal interface CanvasBackend {
    fun open(root: String, workspace: String): String
    fun command(token: String, request: String): String
    fun close(token: String)
}
private object DeviceCanvasBackend : CanvasBackend {
    override fun open(root: String, workspace: String) = KnowledgeCanvasNative.open(root,workspace)
    override fun command(token: String, request: String) = KnowledgeCanvasNative.command(token,request)
    override fun close(token: String) = KnowledgeCanvasNative.close(token)
}

@Serializable internal data class CanvasNode(val id: String, val kind: String, val pageId: String, val text: String, val x: Float, val y: Float, val width: Float, val height: Float, val color: String)
@Serializable internal data class CanvasEdge(val id: String, val from: String, val to: String, val relation: String? = null)
@Serializable internal data class CanvasBoard(val id: String, val title: String, val centerX: Float, val centerY: Float, val zoom: Float, val nodes: List<CanvasNode>, val edges: List<CanvasEdge>, val template: String = "blank")
@Serializable internal data class CanvasSummary(val id: String, val title: String, val template: String = "blank")
@Serializable internal data class CanvasViewReply(val id: String, val centerX: Float, val centerY: Float, val zoom: Float)
@Serializable internal data class CanvasReply(val ok: Boolean, val token: String = "", val revision: Long = 0, val canvas: CanvasBoard? = null, val canvases: List<CanvasSummary> = emptyList(), val canUndo: Boolean = false, val canRedo: Boolean = false, val warning: String = "", val error: String = "", val viewport: CanvasViewReply? = null, val baseRevision: Long? = null)
internal data class CanvasWorkerState(val reply: CanvasReply? = null, val busy: Boolean = true, val error: String = "", val serial: Int = 0, val lastSucceeded: Boolean = false, val quietGesture: Boolean = false)

internal fun mergeCanvasReply(previous: CanvasReply?, next: CanvasReply): CanvasReply? {
    if (!next.ok || next.token.isBlank()) return null
    if (previous != null && (next.token != previous.token || next.revision < previous.revision)) return null
    if (next.canvas != null) return next.takeIf { it.viewport == null && it.baseRevision == null }
    val view = next.viewport ?: return null
    val board = previous?.canvas ?: return null
    if (view.id != board.id || next.baseRevision != previous.revision) return null
    if (!view.centerX.isFinite() || !view.centerY.isFinite() || abs(view.centerX) > 250000f || abs(view.centerY) > 250000f || !view.zoom.isFinite() || view.zoom !in 0.35f..2.5f) return null
    return next.copy(canvas = board.copy(centerX = view.centerX, centerY = view.centerY, zoom = view.zoom), viewport = null, baseRevision = null)
}

// This worker outlives a composition only long enough to finish accepted writes.
// Closing drains the accepted command and releases its native account session.
internal class KnowledgeCanvasModel(root: String, workspace: String, private val backend: CanvasBackend = DeviceCanvasBackend, uiDispatcher: CoroutineDispatcher = Dispatchers.Main.immediate, private val workDispatcher: CoroutineDispatcher = Dispatchers.IO) {
    private val updates = MutableStateFlow(CanvasWorkerState())
    val state: StateFlow<CanvasWorkerState> = updates.asStateFlow()
    val canCreateBoard: Boolean
        get() {
            val current = updates.value
            return !closed && (!current.busy || current.quietGesture) && current.reply?.let { it.canvas != null && it.canvases.size < 128 } == true
        }
    var reply: CanvasReply?
        get() = updates.value.reply
        private set(value) { updates.value = updates.value.copy(reply = value) }
    var busy: Boolean
        get() = updates.value.busy
        private set(value) { updates.value = updates.value.copy(busy = value) }
    var error: String
        get() = updates.value.error
        private set(value) { updates.value = updates.value.copy(error = value) }
    var serial: Int
        get() = updates.value.serial
        private set(value) { updates.value = updates.value.copy(serial = value) }
    var lastSucceeded: Boolean
        get() = updates.value.lastSucceeded
        private set(value) { updates.value = updates.value.copy(lastSucceeded = value) }
    private var token = ""
    private var closed = false
    private val scope = CoroutineScope(SupervisorJob() + uiDispatcher)
    private val requests = Channel<JsonObject>(1)
    private val json = Json { ignoreUnknownKeys = false }
    init {
        scope.launch {
            try {
                accept(withContext(workDispatcher) { json.decodeFromString<CanvasReply>(backend.open(root, workspace)) })
                busy = false
                for (request in requests) {
                    try { accept(withContext(workDispatcher) { json.decodeFromString<CanvasReply>(backend.command(token, request.toString())) }) }
                    catch (failure: Exception) { lastSucceeded = false; error = "画布保存失败，请重试" }
                    finally { updates.value = updates.value.copy(busy = false, quietGesture = false, serial = updates.value.serial + 1) }
                }
            } catch (failure: Exception) { error = "画布载入失败，请返回后重试"; busy = false }
            catch (failure: LinkageError) { error = "画布组件不可用，请重新安装正式版本"; busy = false }
            finally {
                closed = true; busy = false; requests.close()
                if (token.isNotBlank()) withContext(NonCancellable + workDispatcher) { runCatching { backend.close(token) } }
                scope.cancel()
            }
        }
    }
    private fun accept(next: CanvasReply) {
        val accepted = mergeCanvasReply(reply, next)
        lastSucceeded = accepted != null
        if (accepted != null) { token = accepted.token; reply = accepted; error = accepted.warning }
        else error = next.error.ifBlank { "画布操作失败" }
    }
    fun submit(request: JsonObject): Boolean {
        if (closed || busy || reply?.canvas == null) return false
        if (request["op"]?.jsonPrimitive?.contentOrNull == "new" && !canCreateBoard) return false
        // Gesture saves still use the native persistence path, but the in-flight
        // transaction must not blink the header spinner or disable the toolbar.
        val quiet = request["op"]?.jsonPrimitive?.contentOrNull == "gesture"
        updates.value = updates.value.copy(busy = true, error = "", quietGesture = quiet)
        if (!requests.trySend(request).isSuccess) {
            updates.value = updates.value.copy(busy = false, quietGesture = false, error = "画布已关闭，请重新打开")
            return false
        }
        return true
    }
    fun close() { closed = true; requests.close() }
}

private fun canvasRequest(op: String, values: JsonObjectBuilder.() -> Unit = {}) = buildJsonObject { put("op", op); values() }

// Hold the intended card position in world coordinates. The drawing offset
// automatically approaches zero when the committed model receives that position.
internal data class CanvasDragPreview(val id: String, val x: Float, val y: Float) {
    fun offset(node: CanvasNode): Offset = Offset(x - node.x, y - node.y)
}

internal data class CanvasViewport(val x: Float, val y: Float, val zoom: Float) {
    fun world(screen: Offset, size: IntSize, density: Float): Offset = Offset(x, y) + (screen - Offset(size.width / 2f, size.height / 2f)) / (zoom * density)
    fun transformed(centroid: Offset, pan: Offset, factor: Float, size: IntSize, density: Float): CanvasViewport {
        if (!factor.isFinite() || factor <= 0 || !pan.x.isFinite() || !pan.y.isFinite()) return this
        val anchor = world(centroid, size, density)
        val nextZoom = (zoom * factor).coerceIn(0.35f, 2.5f)
        val next = anchor - (centroid + pan - Offset(size.width / 2f, size.height / 2f)) / (nextZoom * density)
        return CanvasViewport(next.x.coerceIn(-250000f, 250000f), next.y.coerceIn(-250000f, 250000f), nextZoom)
    }
}

internal fun canvasHit(nodes: List<CanvasNode>, point: Offset): CanvasNode? = nodes.asReversed().firstOrNull {
    point.x >= it.x && point.x <= it.x + it.width && point.y >= it.y && point.y <= it.y + it.height
}
internal fun canvasVisible(node: CanvasNode, viewport: CanvasViewport, size: IntSize, density: Float): Boolean {
    val halfWidth = size.width / (2f * viewport.zoom * density)
    val halfHeight = size.height / (2f * viewport.zoom * density)
    return node.x + node.width >= viewport.x - halfWidth && node.x <= viewport.x + halfWidth && node.y + node.height >= viewport.y - halfHeight && node.y <= viewport.y + halfHeight
}
// View-only overview: never persist a zoom smaller than the legacy 0.35 limit.
internal data class CanvasOverviewTransform(val originX: Float, val originY: Float, val scale: Float) {
    fun screenAt(world: Offset): Offset = Offset(originX + world.x * scale, originY + world.y * scale)
    fun worldAt(screen: Offset): Offset = Offset((screen.x - originX) / scale, (screen.y - originY) / scale)
}
internal fun fitCanvasOverview(nodes: List<CanvasNode>, size: IntSize): CanvasOverviewTransform {
    val left = nodes.minOfOrNull { it.x } ?: -128f
    val right = nodes.maxOfOrNull { it.x + it.width } ?: 128f
    val top = nodes.minOfOrNull { it.y } ?: -84f
    val bottom = nodes.maxOfOrNull { it.y + it.height } ?: 84f
    val availableWidth = (size.width.toFloat() - 48f).coerceAtLeast(1f)
    val availableHeight = (size.height.toFloat() - 48f).coerceAtLeast(1f)
    val scale = min(availableWidth / (right - left).coerceAtLeast(1f),
        availableHeight / (bottom - top).coerceAtLeast(1f)).coerceAtLeast(0.000001f)
    return CanvasOverviewTransform(
        size.width / 2f - (left + right) / 2f * scale,
        size.height / 2f - (top + bottom) / 2f * scale,
        scale
    )
}
internal fun canvasPageLabel(note: NoteEntry?): Pair<String, String> = when {
    note == null || note.isDeleted() -> "页面不可用" to "页面已删除或尚未同步"
    note.encryption != null -> "已加密页面" to "打开页面后解锁"
    else -> note.displayTitle() to note.previewBody().take(300)
}

// A committed graph owns one bounded buffer. Pan/zoom only select cached lines;
// a drag recomputes incident edges in scratch space without changing the cache.
internal class CanvasEdgeRenderer(nodes: List<CanvasNode>, edges: List<CanvasEdge>) {
    private class Endpoints(val from: CanvasNode, val to: CanvasNode)
    private val links: List<Endpoints>
    private val lines: FloatArray
    private val bounds: FloatArray
    private val scratchLines = FloatArray(12)
    private val scratchBounds = FloatArray(4)
    val visibleLines: FloatArray
    init {
        val index = nodes.associateBy { it.id }
        links = edges.mapNotNull { edge ->
            val from = index[edge.from]; val to = index[edge.to]
            if (from == null || to == null) null else Endpoints(from, to)
        }
        lines = FloatArray(links.size * 12)
        bounds = FloatArray(links.size * 4)
        visibleLines = FloatArray(lines.size)
        links.forEachIndexed { i, edge -> geometry(edge, null, Offset.Zero, lines, i * 12, bounds, i * 4) }
    }
    private fun geometry(edge: Endpoints, dragged: String?, delta: Offset, out: FloatArray, offset: Int, box: FloatArray, boxOffset: Int) {
        val from = edge.from; val to = edge.to
        val ax = from.x + from.width / 2 + if (from.id == dragged) delta.x else 0f
        val ay = from.y + from.height / 2 + if (from.id == dragged) delta.y else 0f
        val bx = to.x + to.width / 2 + if (to.id == dragged) delta.x else 0f
        val by = to.y + to.height / 2 + if (to.id == dragged) delta.y else 0f
        val dx = bx - ax; val dy = by - ay; val length = sqrt(dx * dx + dy * dy)
        if (length <= 0.01f) {
            box[boxOffset] = Float.POSITIVE_INFINITY; box[boxOffset + 1] = Float.POSITIVE_INFINITY
            box[boxOffset + 2] = Float.NEGATIVE_INFINITY; box[boxOffset + 3] = Float.NEGATIVE_INFINITY
            return
        }
        val start = min(if (abs(dx) > 0.001f) from.width / 2 / abs(dx) else Float.MAX_VALUE, if (abs(dy) > 0.001f) from.height / 2 / abs(dy) else Float.MAX_VALUE)
        val end = min(if (abs(dx) > 0.001f) to.width / 2 / abs(dx) else Float.MAX_VALUE, if (abs(dy) > 0.001f) to.height / 2 / abs(dy) else Float.MAX_VALUE)
        val ex = bx - dx * end; val ey = by - dy * end
        val ux = dx / length; val uy = dy / length
        out[offset] = ax + dx * start; out[offset + 1] = ay + dy * start
        out[offset + 2] = ex; out[offset + 3] = ey
        out[offset + 4] = ex; out[offset + 5] = ey
        out[offset + 6] = ex - ux * 10 - uy * 5; out[offset + 7] = ey - uy * 10 + ux * 5
        out[offset + 8] = ex; out[offset + 9] = ey
        out[offset + 10] = ex - ux * 10 + uy * 5; out[offset + 11] = ey - uy * 10 - ux * 5
        var left = Float.POSITIVE_INFINITY; var top = Float.POSITIVE_INFINITY
        var right = Float.NEGATIVE_INFINITY; var bottom = Float.NEGATIVE_INFINITY
        var i = offset
        while (i < offset + 12) {
            left = min(left, out[i]); right = max(right, out[i])
            top = min(top, out[i + 1]); bottom = max(bottom, out[i + 1]); i += 2
        }
        box[boxOffset] = left; box[boxOffset + 1] = top
        box[boxOffset + 2] = right; box[boxOffset + 3] = bottom
    }
    fun updateVisible(left: Float, top: Float, right: Float, bottom: Float, dragged: String?, delta: Offset): Int {
        var used = 0
        links.forEachIndexed { i, edge ->
            var source = lines; var offset = i * 12; var box = bounds; var boxOffset = i * 4
            if (dragged != null && (edge.from.id == dragged || edge.to.id == dragged)) {
                geometry(edge, dragged, delta, scratchLines, 0, scratchBounds, 0)
                source = scratchLines; offset = 0; box = scratchBounds; boxOffset = 0
            }
            if (box[boxOffset] <= right && box[boxOffset + 2] >= left && box[boxOffset + 1] <= bottom && box[boxOffset + 3] >= top) {
                source.copyInto(visibleLines, used, offset, offset + 12); used += 12
            }
        }
        return used
    }
}

private fun canvasIsReferral(edge: CanvasEdge, nodes: List<CanvasNode>): Boolean =
    edge.relation == "referral" || (edge.relation == null &&
        nodes.any { it.id == edge.from && it.kind == "client" } &&
        nodes.any { it.id == edge.to && it.kind == "client" })

private fun canvasTint(name: String): Color = when (name) {
    "green" -> Color(0xFF2D806B); "amber" -> Color(0xFFA06A24); "pink" -> Color(0xFFAF5279); "purple" -> Color(0xFF7C61A8); else -> Color(0xFF457DA3)
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
internal fun KnowledgeCanvasScreen(appData: AppData, onBack: () -> Unit, onOpenPage: (NoteEntry) -> Unit, onCreatePage: (String, (String?) -> Unit) -> Unit) {
    val context = LocalContext.current
    val workspace = LocalNoteMediaWorkspaceKey.current
    val model = remember(workspace) { KnowledgeCanvasModel(context.filesDir.absolutePath, workspace) }
    val modelState by model.state.collectAsState()
    DisposableEffect(model) { onDispose { model.close() } }
    var selected by remember(workspace) { mutableStateOf<String?>(null) }
    var connectFrom by remember(workspace) { mutableStateOf<String?>(null) }
    var connectRelation by remember(workspace) { mutableStateOf("link") }
    var showLibrary by remember(workspace) { mutableStateOf(true) }
    var pendingTemplate by remember(workspace) { mutableStateOf("blank") }
    var panel by remember(workspace) { mutableStateOf("") }
    var dialog by remember(workspace) { mutableStateOf("") }
    var editorText by remember(workspace) { mutableStateOf("") }
    var editorTitle by remember(workspace) { mutableStateOf("") }
    var clientName by remember(workspace) { mutableStateOf("") }
    var clientDetails by remember(workspace) { mutableStateOf("") }
    var clientParentId by remember(workspace) { mutableStateOf<String?>(null) }
    var editTargetId by remember(workspace) { mutableStateOf<String?>(null) }
    var creatingPage by remember(workspace) { mutableStateOf(false) }
    var nodeSearch by remember(workspace) { mutableStateOf("") }
    var pageCreationError by remember(workspace) { mutableStateOf("") }
    var dialogRequestPending by remember(workspace) { mutableStateOf(false) }
    val canvas = modelState.reply?.canvas
    val pages = remember(appData.notes) { appData.notes.filter { it.kind == NoteEntryKind.DOCUMENT && !it.isDeleted() }.associateBy { it.id } }
    val selectedNode = canvas?.nodes?.find { it.id == selected }
    val cardTitle: (CanvasNode) -> String = { node ->
        if (node.kind == "page") canvasPageLabel(pages[node.pageId]).first
        else node.text.substringBefore("\n").ifBlank { if (node.kind == "client") "未命名客户" else "空白便签" }
    }
    val cardBody: (CanvasNode) -> String = { node ->
        if (node.kind == "page") canvasPageLabel(pages[node.pageId]).second
        else node.text.substringAfter("\n", "").trim()
    }
    val showBusy = modelState.busy && !modelState.quietGesture
    val ready = !showBusy && canvas != null
    val boards = modelState.reply?.canvases.orEmpty()
    val createCanvas: (String) -> Unit = { template ->
        val titles = boards.map { it.title }.toSet()
        val prefix = if (template == "customer_network") "客户网络" else "画布"
        var title = prefix
        var number = 2
        while (title in titles) { title = "$prefix $number"; number++ }
        pendingTemplate = template
        editorText = title; panel = ""; dialog = "new"
    }
    val back = { if (!showBusy) { if (connectFrom != null) connectFrom = null else if (!showLibrary) { showLibrary = true; selected = null; panel = "" } else onBack() } }
    BackHandler { back() }
    LaunchedEffect(canvas?.id) { selected = null; connectFrom = null; panel = ""; dialog = "" }
    LaunchedEffect(model.serial) {
        if (canvas?.nodes?.any { it.id == selected } != true) selected = null
        if (dialogRequestPending) {
            if (model.lastSucceeded) { dialog = "" }
            dialogRequestPending = false
        }
    }
    if (showLibrary) {
        Column(Modifier.fillMaxSize().background(MaterialTheme.colorScheme.background).statusBarsPadding().navigationBarsPadding().padding(horizontal = 16.dp)) {
            Row(Modifier.fillMaxWidth().padding(vertical = 12.dp), verticalAlignment = Alignment.CenterVertically) {
                TextButton(onClick = onBack) { Text("返回") }
                Text("我的画布", Modifier.weight(1f), style = MaterialTheme.typography.titleLarge)
                Text("${boards.size} 张", color = MaterialTheme.colorScheme.onSurfaceVariant, style = MaterialTheme.typography.labelMedium)
            }
            Text("为想法建立联系", style = MaterialTheme.typography.titleMedium)
            Text("自由创建画布，或从模板开始。模板不会限制卡片与连线。", style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
            Spacer(Modifier.height(18.dp))
            Row(horizontalArrangement = Arrangement.spacedBy(12.dp)) {
                Surface(onClick = { createCanvas("blank") }, enabled = model.canCreateBoard, modifier = Modifier.weight(1f).height(124.dp), shape = RoundedCornerShape(20.dp), color = MaterialTheme.colorScheme.primaryContainer) {
                    Column(Modifier.padding(16.dp), verticalArrangement = Arrangement.SpaceBetween) {
                        Text("＋", style = MaterialTheme.typography.headlineMedium, color = MaterialTheme.colorScheme.primary)
                        Text("空白画布", style = MaterialTheme.typography.titleMedium)
                        Text("便签 · 页面 · 自由连线", style = MaterialTheme.typography.labelSmall)
                    }
                }
                Surface(onClick = { createCanvas("customer_network") }, enabled = model.canCreateBoard, modifier = Modifier.weight(1f).height(124.dp), shape = RoundedCornerShape(20.dp), color = MaterialTheme.colorScheme.surfaceVariant) {
                    Column(Modifier.padding(16.dp), verticalArrangement = Arrangement.SpaceBetween) {
                        Text("◎", style = MaterialTheme.typography.headlineMedium, color = MaterialTheme.colorScheme.primary)
                        Text("客户网络模板", style = MaterialTheme.typography.titleMedium)
                        Text("推荐关系 · 共享基础工具", style = MaterialTheme.typography.labelSmall)
                    }
                }
            }
            Spacer(Modifier.height(20.dp))
            Row(verticalAlignment = Alignment.CenterVertically) {
                Text("全部画布", Modifier.weight(1f), style = MaterialTheme.typography.titleMedium)
                if (modelState.busy) CircularProgressIndicator(Modifier.size(18.dp), strokeWidth = 2.dp)
            }
            if (model.error.isNotBlank()) Text(model.error, color = MaterialTheme.colorScheme.error, style = MaterialTheme.typography.bodySmall)
            LazyColumn(Modifier.weight(1f), contentPadding = PaddingValues(vertical = 8.dp), verticalArrangement = Arrangement.spacedBy(4.dp)) {
                items(boards, key = { it.id }) { board ->
                    Surface(onClick = {
                        if (board.id == canvas?.id) showLibrary = false
                        else if (model.submit(canvasRequest("switch") { put("id",board.id) })) showLibrary = false
                    }, enabled = ready, shape = RoundedCornerShape(16.dp), color = MaterialTheme.colorScheme.surfaceVariant.copy(alpha = 0.4f)) {
                        Row(Modifier.fillMaxWidth().padding(horizontal = 16.dp, vertical = 14.dp), verticalAlignment = Alignment.CenterVertically) {
                            Text("▤", style = MaterialTheme.typography.titleLarge, color = MaterialTheme.colorScheme.primary)
                            Spacer(Modifier.width(12.dp))
                            Column(Modifier.weight(1f)) {
                                Text(board.title, maxLines = 1, overflow = TextOverflow.Ellipsis, style = MaterialTheme.typography.titleMedium)
                                Text(if (board.template == "customer_network") "客户网络模板" else "通用画布", style = MaterialTheme.typography.labelSmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
                            }
                            Text("›", style = MaterialTheme.typography.titleLarge, color = MaterialTheme.colorScheme.onSurfaceVariant)
                        }
                    }
                }
            }
        }
    } else {
    Column(Modifier.fillMaxSize().background(MaterialTheme.colorScheme.background).statusBarsPadding().navigationBarsPadding()) {
        Row(Modifier.fillMaxWidth().padding(start = 4.dp, end = 8.dp, top = 2.dp, bottom = 2.dp), verticalAlignment = Alignment.CenterVertically) {
            TextButton(onClick = back, enabled = !showBusy) { Text("返回") }
            Column(Modifier.weight(1f), horizontalAlignment = Alignment.CenterHorizontally) {
                TextButton(onClick = { panel = "boards" }, enabled = ready, contentPadding = PaddingValues(horizontal = 8.dp, vertical = 0.dp)) {
                    Text(canvas?.title ?: "画布", maxLines = 1, overflow = TextOverflow.Ellipsis, style = MaterialTheme.typography.titleMedium)
                    Text(" ⌄", style = MaterialTheme.typography.bodySmall)
                }
                Text("${canvas?.nodes?.size ?: 0} 张卡片 · ${canvas?.edges?.size ?: 0} 条关系", style = MaterialTheme.typography.labelSmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
            }
            if (showBusy) CircularProgressIndicator(Modifier.padding(12.dp).size(18.dp), strokeWidth = 2.dp)
            else TextButton(onClick = { panel = "more" }, enabled = ready) { Text("⋯ 更多") }
        }
        Row(Modifier.fillMaxWidth().padding(horizontal = 12.dp, vertical = 5.dp), horizontalArrangement = Arrangement.spacedBy(8.dp), verticalAlignment = Alignment.CenterVertically) {
            Button(enabled = ready, onClick = { panel = "addActions" }, shape = RoundedCornerShape(14.dp), modifier = Modifier.weight(1f)) { Text("＋ 添加") }
            OutlinedButton(enabled = ready, onClick = { panel = "nodes" }, shape = RoundedCornerShape(14.dp)) { Text("查找") }
            TextButton(enabled = ready && model.reply?.canUndo == true, onClick = { model.submit(canvasRequest("undo")) }, modifier = Modifier.width(42.dp).semantics { contentDescription = "撤销" }) { Text("↶", style = MaterialTheme.typography.titleMedium) }
            TextButton(enabled = ready && model.reply?.canRedo == true, onClick = { model.submit(canvasRequest("redo")) }, modifier = Modifier.width(42.dp).semantics { contentDescription = "重做" }) { Text("↷", style = MaterialTheme.typography.titleMedium) }
        }
        if (model.error.isNotBlank()) Text(model.error, Modifier.padding(horizontal = 14.dp, vertical = 5.dp), color = MaterialTheme.colorScheme.error, style = MaterialTheme.typography.bodySmall)
        if (connectFrom != null) Surface(color = MaterialTheme.colorScheme.primaryContainer, shape = RoundedCornerShape(12.dp), modifier = Modifier.fillMaxWidth().padding(horizontal = 12.dp, vertical = 5.dp)) {
            Row(Modifier.padding(start = 12.dp, end = 6.dp), verticalAlignment = Alignment.CenterVertically) {
                Text(if (connectRelation == "referral") "请选择被推荐的客户" else "请选择另一张卡片以建立普通连线", Modifier.weight(1f), style = MaterialTheme.typography.bodySmall)
                TextButton(onClick = { connectFrom = null }) { Text("取消") }
            }
        }
        if (canvas != null) {
            CanvasSurface(canvas, pages, model, selected, connectFrom, Modifier.weight(1f).fillMaxWidth(),
                onSelect = { selected = it }, onConnect = { target ->
                    val from = connectFrom
                    if (from != null && from != target) {
                        if (model.submit(canvasRequest("connect") { put("from",from); put("to",target); put("relation",connectRelation) })) connectFrom = null
                    }
                },
                onAddFirst = { if (canvas.template == "customer_network") { clientName = ""; clientDetails = ""; clientParentId = null; dialog = "clientAdd" } else { editorTitle = ""; editorText = ""; dialog = "noteAdd" } })
        } else Box(Modifier.weight(1f).fillMaxWidth(), contentAlignment = Alignment.Center) {
            Text(if (model.busy) "正在载入画布" else "画布未能载入", color = MaterialTheme.colorScheme.onSurfaceVariant)
        }
    }
    }

    if (!showLibrary && selectedNode != null) {
        ModalBottomSheet(onDismissRequest = { selected = null }, dragHandle = { BottomSheetDefaults.DragHandle() }) {
            Column(Modifier.fillMaxWidth().padding(horizontal = 20.dp).padding(bottom = 28.dp), verticalArrangement = Arrangement.spacedBy(12.dp)) {
                Text(cardTitle(selectedNode), style = MaterialTheme.typography.titleLarge, maxLines = 2, overflow = TextOverflow.Ellipsis)
                val kindLabel = when (selectedNode.kind) { "client" -> "客户"; "page" -> "知识页面"; else -> "便签" }
                val children = canvas?.let { board -> board.edges.count { edge -> edge.from == selectedNode.id && canvasIsReferral(edge, board.nodes) } } ?: 0
                val parent = canvas?.let { board -> board.edges.firstOrNull { edge -> edge.to == selectedNode.id && canvasIsReferral(edge, board.nodes) } }
                val parentNode = canvas?.nodes?.firstOrNull { it.id == parent?.from }
                Text(if (selectedNode.kind == "client") "客户 · 直接推荐 ${children} 人" else kindLabel, style = MaterialTheme.typography.labelMedium, color = MaterialTheme.colorScheme.primary)
                if (parentNode != null) Text("推荐人：${cardTitle(parentNode)}", color = MaterialTheme.colorScheme.onSurfaceVariant, style = MaterialTheme.typography.bodyMedium)
                val body = cardBody(selectedNode)
                if (body.isNotBlank()) Text(body, style = MaterialTheme.typography.bodyMedium, maxLines = 8, overflow = TextOverflow.Ellipsis)
                else Text(if (selectedNode.kind == "client") "尚未填写客户备注" else "尚无其他内容", style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onSurfaceVariant)
                if (selectedNode.kind == "client") {
                    Button(onClick = {
                        clientParentId = selectedNode.id; clientName = ""; clientDetails = ""; selected = null; dialog = "clientAdd"
                    }, enabled = ready, modifier = Modifier.fillMaxWidth(), shape = RoundedCornerShape(14.dp)) { Text("＋ 添加此客户推荐的人") }
                }
                Column(Modifier.fillMaxWidth().heightIn(max = 420.dp).verticalScroll(rememberScrollState()), verticalArrangement = Arrangement.spacedBy(6.dp)) {
                    Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    if (selectedNode.kind == "page") {
                        OutlinedButton(onClick = { pages[selectedNode.pageId]?.let(onOpenPage); selected = null }, enabled = ready && pages[selectedNode.pageId] != null, modifier = Modifier.weight(1f)) { Text("打开页面") }
                    } else {
                        OutlinedButton(onClick = {
                            editTargetId = selectedNode.id
                            if (selectedNode.kind == "client") {
                                clientName = cardTitle(selectedNode); clientDetails = cardBody(selectedNode); dialog = "clientEdit"
                            } else {
                                editorTitle = cardTitle(selectedNode).take(80)
                                editorText = selectedNode.text.removePrefix(editorTitle).trimStart('\n')
                                dialog = "noteEdit"
                            }
                            selected = null
                        }, enabled = ready, modifier = Modifier.weight(1f)) { Text(if (selectedNode.kind == "client") "编辑客户" else "编辑便签") }
                    }
                    OutlinedButton(onClick = { connectRelation = "link"; connectFrom = selectedNode.id; selected = null }, enabled = ready && (canvas?.nodes?.size ?: 0) > 1, modifier = Modifier.weight(1f)) { Text("自由连线") }
                    }
                    Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    if (selectedNode.kind == "client") OutlinedButton(onClick = { connectRelation = "referral"; connectFrom = selectedNode.id; selected = null }, enabled = ready && (canvas?.nodes?.count { it.kind == "client" } ?: 0) > 1, modifier = Modifier.weight(1f)) { Text("绑定推荐") }
                    OutlinedButton(onClick = { editTargetId = selectedNode.id; panel = "edges"; selected = null }, enabled = ready, modifier = Modifier.weight(1f)) { Text("管理连线") }
                    }
                    Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    OutlinedButton(onClick = {
                        val isLarge = selectedNode.width >= 350f
                        val targetWidth = if (isLarge) { if (selectedNode.kind == "client") 192f else 256f } else 384f
                        val targetHeight = if (isLarge) { if (selectedNode.kind == "client") 132f else 168f } else 252f
                        if (model.submit(canvasRequest("resize") { put("id",selectedNode.id); put("width",targetWidth); put("height",targetHeight) })) selected = null
                    }, enabled = ready, modifier = Modifier.weight(1f)) { Text(if (selectedNode.width >= 350f) "缩小卡片" else "放大卡片") }
                    OutlinedButton(onClick = { editTargetId = selectedNode.id; panel = "colors"; selected = null }, enabled = ready, modifier = Modifier.weight(1f)) { Text("颜色") }
                    }
                    Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    OutlinedButton(onClick = { editTargetId = selectedNode.id; dialog = "deleteNode"; selected = null }, enabled = ready, modifier = Modifier.weight(1f)) { Text("移除卡片") }
                    }
                }
                Text("卡片位置、大小和连线可以自由编辑。移除卡片不会删除原知识页面。", color = MaterialTheme.colorScheme.onSurfaceVariant, style = MaterialTheme.typography.labelSmall)
            }
        }
    }

    if (panel.isNotBlank()) {
        Dialog(onDismissRequest = { panel = "" }, properties = DialogProperties(usePlatformDefaultWidth = false)) {
            Surface(Modifier.fillMaxWidth().padding(18.dp).heightIn(max = 580.dp), shape = RoundedCornerShape(24.dp)) {
                Column(Modifier.padding(18.dp)) {
                    Row(verticalAlignment = Alignment.CenterVertically) {
                        Text(when(panel) { "addActions" -> "添加到画布"; "boards" -> "全部画布（${boards.size}）"; "pages" -> "引用已有页面"; "nodes" -> "定位卡片"; "edges" -> "卡片连线"; "colors" -> "卡片颜色"; else -> "画布工具" }, Modifier.weight(1f), style = MaterialTheme.typography.titleLarge)
                        TextButton(onClick = { panel = "" }) { Text("关闭") }
                    }
                    if (model.error.isNotBlank()) Text(model.error, color = MaterialTheme.colorScheme.error, style = MaterialTheme.typography.bodySmall)
                    when (panel) {
                        "addActions" -> {
                            Text("选择一种卡片类型", style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
                            if (canvas?.template == "customer_network") {
                                TextButton(onClick = { panel = ""; clientName = ""; clientDetails = ""; clientParentId = null; dialog = "clientAdd" }, enabled = ready, modifier = Modifier.fillMaxWidth()) { Text("＋ 新建客户", Modifier.fillMaxWidth()) }
                                HorizontalDivider()
                            }
                            TextButton(onClick = { panel = ""; editorTitle = ""; editorText = ""; dialog = "noteAdd" }, enabled = ready, modifier = Modifier.fillMaxWidth()) { Text("＋ 新建便签", Modifier.fillMaxWidth()) }
                            HorizontalDivider()
                            TextButton(onClick = { panel = ""; editorText = ""; pageCreationError = ""; dialog = "pageAdd" }, enabled = ready, modifier = Modifier.fillMaxWidth()) { Text("＋ 新建知识页面", Modifier.fillMaxWidth()) }
                            HorizontalDivider()
                            TextButton(onClick = { panel = "pages" }, enabled = ready, modifier = Modifier.fillMaxWidth()) { Text("＋ 引用已有知识页面", Modifier.fillMaxWidth()) }
                            if (canvas?.template != "customer_network") {
                                HorizontalDivider()
                                TextButton(onClick = { panel = ""; clientName = ""; clientDetails = ""; clientParentId = null; dialog = "clientAdd" }, enabled = ready, modifier = Modifier.fillMaxWidth()) { Text("＋ 更多卡片：客户", Modifier.fillMaxWidth()) }
                            }
                        }
                        "boards" -> {
                            Button(enabled = model.canCreateBoard, onClick = { panel = ""; showLibrary = true }, modifier = Modifier.fillMaxWidth()) { Text("＋ 新建画布") }
                            if (boards.size >= 128) Text("已达 128 张画布上限", Modifier.padding(vertical = 8.dp), style = MaterialTheme.typography.bodySmall)
                            LazyColumn(modifier = Modifier.weight(1f, fill = false)) { items(boards, key = { it.id }) { board ->
                                val current = board.id == canvas?.id
                                TextButton(enabled = ready && !current, modifier = Modifier.fillMaxWidth(), onClick = { model.submit(canvasRequest("switch") { put("id",board.id) }) }) {
                                    Text(board.title, Modifier.weight(1f), maxLines = 2, overflow = TextOverflow.Ellipsis)
                                    if (current) Text("当前", Modifier.padding(start = 12.dp), color = MaterialTheme.colorScheme.primary)
                                    else if (board.template == "customer_network") Text("模板", Modifier.padding(start = 12.dp), color = MaterialTheme.colorScheme.onSurfaceVariant)
                                }
                            } }
                        }
                        "pages" -> {
                            var search by remember { mutableStateOf("") }
                            OutlinedTextField(value = search, onValueChange = { search = it.take(100) }, singleLine = true, label = { Text("搜索页面") }, modifier = Modifier.fillMaxWidth())
                            val available = remember(pages, search, canvas?.nodes) { val used = canvas?.nodes?.map { it.pageId }?.toSet().orEmpty(); pages.values.filter { it.id !in used && (search.isBlank() || canvasPageLabel(it).first.contains(search, true)) } }
                            if (available.isEmpty()) Text("没有可加入的页面", Modifier.padding(vertical = 20.dp))
                            LazyColumn { items(available, key = { it.id }) { page ->
                                TextButton(enabled = ready, modifier = Modifier.fillMaxWidth(), onClick = { model.submit(canvasRequest("addPage") { put("pageId",page.id) }); panel = "" }) { Text(canvasPageLabel(page).first, Modifier.fillMaxWidth(), maxLines = 2, overflow = TextOverflow.Ellipsis) }
                            } }
                        }
                        "nodes" -> {
                            OutlinedTextField(value = nodeSearch, onValueChange = { nodeSearch = it.take(100) }, singleLine = true, label = { Text("搜索客户、便签或页面") }, modifier = Modifier.fillMaxWidth())
                            val matches = canvas?.nodes.orEmpty().filter { node -> nodeSearch.isBlank() || cardTitle(node).contains(nodeSearch, ignoreCase = true) || cardBody(node).contains(nodeSearch, ignoreCase = true) }
                            if (matches.isEmpty()) Text(if (nodeSearch.isBlank()) "还没有卡片" else "没有找到匹配的卡片", Modifier.padding(vertical = 20.dp))
                            LazyColumn { items(matches, key = { it.id }) { node ->
                                TextButton(enabled = ready, modifier = Modifier.fillMaxWidth(), onClick = {
                                    val from = connectFrom
                                    if (from != null) {
                                        if (from != node.id && model.submit(canvasRequest("connect") { put("from",from); put("to",node.id); put("relation",connectRelation) })) {
                                            connectFrom = null; selected = null; panel = ""
                                        }
                                    } else {
                                        selected = node.id; panel = ""
                                        model.submit(canvasRequest("view") { put("x",node.x + node.width/2); put("y",node.y + node.height/2); put("zoom",1f) })
                                    }
                                }) { Text(cardTitle(node), Modifier.fillMaxWidth(), maxLines = 2, overflow = TextOverflow.Ellipsis) }
                            } }
                        }
                        "edges" -> {
                            val edges = canvas?.edges.orEmpty().filter { it.from == editTargetId || it.to == editTargetId }
                            val nodeIndex = remember(canvas?.nodes) { canvas?.nodes?.associateBy { it.id }.orEmpty() }
                            if (edges.isEmpty()) Text("这张卡片还没有连线", Modifier.padding(vertical = 20.dp))
                            LazyColumn { items(edges, key = { it.id }) { edge ->
                                val other = nodeIndex[if(edge.from == editTargetId) edge.to else edge.from]
                                Row(verticalAlignment = Alignment.CenterVertically) {
                                    Text((if(edge.from == editTargetId) "→ " else "← ") + (other?.let(cardTitle) ?: "未知卡片") + (if (canvasIsReferral(edge, canvas?.nodes.orEmpty())) " · 推荐" else " · 普通"), Modifier.weight(1f), maxLines = 2, overflow = TextOverflow.Ellipsis)
                                    TextButton(enabled = ready, onClick = { model.submit(canvasRequest("removeEdge") { put("id",edge.id) }) }) { Text("移除") }
                                }
                            } }
                        }
                        "colors" -> listOf("blue" to "蓝色","green" to "绿色","amber" to "琥珀","pink" to "粉色","purple" to "紫色").forEach { (color,label) ->
                            TextButton(enabled = ready && editTargetId != null, modifier = Modifier.fillMaxWidth(), onClick = { editTargetId?.let { id -> model.submit(canvasRequest("color") { put("id",id); put("color",color) }) }; panel = "" }) { Text(label, color = canvasTint(color)) }
                        }
                        else -> {
                            TextButton(enabled = ready && model.reply?.canUndo == true, onClick = { model.submit(canvasRequest("undo")); panel = "" }) { Text("撤销上一步") }
                            TextButton(enabled = ready && model.reply?.canRedo == true, onClick = { model.submit(canvasRequest("redo")); panel = "" }) { Text("重做") }
                            TextButton(enabled = ready && canvas?.nodes?.any { it.kind == "client" } == true, onClick = { model.submit(canvasRequest("arrangeTree")); panel = "" }) { Text("按推荐层级排列客户") }
                            TextButton(enabled = ready && !canvas?.nodes.isNullOrEmpty(), onClick = { model.submit(canvasRequest("arrange")); panel = "" }) { Text("按网格排列所有卡片") }
                            HorizontalDivider()
                            TextButton(enabled = ready, onClick = { editorText = canvas!!.title; dialog = "rename"; panel = "" }) { Text("重命名画布") }
                            TextButton(enabled = ready, onClick = { panel = "boards" }) { Text("管理全部画布") }
                            TextButton(enabled = ready, onClick = { dialog = "deleteCanvas"; panel = "" }) { Text("删除画布", color = MaterialTheme.colorScheme.error) }
                            Text("保存在当前账户的本机工作区", Modifier.padding(top = 12.dp), style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
                        }
                    }
                }
            }
        }
    }
    if (dialog.isNotBlank()) {
        val noteDialog = dialog == "noteAdd" || dialog == "noteEdit"
        val clientDialog = dialog == "clientAdd" || dialog == "clientEdit"
        val titleEdit = dialog == "new" || dialog == "rename" || dialog == "pageAdd"
        val isEditing = dialog == "clientEdit" || dialog == "noteEdit"
        AlertDialog(
            onDismissRequest = { if (!model.busy) dialog = "" },
            shape = RoundedCornerShape(24.dp),
            title = { Text(when(dialog) { "clientAdd" -> "新建客户"; "clientEdit" -> "编辑客户"; "noteAdd" -> "新建便签"; "noteEdit" -> "编辑便签"; "new" -> "新建画布"; "rename" -> "重命名画布"; "pageAdd" -> "新建知识页面"; "deleteNode" -> "移除这张卡片？"; else -> "删除整张画布？" }) },
            text = {
                Column(verticalArrangement = Arrangement.spacedBy(10.dp)) {
                    if (clientDialog) {
                        OutlinedTextField(value = clientName, onValueChange = { clientName = it.take(80) }, label = { Text("客户姓名") }, placeholder = { Text("例如：张先生") }, singleLine = true, modifier = Modifier.fillMaxWidth())
                        if (dialog == "clientAdd" && clientParentId != null) Text("会自动连接到所选推荐人", style = MaterialTheme.typography.labelSmall, color = MaterialTheme.colorScheme.primary)
                        OutlinedTextField(value = clientDetails, onValueChange = { clientDetails = it.take(4000) }, label = { Text("客户资料与跟进记录") }, minLines = 3, modifier = Modifier.fillMaxWidth().heightIn(max = 220.dp))
                    } else if (noteDialog) {
                        OutlinedTextField(value = editorTitle, onValueChange = { editorTitle = it.take(80) }, label = { Text("标题") }, singleLine = true, modifier = Modifier.fillMaxWidth())
                        OutlinedTextField(value = editorText, onValueChange = { editorText = it.take(7800) }, label = { Text("内容") }, minLines = 3, modifier = Modifier.fillMaxWidth().heightIn(max = 220.dp))
                    } else if (titleEdit) {
                        OutlinedTextField(value = editorText, onValueChange = { editorText = it.take(80) }, label = { Text(if (dialog == "pageAdd") "页面标题" else "画布名称") }, singleLine = true, modifier = Modifier.fillMaxWidth())
                    } else Text(if(dialog == "deleteNode") "移除后相关连线也会消失，可以撤销。原知识页面不会删除。" else "删除此画布和卡片？其他画布及知识页面不受影响。")
                    if (model.error.isNotBlank()) Text(model.error, color = MaterialTheme.colorScheme.error, style = MaterialTheme.typography.bodySmall)
                    if (pageCreationError.isNotBlank() && dialog == "pageAdd") Text(pageCreationError, color = MaterialTheme.colorScheme.error, style = MaterialTheme.typography.bodySmall)
                }
            },
            confirmButton = {
                val allowed = ready && !creatingPage && (dialog != "new" || model.canCreateBoard) &&
                    (!titleEdit || editorText.isNotBlank()) &&
                    (!clientDialog || clientName.isNotBlank()) &&
                    (!noteDialog || editorTitle.isNotBlank())
                TextButton(enabled = allowed, onClick = {
                    if (dialog == "pageAdd") {
                        creatingPage = true; pageCreationError = ""
                        onCreatePage(editorText.trim()) { createdId ->
                            creatingPage = false
                            if (createdId == null) pageCreationError = "页面创建失败，请重新尝试"
                            else if (model.submit(canvasRequest("addPage") { put("pageId", createdId) })) dialogRequestPending = true
                            else pageCreationError = "页面已创建，请在引用已有页面中加入"
                        }
                    } else {
                    val request = when(dialog) {
                        "clientAdd" -> canvasRequest("addClient") {
                            put("name",clientName.trim()); put("details",clientDetails)
                            clientParentId?.let { put("parentId",it) }
                        }
                        "clientEdit" -> canvasRequest("updateClient") {
                            put("id",editTargetId.orEmpty()); put("name",clientName.trim()); put("details",clientDetails)
                        }
                        "noteAdd" -> canvasRequest("addNote") { put("text",editorTitle.trim()+"\n"+editorText) }
                        "noteEdit" -> canvasRequest("text") { put("id",editTargetId.orEmpty()); put("text",editorTitle.trim()+"\n"+editorText) }
                        "new" -> canvasRequest("new") { put("title",editorText.trim()); put("template",pendingTemplate) }
                        "rename" -> canvasRequest("rename") { put("title",editorText.trim()) }
                        "deleteNode" -> canvasRequest("removeNode") { put("id",editTargetId.orEmpty()) }
                        else -> canvasRequest("deleteCanvas")
                    }
                    if (model.submit(request)) { dialogRequestPending = true; if (dialog == "new") showLibrary = false }
                    }
                }) { Text(if (clientDialog || noteDialog || titleEdit) { if (isEditing || dialog == "rename") "保存" else "创建" } else "删除") }
            },
            dismissButton = { TextButton(enabled = !model.busy && !creatingPage, onClick = { dialog = "" }) { Text("取消") } }
        )
    }
}

private class CanvasTextCache {
    private val paint = TextPaint(Paint.ANTI_ALIAS_FLAG).apply { textSize = 15f; color = android.graphics.Color.rgb(40,45,50) }
    private data class TextLayoutEntry(val width: Float, val height: Float, val title: String, val body: String, val color: Int, val layout: StaticLayout)
    private val cache = object : LinkedHashMap<String, TextLayoutEntry>(128, 0.75f, true) {
        override fun removeEldestEntry(eldest: MutableMap.MutableEntry<String, TextLayoutEntry>?): Boolean = size > 128
    }
    private val labels = object : LinkedHashMap<String, Pair<NoteEntry,Pair<String,String>>>(128,0.75f,true) {
        override fun removeEldestEntry(eldest: MutableMap.MutableEntry<String,Pair<NoteEntry,Pair<String,String>>>?): Boolean = size > 128
    }
    fun label(note: NoteEntry?): Pair<String,String> {
        if(note == null) return canvasPageLabel(null)
        val old = labels[note.id]
        if(old != null && old.first === note) return old.second
        return canvasPageLabel(note).also { labels[note.id] = note to it }
    }
    fun layout(node: CanvasNode, title: String, body: String, textColor: Int): StaticLayout {
        val old = cache[node.id]
        if(old != null && old.width == node.width && old.height == node.height && old.title == title && old.body == body && old.color == textColor) return old.layout
        return run {
            val drawPaint = TextPaint(paint).apply { color = textColor }
            val content = (title.take(80) + if(body.isBlank()) "" else "\n\n" + body.take(140))
            StaticLayout.Builder.obtain(content,0,content.length,drawPaint,(node.width-28).toInt().coerceAtLeast(1))
                .setAlignment(Layout.Alignment.ALIGN_NORMAL).setIncludePad(false).setLineSpacing(2f,1f)
                .setMaxLines(((node.height-30)/20).toInt().coerceAtLeast(1)).setEllipsize(android.text.TextUtils.TruncateAt.END).build()
                .also { cache[node.id] = TextLayoutEntry(node.width,node.height,title,body,textColor,it) }
        }
    }
}

@Composable
private fun CanvasSurface(canvas: CanvasBoard, pages: Map<String,NoteEntry>, model: KnowledgeCanvasModel, selected: String?, connectFrom: String?, modifier: Modifier, onSelect: (String?) -> Unit, onConnect: (String) -> Unit, onAddFirst: () -> Unit) {
    val modelState by model.state.collectAsState()
    val visualReady = !modelState.busy || modelState.quietGesture
    val density = LocalDensity.current.density
    var size by remember { mutableStateOf(IntSize.Zero) }
    var viewport by remember(canvas.id) { mutableStateOf(CanvasViewport(canvas.centerX,canvas.centerY,canvas.zoom)) }
    val zoomPercent = remember(canvas.id) { derivedStateOf { (viewport.zoom * 100).roundToInt() } }
    var dragPreview by remember(canvas.id) { mutableStateOf<CanvasDragPreview?>(null) }
    var overview by remember(canvas.id) { mutableStateOf(false) }
    var committedViewport by remember(canvas.id) { mutableStateOf(CanvasViewport(canvas.centerX,canvas.centerY,canvas.zoom)) }
    val currentCanvas by rememberUpdatedState(canvas)
    val currentConnect by rememberUpdatedState(connectFrom)
    val currentSelect by rememberUpdatedState(onSelect)
    val currentOnConnect by rememberUpdatedState(onConnect)
    LaunchedEffect(modelState.serial, canvas.id) {
        val next = CanvasViewport(canvas.centerX,canvas.centerY,canvas.zoom)
        if (next != committedViewport) viewport = next
        committedViewport = next
        dragPreview = null
    }
    // An added or list-selected card must not remain invisible off screen.
    LaunchedEffect(selected, canvas.id, size) {
        val node = canvas.nodes.firstOrNull { it.id == selected }
        if (node != null && size.width > 0 && size.height > 0) {
            val centerX = node.x + node.width / 2f
            val centerY = node.y + node.height / 2f
            val halfWidth = size.width / (2f * viewport.zoom * density)
            val halfHeight = size.height / (2f * viewport.zoom * density)
            if (abs(centerX - viewport.x) > halfWidth * 0.6f ||
                abs(centerY - viewport.y) > halfHeight * 0.6f) {
                viewport = viewport.copy(x = centerX, y = centerY)
            }
        }
    }
    // Retire old previews when pages change, including deletion and encryption.
    val textCache = remember(canvas.id, pages) { CanvasTextCache() }
    val paint = remember { Paint(Paint.ANTI_ALIAS_FLAG) }
    val edgeRenderer = remember(canvas.id, canvas.nodes, canvas.edges) { CanvasEdgeRenderer(canvas.nodes, canvas.edges) }
    val primary = MaterialTheme.colorScheme.primary.toArgb()
    val surface = MaterialTheme.colorScheme.surface.toArgb()
    val textColor = MaterialTheme.colorScheme.onSurface.toArgb()
    val lineColor = MaterialTheme.colorScheme.outlineVariant.toArgb()
    Box(modifier.background(MaterialTheme.colorScheme.surfaceVariant.copy(alpha = 0.3f)).clipToBounds().onSizeChanged { size = it }) {
        Canvas(Modifier.fillMaxSize().semantics { contentDescription = "知识画布，${canvas.nodes.size} 张卡片，${canvas.edges.size} 条连线。通过卡片列表选择和编辑。" }
            .pointerInput(model,canvas.id,density) {
                awaitEachGesture {
                    val first = awaitFirstDown(requireUnconsumed = false)
                    if (model.busy) { do { val event = awaitPointerEvent() } while(event.changes.any { it.pressed }); return@awaitEachGesture }
                    val start = first.position
                    val hit = canvasHit(currentCanvas.nodes,viewport.world(start,size,density))
                    val startView = viewport
                    var moving = false
                    var transformed = false
                    var total = Offset.Zero
                    var movedNode: CanvasNode? = null
                    do {
                        val event = awaitPointerEvent()
                        val pressed = event.changes.count { it.pressed }
                        if (pressed >= 2) {
                            moving = true; transformed = true
                            viewport = viewport.transformed(event.calculateCentroid(useCurrent = false),event.calculatePan(),event.calculateZoom(),size,density)
                        } else {
                            val change = if(transformed) event.changes.firstOrNull { it.pressed } else event.changes.firstOrNull { it.id == first.id }
                            if (change != null && change.pressed) {
                                total += change.positionChange()
                                if (!moving && total.getDistance() > viewConfiguration.touchSlop) moving = true
                                if (moving) {
                                    if (hit != null && !transformed && currentConnect == null) {
                                        movedNode = hit
                                        dragPreview = CanvasDragPreview(hit.id,
                                            (hit.x + total.x / (viewport.zoom * density)).coerceIn(-250000f, 250000f),
                                            (hit.y + total.y / (viewport.zoom * density)).coerceIn(-250000f, 250000f))
                                        // Dragging must never switch the selection toolbar midway
                                        // through the gesture, since that resizes the viewport.
                                    } else viewport = viewport.transformed(change.previousPosition,change.positionChange(),1f,size,density)
                                }
                            }
                        }
                        if (moving) event.changes.forEach { it.consume() }
                    } while(event.changes.any { it.pressed })
                    if (moving) {
                        val node = movedNode
                        val preview = dragPreview?.takeIf { it.id == node?.id }
                        if (viewport != startView || preview != null) {
                            val accepted = model.submit(canvasRequest("gesture") {
                                put("viewX", viewport.x); put("viewY", viewport.y); put("zoom", viewport.zoom)
                                if (node != null && preview != null) {
                                    put("id", node.id); put("x", preview.x); put("y", preview.y)
                                }
                            })
                            if (!accepted) dragPreview = null
                        }
                    } else if(hit != null && currentConnect != null) currentOnConnect(hit.id) else currentSelect(hit?.id)
                }
            }) {
            val native = drawContext.canvas.nativeCanvas
            val scale = viewport.zoom*density
            val left = viewport.x - this.size.width/(2*scale)
            val top = viewport.y - this.size.height/(2*scale)
            val right = viewport.x + this.size.width/(2*scale)
            val bottom = viewport.y + this.size.height/(2*scale)
            native.save()
            native.translate(this.size.width/2-viewport.x*scale,this.size.height/2-viewport.y*scale)
            native.scale(scale,scale)
            paint.color = lineColor; paint.style = Paint.Style.FILL
            val step = if(viewport.zoom < 0.6f) 64f else 32f
            var gx = floor(left/step)*step
            while(gx <= right) { var gy=floor(top/step)*step; while(gy<=bottom) { native.drawCircle(gx,gy,0.8f/viewport.zoom,paint); gy+=step }; gx+=step }
            paint.color = lineColor; paint.strokeWidth = 2f / viewport.zoom
            // Include half the stroke outside the viewport when culling edge geometry.
            val strokeMargin = 1f / viewport.zoom
            val preview = dragPreview
            val moved = preview?.let { target -> canvas.nodes.firstOrNull { it.id == target.id } }
            val previewOffset = if (preview != null && moved != null) preview.offset(moved) else Offset.Zero
            val lineCount = edgeRenderer.updateVisible(left-strokeMargin, top-strokeMargin, right+strokeMargin, bottom+strokeMargin, preview?.id, previewOffset)
            if (lineCount > 0) native.drawLines(edgeRenderer.visibleLines, 0, lineCount, paint)
            canvas.nodes.forEach { node ->
                val nodeX = preview?.takeIf { it.id == node.id }?.x ?: node.x
                val nodeY = preview?.takeIf { it.id == node.id }?.y ?: node.y
                if(nodeX + node.width >= left && nodeX <= right && nodeY + node.height >= top && nodeY <= bottom) {
                    paint.style=Paint.Style.FILL; paint.color=surface
                    native.drawRoundRect(nodeX,nodeY,nodeX+node.width,nodeY+node.height,16f,16f,paint)
                    paint.style=Paint.Style.STROKE; paint.strokeWidth=if(node.id==selected || node.id==connectFrom) 2.5f/viewport.zoom else 1f/viewport.zoom
                    paint.color=if(node.id==selected || node.id==connectFrom) primary else canvasTint(node.color).copy(alpha=0.45f).toArgb()
                    native.drawRoundRect(nodeX,nodeY,nodeX+node.width,nodeY+node.height,16f,16f,paint)
                    paint.style=Paint.Style.FILL; paint.color=if (node.kind == "client") canvasTint("green").toArgb() else canvasTint(node.color).toArgb()
                    native.drawRoundRect(nodeX+12,nodeY+13,nodeX+16,nodeY+35,2f,2f,paint)
                    val textLayout = if(node.kind=="page") {
                        val label = textCache.label(pages[node.pageId]); textCache.layout(node,label.first,label.second,textColor)
                    } else {
                        val title = node.text.substringBefore("\n").ifBlank { if (node.kind == "client") "未命名客户" else "空白便签" }
                        val body = node.text.substringAfter("\n", "").trim()
                        textCache.layout(node,if(node.kind == "client") "●  "+title else title,if (node.kind == "client") "点击查看客户资料" else body,textColor)
                    }
                    native.save(); native.clipRect(nodeX+20,nodeY+12,nodeX+node.width-10,nodeY+node.height-10); native.translate(nodeX+24,nodeY+14)
                    textLayout.draw(native)
                    native.restore()
                }
            }
            native.restore()
        }
        if(canvas.nodes.isEmpty()) Column(Modifier.align(Alignment.Center).padding(24.dp), horizontalAlignment = Alignment.CenterHorizontally, verticalArrangement = Arrangement.spacedBy(12.dp)) {
            Text(if (canvas.template == "customer_network") "整理客户和推荐关系" else "从一个想法开始", style = MaterialTheme.typography.titleMedium)
            Text(if (canvas.template == "customer_network") "客户、便签和知识页面都可以自由连接" else "添加便签或页面，自由组织卡片与连线", style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
            FilledTonalButton(onClick = onAddFirst, enabled = visualReady) { Text(if (canvas.template == "customer_network") "＋ 添加第一个客户" else "＋ 添加第一张便签") }
        }
        Row(Modifier.align(Alignment.BottomEnd).padding(10.dp).background(MaterialTheme.colorScheme.surface.copy(alpha = 0.96f), RoundedCornerShape(18.dp)), verticalAlignment=Alignment.CenterVertically) {
            TextButton(enabled=visualReady, onClick={
                val nodes=canvas.nodes
                if(nodes.isNotEmpty() && size.width>0 && size.height>0) {
                    val left=nodes.minOf { it.x }; val right=nodes.maxOf { it.x+it.width }; val top=nodes.minOf { it.y }; val bottom=nodes.maxOf { it.y+it.height }
                    val fitZoom=min(size.width/density/(right-left+64),size.height/density/(bottom-top+64))
                    if (fitZoom < 0.35f) overview = true
                    else model.submit(canvasRequest("view") { put("x",(left+right)/2); put("y",(top+bottom)/2); put("zoom",fitZoom.coerceAtMost(2.5f)) })
                } else model.submit(canvasRequest("view") { put("x",0f); put("y",0f); put("zoom",1f) })
            }) { Text("适应") }
            TextButton(enabled=visualReady && canvas.nodes.isNotEmpty(), onClick={ overview = true }) { Text("总览") }
            TextButton(enabled=visualReady, onClick={ model.submit(canvasRequest("view") { put("x",viewport.x); put("y",viewport.y); put("zoom",1f) }) }) { CanvasZoomLabel(zoomPercent) }
        }
    }

    if (overview) {
        Dialog(onDismissRequest = { overview = false }, properties = DialogProperties(usePlatformDefaultWidth = false)) {
            Surface(Modifier.fillMaxWidth().fillMaxHeight(0.84f).padding(horizontal = 12.dp), shape = RoundedCornerShape(20.dp)) {
                Column(Modifier.padding(14.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
                    Row(verticalAlignment = Alignment.CenterVertically) {
                        Text("画布总览", Modifier.weight(1f), style = MaterialTheme.typography.titleMedium)
                        TextButton(onClick = { overview = false }) { Text("关闭") }
                    }
                    Text("点击概览中的位置跳转。这里仅浏览，不会改变已保存的缩放范围。",
                        style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
                    var overviewSize by remember(canvas.id) { mutableStateOf(IntSize.Zero) }
                    val transform = remember(canvas.nodes, overviewSize) { fitCanvasOverview(canvas.nodes, overviewSize) }
                    val index = remember(canvas.nodes) { canvas.nodes.associateBy { it.id } }
                    val overviewPaint = remember { Paint(Paint.ANTI_ALIAS_FLAG) }
                    Canvas(Modifier.fillMaxWidth().weight(1f).onSizeChanged { overviewSize = it }
                        .semantics { contentDescription = "画布总览，点击任意位置跳转" }
                        .pointerInput(canvas.id, overviewSize) {
                            detectTapGestures { point ->
                                if (!model.busy && overviewSize.width > 0 && overviewSize.height > 0) {
                                    val world = transform.worldAt(point)
                                    if (model.submit(canvasRequest("view") {
                                        put("x", world.x.coerceIn(-250000f, 250000f))
                                        put("y", world.y.coerceIn(-250000f, 250000f))
                                        put("zoom", 1f)
                                    })) overview = false
                                }
                            }
                        }) {
                        val native = drawContext.canvas.nativeCanvas
                        overviewPaint.style = Paint.Style.STROKE
                        overviewPaint.strokeWidth = 1.3f
                        overviewPaint.color = lineColor
                        canvas.edges.forEach { edge ->
                            val from = index[edge.from]
                            val to = index[edge.to]
                            if (from != null && to != null) {
                                val a = transform.screenAt(Offset(from.x + from.width / 2f, from.y + from.height / 2f))
                                val b = transform.screenAt(Offset(to.x + to.width / 2f, to.y + to.height / 2f))
                                native.drawLine(a.x, a.y, b.x, b.y, overviewPaint)
                            }
                        }
                        canvas.nodes.forEach { node ->
                            val a = transform.screenAt(Offset(node.x, node.y))
                            val w = max(2.5f, node.width * transform.scale)
                            val h = max(2.5f, node.height * transform.scale)
                            overviewPaint.style = Paint.Style.FILL
                            overviewPaint.color = canvasTint(node.color).toArgb()
                            native.drawRoundRect(a.x, a.y, a.x + w, a.y + h, 2f, 2f, overviewPaint)
                            if (node.id == selected) {
                                overviewPaint.style = Paint.Style.STROKE
                                overviewPaint.strokeWidth = 2f
                                overviewPaint.color = primary
                                native.drawRoundRect(a.x - 2f, a.y - 2f, a.x + w + 2f, a.y + h + 2f, 3f, 3f, overviewPaint)
                            }
                        }
                        if (size.width > 0 && size.height > 0) {
                            val halfWidth = size.width / (2f * viewport.zoom * density)
                            val halfHeight = size.height / (2f * viewport.zoom * density)
                            val upperLeft = transform.screenAt(Offset(viewport.x - halfWidth, viewport.y - halfHeight))
                            val lowerRight = transform.screenAt(Offset(viewport.x + halfWidth, viewport.y + halfHeight))
                            overviewPaint.style = Paint.Style.STROKE
                            overviewPaint.strokeWidth = 2f
                            overviewPaint.color = primary
                            native.drawRect(upperLeft.x, upperLeft.y, lowerRight.x, lowerRight.y, overviewPaint)
                        }
                    }
                }
            }
        }
    }
}

@Composable
private fun CanvasZoomLabel(percent: State<Int>) { Text("${percent.value}%") }
"####;
