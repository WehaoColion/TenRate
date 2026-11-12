#!/usr/bin/env python3
# API35 unattended feature-tour evidence. All PNGs come from adb screencap.
import csv
import hashlib
from functools import lru_cache
import json
import os
import re
import shutil
import subprocess
import time
import unicodedata
import xml.etree.ElementTree as ET
from pathlib import Path

PACKAGE = "com.ofairyo.gridtimer"
ACTIVITY = "com.ofairyo.gridtimer/.MainActivity"
ROOT = Path("feature_tour_evidence")
IMAGES = ROOT / "截图"
HIERARCHY = ROOT / "界面结构"
MODULES = ["计时", "便签", "知识", "风控"]
RISKY = ("删除", "清空", "注销", "退出登录", "重置", "永久", "付款", "支付", "充值", "结算",
         "发送", "上传", "同步", "共享", "分享", "发布", "恢复出厂", "清除数据", "移除账号",
         "移除所有", "解绑", "导出", "确认删除", "覆盖原文", "向", "购买",
         "Reset", "Remove", "Delete", "Clear data", "Sign out", "Send", "Sync", "Upload", "Purchase")
BORING = ("电池", "状态栏", "通知", "系统导航", "Accessibility", "Search by voice", "Pixel Launcher")
rows = []
errors = []
num = 0
seen = set()
seen_png = set()
module_counts = {}
MAX_PER_MODULE = 180
MAX_TOTAL = 850
MAX_DEPTH = 5
DUMP = "/sdcard/feature_tour.xml"

def adb(*args, timeout=22, binary=False):
    p = subprocess.run(["adb", *map(str,args)], stdout=subprocess.PIPE,
                       stderr=subprocess.PIPE, timeout=timeout)
    if p.returncode != 0:
        raise RuntimeError("adb " + " ".join(map(str,args)) + ": " + p.stderr.decode("utf-8","replace")[:180])
    return p.stdout if binary else p.stdout.decode("utf-8","replace")

def safe_adb(*args, **kw):
    try: return adb(*args, **kw)
    except Exception as ex:
        errors.append(str(ex))
        return b"" if kw.get("binary") else ""

def get_xml():
    for attempt in range(3):
        try:
            adb("shell", "uiautomator", "dump", DUMP, timeout=24)
            raw = adb("exec-out", "cat", DUMP, timeout=15)
            if "<hierarchy" in raw: return raw
        except Exception as ex:
            errors.append(str(ex))
        time.sleep(0.35)
    return ""

def nodes(xml):
    if not xml: return []
    try:
        root=ET.fromstring(xml)
        output=[]
        for e in root.iter("node"):
            a=e.attrib
            bs=re.findall(r"\d+", a.get("bounds",""))
            if len(bs)!=4: continue
            x1,y1,x2,y2=map(int,bs)
            text=a.get("text","").strip()
            desc=a.get("content-desc","").strip()
            label=text or desc
            if not label or x2<=x1 or y2<=y1: continue
            output.append(dict(text=text,desc=desc,label=label,x=(x1+x2)//2,y=(y1+y2)//2,
                               bounds=(x1,y1,x2,y2),click=a.get("clickable","false")=="true",
                               scroll=a.get("scrollable","false")=="true",class_=a.get("class","")))
        return output
    except Exception as ex:
        errors.append("UI XML parse: " + str(ex))
        return []

def visible(xml):
    values=[]
    for n in nodes(xml):
        v=n["label"].replace("\n"," ").strip()
        if v and len(v)<150 and v not in values:
            values.append(v)
    return values

def identity(xml):
    labels=visible(xml)
    return hashlib.sha256("\u241e".join(labels[:110]).encode("utf-8")).hexdigest()[:18]

def clean(s):
    s=re.sub(r"[^\w\u4e00-\u9fff（）()、，.％%+-]+","_",str(s).strip())
    return s[:41].strip("_") or "无标题"

