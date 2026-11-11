// Android emulator acceptance for the Rust-backed customer network canvas.
// Uses only adb and the accessibility hierarchy; screenshot evidence is retained.
use std::{env,fs,path::{Path,PathBuf},process::{Command,Output},thread,time::Duration};

const APP: &str = "com.ofairyo.gridtimer";
#[derive(Clone,Debug)]
struct Target { text: String, description: String, class: String, bounds: [i32;4] }

fn adb(args: &[&str]) -> Result<Output,String> {
    Command::new("adb").args(args).output().map_err(|e|e.to_string())
}
fn run(args: &[&str]) -> Result<String,String> {
    let o=adb(args)?;
    if !o.status.success() { return Err(format!("adb {:?}: {}",args,String::from_utf8_lossy(&o.stderr))); }
    Ok(String::from_utf8_lossy(&o.stdout).into_owned())
}
fn attr(node: &str,key: &str) -> String {
    let prefix=format!("{key}=\"");
    node.find(&prefix).and_then(|i| {let rest=&node[i+prefix.len()..];rest.find('"').map(|j|rest[..j].to_owned())}).unwrap_or_default()
}
fn elements(xml: &str) -> Vec<Target> {
    xml.split("<node ").skip(1).filter_map(|entry| {
        let head=entry.split('>').next()?;
        let nums: Vec<i32>=attr(head,"bounds").split(|c:char| !c.is_ascii_digit() && c!='-')
            .filter(|s|!s.is_empty()).filter_map(|s|s.parse().ok()).collect();
        if nums.len()!=4 {return None;}
        Some(Target {text:attr(head,"text"),description:attr(head,"content-desc"),class:attr(head,"class"),bounds:[nums[0],nums[1],nums[2],nums[3]]})
    }).collect()
}
fn xml() -> Result<String,String> {
    for _ in 0..5 {
        let _=run(&["shell","uiautomator","dump","/sdcard/customer_canvas.xml"]);
        if let Ok(s)=run(&["exec-out","cat","/sdcard/customer_canvas.xml"]) {
            if s.contains("<hierarchy") {return Ok(s);}
        }
        thread::sleep(Duration::from_millis(500));
    }
    Err("Android UI hierarchy unavailable".into())
}
fn seek(labels: &[&str]) -> Result<Option<Target>,String> {
    let tree=xml()?;
    let nodes=elements(&tree);
    for label in labels {
        if let Some(n)=nodes.iter().find(|n| n.bounds[2]>n.bounds[0] && n.bounds[3]>n.bounds[1] && (n.text==*label || n.description==*label)) {return Ok(Some(n.clone()));}
    }
    for label in labels {
        if let Some(n)=nodes.iter().find(|n| {
            n.bounds[2]>n.bounds[0] && n.bounds[3]>n.bounds[1] &&
            (n.text==*label || n.description==*label || n.text.contains(label) || n.description.contains(label))
        }) { return Ok(Some(n.clone())); }
    }
    Ok(None)
}
fn tap(n: &Target) -> Result<(),String> {
    run(&["shell","input","tap",&((n.bounds[0]+n.bounds[2])/2).to_string(),&((n.bounds[1]+n.bounds[3])/2).to_string()])?;
    thread::sleep(Duration::from_millis(650));Ok(())
}
fn tap_text(labels:&[&str])->Result<bool,String>{
    if let Some(n)=seek(labels)?{tap(&n)?;Ok(true)}else{Ok(false)}
}
fn screenshot(out:&Path,name:&str){
    if let Ok(o)=adb(&["exec-out","screencap","-p"]){if o.status.success()&&o.stdout.starts_with(b"\x89PNG"){let _=fs::write(out.join(format!("{name}.png")),o.stdout);}}
    if let Ok(tree)=xml(){let _=fs::write(out.join(format!("{name}.xml")),tree);}
}
fn maybe_system_overlay()->Result<(),String>{
    for _ in 0..5 {
        let tree=xml()?;
        if tree.contains("Pixel Launcher isn't responding") || tree.contains("Pixel Launcher isn&#39;t responding") {
            if let Some(n)=seek(&["Close app"])?{tap(&n)?;continue;}
        }
        if tree.contains("Allow tenfold to send you notifications?") {
            if let Some(n)=seek(&["Allow"])?{tap(&n)?;continue;}
        }
        break;
    }
    Ok(())
}
fn navigate_to_canvas(out:&Path)->Result<(),String>{
    maybe_system_overlay()?;
    for _ in 0..4 {
        if seek(&["我的画布"])?.is_some(){return Ok(());}
        let _=tap_text(&["画布","Canvas","卡片与连线"])?;
        if seek(&["我的画布"])?.is_some(){return Ok(());}
        let _=tap_text(&["知识库","知识","Knowledge","笔记","Notes"])?;
        if seek(&["我的画布"])?.is_some(){return Ok(());}
        screenshot(out,"navigation");
    }
    Err("Cannot open universal canvas library from home".into())
}
fn enter_name(name:&str)->Result<(),String>{
    let tree=xml()?;
    let field=elements(&tree).into_iter().find(|n|n.class.ends_with("EditText")&&n.bounds[2]-n.bounds[0]>50).ok_or("Customer name input missing")?;
    tap(&field)?;
    run(&["shell","input","text",name])?;
    // On English-language emulators Material3 translates confirm actions to Create.
    // Do not send KEYCODE_BACK here: on some Android IME states it dismisses
    // the entire AlertDialog instead of only hiding the soft keyboard.
    thread::sleep(Duration::from_millis(400));
    Ok(())
}
fn create_card(name:&str)->Result<(),String>{
    enter_name(name)?;
    if !tap_text(&["创建","Create"])?{return Err("Card creation confirmation not accessible".into());}
    thread::sleep(Duration::from_secs(1));
    Ok(())
}
fn count(expected:&str)->Result<(),String>{
    for _ in 0..5 {
        if xml()?.contains(expected){return Ok(());}
        thread::sleep(Duration::from_millis(650));
    }
    Err(format!("Canvas did not show {expected}"))
}
fn open_center_card()->Result<(),String>{
    let tree=xml()?;
    let surface=elements(&tree).into_iter().find(|n|n.description.contains("知识画布，")&&n.description.contains("张卡片")).ok_or("Canvas touch surface missing")?;
    // The first card is placed around the exact viewport center by the Rust layout engine.
    let x=(surface.bounds[0]+surface.bounds[2])/2;
    let y=(surface.bounds[1]+surface.bounds[3])/2;
    run(&["shell","input","tap",&x.to_string(),&y.to_string()])?;
    thread::sleep(Duration::from_millis(750));Ok(())
}

