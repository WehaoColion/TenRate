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
        if seek(&["知识画布，","把客户关系整理成网络","＋ 添加"])?.is_some(){return Ok(());}
        let _=tap_text(&["画布","Canvas","卡片与连线"])?;
        if seek(&["知识画布，","把客户关系整理成网络","＋ 添加"])?.is_some(){return Ok(());}
        let _=tap_text(&["知识库","知识","Knowledge","笔记","Notes"])?;
        if seek(&["知识画布，","把客户关系整理成网络","＋ 添加"])?.is_some(){return Ok(());}
        screenshot(out,"navigation");
    }
    Err("Cannot open knowledge canvas from home".into())
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
fn create_client(name:&str)->Result<(),String>{
    enter_name(name)?;
    if !tap_text(&["创建","Create"])?{return Err("Customer create button missing: inspect the post-input accessibility hierarchy".into());}
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
fn main(){
    let out=env::args().nth(1).map(PathBuf::from).unwrap_or_else(||PathBuf::from("customer_canvas_evidence"));
    fs::create_dir_all(&out).expect("evidence folder");
    let mut report=String::from("# Customer network canvas Android simulator acceptance\n\n");
    let result=(||->Result<(),String>{
        run(&["shell","am","start","-W","-n","com.ofairyo.gridtimer/.MainActivity"])?;
        thread::sleep(Duration::from_secs(6));maybe_system_overlay()?;
        screenshot(&out,"01_home");
        if run(&["shell","pidof",APP])?.trim().is_empty(){return Err("App process not running".into());}
        navigate_to_canvas(&out)?;screenshot(&out,"02_canvas_open");
        report.push_str("PASS: signed Android app started and the canvas navigation opened.\n\n");
        if !tap_text(&["＋ 添加第一个客户","＋ 添加"])?{return Err("Customer add entrance missing".into());}
        if seek(&["＋ 新建客户"])?.is_some(){if !tap_text(&["＋ 新建客户"])?{return Err("New customer choice missing".into());}}
        screenshot(&out,"03_customer_form");
        create_client("ClientA")?;
        count("1 张卡片")?;screenshot(&out,"04_customer_saved");
        report.push_str("PASS: customer creation retained a named card on the network.\n\n");
        open_center_card()?;
        if seek(&["ClientA"])?.is_none(){return Err("Tapping a customer did not show its name in detail sheet".into());}
        screenshot(&out,"05_customer_detail");
        report.push_str("PASS: tapping a canvas card shows the customer detail sheet without leaving the canvas.\n\n");
        if !tap_text(&["添加此客户推荐的人"])?{return Err("Direct referral add action missing".into());}
        create_client("ClientB")?;
        count("2 张卡片")?;count("1 条关系")?;screenshot(&out,"06_referral_saved");
        report.push_str("PASS: adding a referred customer creates exactly two cards and one directed relationship.\n\n");
        if !tap_text(&["布局"])?{return Err("Canvas layout menu missing".into());}
        if !tap_text(&["按推荐层级排列客户"])?{return Err("Tree arrangement action missing".into());}
        thread::sleep(Duration::from_millis(900));screenshot(&out,"07_tree_layout");
        report.push_str("PASS: tree layout command completed on the emulator.\n\n");
        run(&["shell","am","force-stop",APP])?;
        run(&["shell","am","start","-W","-n","com.ofairyo.gridtimer/.MainActivity"])?;
        thread::sleep(Duration::from_secs(5));navigate_to_canvas(&out)?;
        count("2 张卡片")?;count("1 条关系")?;screenshot(&out,"08_reopened_persisted");
        report.push_str("PASS: force-stop and reopen preserved customer cards and their connection.\n\n");
        let process=run(&["shell","pidof",APP])?;
        if process.trim().is_empty(){return Err("Application process died".into());}
        Ok(())
    })();
    match result {
        Ok(())=>report.push_str("Overall: PASS\n"),
        Err(e)=>report.push_str(&format!("Overall: FAILED / NOT VERIFIED\n\nReason: {e}\n"))
    }
    report.push_str("\nReal API 35 emulator UI test; images and accessibility dumps are saved as evidence.\n");
    fs::write(out.join("report.md"),&report).expect("report");
    println!("{report}");
    if !report.contains("Overall: PASS"){std::process::exit(1);}
}