def image_bytes():
    try:
        data=adb("exec-out","screencap","-p",timeout=20,binary=True)
        if data.startswith(b"\x89PNG"): return data
    except Exception as ex:
        errors.append(str(ex))
    return b""

def capture(module, action, xml=None, detail="", force=False):
    global num
    if num>=MAX_TOTAL: return False
    if not force and module_counts.get(module,0)>=MAX_PER_MODULE: return False
    xml=xml if xml is not None else get_xml()
    if not xml: return False
    key=body_signature(xml)
    if not force and key in seen: return False
    data=image_bytes()
    if not data: return False
    png_hash=hashlib.sha256(data).hexdigest()
    if png_hash in seen_png:
        errors.append("像素完全重复，跳过："+module+" / "+action)
        return False
    seen_png.add(png_hash)
    num+=1
    module_counts[module]=module_counts.get(module,0)+1
    seen.add(key)
    dirname=IMAGES / clean(module)
    dirname.mkdir(parents=True,exist_ok=True)
    stem=f"{num:04d}_{clean(module)}_{clean(action)}"
    (dirname/(stem+".png")).write_bytes(data)
    xdir=HIERARCHY/clean(module)
    xdir.mkdir(parents=True,exist_ok=True)
    (xdir/(stem+".xml")).write_text(xml,encoding="utf-8")
    labels=visible(xml)
    rows.append([num,module,action,str(Path("截图")/clean(module)/(stem+".png")),
                 detail or "Android 35 实际界面截图", "；".join(labels[:12])[:360]])
    print("CAPTURE",num,module,action,flush=True)
    return True

def tap_node(n):
    safe_adb("shell","input","tap",n["x"],n["y"],timeout=15)
    time.sleep(0.45)

def tap_text(labels, exact=True):
    xml=get_xml()
    candidates=nodes(xml)
    for target in labels:
        for n in candidates:
            label=n["text"] or n["desc"]
            if ((label==target) if exact else (target in label)) and len(label)<=180:
                tap_node(n)
                return True
    return False

def click(labels):
    return tap_text(labels,True) or tap_text(labels,False)

def back():
    safe_adb("shell","input","keyevent","4")
    time.sleep(0.45)

def app_alive():
    return bool(safe_adb("shell","pidof",PACKAGE).strip())

def foreground_ok():
    raw=safe_adb("shell","dumpsys","window","windows",timeout=15)
    if "mCurrentFocus=null" in raw: return True
    match=re.search(r"mCurrentFocus=.*?(com\.[\w.]+|com\.ofairyo\.gridtimer)",raw)
    return match is None or PACKAGE in match.group(0) or "com.google.android.inputmethod" in match.group(0)

def open_app():
    safe_adb("shell","am","start","-W","-n",ACTIVITY,timeout=40)
    time.sleep(2.2)
    for _ in range(4):
        x=get_xml()
        if any(p in x for p in ("Allow tenfold", "Allow notifications", "发送通知")):
            if not click(["Allow","允许","允许通知"]): break
        elif any(p in x for p in ("Pixel Launcher isn't responding","Pixel Launcher isn")):
            if not click(["Close app","关闭应用"]): break
        else: break

@lru_cache(maxsize=1)
def screen_height():
    result = safe_adb("shell","wm","size")
    found = re.search(r"(\d+)x(\d+)",result)
    return int(found.group(2)) if found else 2400

def bottom_nav(xml):
    height=screen_height()
    candidates=nodes(xml)
    return {name: next((n for n in candidates if n["label"]==name and n["y"]>height*.82),None)
            for name in MODULES}

def require_zh(xml):
    tabs=bottom_nav(xml)
    missing=[name for name in ("计时","便签","知识","风控") if tabs.get(name) is None]
    if missing:
        raise RuntimeError("简体中文界面验证失败，缺少底部导航："+"、".join(missing)+
                           "；实见："+ "；".join(visible(xml)[:35]))
    if any(x in xml for x in ("Quick notes", "Risk Control", "Start anew", "Suggested action")):
        raise RuntimeError("检测到英文界面，禁止采集并归类为中文截图")

