// v2.22.49.10 - Localize Android interface text from bundled language catalogs.
pub const KOTLIN_PATH: &str = "com/ofairyo/gridtimer/i18n/LocalizedText.kt";

pub const KOTLIN_CONTENTS: &str = r####"package com.ofairyo.gridtimer.i18n

import android.content.Context
import android.widget.Toast
import androidx.compose.foundation.text.InlineTextContent
import androidx.compose.material3.LocalTextStyle
import androidx.compose.material3.Text as MaterialText
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalConfiguration
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.AnnotatedString
import androidx.compose.ui.text.TextLayoutResult
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextDecoration
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.TextUnit
import com.ofairyo.gridtimer.R
import org.json.JSONObject
import java.util.Collections
import java.util.IdentityHashMap
import java.util.Locale

private object UiTextCatalog {
    @Volatile private var loadedLanguage: String? = null
    @Volatile private var loadedStrings: Map<String, String> = emptyMap()
    @Volatile private var loadedLiteralReferences: Set<String> = emptySet()

    fun translate(context: Context, language: String, original: String, sourceLiteralOnly: Boolean): String {
        if (language == "zh" || original.isBlank()) return original
        if (loadedLanguage != language) {
            synchronized(this) {
                if (loadedLanguage != language) {
                    val (strings, literals) = load(context, language)
                    loadedStrings = strings
                    loadedLiteralReferences = literals
                    loadedLanguage = language
                }
            }
        }
        // A note or canvas title can equal a UI label. Translate only source-code
        // literals in the Text wrapper; keep text loaded from user data verbatim.
        if (sourceLiteralOnly && !loadedLiteralReferences.contains(original)) return original
        return loadedStrings[original] ?: original
    }

    private fun load(context: Context, language: String): Pair<Map<String, String>, Set<String>> {
        val resourceId = when (language) {
            "en" -> R.raw.ui_en
            "hi" -> R.raw.ui_hi
            "es" -> R.raw.ui_es
            "ar" -> R.raw.ui_ar
            "fr" -> R.raw.ui_fr
            "bn" -> R.raw.ui_bn
            "pt" -> R.raw.ui_pt
            "id", "in" -> R.raw.ui_id
            "ur" -> R.raw.ui_ur
            "ja" -> R.raw.ui_ja
            else -> return emptyMap<String, String>() to emptySet()
        }
        return runCatching<Pair<Map<String, String>, Set<String>>> {
            val source = context.resources.openRawResource(resourceId)
                .bufferedReader(Charsets.UTF_8).use { it.readText() }
            val json = JSONObject(source)
            val result = HashMap<String, String>(json.length())
            val literalReferences = Collections.newSetFromMap(IdentityHashMap<String, Boolean>())
            val keys = json.keys()
            while (keys.hasNext()) {
                val key = keys.next()
                val value = json.optString(key)
                if (value.isNotBlank()) {
                    result[key] = value
                    literalReferences.add(key.intern())
                }
            }
            result to literalReferences
        }.getOrDefault(emptyMap<String, String>() to emptySet<String>())
    }
}

fun localizedToast(context: Context, message: CharSequence, duration: Int): Toast {
    val language = context.resources.configuration.locales[0]
        ?.language?.lowercase(Locale.ROOT)
        ?: Locale.getDefault().language.lowercase(Locale.ROOT)
    val localized = UiTextCatalog.translate(context, language, message.toString(), sourceLiteralOnly = true)
    return Toast.makeText(context, localized, duration)
}

fun localizedToast(context: Context, messageResource: Int, duration: Int): Toast =
    Toast.makeText(context, messageResource, duration)

@Composable
fun localizedUiText(text: String, sourceLiteralOnly: Boolean = false): String {
    val language = LocalConfiguration.current.locales[0]
        ?.language?.lowercase(Locale.ROOT)
        ?: Locale.getDefault().language.lowercase(Locale.ROOT)
    return UiTextCatalog.translate(LocalContext.current, language, text, sourceLiteralOnly)
}

