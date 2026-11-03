//! Rust-owned Android AI image picker, preview and bounded content-URI reader.
//! Android framework glue is generated from this Rust template. Image format
//! verification and request serialization stay in the Rust native library.
pub const PATH: &str = "com/ofairyo/gridtimer/ui/KnowledgeAiImagePicker.kt";
pub const CONTENTS: &str = r####"package com.ofairyo.gridtimer.ui

import android.content.Context
import android.graphics.BitmapFactory
import android.net.Uri
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.Image
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.produceState
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.asImageBitmap
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.unit.dp
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import java.io.ByteArrayOutputStream

// These limits mirror native/android_ai_images.rs. Content URIs are selected
// by the user; they are not persisted or enumerated from the knowledge base.
private const val MAX_AI_IMAGES = 5
private const val MAX_AI_IMAGE_BYTES = 6 * 1024 * 1024
private const val MAX_AI_TOTAL_BYTES = 20 * 1024 * 1024

internal fun readSelectedAiImages(context: Context, selected: List<Uri>): Array<ByteArray> {
    require(selected.size <= MAX_AI_IMAGES) { "每轮最多选择 5 张图片。" }
    var total = 0
    return selected.mapIndexed { index, uri ->
        val result = ByteArrayOutputStream()
        val stream = context.contentResolver.openInputStream(uri)
            ?: throw IllegalArgumentException("无法读取第 ${index + 1} 张图片，请重新选择。")
        stream.use { input ->
            val buffer = ByteArray(16 * 1024)
            while (true) {
                val count = input.read(buffer)
                if (count < 0) break
                if (count == 0) continue
                if (result.size() + count > MAX_AI_IMAGE_BYTES) {
                    throw IllegalArgumentException("第 ${index + 1} 张图片超过 6 MiB，请选择较小的图片。")
                }
                total += count
                if (total > MAX_AI_TOTAL_BYTES) {
                    throw IllegalArgumentException("图片合计超过 20 MiB，请减少数量。")
                }
                result.write(buffer, 0, count)
            }
        }
        if (result.size() == 0) throw IllegalArgumentException("第 ${index + 1} 张图片没有可读取的内容。")
        result.toByteArray()
    }.toTypedArray()
}

@Composable
private fun KnowledgeAiThumbnail(uri: Uri) {
    val context = LocalContext.current
    val thumbnail by produceState<androidx.compose.ui.graphics.ImageBitmap?>(null, uri) {
        value = withContext(Dispatchers.IO) {
            runCatching {
                val bounds = BitmapFactory.Options().apply { inJustDecodeBounds = true }
                context.contentResolver.openInputStream(uri)?.use {
                    BitmapFactory.decodeStream(it, null, bounds)
                }
                if (bounds.outWidth <= 0 || bounds.outHeight <= 0) return@runCatching null
                var sample = 1
                while (maxOf(bounds.outWidth, bounds.outHeight) / sample > 144) sample *= 2
                val options = BitmapFactory.Options().apply { inSampleSize = sample }
                context.contentResolver.openInputStream(uri)?.use {
                    BitmapFactory.decodeStream(it, null, options)?.asImageBitmap()
                }
            }.getOrNull()
        }
    }
    if (thumbnail != null) Image(
        bitmap = thumbnail!!,
        contentDescription = "已选图片缩略图",
        modifier = Modifier.size(48.dp)
    ) else Text("图片", style = MaterialTheme.typography.labelSmall)
}

@Composable
internal fun KnowledgeAiImagePicker(
    selected: List<Uri>,
    enabled: Boolean,
    onChange: (List<Uri>) -> Unit
) {
    val launcher = rememberLauncherForActivityResult(ActivityResultContracts.GetMultipleContents()) { uris ->
        if (uris.isNotEmpty()) onChange((selected + uris).distinct().take(MAX_AI_IMAGES))
    }
    Column(verticalArrangement = Arrangement.spacedBy(6.dp)) {
        Row(verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            OutlinedButton(enabled = enabled && selected.size < MAX_AI_IMAGES,
                onClick = { launcher.launch("image/*") }) {
                Text("添加图片 ${selected.size}/$MAX_AI_IMAGES")
            }
            if (selected.isNotEmpty()) {
                TextButton(enabled = enabled, onClick = { onChange(emptyList()) }) { Text("清空图片") }
            }
        }
        selected.forEachIndexed { index, uri ->
            Row(verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(10.dp)) {
                KnowledgeAiThumbnail(uri)
                Text("图片 ${index + 1}", style = MaterialTheme.typography.bodySmall)
                TextButton(enabled = enabled, onClick = {
                    onChange(selected.filterIndexed { at, _ -> at != index })
                }) { Text("移除") }
            }
        }
        if (selected.isNotEmpty()) {
            Text("仅发送本次选择的图片到当前 AI 接口；可能包含拍摄元数据。支持 JPG、PNG、GIF、WebP，单张最多 6 MiB。",
                style = MaterialTheme.typography.labelSmall)
        }
    }
}
"####;

#[cfg(test)]
mod tests {
    use super::CONTENTS;
    #[test]
    fn picker_has_bounded_reads_and_explicit_user_selection() {
        assert!(CONTENTS.contains("GetMultipleContents()"));
        assert!(CONTENTS.contains("launcher.launch(\"image/*\")"));
        assert!(CONTENTS.contains("MAX_AI_IMAGE_BYTES"));
        assert!(CONTENTS.contains("MAX_AI_TOTAL_BYTES"));
        assert!(CONTENTS.contains("withContext(Dispatchers.IO)"));
        assert!(CONTENTS.contains("onChange(emptyList())"));
        assert!(!CONTENTS.contains("READ_MEDIA_IMAGES"));
    }
}