def body_signature(xml):
    height=screen_height()
    main=[n["label"] for n in nodes(xml)
          if n["y"]<height*.84 and n["label"] not in MODULES
          and not re.fullmatch(r"v?\d+(?:\.\d+){2,}", n["label"])
          and "时钟" not in n["desc"]]
    stable=[]
    for label in main:
        value=re.sub(r"\b\d{1,2}:\d{2}(?::\d{2})?\b","<时间>",label)
        value=re.sub(r"\b\d+\s*(?:分|秒|小时|m|h)\b","<时长>",value)
        value=re.sub(r"\d+(?:\.\d+)?%","<百分比>",value)
        if value not in stable: stable.append(value)
    return hashlib.sha256("\u241e".join(stable[:100]).encode("utf-8")).hexdigest()

def root_module(xml):
    # Distinguish top-level screens from their actual Chinese body labels, never from a filename.
    labels=set(visible(xml))
    if "快捷记入今天" in labels and ("记支出" in labels or "记收入" in labels):
        return "风控"
    if "最近便签" in labels and "回收站" in labels:
        return "便签"
    if "AI 对话" in labels and ("目录" in labels or "画布" in labels):
        return "知识"
    if "建议动作" in labels and ("开始提示" in labels or "展开概览" in labels):
        return "计时"
    return None

def at_module(module):
    if module not in MODULES:
        raise RuntimeError("未知模块："+module)
    if not app_alive() or not foreground_ok():
        open_app()
    for attempt in range(12):
        xml=get_xml()
        if not xml:
            open_app()
            continue
        if root_module(xml)==module and all(bottom_nav(xml).get(x) for x in MODULES):
            return xml
        tabs=bottom_nav(xml)
        if tabs.get(module):
            tap_node(tabs[module])
            time.sleep(0.7)
            after=get_xml()
            if root_module(after)==module:
                return after
            # Compose may preserve scroll position after reselecting a tab.
            for _ in range(2):
                scroll_screen("up")
                after=get_xml()
                if root_module(after)==module:
                    return after
        if attempt < 4:
            back()
        elif attempt in (4,8):
            # A blank editor, keyboard or modal may leave the root navigation inaccessible.
            safe_adb("shell","am","force-stop",PACKAGE)
            open_app()
        else:
            scroll_screen("up")
    raise RuntimeError("无法返回中文模块首页："+module+
                       "；当前可见："+ "；".join(visible(get_xml())[:25]))

def scroll_screen(direction="down"):
    # Pixel 6 emulator uses approx. 1080x2400 coordinates; read real size.
    wh=safe_adb("shell","wm","size")
    m=re.search(r"(\d+)x(\d+)",wh)
    w,h=map(int,m.groups()) if m else (1080,2400)
    x=w//2
    start=int(h*(0.79 if direction=="down" else 0.35))
    end=int(h*(0.32 if direction=="down" else 0.79))
    safe_adb("shell","input","swipe",x,start,x,end,370,timeout=20)
    time.sleep(0.4)

SAFE_ACTION_WORDS = (
    "新建","新增","添加","更多","设置","查看","搜索","编辑","分类","任务","历史","筛选","排序",
    "统计","画布","目录","页面","文件夹","风险","预算","回收站","模板","属性","时间",
    "账单","提醒","目标","标签","帮助","选择","管理","详情","今日","本周","本月",
    "列表","清单","汇总","趋势","总览","最近","记录","收入","支出","全部","结构",
    "新开始","开始","暂停","继续","结束","停止","完成","取消","创建","闪念","会议","复盘",
    "视图","日账","月快照","季度","年度","收支","上一周期","下一周期","回到当前周期",
    "适应","卡片","客户","纸张","内容","版式","日期","计时","时钟","置顶","粗体",
    "最新","版本","确认","草稿","输入","备注","金额","类型","来源","添加第一张",
    "展开","折叠","待办","插入","移动","链接","分组","归档","颜色","格式","图表",
    "文本","文档","快捷操作","项目","通知","收藏","复制","预览","账户","个人"
)