@Composable
fun Text(
    text: String,
    modifier: Modifier = Modifier,
    color: Color = Color.Unspecified,
    fontSize: TextUnit = TextUnit.Unspecified,
    fontStyle: FontStyle? = null,
    fontWeight: FontWeight? = null,
    fontFamily: FontFamily? = null,
    letterSpacing: TextUnit = TextUnit.Unspecified,
    textDecoration: TextDecoration? = null,
    textAlign: TextAlign? = null,
    lineHeight: TextUnit = TextUnit.Unspecified,
    overflow: TextOverflow = TextOverflow.Clip,
    softWrap: Boolean = true,
    maxLines: Int = Int.MAX_VALUE,
    minLines: Int = 1,
    onTextLayout: (TextLayoutResult) -> Unit = {},
    style: TextStyle = LocalTextStyle.current
) {
    MaterialText(
        text = localizedUiText(text, sourceLiteralOnly = true), modifier = modifier, color = color, fontSize = fontSize,
        fontStyle = fontStyle, fontWeight = fontWeight, fontFamily = fontFamily,
        letterSpacing = letterSpacing, textDecoration = textDecoration, textAlign = textAlign,
        lineHeight = lineHeight, overflow = overflow, softWrap = softWrap, maxLines = maxLines,
        minLines = minLines, onTextLayout = onTextLayout, style = style
    )
}

@Composable
fun Text(
    text: AnnotatedString,
    modifier: Modifier = Modifier,
    color: Color = Color.Unspecified,
    fontSize: TextUnit = TextUnit.Unspecified,
    fontStyle: FontStyle? = null,
    fontWeight: FontWeight? = null,
    fontFamily: FontFamily? = null,
    letterSpacing: TextUnit = TextUnit.Unspecified,
    textDecoration: TextDecoration? = null,
    textAlign: TextAlign? = null,
    lineHeight: TextUnit = TextUnit.Unspecified,
    overflow: TextOverflow = TextOverflow.Clip,
    softWrap: Boolean = true,
    maxLines: Int = Int.MAX_VALUE,
    minLines: Int = 1,
    inlineContent: Map<String, InlineTextContent> = mapOf(),
    onTextLayout: (TextLayoutResult) -> Unit = {},
    style: TextStyle = LocalTextStyle.current
) {
    MaterialText(
        text = text, modifier = modifier, color = color, fontSize = fontSize,
        fontStyle = fontStyle, fontWeight = fontWeight, fontFamily = fontFamily,
        letterSpacing = letterSpacing, textDecoration = textDecoration, textAlign = textAlign,
        lineHeight = lineHeight, overflow = overflow, softWrap = softWrap, maxLines = maxLines,
        minLines = minLines, inlineContent = inlineContent, onTextLayout = onTextLayout,
        style = style
    )
}
"####;

pub const RESOURCES: &[(&str, &str)] = &[
    ("res/raw/ui_en.json", include_str!("ui_locales/ui_en.json")),
    ("res/raw/ui_hi.json", include_str!("ui_locales/ui_hi.json")),
    ("res/raw/ui_es.json", include_str!("ui_locales/ui_es.json")),
    ("res/raw/ui_ar.json", include_str!("ui_locales/ui_ar.json")),
    ("res/raw/ui_fr.json", include_str!("ui_locales/ui_fr.json")),
    ("res/raw/ui_bn.json", include_str!("ui_locales/ui_bn.json")),
    ("res/raw/ui_pt.json", include_str!("ui_locales/ui_pt.json")),
    ("res/raw/ui_id.json", include_str!("ui_locales/ui_id.json")),
    ("res/raw/ui_ur.json", include_str!("ui_locales/ui_ur.json")),
    ("res/raw/ui_ja.json", include_str!("ui_locales/ui_ja.json")),
];

