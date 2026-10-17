// v2.23.2.41 - Require a painted, nonblank body before accepting share-image raster output.
// v2.23.2.40 - Render share-image Markdown and formulas before rasterizing the knowledge page body.
// v2.23.2.33 - Keep preview generation aligned with the compact editor prefix.
//! v2.23.2.28 - Keep the document visible during a durable exit save.
//! No repository, queue ordering, checkpoint or encryption changes.

const PATH: &str = "com/ofairyo/gridtimer/ui/NoteDocumentEditor.kt";
const SHARE_PATH: &str = "com/ofairyo/gridtimer/ui/NoteMediaUi.kt";

fn replace(source: &mut String, before: &str, after: &str) -> Result<(), String> {
    if source.matches(before).count() != 1 {
        return Err(format!(
            "document exit feedback anchor missing or repeated: {before}"
        ));
    }
    *source = source.replacen(before, after, 1);
    Ok(())
}

pub fn render(path: &str, base: &str) -> Result<String, String> {
    if path == SHARE_PATH {
        let mut source = base.to_owned();
        // The old long-image path paints note.plainContent() with StaticLayout, so
        // Markdown and TeX delimiters are exported literally. Render the body with
        // the same offline Markdown/KaTeX engine used by the on-screen preview, then
        // composite that raster into the existing share card. If WebView rendering
        // cannot complete, preserve the previous StaticLayout path as a safe fallback.
        replace(
            &mut source,
            SHARE_BODY_LAYOUT_BEFORE,
            SHARE_BODY_LAYOUT_AFTER,
        )?;
        replace(
            &mut source,
            SHARE_TOTAL_HEIGHT_BEFORE,
            SHARE_TOTAL_HEIGHT_AFTER,
        )?;
        replace(&mut source, SHARE_BODY_DRAW_BEFORE, SHARE_BODY_DRAW_AFTER)?;

        source.push_str(SHARE_IMAGE_HELPER);
        source.push_str(SHARE_RASTER_POLICY);
        return Ok(source);
    }
    if path != PATH {
        return Ok(base.to_owned());
    }
    let mut source = base.to_owned();
    for import in [
        "androidx.compose.runtime.derivedStateOf",
        "androidx.compose.ui.platform.LocalSoftwareKeyboardController",
        "androidx.compose.ui.semantics.clearAndSetSemantics",
    ] {
        let statement = format!("import {import}\n");
        if !source.contains(&statement) {
            replace(
                &mut source,
                "package com.ofairyo.gridtimer.ui\n",
                &format!("package com.ofairyo.gridtimer.ui\n{statement}"),
            )?;
        }
    }
    replace(&mut source, "    val focusManager = LocalFocusManager.current", "    val focusManager = LocalFocusManager.current\n    val exitKeyboard = LocalSoftwareKeyboardController.current\n    val documentTransitionActive = LocalDocumentTransitionActive.current")?;
    replace(
        &mut source,
        "    fun handleBack() {",
        "    fun handleBack() {\n        if (!documentTransitionActive) return",
    )?;
    replace(&mut source, "        enabled = !revisionDialogVisible && !callPickerVisible && htmlPreview == null", "        enabled = documentTransitionActive && !revisionDialogVisible && !callPickerVisible && htmlPreview == null")?;
    replace(&mut source, "        isSavingDraft = true\n        persistLatestDraft(", "        pendingFocusBlockId = null\n        focusManager.clearFocus(force = true)\n        exitKeyboard?.hide()\n        isSavingDraft = true\n        persistLatestDraft(")?;
    replace(&mut source, "    val markdownPreviewText = documentMarkdownPreviewText(initialDocument, blocks, textFieldStates)", PREVIEW)?;
    replace(&mut source, "        if (documentMarkdownPreviewText(initialDocument, blocks, textFieldStates) == null) return", "        if (!documentMarkdownPreviewAvailable(initialDocument, blocks, textFieldStates)) return")?;
    // Caret indexing no longer depends on a separate preview-switch list item.
    replace(
        &mut source,
        "        if (markdownPreviewText != null) {",
        "        if (markdownPreviewAvailable) {",
    )?;
    replace(&mut source, "    Column(\n        modifier = Modifier\n            .fillMaxSize()\n            .background(flowusEditorBackground())", "    Box(modifier = Modifier.fillMaxSize()) {\n    Column(\n        modifier = Modifier\n            .fillMaxSize()\n            .then(if (isSavingDraft) Modifier.clearAndSetSemantics { } else Modifier)\n            .background(flowusEditorBackground())")?;
    replace(
        &mut source,
        "    if (insertCommandsVisible) {\n        DocumentInsertCommandDialog(",
        SAVE_FEEDBACK,
    )?;
    replace(&mut source, "    if (isSavingDraft || isRenderingLongImage || isRenderingHtml || isTranscribingMedia) {", "    if (isRenderingLongImage || isRenderingHtml || isTranscribingMedia) {")?;
    replace(
        &mut source,
        "                        isSavingDraft -> \"正在保存…\"\n",
        "",
    )?;
    replace(&mut source,
        "    var selectedBlockMenuId by remember { mutableStateOf<String?>(null) }",
        "    var selectedBlockMenuId by remember { mutableStateOf<String?>(null) }\n    LaunchedEffect(documentTransitionActive) {\n        if (!documentTransitionActive) {\n            focusManager.clearFocus(force = true)\n            exitKeyboard?.hide()\n            documentMoreVisible = false\n            shareExportVisible = false\n            documentPropertiesVisible = false\n            insertCommandsVisible = false\n            selectedBlockMenuId = null\n            revisionDialogVisible = false\n            callPickerVisible = false\n            htmlPreview = null\n            encryptDialogVisible = false\n            encryptionSettingsVisible = false\n            changePasswordDialogVisible = false\n            disableEncryptionDialogVisible = false\n        }\n    }")?;
    replace(&mut source, "    fun recordTitleChange(nextTitle: String) {",
        "    fun recordTitleChange(nextTitle: String) {\n        if (!documentTransitionActive || isSavingDraft) return")?;
    replace(&mut source, "    fun recordChange(next: NoteDraftState) {",
        "    fun recordChange(next: NoteDraftState) {\n        if (!documentTransitionActive || isSavingDraft) return")?;
    replace(&mut source, "                onTextChanged = { updated ->\n                    activeTextBlockId = block.id",
        "                onTextChanged = textChange@ { updated ->\n                    if (!documentTransitionActive || isSavingDraft) return@textChange\n                    activeTextBlockId = block.id")?;
    replace(&mut source, "    if (insertCommandsVisible) {\n        DocumentInsertCommandDialog(",
        "    if (documentTransitionActive) {\n    if (insertCommandsVisible) {\n        DocumentInsertCommandDialog(")?;
    replace(
        &mut source,
        "}\n\nprivate fun NoteDocument.ensureEditableBlocks()",
        "    }\n}\n\nprivate fun NoteDocument.ensureEditableBlocks()",
    )?;

    source.push_str(PREVIEW_HELPER);
    Ok(source)
}