def eligible(n):
    s=n["label"].strip()
    if len(s)>48 or len(s)<2: return False
    if any(w in s for w in RISKY+BORING): return False
    if s.startswith("http") or "@" in s: return False
    if s in MODULES or re.fullmatch(r"v?\d+(?:\.\d+){2,}",s):return False
    if not any(hint in s for hint in SAFE_ACTION_WORDS): return False
    if n["y"]<100 or n["y"]>screen_height()*.91: return False
    if n["class_"] not in ("android.widget.TextView","android.view.View") and not n["class_"].endswith(("Button","ImageButton")): return False
    return True

def action_candidates(xml,module):
    found=[]; seen_labels=set()
    for n in nodes(xml):
        if not eligible(n): continue
        label=n["label"]
        if label in seen_labels:continue
        seen_labels.add(label);found.append(n)
    priority=("更多","新建","新增","添加","搜索","设置","筛选","分类","目录","页面",
              "统计","画布","列表","查看","详情","编辑","历史","预算")
    def score(n):
        label=n["label"]
        return (0 if any(w in label for w in priority) else 1, n["y"],n["x"])
    return sorted(found,key=score)[:24]

def explore(module, depth, trace, budget=0):
    if depth>MAX_DEPTH or num>=MAX_TOTAL or module_counts.get(module,0)>=MAX_PER_MODULE:
        return
    screen=get_xml()
    if not screen: return
    start_identity=body_signature(screen)
    sig=(module,start_identity,depth)
    if sig in visited: return
    visited.add(sig)
    candidates=action_candidates(screen,module)

    if depth<3:
        scroll_screen("down")
        scrolled=get_xml()
        capture(module,"下滑查看"+(trace[-1] if trace else "列表"),scrolled,"滚动后真实视图")
        if body_signature(scrolled)!=start_identity:
            scroll_screen("up")
            screen=get_xml()
        else:
            scroll_screen("up")

    for n in candidates:
        if num>=MAX_TOTAL or module_counts.get(module,0)>=MAX_PER_MODULE: break
        if not foreground_ok():
            safe_adb("shell","am","force-stop",PACKAGE)
            open_app()
            break
        current=get_xml()
        if not current: break
        if depth==0 and module in MODULES and root_module(current)!=module:
            current=at_module(module)
        elif depth==0 and module=="画布" and "知识画布，" not in current:
            current=at_canvas()
        current_nodes=nodes(current)
        found=next((x for x in current_nodes if x["label"]==n["label"] and
                    abs(x["y"]-n["y"])<170),None)
        if not found:continue
        prior=body_signature(current)
        tap_node(found)
        after=get_xml()
        if not after: continue
        changed=body_signature(after)!=prior
        if not changed:
            # Important: do not press Back when a tap had no effect.
            # Doing so exited the entire application in the prior run.
            continue
        label=n["label"].replace("\n"," ")[:32]
        capture(module,"→".join(trace[-2:]+[label]),after,
                "实际点击："+n["label"][:100])
        if depth<MAX_DEPTH and (module,body_signature(after),depth+1) not in visited:
            explore(module,depth+1,trace+[label])
        # Back is valid only after a proven navigation to a distinct page.
        restored=False
        for _ in range(3):
            now=get_xml()
            if now and body_signature(now)==prior:
                restored=True
                break
            back()
        if not restored:
            if depth==0:
                try:
                    if module=="画布":
                        restored=("知识画布，" in at_canvas())
                    else:
                        restored=(root_module(at_module(module))==module)
                except Exception as ex:
                    errors.append("页面返回失败："+module+" / "+label+" / "+str(ex)[:90])
            if not restored:
                # Do not tag unknown screens as this module.
                break