// Additions for literal UI text seen in English-language Android simulator audits.
// Preserve reviewed translations in the bundled catalog; only fill missing keys.
const EN_UI_GAP_TRANSLATIONS: &[(&str, &str)] = &[
    ("搜索", "Search"),
    ("目录", "Directory"),
    ("文档库", "Document library"),
    ("页面与文件夹", "Pages and folders"),
    ("AI 对话", "AI chat"),
    ("连续追问，保留本轮问答", "Ask follow-up questions in this conversation"),
    ("开始对话", "Start chat"),
    ("结构页", "Structured pages"),
    ("在手机上编辑待办、折叠内容、表格与属性。", "Edit to-dos, collapsible content, tables and properties on your phone."),
    ("排序", "Sort"),
    ("全部文档", "All documents"),
    ("还没有文档", "No documents yet"),
    ("点击新建，添加第一篇文档。", "Select New to create your first document."),
    ("快捷记入今天", "Quick entry for today"),
    ("风险分析", "Risk analysis"),
    ("记支出", "Record expense"),
    ("记收入", "Record income"),
    ("日账", "Daily ledger"),
    ("月快照", "Monthly snapshot"),
    ("当日收入", "Income today"),
    ("当日支出", "Expenses today"),
    ("今日备注", "Today's notes"),
    ("新增收入", "Add income"),
    ("新增支出", "Add expense"),
    ("查看当日统计与预算", "View today's totals and budget"),
    ("点新增收入，或直接套用上面的模板。这里只保留你真正填写过的记录。", "Tap Add income or use a template. Only records you enter are saved."),
    ("没有支出就让它空着。要补账时，新增一行或套用常用支出模板。", "Leave this blank if you spent nothing. Add a row or use a template to catch up."),
    ("收支与快照", "Transactions and snapshots"),
    ("查看日期", "Choose date"),
    ("季度", "Quarterly"),
    ("年度", "Yearly"),
    ("日期", "Date"),
    ("待填写 API Key", "API key required"),
    ("问 AI", "Ask AI"),
    ("直接提问，或依据知识页回答", "Ask directly or use knowledge pages as context"),
    ("先配置", "Configure first"),
    ("法律风险线索", "Legal risk indicators"),
    ("先查看扫描范围和发送预览，再由你确认发送", "Review scan scope and message preview before sending"),
    ("登录 / 充值 / 创建密钥", "Sign in / Add credit / Create key"),
    ("保存并检测连接", "Save and test connection"),
    ("正在保存配置", "Saving configuration"),
    ("正在检测连接", "Testing connection"),
    ("接口信息待补全", "Complete endpoint details"),
    ("配置有修改，尚未保存", "Unsaved configuration changes"),
    ("已连接 · 文字和图片可用", "Connected · Text and images available"),
    ("已连接 · 部分能力可用", "Connected · Limited capabilities"),
    ("配置已保存 · 连接未通过", "Saved · Connection test failed"),
    ("配置已保存 · 可以进入 AI 功能", "Saved · AI is available"),
    ("接收：api.deepseek.com · 模型：deepseek-flash", "Endpoint: api.deepseek.com · Model: deepseek-flash"),
    ("修改接口与模型", "Change endpoint and model"),
    ("正在运行", "Running"),
    ("显示", "Showing"),
    ("页", "pages"),
    ("搜索文档回收站", "Search document trash"),
    ("标题、正文、文档库都能搜", "Search titles, content, and libraries"),
    ("今日", "Today"),
    ("进行中", "Running"),
];

pub fn render_resource(path: &str, contents: &str) -> Result<String, String> {
    if path != "res/raw/ui_en.json" { return Ok(contents.to_owned()); }
    let mut dictionary: serde_json::Map<String, serde_json::Value> =
        serde_json::from_str(contents).map_err(|error| error.to_string())?;
    for (source, english) in EN_UI_GAP_TRANSLATIONS {
        dictionary.entry((*source).to_string())
            .or_insert_with(|| serde_json::Value::String((*english).to_string()));
    }
    serde_json::to_string(&dictionary).map_err(|error| error.to_string())
}