const PREVIEW: &str = r####"    // Reading the snapshot map inside derivedStateOf tracks IME text changes.
    // Save-state and transition frames do not reparse the complete Markdown body.
    val markdownPreviewAvailable by remember(initialDocument, blocks, textFieldStates) {
        derivedStateOf { documentMarkdownPreviewAvailable(initialDocument, blocks, textFieldStates) }
    }
    val markdownPreviewText by remember(initialDocument, blocks, textFieldStates, markdownPreview) {
        derivedStateOf {
            if (markdownPreview) documentMarkdownPreviewText(initialDocument, blocks, textFieldStates) else null
        }
    }"####;

const PREVIEW_HELPER: &str = r####"

private fun documentMarkdownPreviewAvailable(
    sourceDocument: NoteDocument,
    draftBlocks: List<NoteBlock>,
    textFieldStates: Map<String, TextFieldValue>
): Boolean = sourceDocument.markdownEnabled && !sourceDocument.richTextEnabled &&
    draftBlocks.all { it.type == NoteBlockType.TEXT } &&
    draftBlocks.any { (textFieldStates[it.id]?.text ?: it.text).isNotBlank() }
"####;

const SAVE_FEEDBACK: &str = r####"    // Block new edits after capturing the exit draft without blanking the page.
    // The delayed indicator avoids flashing for saves completed within one beat.
    if (isSavingDraft) {
        DocumentEditorInputBarrier()
        var showExitProgress by remember { mutableStateOf(false) }
        LaunchedEffect(Unit) { delay(180L); showExitProgress = true }
        if (showExitProgress) {
            Surface(
                modifier = Modifier.align(Alignment.BottomCenter).navigationBarsPadding().padding(16.dp),
                shape = RoundedCornerShape(16.dp),
                color = MaterialTheme.colorScheme.surfaceContainerHigh
            ) {
                Row(
                    modifier = Modifier.padding(horizontal = 16.dp, vertical = 12.dp),
                    verticalAlignment = Alignment.CenterVertically,
                    horizontalArrangement = Arrangement.spacedBy(10.dp)
                ) {
                    CircularProgressIndicator(modifier = Modifier.size(18.dp), strokeWidth = 2.dp)
                    Text("正在保存…", style = MaterialTheme.typography.bodyMedium)
                }
            }
        }
    }
    }

    if (insertCommandsVisible) {
        DocumentInsertCommandDialog("####;

const SHARE_BODY_LAYOUT_BEFORE: &str =
    r####"    val bodyLayout = buildStaticLayout(note.plainContent(), bodyPaint, contentWidth)"####;
const SHARE_BODY_LAYOUT_AFTER: &str = r####"    val renderedBody = try {
        renderMarkdownShareBody(context, note.plainContent(), contentWidth)
    } catch (cancelled: kotlinx.coroutines.CancellationException) {
        throw cancelled
    } catch (_: Exception) {
        null
    }
    val bodyLayout = if (renderedBody == null) buildStaticLayout(note.plainContent(), bodyPaint, contentWidth) else null
    val bodyHeight = renderedBody?.height ?: bodyLayout!!.height"####;

const SHARE_TOTAL_HEIGHT_BEFORE: &str = r####"    val totalHeight = padding + titleLayout.height + 28 + metaLayout.height + 36 + bodyLayout.height +
        attachmentTopGap + attachmentHeights + 32 + footerLayout.height + padding"####;
const SHARE_TOTAL_HEIGHT_AFTER: &str = r####"    val totalHeight = padding + titleLayout.height + 28 + metaLayout.height + 36 + bodyHeight +
        attachmentTopGap + attachmentHeights + 32 + footerLayout.height + padding"####;

const SHARE_BODY_DRAW_BEFORE: &str = r####"    canvas.save()
    canvas.translate(padding.toFloat(), top)
    bodyLayout.draw(canvas)
    canvas.restore()
    top += bodyLayout.height + 24f"####;
const SHARE_BODY_DRAW_AFTER: &str = r####"    if (renderedBody != null) {
        canvas.drawBitmap(renderedBody, padding.toFloat(), top, Paint(Paint.ANTI_ALIAS_FLAG))
        renderedBody.recycle()
    } else {
        canvas.save()
        canvas.translate(padding.toFloat(), top)
        bodyLayout!!.draw(canvas)
        canvas.restore()
    }
    top += bodyHeight + 24f"####;