def at_canvas():
    # Canvas is a nested route under Knowledge, not a fifth bottom-navigation module.
    current=get_xml()
    if "知识画布，" in current and "＋ 添加" in current:
        return current
    at_module("知识")
    if not click(["画布","卡片与连线"]):
        raise RuntimeError("知识模块没有画布入口")
    current=get_xml()
    if "知识画布，" not in current:
        if not click(["知识画布","打开画布"]):
            raise RuntimeError("画布列表中没有知识画布")
        current=get_xml()
    if "知识画布，" not in current or "＋ 添加" not in current:
        raise RuntimeError("未进入实际画布编辑正文")
    return current

def seed_editor(module):
    # Only insert disposable, local demo data; never invoke AI, sync or payment.
    if module=="便签":
        words=["＋ 新建便签","新建便签","新增便签","新建","＋ 添加","添加便签"]
    elif module=="知识":
        words=["＋ 新建知识页面","新建页面","创建页面","新建","＋ 添加"]
    elif module=="计时":
        words=["＋ 新增任务","新建任务","添加任务","新建计时","＋ 添加"]
    else:
        words=["记支出","记收入"]
    if click(words):
        capture(module,"创建或录入入口",get_xml(), "演示入口及空白表单")
        if module in ("便签","知识","计时"):
            xml=get_xml()
            edits=[n for n in nodes(xml) if n["class_"].endswith("EditText")]
            if edits:
                tap_node(edits[0])
                safe_adb("shell","input","text","GalleryDemo")
                capture(module,"输入演示内容",get_xml(),"仅使用虚构演示内容 GalleryDemo")
                # Only save explicit local creation, never send external data.
                if click(["创建","保存","完成","确定","Create","Save"]):
                    capture(module,"本地创建完成",get_xml(),"本地演示数据")
        back()

