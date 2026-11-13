#!/usr/bin/env python3
"""Independent Android gallery QA: real visible content, not PNG header/clock.
Only reads evidence; never changes the app or fabricates screenshots.
"""
import csv
import hashlib
import json
import re
import sys
import xml.etree.ElementTree as ET
from collections import Counter, defaultdict
from pathlib import Path

from PIL import Image, ImageStat

MODULES = ("计时", "便签", "知识", "风控", "画布", "工作台")
TIMER_ACTIONS = ("暂停计时", "继续计时", "格子详情", "开始计时")


def dhash(image):
    pixels = list(image.convert("L").resize((33, 48)).getdata())
    bits = 0
    for y in range(48):
        for x in range(32):
            k = y * 33 + x
            bits = (bits << 1) | (pixels[k] > pixels[k + 1])
    return bits


def normalize_dynamic_label(label, timer_screen):
    # Numeric changes in a running timer do not demonstrate another feature.
    label = re.sub(r"(?<![0-9A-Za-z])\d{1,2}:\d{2}(?::\d{2})?(?!\d)", "<时间>", label)
    label = re.sub(r"(?<![0-9A-Za-z])\d+(?:\.\d+)?\s*(?:小时|分钟|分|秒)(?![0-9A-Za-z])", "<时长>", label)
    if timer_screen:
        label = re.sub(r"记录\s*\d+(?!\d)", "记录 <次数>", label)
        label = re.sub(r"(?<!\d)\d+\s*次记录", "<次数>次记录", label)
        label = re.sub(r"(?<!\d)\d+\s*个格子正在计时", "<数量>个格子正在计时", label)
        label = re.sub(r"(?<!\d)\d+(?:\.\d+)?%", "<百分比>", label)
    label = re.sub(r"^\d+\s*张$", "<列表数量>张", label)
    return label


def semantic_text(xml_file):
    root = ET.parse(xml_file).getroot()
    first = next(root.iter("node"), None)
    owner = first.attrib.get("package", "") if first is not None else ""
    if owner != "com.ofairyo.gridtimer":
        raise ValueError("前台并非 TenRate 应用：" + (owner or "未知") +
                         "；系统权限弹窗和 DocumentsUI 不属于功能截图")
    raw = ET.tostring(root, encoding="unicode")
    timer_screen = ("格子详情" in raw or ("建议动作" in raw and "任务" in raw))
    labels = []
    for node in root.iter("node"):
        a = node.attrib
        if a.get("package") and a["package"] != "com.ofairyo.gridtimer":
            continue
        label = (a.get("text") or a.get("content-desc") or "").strip()
        if not label or label in MODULES or len(label) > 120:
            continue
        label = normalize_dynamic_label(label, timer_screen)
        if label not in labels:
            labels.append(label)
    return hashlib.sha256("\u241e".join(labels).encode()).hexdigest()[:20]

def audit(root):
    index = root / "操作说明与截图索引.csv"
    if not index.is_file():
        raise ValueError("缺少操作说明与截图索引.csv")
    with index.open(encoding="utf-8-sig", newline="") as f:
        records = list(csv.DictReader(f))
    seen_paths, seen_pixels = set(), {}
    signatures, issues = [], []
    semantic_keys = set()  # XML candidate states, not independently proven business functions.
    module_semantics = defaultdict(set)
    counts = Counter()
    for record in records:
        module = record["功能模块"]
        rel = record["截图相对路径"]
        action = record["操作说明"]
        if module not in MODULES or not re.search(r"[\u4e00-\u9fff]", action):
            issues.append("模块或中文动作异常：" + rel)
            continue
        if rel in seen_paths:
            issues.append("重复索引：" + rel)
            continue
        seen_paths.add(rel)
        path = root / rel
        xml = root / "界面结构" / module / (path.stem + ".xml")
        if not path.is_file() or not xml.is_file():
            issues.append("截图或 UI XML 缺失：" + rel)
            continue
        if path.parent.name != module:
            issues.append("模块目录不一致：" + rel)
        if module != "计时" and any(w in action for w in TIMER_ACTIONS):
            issues.append("疑似跨模块误标：" + rel)
        # The timer detail can overlay the Knowledge canvas with a perfectly
        # valid TenRate package. Check real XML content, not only the action.
        try:
            raw = xml.read_text(encoding="utf-8")
            if module != "计时" and "格子详情" in raw:
                issues.append("实际计时详情误标为" + module + "：" + rel)
            if module == "画布" and not any(w in raw for w in ("画布", "客户", "Client", "卡片", "推荐")):
                issues.append("画布截图缺少画布或客户内容证据：" + rel)
        except Exception as ex:
            issues.append("XML 无法读取：" + rel + "：" + str(ex))
        try:
            semantic = semantic_text(xml)
            with Image.open(path) as im:
                im.verify()
            with Image.open(path) as im:
                frame = im.convert("RGB")
                w, h = frame.size
                if w < 300 or h < 500:
                    raise ValueError("尺寸异常")
                # Ignore status bar, navigation bar and their changing clocks.
                app = frame.crop((0, int(h * .075), w, int(h * .925)))
                center = app.crop((w // 8, h // 8, 7 * w // 8, int(h * .75)))
                stats = ImageStat.Stat(center)
                if max(stats.mean) < 8 and max(stats.stddev) < 4:
                    issues.append("App 内容区全黑：" + rel)
                digest = hashlib.sha256(app.tobytes()).hexdigest()
                if digest in seen_pixels:
                    issues.append("App 内容相同，状态栏差异无效：" + seen_pixels[digest] + " / " + rel)
                else:
                    seen_pixels[digest] = rel
                signatures.append((rel, module, semantic, dhash(app)))
                semantic_keys.add((module, semantic))
                module_semantics[module].add(semantic)
                counts[module] += 1
        except Exception as ex:
            issues.append("图片/XML 解码失败：" + rel + "：" + str(ex))
    actual = {str(p.relative_to(root)) for p in (root / "截图").rglob("*.png")}
    if actual != seen_paths:
        issues.append(f"索引与实际图片不符：未索引 {len(actual-seen_paths)}，无文件 {len(seen_paths-actual)}")
    near = []
    for i, (path, module, semantic, fingerprint) in enumerate(signatures):
        for other, other_module, other_semantic, other_fingerprint in signatures[:i]:
            if module != other_module:
                continue
            distance = (fingerprint ^ other_fingerprint).bit_count()
            if distance <= 5:
                near.append({"第一张": other, "第二张": path,
                             "视觉距离": distance, "正文签名相同": semantic == other_semantic})
    return {"索引总数": len(records), "App内容唯一数": len(seen_pixels),
            "XML语义候选唯一数": len(semantic_keys),
            "各模块XML语义候选唯一数": {k: len(v) for k, v in module_semantics.items()},
            "各模块张数": dict(counts), "阻断问题": issues,
            "高度相似待人工复核": near, "高度相似对数": len(near)}


def main():
    root = Path(sys.argv[1] if len(sys.argv) > 1 else "feature_tour_evidence")
    result = audit(root)
    output = root / "视觉质量审计.json"
    output.write_text(json.dumps(result, ensure_ascii=False, indent=2), encoding="utf-8")
    print(f"App 内容唯一 {result['App内容唯一数']}/{result['索引总数']}；"
          f"阻断问题 {len(result['阻断问题'])}；近似待审 {result['高度相似对数']}")
    for issue in result["阻断问题"][:50]:
        print("FAIL:", issue)
    if result["阻断问题"]:
        raise SystemExit(1)


if __name__ == "__main__":
    main()