const SHARE_IMAGE_HELPER: &str = r####"

private const val SHARE_MARKDOWN_SNAPSHOT = """
(()=>{const root=document.getElementById('android-ai-answer');const formulas=root?root.querySelectorAll('.math-source[data-latex]'):[];return [String(location.href),document.readyState,!!root,root?root.dataset.renderSession:'',formulas.length,document.documentElement.dataset.mathReady==='true',root?root.querySelectorAll('[data-math-rendered=true]').length:0,root?root.querySelectorAll('[data-math-error=true]').length:0,Math.ceil(document.body.getBoundingClientRect().height),!document.fonts||document.fonts.status==='loaded'];})()
"""

@android.annotation.SuppressLint("SetJavaScriptEnabled")
private suspend fun renderMarkdownShareBody(
    context: android.content.Context,
    content: String,
    width: Int,
): android.graphics.Bitmap? {
    if (content.isBlank() || width <= 0) return null
    val html = kotlinx.coroutines.withContext(kotlinx.coroutines.Dispatchers.IO) {
        com.ofairyo.gridtimer.core.NativeOptimizerBridge.renderAndroidAiAnswerHtml(content, false)
    }?.takeIf(String::isNotBlank) ?: return null

    val raster = kotlinx.coroutines.withContext(kotlinx.coroutines.Dispatchers.Main.immediate) {
        kotlinx.coroutines.suspendCancellableCoroutine<android.graphics.Bitmap?> { continuation ->
            val token = java.util.UUID.randomUUID().toString().replace("-", "")
            val documentUrl = ANDROID_MARKDOWN_DOCUMENT_PREFIX + token + "/index.html"
            val document = html.replaceFirst(
                "<main id=\"android-ai-answer\">",
                "<main id=\"android-ai-answer\" data-render-session=\"$token\">",
            )
            val documentBytes = document.toByteArray(Charsets.UTF_8)
            var owner: android.content.Context? = context
            while (owner is android.content.ContextWrapper && owner !is android.app.Activity) {
                val next = owner.baseContext
                if (next === owner) break
                owner = next
            }
            val activity = owner as? android.app.Activity
            val host = activity?.window?.decorView as? android.view.ViewGroup
            if (activity == null || host == null || activity.isFinishing || activity.isDestroyed) {
                continuation.resumeWith(Result.success(null))
                return@suspendCancellableCoroutine
            }
            val view = android.webkit.WebView(activity)
            val handler = android.os.Handler(android.os.Looper.getMainLooper())
            var deadline: Runnable? = null
            val density = context.resources.displayMetrics.density.coerceAtLeast(1f)
            var finished = false

            fun finish(result: android.graphics.Bitmap?) {
                if (finished) {
                    result?.recycle()
                    return
                }
                finished = true
                deadline?.let(handler::removeCallbacks)
                (view.parent as? android.view.ViewGroup)?.removeView(view)
                view.stopLoading()
                view.webViewClient = android.webkit.WebViewClient()
                view.destroy()
                if (continuation.isActive) continuation.resumeWith(Result.success(result))
                else result?.recycle()
            }

            fun capture(attempt: Int) {
                if (finished) return
                view.evaluateJavascript(SHARE_MARKDOWN_SNAPSHOT) { raw ->
                    if (finished) return@evaluateJavascript
                    val state = runCatching { org.json.JSONArray(raw) }.getOrNull()
                    val formulaCount = state?.optInt(4, -1) ?: -1
                    val renderedCount = state?.optInt(6, -1) ?: -1
                    val errorCount = state?.optInt(7, -1) ?: -1
                    val ready = state != null &&
                        state.optString(0) == documentUrl &&
                        state.optString(1) == "complete" &&
                        state.optBoolean(2) &&
                        state.optString(3) == token &&
                        formulaCount >= 0 && renderedCount >= 0 && errorCount >= 0 &&
                        (formulaCount == 0 || (state.optBoolean(5) && renderedCount + errorCount == formulaCount)) &&
                        state.optBoolean(9)
                    if (!ready) {
                        if (attempt < 40) view.postDelayed({ capture(attempt + 1) }, 50L) else finish(null)
                        return@evaluateJavascript
                    }
                    val cssHeight = state?.optDouble(8, Double.NaN) ?: Double.NaN
                    if (!cssHeight.isFinite() || cssHeight <= 0.0) {
                        finish(null)
                        return@evaluateJavascript
                    }
                    val pixelHeightLong = kotlin.math.ceil(cssHeight * density).toLong()
                    if (!shareRasterSizeAllowed(width, pixelHeightLong)) {
                        finish(null)
                        return@evaluateJavascript
                    }
                    val pixelHeight = pixelHeightLong.toInt()
                    val widthSpec = android.view.View.MeasureSpec.makeMeasureSpec(width, android.view.View.MeasureSpec.EXACTLY)
                    val heightSpec = android.view.View.MeasureSpec.makeMeasureSpec(pixelHeight, android.view.View.MeasureSpec.EXACTLY)
                    // DOM readiness is not a compositor/paint fence. Keep the WebView
                    // attached and visible behind the app until its resized frame is ready.
                    view.layoutParams = view.layoutParams.apply { height = pixelHeight }
                    view.measure(widthSpec, heightSpec)
                    view.layout(0, 0, width, pixelHeight)
                    view.postVisualStateCallback(1L, object : android.webkit.WebView.VisualStateCallback() {
                        override fun onComplete(requestId: Long) {
                            // The guaranteed frame is the next draw AFTER this callback
                            // returns. Drawing inside onComplete can still see the old frame.
                            handler.post {
                                if (finished) return@post
                                var output: android.graphics.Bitmap? = null
                                val bitmap = runCatching {
                                    output = android.graphics.Bitmap.createBitmap(width, pixelHeight, android.graphics.Bitmap.Config.ARGB_8888)
                                    val canvas = android.graphics.Canvas(output!!)
                                    canvas.drawColor(android.graphics.Color.WHITE)
                                    view.draw(canvas)
                                    output
                                }.getOrElse {
                                    output?.recycle()
                                    null
                                }
                                finish(bitmap)
                            }
                        }
                    })
                }
            }

            try {
                view.setBackgroundColor(android.graphics.Color.WHITE)
                view.setLayerType(android.view.View.LAYER_TYPE_SOFTWARE, null)
                view.settings.apply {
                    javaScriptEnabled = true
                    domStorageEnabled = false
                    allowFileAccess = false
                    allowContentAccess = false
                    blockNetworkLoads = true
                    loadsImagesAutomatically = false
                    cacheMode = android.webkit.WebSettings.LOAD_NO_CACHE
                    mixedContentMode = android.webkit.WebSettings.MIXED_CONTENT_NEVER_ALLOW
                    setSupportMultipleWindows(false)
                    useWideViewPort = false
                    loadWithOverviewMode = false
                    textZoom = ((34f / (16f * density)) * 100f).roundToInt().coerceIn(50, 300)
                }
                view.webViewClient = object : android.webkit.WebViewClient() {
                    override fun shouldInterceptRequest(
                        webView: android.webkit.WebView,
                        request: android.webkit.WebResourceRequest,
                    ): android.webkit.WebResourceResponse? {
                        val url = request.url.toString()
                        if (request.isForMainFrame && androidMarkdownDocumentAllowed(url, documentUrl)) {
                            return android.webkit.WebResourceResponse(
                                "text/html",
                                "utf-8",
                                200,
                                "OK",
                                mapOf("Cache-Control" to "no-store"),
                                java.io.ByteArrayInputStream(documentBytes),
                            )
                        }
                        if (androidMarkdownResourceAllowed(url, documentUrl, request.isForMainFrame)) return null
                        return android.webkit.WebResourceResponse(
                            "text/plain",
                            "utf-8",
                            403,
                            "Blocked",
                            emptyMap(),
                            java.io.ByteArrayInputStream(ByteArray(0)),
                        )
                    }

                    override fun shouldOverrideUrlLoading(
                        webView: android.webkit.WebView,
                        request: android.webkit.WebResourceRequest,
                    ): Boolean = !(request.isForMainFrame && androidMarkdownDocumentAllowed(request.url.toString(), documentUrl))

                    override fun onPageFinished(webView: android.webkit.WebView, url: String?) {
                        if (androidMarkdownDocumentAllowed(url, documentUrl)) capture(0) else finish(null)
                    }

                    override fun onReceivedError(
                        webView: android.webkit.WebView,
                        request: android.webkit.WebResourceRequest,
                        error: android.webkit.WebResourceError,
                    ) {
                        if (request.isForMainFrame) finish(null)
                    }

                    override fun onRenderProcessGone(
                        webView: android.webkit.WebView,
                        detail: android.webkit.RenderProcessGoneDetail,
                    ): Boolean {
                        finish(null)
                        return true
                    }
                }
                val widthSpec = android.view.View.MeasureSpec.makeMeasureSpec(width, android.view.View.MeasureSpec.EXACTLY)
                val initialHeight = (400f * density).roundToInt()
                val initialHeightSpec = android.view.View.MeasureSpec.makeMeasureSpec(initialHeight, android.view.View.MeasureSpec.EXACTLY)
                view.importantForAccessibility = android.view.View.IMPORTANT_FOR_ACCESSIBILITY_NO_HIDE_DESCENDANTS
                view.descendantFocusability = android.view.ViewGroup.FOCUS_BLOCK_DESCENDANTS
                view.isFocusable = false
                view.isClickable = false
                // Index zero keeps the temporary renderer behind the existing application
                // content. It remains VISIBLE/attached so Chromium must produce a frame.
                host.addView(view, 0, android.view.ViewGroup.LayoutParams(width, initialHeight))
                view.measure(widthSpec, initialHeightSpec)
                view.layout(0, 0, width, initialHeight)
                deadline = Runnable { finish(null) }.also { handler.postDelayed(it, 8_000L) }
                continuation.invokeOnCancellation {
                    handler.post {
                        if (!finished) finish(null)
                    }
                }
                view.loadUrl(documentUrl)
            } catch (_: Exception) {
                finish(null)
            }
        }
    } ?: return null
    // Never equate an allocated bitmap with rendered text. Some WebView providers
    // can return an all-white/transparent frame even after a visual callback.
    return try {
        kotlinx.coroutines.withContext(kotlinx.coroutines.Dispatchers.IO) {
            if (shareRasterHasReadableInk(raster.width, raster.height) { y, row ->
                    raster.getPixels(row, 0, raster.width, 0, y, raster.width, 1)
                }) raster else { raster.recycle(); null }
        }
    } catch (cancelled: kotlinx.coroutines.CancellationException) {
        raster.recycle()
        throw cancelled
    } catch (_: Exception) {
        raster.recycle()
        null
    }
}
"####;