def main():
    global visited
    visited=set()
    IMAGES.mkdir(parents=True,exist_ok=True)
    HIERARCHY.mkdir(parents=True,exist_ok=True)
    safe_adb("wait-for-device",timeout=90)
    pkg=safe_adb("shell","dumpsys","package",PACKAGE,timeout=25)
    version=re.search(r"versionName=([^\s]+)",pkg)
    ver=version.group(1) if version else "未知"
    (ROOT/"验证元数据.json").write_text(json.dumps({
        "应用包名":PACKAGE,"安装版本":ver,"模拟器":"Android API35 / Pixel 6；中文模式 zh-CN",
        "来源提交":"abbda87fea4fdd8b4725dfc504af340214421a1b",
        "方法":"Android 真正启动；adb screencap + uiautomator XML 自动抓取，未手工制造界面"
    },ensure_ascii=False,indent=2),encoding="utf-8")
    if ver!="2.13.5.1":
        raise RuntimeError("安装版本不符合要求："+ver)
    open_app()
    home=get_xml()
    require_zh(home)
    global timer_body_signature
    timer_body_signature=body_signature(home)
    (ROOT/"中文界面验收.txt").write_text(
        "简体中文导航：通过\n底部入口：计时、便签、知识、风控\n"+
        "系统语言："+safe_adb("shell","getprop","persist.sys.locale").strip()+"\n"+
        "App独立语言："+safe_adb("shell","cmd","locale","get-app-locales",PACKAGE,"--user","0").strip()+"\n",
        encoding="utf-8")
    capture("工作台","简体中文首页",home,force=True)
    module_root_signatures={}
    # First validate every top-level Chinese page, before attempting any editing.
    for module in MODULES:
        xml=at_module(module)
        sig=body_signature(xml)
        if sig in module_root_signatures.values():
            raise RuntimeError("不同模块显示相同正文，拒绝错误标注："+module)
        module_root_signatures[module]=sig
        capture(module,"中文模块首页已验证",xml,force=True)
    (ROOT/"中文模块导航验收.json").write_text(
        json.dumps({"界面语言":"简体中文","导航核验":"通过",
                    "模块根页面签名":module_root_signatures,
                    "说明":"已逐个进入并核对四个不同模块的真实正文界面"},ensure_ascii=False,indent=2),encoding="utf-8")
    # Only after the navigation gate has passed may deeper exploration begin.
    for module in MODULES:
        try:
            xml=at_module(module)
            if body_signature(xml)!=module_root_signatures[module]:
                errors.append("模块入口内容发生变化（继续按真实画面记录）："+module)
            if module in ("便签","知识","计时"): seed_editor(module)
            xml=at_module(module)
            explore(module,0,[module])
        except Exception as ex:
            errors.append("模块 "+module+" 探索异常："+str(ex))
            (ROOT/"模块导航问题.txt").write_text("\n".join(errors),encoding="utf-8")
            # Keep already captured valid modules as partial evidence. Do not forge coverage.
            continue
    # Canvas is a distinct nested route; capture its actual list and editing surface.
    try:
        at_module("知识")
        if click(["画布","卡片与连线"]):
            capture("画布","入口与画布列表",get_xml(),force=True)
            canvas=at_canvas()
            capture("画布","画布打开",canvas,force=True)
            explore("画布",0,["画布"])
    except Exception as ex:
        errors.append("画布探索异常："+str(ex))
    # Final restart demonstrates app still starts after navigation.
    try:
        safe_adb("shell","am","force-stop",PACKAGE)
        open_app()
        capture("工作台","强制停止后重新打开",get_xml(),force=True)
    except Exception as ex:
        errors.append("重启异常："+str(ex))
    (ROOT/"操作说明与截图索引.csv").write_text("",encoding="utf-8-sig")
    with (ROOT/"操作说明与截图索引.csv").open("w",newline="",encoding="utf-8-sig") as f:
        writer=csv.writer(f)
        writer.writerow(["序号","功能模块","操作说明","截图相对路径","验证说明","屏幕可见文字摘录"])
        writer.writerows(rows)
    (ROOT/"未覆盖与异常记录.txt").write_text("\n".join(errors) or "无自动化执行错误",encoding="utf-8")
    README=("十倍 Android 2.13.5.1 模拟器交互截图\n"
            "===================================\n"
            f"实际截图：{num} 张；安装版本：{ver}；Android API 35。\n"
            "截图按执行顺序编号；“操作说明与截图索引.csv”有点击动作及可见文本；每张 PNG 对应同名 UI XML。\n"
            "采集方式：在 GitHub Actions Pixel 6 模拟器上真实启动签名 APK，自动点击无外部副作用的入口，滚动与截图。\n"
            "注意：自动探索不等于全部功能通过测试。未登录账户、真实同步、真实 AI 调用、付款、删除和外部分享等未执行。\n"
            "遇到导航失败将留在“未覆盖与异常记录.txt”；真实覆盖量以索引和图片数量为准。\n"
            "虚构本地演示数据：GalleryDemo。截图为应用实际运行画面。\n"
            "APK 来源提交：abbda87fea4fdd8b4725dfc504af340214421a1b\n"
            "源码分支：feat/android-referral-forest-layout-v2.13.5.1\n"
            + "\n各模块张数：\n" + "\n".join(f"{m}：{c}" for m,c in module_counts.items()) + "\n")
    (ROOT/"请先阅读.txt").write_text(README,encoding="utf-8")
    print(README)
    if num<12:
        raise RuntimeError(f"有效截图仅 {num} 张，不足以证明自动化正常；请查阅日志和错误记录")
    if len(module_root_signatures)!=len(MODULES):
        raise RuntimeError("模块导航验收未覆盖全部中文模块")
if __name__=="__main__":
    main()