pub fn render(path: &str, source: &str) -> String {
    let should_localize = matches!(
        path,
        "com/ofairyo/gridtimer/ui/GridTimerScreen.kt"
            | "com/ofairyo/gridtimer/ui/FinanceLedgerSection.kt"
            | "com/ofairyo/gridtimer/ui/NoteDocumentEditor.kt"
            | "com/ofairyo/gridtimer/ui/NoteMediaUi.kt"
            | "com/ofairyo/gridtimer/ui/NoteStudioSheet.kt"
            | "com/ofairyo/gridtimer/ui/SmartisanNoteUi.kt"
            | "com/ofairyo/gridtimer/ui/DiagnosticsExportUi.kt"
            | "com/ofairyo/gridtimer/ui/KnowledgeCanvasScreen.kt"
            | "com/ofairyo/gridtimer/ui/StructuredKnowledgeReader.kt"
            | "com/ofairyo/gridtimer/ui/MyAccountScreen.kt"
    );
    if !should_localize {
        return source.to_owned();
    }
    let text_import = "import androidx.compose.material3.Text\n";
    let localized_import = "import com.ofairyo.gridtimer.i18n.Text\n";
    let localized = if source.contains(text_import) {
        source.replacen(text_import, localized_import, 1)
    } else {
        source.replacen(
            "package com.ofairyo.gridtimer.ui\n",
            "package com.ofairyo.gridtimer.ui\n\nimport com.ofairyo.gridtimer.i18n.Text\n",
            1,
        )
    };
    let localized = if localized.contains("Toast.makeText(") {
        localized
            .replacen(
                localized_import,
                "import com.ofairyo.gridtimer.i18n.Text\nimport com.ofairyo.gridtimer.i18n.localizedToast\n",
                1,
            )
            .replace("Toast.makeText(", "localizedToast(")
    } else {
        localized
    };
    if path == "com/ofairyo/gridtimer/ui/KnowledgeCanvasScreen.kt" {
        return localized
            .replacen(
                localized_import,
                "import com.ofairyo.gridtimer.i18n.Text\nimport com.ofairyo.gridtimer.i18n.localizedUiText\n",
                1,
            )
            .replace(
                "\"全部画布（${boards.size}）\"",
                "\"${localizedUiText(\"全部画布\")} (${boards.size})\"",
            )
            .replace(
                "\"卡片 ${canvas?.nodes?.size ?: 0}\"",
                "\"${localizedUiText(\"卡片\")} ${canvas?.nodes?.size ?: 0}\"",
            );
    }
    if path == "com/ofairyo/gridtimer/ui/StructuredKnowledgeReader.kt" {
        return localized
            .replacen(
                localized_import,
                "import com.ofairyo.gridtimer.i18n.Text\nimport com.ofairyo.gridtimer.i18n.localizedUiText\n",
                1,
            )
            .replace(
                "\"${file.sizeBytes} 字节\"",
                "\"${file.sizeBytes} ${localizedUiText(\"字节\")}\"",
            )
            .replace(
                "\"已解决 · \"",
                "\"${localizedUiText(\"已解决\")} · \"",
            );
    }
    if path == "com/ofairyo/gridtimer/ui/SmartisanNoteUi.kt" {
        return localized
            .replacen(
                localized_import,
                "import com.ofairyo.gridtimer.i18n.Text\nimport com.ofairyo.gridtimer.i18n.localizedUiText\n",
                1,
            )
            .replace(
                "\"创建于 ${safeFormat(dateTimeFormatter, version?.createdAtEpochMillis ?: 0L, \"--\")}\"",
                "\"${localizedUiText(\"创建于\")} ${safeFormat(dateTimeFormatter, version?.createdAtEpochMillis ?: 0L, \"--\")}\"",
            );
    }
    if path == "com/ofairyo/gridtimer/ui/GridTimerScreen.kt" {
        let localized = localized
            .replacen(
                localized_import,
                "import com.ofairyo.gridtimer.i18n.Text\nimport com.ofairyo.gridtimer.i18n.localizedUiText\n",
                1,
            )
            .replace(
                "text = \"格子 ${displayIndex.toString().padStart(2, '0')}\"",
                "text = \"${localizedUiText(\"格子\")} ${displayIndex.toString().padStart(2, '0')}\"",
            )
            .replace(
                "text = \"格子 ${slot.id.toString().padStart(2, '0')} 快捷操作\"",
                "text = \"${localizedUiText(\"格子\")} ${slot.id.toString().padStart(2, '0')} ${localizedUiText(\"快捷操作\")}\"",
            );
        let start = localized.find("private fun DockNavigationButton(");
        let end = localized.find("private fun HeaderBlock(");
        if let (Some(start), Some(end)) = (start, end) {
            let mut button = localized[start..end].to_owned();
            button = button.replacen(".size(28.dp)", ".size(24.dp)", 1);
            button = button.replacen("size = 25.dp", "size = 23.dp", 1);
            button = button.replacen(
                "Spacer(modifier = Modifier.height(2.dp))",
                "Spacer(modifier = Modifier.height(1.dp))",
                1,
            );
            button = button.replacen(
                "maxLines = 1,\n                softWrap = false,",
                "maxLines = 2,\n                softWrap = true,",
                1,
            );
            button = button.replacen(
                "style = MaterialTheme.typography.labelMedium.copy(\n                    color = contentColor,\n                    fontWeight = if (selected) FontWeight.Bold else FontWeight.SemiBold,\n                    lineHeight = 14.sp",
                "style = MaterialTheme.typography.labelSmall.copy(\n                    color = contentColor,\n                    fontWeight = if (selected) FontWeight.Bold else FontWeight.SemiBold,\n                    fontSize = when {\n                        label.length > 10 -> 10.sp\n                        label.length > 6 -> 11.sp\n                        else -> 12.sp\n                    },\n                    lineHeight = 12.sp",
                1,
            );
            return format!("{}{}{}", &localized[..start], button, &localized[end..]);
        }
        return localized;
    }
    localized
}