#[cfg(test)]
mod tests {
    use super::*;

    fn editor() -> &'static str {
        super::super::kotlin_sources::SOURCES
            .iter()
            .find(|item| item.path == SHARE_PATH)
            .expect("real note media export template")
            .contents
    }

    #[test]
    fn share_image_routes_markdown_through_offline_formula_renderer() {
        let base = editor();
        let rendered = render(SHARE_PATH, base).unwrap();
        for marker in [
            "renderMarkdownShareBody(context, note.plainContent(), contentWidth)",
            "NativeOptimizerBridge.renderAndroidAiAnswerHtml(content, false)",
            "ANDROID_MARKDOWN_DOCUMENT_PREFIX",
            "androidMarkdownDocumentAllowed",
            "androidMarkdownResourceAllowed",
            "[data-math-rendered=true]",
            "val bodyHeight = renderedBody?.height ?: bodyLayout!!.height",
            "renderedBody.recycle()",
        ] {
            assert!(rendered.contains(marker), "missing {marker}");
        }
        assert!(!rendered.contains(SHARE_BODY_LAYOUT_BEFORE));
        assert!(!rendered.contains(SHARE_TOTAL_HEIGHT_BEFORE));
        assert!(!rendered.contains(SHARE_BODY_DRAW_BEFORE));
        assert!(render(SHARE_PATH, &rendered).is_err());
    }

    #[test]
    fn share_image_patch_fails_closed_when_export_shape_drifts() {
        let base = editor();
        for anchor in [
            SHARE_BODY_LAYOUT_BEFORE,
            SHARE_TOTAL_HEIGHT_BEFORE,
            SHARE_BODY_DRAW_BEFORE,
        ] {
            assert!(render(SHARE_PATH, &base.replacen(anchor, "", 1)).is_err());
            assert!(render(SHARE_PATH, &format!("{base}\n{anchor}")).is_err());
        }
    }
}