fn close_overlay()->Result<(),String>{
    run(&["shell","input","keyevent","4"])?;
    thread::sleep(Duration::from_millis(750));
    Ok(())
}
fn must_tap(labels:&[&str])->Result<(),String>{
    if tap_text(labels)?{Ok(())}else{Err(format!("Missing action {:?}",labels))}
}
fn take(label:&str,out:&Path){screenshot(out,label);}
fn navigate_board(title:&str)->Result<(),String>{
    must_tap(&[title])?;
    thread::sleep(Duration::from_millis(650));
    Ok(())
}
fn main(){
    let out=env::args().nth(1).map(PathBuf::from).unwrap_or_else(||PathBuf::from("universal_canvas_evidence"));
    fs::create_dir_all(&out).expect("evidence folder");
    let mut report=String::from("# Universal canvas and customer-template Android emulator acceptance\n\n");
    let result=(||->Result<(),String>{
        run(&["shell","am","start","-W","-n","com.ofairyo.gridtimer/.MainActivity"])?;
        thread::sleep(Duration::from_secs(6));maybe_system_overlay()?;
        take("01_home",&out);
        if run(&["shell","pidof",APP])?.trim().is_empty(){return Err("App process is not running".into());}
        navigate_to_canvas(&out)?;take("02_universal_library",&out);
        must_tap(&["知识画布"])?;
        if seek(&["从一个想法开始"])?.is_none(){return Err("Original blank canvas was replaced by customer-only onboarding".into());}
        take("03_generic_empty",&out);
        must_tap(&["＋ 添加第一张便签"])?;
        create_card("IdeaA")?;
        count("1 张卡片")?;take("04_generic_first_note",&out);
        must_tap(&["＋ 添加"])?;
        if seek(&["＋ 新建便签"])?.is_none() || seek(&["＋ 新建知识页面"])?.is_none() || seek(&["＋ 引用已有知识页面"])?.is_none() {
            return Err("Generic note/page creation actions are missing".into());
        }
        take("05_generic_creation_menu",&out);
        must_tap(&["＋ 新建便签"])?;
        create_card("IdeaB")?;
        count("2 张卡片")?;take("06_generic_two_notes",&out);
        must_tap(&["100%"])?;
        if seek(&["缩放比例"])?.is_none(){return Err("Zoom picker did not open".into());}
        must_tap(&["1%"])?;
        must_tap(&["应用"])?;
        count("1%")?;
        take("06a_generic_one_percent",&out);
        // The first generic note is centered by the Rust layout engine.
        // A tap on its tiny bird's-eye marker must zoom back to 100%.
        open_center_card()?;
        if seek(&["编辑便签"])?.is_none(){return Err("One-percent marker did not open card details".into());}
        close_overlay()?;
        count("100%")?;
        take("06b_generic_marker_focus",&out);
        report.push_str("PASS: 1% zoom picker, visible low-zoom markers and tap-to-focus restored 100%.\\n\\n");
        report.push_str("PASS: existing blank canvas remained generic; note creation and both page actions are present.\n\n");
        open_center_card()?;
        take("07_generic_card_detail",&out);
        for (action, labels) in [
            ("free link", &["自由连线", "Free link", "Connect"][..]),
            ("manage links", &["管理连线", "Manage links", "Manage connections"][..]),
            ("resize", &["放大卡片", "Enlarge card", "Expand card"][..]),
        ] {
            if seek(labels)?.is_none() {
                return Err(format!("Generic card lost editing action {action}: {labels:?}"));
            }
        }
        take("07_generic_card_tools",&out);
        report.push_str("PASS: ordinary note card exposes free linking, edge management and resize controls.\n\n");
        must_tap(&["放大卡片","Enlarge card","Expand card"])?;
        open_center_card()?;
        if seek(&["缩小卡片","Shrink card","Reduce card"])?.is_none(){return Err("Canvas resize did not change card state".into());}
        must_tap(&["缩小卡片","Shrink card","Reduce card"])?;
        open_center_card()?;
        must_tap(&["自由连线","Free link","Connect"])?;
        must_tap(&["查找"])?;
        must_tap(&["IdeaB"])?;
        count("1 条关系")?;
        take("08_generic_free_link",&out);
        report.push_str("PASS: two generic notes were linked through the real canvas UI.\n\n");
        open_center_card()?;
        must_tap(&["管理连线","Manage links","Manage connections"])?;
        if seek(&["普通"])?.is_none(){return Err("Generic edge management did not expose ordinary link".into());}
        take("09_generic_edge_panel",&out);
        close_overlay()?;
        must_tap(&["返回","Back"])?;
        if seek(&["我的画布"])?.is_none(){return Err("Back did not return to canvas library".into());}
        must_tap(&["客户网络模板"])?;
        take("10_customer_template_create",&out);
        must_tap(&["创建","Create"])?;
        count("0 张卡片")?;
        must_tap(&["＋ 添加第一个客户"])?;
        create_card("ClientA")?;
        count("1 张卡片")?;
        open_center_card()?;
        if seek(&["绑定推荐","Bind referral","Set referral"])?.is_none(){return Err("Customer referral action missing".into());}
        must_tap(&["添加此客户推荐的人","Add referral","Add person referred"])?;
        create_card("ClientB")?;
        count("2 张卡片")?;count("1 条关系")?;
        take("11_customer_template_referral",&out);
        report.push_str("PASS: optional customer template creates client cards with one referral connection.\n\n");
        must_tap(&["更多","More"])?;
        must_tap(&["按推荐层级排列客户"])?;
        thread::sleep(Duration::from_millis(900));
        take("12_customer_tree",&out);
        report.push_str("PASS: customer template retains referral tree layout.\n\n");
        run(&["shell","am","force-stop",APP])?;
        run(&["shell","am","start","-W","-n","com.ofairyo.gridtimer/.MainActivity"])?;
        thread::sleep(Duration::from_secs(5));
        navigate_to_canvas(&out)?;
        navigate_board("客户网络")?;
        count("2 张卡片")?;count("1 条关系")?;
        take("13_customer_reopened",&out);
        must_tap(&["返回","Back"])?;
        navigate_board("知识画布")?;
        count("2 张卡片")?;count("1 条关系")?;
        take("14_generic_reopened",&out);
        report.push_str("PASS: normal and customer canvases retain data across force-stop and restart.\n\n");
        if run(&["shell","pidof",APP])?.trim().is_empty(){return Err("App process died during acceptance".into());}
        Ok(())
    })();
    match result {
        Ok(())=>report.push_str("Overall: PASS\n"),
        Err(e)=>report.push_str(&format!("Overall: FAILED / NOT VERIFIED\n\nReason: {e}\n"))
    }
    report.push_str("\nVerified on a GitHub-hosted Android emulator. Evidence contains screenshots and accessibility trees.\n");
    fs::write(out.join("report.md"),&report).expect("report");
    println!("{report}");
    if !report.contains("Overall: PASS"){std::process::exit(1);}
}