#[cfg(test)]
mod english_ui_regression_tests {
    use super::{render, render_resource, RESOURCES};

    #[test]
    fn sourcegen_localizes_account_text_without_global_user_content_replacement() {
        let source = crate::my_account_source::CONTENTS;
        let generated = render(crate::my_account_source::PATH, source);
        assert!(generated.contains("import com.ofairyo.gridtimer.i18n.Text"));
        assert!(generated.contains("import com.ofairyo.gridtimer.i18n.localizedUiText"));
        assert!(!generated.contains("import androidx.compose.material3.Text\n"));
        assert!(generated.contains("syncSession.email"));
    }

    #[test]
    fn english_simulator_ui_labels_have_catalog_entries() {
        let english = RESOURCES.iter().find(|(path, _)| *path == "res/raw/ui_en.json").unwrap().1;
        let merged = render_resource("res/raw/ui_en.json", english).unwrap();
        let catalog: serde_json::Map<String, serde_json::Value> = serde_json::from_str(&merged).unwrap();
        for key in [
            "搜索", "AI 对话", "快捷记入今天", "文档库",
            "待填写 API Key", "法律风险线索", "风险分析",
        ] {
            assert!(catalog.get(key).and_then(|value| value.as_str()).is_some_and(|text| !text.is_empty()), "missing English catalog key: {key}");
        }
        let original: serde_json::Map<String, serde_json::Value> = serde_json::from_str(english).unwrap();
        for (key, value) in original {
            assert_eq!(catalog.get(&key), Some(&value), "previously reviewed English translation changed: {key}");
        }
    }
}