pub const SHARE_RASTER_POLICY: &str = r####"

// Export acceptance, independent of DOM readiness and bitmap allocation.
internal fun shareRasterSizeAllowed(width: Int, height: Long): Boolean =
    width > 0 && height > 0L && height <= (32L * 1024L * 1024L) / 4L / width

internal fun shareRasterHasReadableInk(width: Int, height: Int, readRow: (Int, IntArray) -> Unit): Boolean {
    if (!shareRasterSizeAllowed(width, height.toLong())) return false
    val row = IntArray(width)
    var ink = 0
    var paper = 0
    for (y in 0 until height) {
        readRow(y, row)
        for (pixel in row) {
            val alpha = pixel ushr 24
            if (alpha < 192) continue
            val red = (pixel ushr 16) and 255
            val green = (pixel ushr 8) and 255
            val blue = pixel and 255
            if (red < 192 && green < 192 && blue < 192) ink++
            if (red > 240 && green > 240 && blue > 240) paper++
            if (ink >= 8 && paper >= 8) return true
        }
    }
    return false
}
"####;

pub const SHARE_RASTER_TEST_PATH: &str = "com/ofairyo/gridtimer/ui/ShareRasterPolicyTest.kt";
pub const SHARE_RASTER_TESTS: &str = r####"package com.ofairyo.gridtimer.ui

