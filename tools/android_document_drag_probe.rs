//! GitHub Android emulator acceptance of document block grip reordering.
//! Rust-only adb driver: inspect UI, input three blocks, swipe a grip, verify order.
use std::{env, fs, path::{Path, PathBuf}, process::{Command, Output}, thread::sleep, time::Duration};

const APP: &str = "com.ofairyo.gridtimer";
const ACTIVITY: &str = "com.ofairyo.gridtimer/.MainActivity";
const GRIP: &str = "拖动调整块顺序，点击打开操作菜单";
const TITLE: &str = "DragSortTest";

#[derive(Clone, Debug)]
struct Node {
    text: String,
    description: String,
    class: String,
    bounds: [i32;4],
}
fn adb(args: &[&str]) -> Result<Output,String> {
    Command::new("adb").args(args).output().map_err(|e| e.to_string())
}
fn command(args: &[&str]) -> Result<String,String> {
    let out=adb(args)?;
    if !out.status.success() {
        return Err(format!("adb {args:?}: {}",String::from_utf8_lossy(&out.stderr)));
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}
fn attr(node: &str,key:&str)->String{
    let pat=format!("{key}=\"");
    node.find(&pat).and_then(|i| {
        let tail=&node[i+pat.len()..];
        tail.find('"').map(|end|tail[..end].to_owned())
    }).unwrap_or_default()
}
fn nodes(xml:&str)->Vec<Node>{
    xml.split("<node ").skip(1).filter_map(|record|{
        let head=record.split('>').next()?;
        let bounds:Vec<i32>=attr(head,"bounds")
            .split(|c:char|!c.is_ascii_digit()&&c!='-')
            .filter(|p|!p.is_empty()).filter_map(|p|p.parse().ok()).collect();
        if bounds.len()!=4{return None;}
        Some(Node{
            text:attr(head,"text"), description:attr(head,"content-desc"),
            class:attr(head,"class"),bounds:[bounds[0],bounds[1],bounds[2],bounds[3]]
        })
    }).collect()
}
fn tree()->Result<String,String>{
    for _ in 0..5 {
        let _=command(&["shell","uiautomator","dump","/sdcard/drag_sort_test.xml"]);
        if let Ok(xml)=command(&["exec-out","cat","/sdcard/drag_sort_test.xml"]){
            if xml.contains("<hierarchy"){return Ok(xml);}
        }
        sleep(Duration::from_millis(550));
    }
    Err("Cannot read emulator accessibility hierarchy".into())
}
fn evidence(path:&Path, label:&str){
    if let Ok(xml)=tree(){let _=fs::write(path.join(format!("{label}.xml")),xml);}
    if let Ok(o)=adb(&["exec-out","screencap","-p"]){
        if o.status.success()&&o.stdout.starts_with(b"\x89PNG"){
            let _=fs::write(path.join(format!("{label}.png")),o.stdout);
        }
    }
}
fn center(n:&Node)->(i32,i32) {
    ((n.bounds[0]+n.bounds[2])/2,(n.bounds[1]+n.bounds[3])/2)
}
fn tap(n:&Node)->Result<(),String> {
    let (x,y)=center(n);
    command(&["shell","input","tap",&x.to_string(),&y.to_string()])?;
    sleep(Duration::from_millis(850));
    Ok(())
}
fn seek(labels:&[&str])->Result<Option<Node>,String> {
    let ns=nodes(&tree()?);
    for exact in [true,false]{
        for label in labels{
            if let Some(node)=ns.iter().find(|n|
                n.bounds[2]>n.bounds[0]&&n.bounds[3]>n.bounds[1]&&
                if exact{n.text==*label||n.description==*label}
                else{n.text.contains(label)||n.description.contains(label)}
            ){return Ok(Some(node.clone()));}
        }
    }
    Ok(None)
}
fn tap_label(labels:&[&str])->Result<bool,String>{
    if let Some(n)=seek(labels)?{tap(&n)?;Ok(true)}else{Ok(false)}
}
fn must_tap(labels:&[&str])->Result<(),String>{
    if tap_label(labels)?{Ok(())}else{Err(format!("Cannot find touch target {labels:?}"))}
}
fn clear_overlay()->Result<(),String>{
    for _ in 0..5{
        let ui=tree()?;
        if ui.contains("Pixel Launcher isn't responding") || ui.contains("Pixel Launcher isn&#39;t responding"){
            if tap_label(&["Close app"])?{continue;}
        }
        if ui.contains("Allow tenfold to send you notifications?"){
            if tap_label(&["Allow"])?{continue;}
        }
        break;
    }
    Ok(())
}
fn knowledge(evidence_root:&Path)->Result<(),String>{
    for attempt in 0..9 {
        clear_overlay()?;
        let ui=tree()?;
        if ui.contains("知识目录") || ui.contains("新建页面") || ui.contains("新建文件夹") ||
            ui.contains("Directory") || ui.contains("New page") || ui.contains("Blank page"){
            return Ok(());
        }
        evidence(evidence_root,&format!("navigation_{attempt}"));
        let _=tap_label(&["知识目录","知识库","知识","Knowledge","Notes","目录"])?;
    }
    Err("Knowledge directory could not be reached".into())
}
fn add_page(root:&Path)->Result<(),String>{
    // On a fresh English emulator the knowledge home already exposes a
    // "Blank page" template. That creates a page without an intermediate
    // "New page" choice; do not accidentally create two pages.
    if tap_label(&["Blank page","空白页"])? {
        evidence(root,"page_created_from_blank_template");
        return Ok(());
    }
    if tap_label(&["New page","新建页面"])? {
        evidence(root,"page_created_from_new_action");
        return Ok(());
    }
    evidence(root,"page_creation_missing");
    Err("Neither the blank-page template nor new-page button was visible".into())
}
fn open_persisted_page(root:&Path)->Result<(),String>{
    // The document list is below multiple full-height knowledge dashboard cards.
    // UIAutomator lists only composed LazyColumn items. After a fresh launch,
    // the page row exists in storage but is outside the current viewport.
    for attempt in 0..9 {
        if let Some(node)=seek(&[TITLE])?{
            tap(&node)?;
            sleep(Duration::from_millis(850));
            if tree()?.contains(GRIP) {
                return Ok(());
            }
        }
        evidence(root,&format!("reopen_scroll_{attempt}"));
        command(&["shell","input","swipe","540","1860","540","670","580"])?;
        sleep(Duration::from_millis(900));
    }
    Err("Saved page was not accessible after scrolling the full knowledge list".into())
}
fn edit_fields()->Result<Vec<Node>,String>{
    let ui=tree()?;
    let mut fields:Vec<Node>=nodes(&ui).into_iter()
        .filter(|n|n.class.ends_with("EditText")&&n.bounds[2]-n.bounds[0]>80)
        .collect();
    fields.sort_by_key(|n|center(n).1);
    Ok(fields)
}
fn write_text(s:&str)->Result<(),String>{
    command(&["shell","input","text",s])?;
    sleep(Duration::from_millis(800));
    Ok(())
}
fn key(code:&str)->Result<(),String>{
    command(&["shell","input","keyevent",code])?;
    sleep(Duration::from_millis(750));
    Ok(())
}
fn order()->Result<Option<[i32;3]>,String>{
    let ns=nodes(&tree()?);
    let mut positions=[None;3];
    for (i,term) in ["DragAlpha","DragBeta","DragGamma"].iter().enumerate(){
        positions[i]=ns.iter().find(|n|n.text.contains(term))
            .map(|n|center(n).1);
    }
    match positions{
        [Some(a),Some(b),Some(c)]=>Ok(Some([a,b,c])),
        _=>Ok(None)
    }
}
fn grips()->Result<Vec<Node>,String>{
    let mut found:Vec<Node>=nodes(&tree()?).into_iter()
        .filter(|n|n.description.contains(GRIP)).collect();
    found.sort_by_key(|n|center(n).1);
    Ok(found)
}
fn drag(from:&Node,to:&Node)->Result<(),String>{
    let (sx,sy)=center(from);
    let (tx,ty)=center(to);
    let end_y=(ty-18).max(100);
    command(&["shell","input","swipe",&sx.to_string(),&sy.to_string(),
        &tx.to_string(),&end_y.to_string(),"1200"])?;
    sleep(Duration::from_secs(2));
    Ok(())
}
fn run(out:&Path)->Result<String,String>{
    command(&["wait-for-device"])?;
    let info=command(&["shell","dumpsys","package",APP])?;
    if !info.contains("versionName=2.23.4.18"){
        return Err("Expected signed 2.23.4.18 APK not installed".into());
    }
    command(&["shell","am","start","-W","-n",ACTIVITY])?;
    sleep(Duration::from_secs(7));
    evidence(out,"01_started");
    knowledge(out)?;
    evidence(out,"02_directory");
    add_page(out)?;
    let fields=edit_fields()?;
    if fields.is_empty(){return Err("No editable knowledge field visible".into());}
    // The document title is the first editor field and body is the last.
    tap(&fields[0])?;
    write_text(TITLE)?;
    let body=edit_fields()?.into_iter().last()
        .ok_or("Document body EditText missing")?;
    tap(&body)?;
    write_text("DragAlpha")?;
    key("66")?; // ENTER splits the current text block.
    write_text("DragBeta")?;
    key("66")?;
    write_text("DragGamma")?;
    key("4")?; // close IME without changing the document
    sleep(Duration::from_secs(1));
    evidence(out,"03_three_blocks");
    let initial=order()?.ok_or("Cannot read the three unique text blocks")?;
    if !(initial[0]<initial[1]&&initial[1]<initial[2]){
        return Err(format!("Initial text order incorrect: {initial:?}"));
    }
    let handles=grips()?;
    if handles.len()<3 {
        return Err(format!("Expected three accessible drag handles, saw {}",handles.len()));
    }
    // Drag the last block over the first block. This must not open the menu.
    drag(handles.last().ok_or("last handle")?,handles.first().ok_or("first handle")?)?;
    evidence(out,"04_after_swipe");
    let after=order()?.ok_or("Text block visibility lost after dragging")?;
    if !(after[2]<after[0] && after[0]<after[1]){
        return Err(format!("Drag did not reorder Gamma, Alpha, Beta: {after:?}"));
    }
    let handles=grips()?;
    tap(handles.first().ok_or("No drag handle after reorder")?)?;
    let menu=tree()?;
    if !(menu.contains("复制块") || menu.contains("Copy block")) ||
        !(menu.contains("删除块") || menu.contains("Delete block")){
        return Err("Tapping the grip no longer opens block actions".into());
    }
    evidence(out,"05_tap_menu");
    key("4")?;
    command(&["shell","am","force-stop",APP])?;
    command(&["shell","am","start","-W","-n",ACTIVITY])?;
    sleep(Duration::from_secs(6));
    knowledge(out)?;
    open_persisted_page(out)?;
    sleep(Duration::from_secs(2));
    evidence(out,"06_reopened");
    let reopened=order()?.ok_or("Text blocks unavailable after restart")?;
    if !(reopened[2]<reopened[0] && reopened[0]<reopened[1]){
        return Err(format!("Reordered blocks were not persisted: {reopened:?}"));
    }
    if command(&["shell","pidof",APP])?.trim().is_empty(){
        return Err("App process terminated after reordering".into());
    }
    Ok("PASS: launched signed Android app, created three blocks, dragged grip, retained click menu and persisted reordered blocks after process restart.".into())
}
fn main(){
    let path=env::args_os().nth(1).map(PathBuf::from)
        .unwrap_or_else(||PathBuf::from("document_drag_evidence"));
    fs::create_dir_all(&path).expect("evidence directory");
    let result=run(&path);
    evidence(&path,"07_final");
    let report=match &result{
        Ok(success)=>format!("# Android knowledge block drag simulator acceptance\n\n{success}\n\nOverall: PASS\n"),
        Err(reason)=>format!("# Android knowledge block drag simulator acceptance\n\nFailure: {reason}\n\nOverall: FAILED / NOT VERIFIED\n"),
    };
    fs::write(path.join("report.md"),&report).expect("report");
    println!("{report}");
    if result.is_err(){std::process::exit(1);}
}