import org.junit.Assert.*
import org.junit.Test

class ShareRasterPolicyTest {
    private fun accepts(pixels: IntArray, width: Int = 32): Boolean =
        shareRasterHasReadableInk(width, pixels.size / width) { y, row ->
            pixels.copyInto(row, 0, y * width, (y + 1) * width)
        }

    @Test fun allocatedButUnpaintedFrameIsRejected() {
        assertFalse(accepts(IntArray(1024)))
        assertFalse(accepts(IntArray(1024) { -1 }))
        assertFalse(accepts(IntArray(1024) { 0x00ffffff }))
        assertFalse(accepts(IntArray(1024) { 0x7f000000 }))
    }
    @Test fun readableBodyAtBottomOfLongFrameIsAccepted() {
        val pixels = IntArray(32 * 2048) { -1 }
        for (i in pixels.lastIndex - 31..pixels.lastIndex) pixels[i] = 0xff2c2c2c.toInt()
        assertTrue(accepts(pixels))
    }
    @Test fun solidFrameAndSinglePixelNoiseAreRejected() {
        assertFalse(accepts(IntArray(1024) { 0xff000000.toInt() }))
        val pixels = IntArray(1024) { -1 }
        pixels[0] = 0xff000000.toInt()
        assertFalse(accepts(pixels))
    }
    @Test fun dimensionsFailClosedBeforeReadingPixels() {
        for ((w, h) in listOf(0 to 32, 32 to 0, -1 to 32, Int.MAX_VALUE to Int.MAX_VALUE)) {
            assertFalse(shareRasterHasReadableInk(w, h) { _, _ -> error("must not read") })
        }
        assertFalse(shareRasterSizeAllowed(1074, 80000))
        assertFalse(shareRasterSizeAllowed(1074, Long.MAX_VALUE))
        assertTrue(shareRasterSizeAllowed(1074, 1000))
    }
}
"####;
