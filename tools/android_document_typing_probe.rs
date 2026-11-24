//! GitHub Android emulator acceptance of stable text entry and knowledge share images.
use std::{env, fs, path::{Path, PathBuf}, process::{Command, Output}, thread::sleep, time::Duration};

const APP: &str = "com.ofairyo.gridtimer";
const ACTIVITY: &str = "com.ofairyo.gridtimer/.MainActivity";
mod android_save_probe_fixture;
use android_save_probe_fixture::{installed_version, SqliteWriter};
mod android_knowledge_share_probe;

const GRIP: &str = "拖动调整块顺序，点击打开操作菜单";
const TITLE: &str = "TypingStabilityCheck";

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
        let _=command(&["shell","uiautomator","dump","/sdcard/typing_stability_test.xml"]);
        if let Ok(xml)=command(&["exec-out","cat","/sdcard/typing_stability_test.xml"]){
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
fn grips()->Result<Vec<Node>,String>{
    let mut found:Vec<Node>=nodes(&tree()?).into_iter()
        .filter(|n|n.description.contains(GRIP)).collect();
    found.sort_by_key(|n|center(n).1);
    Ok(found)
}
fn run(out:&Path)->Result<String,String>{
    command(&["wait-for-device"])?;
    let info=command(&["shell","dumpsys","package",APP])?;
    installed_version(&info)?;
    command(&["root"])?;
    command(&["wait-for-device"])?;
    command(&["shell","am","start","-W","-n",ACTIVITY])?;
    sleep(Duration::from_secs(7));
    evidence(out,"01_app_launched");
    knowledge(out)?;
    add_page(out)?;
    let fields=edit_fields()?;
    if fields.is_empty(){return Err("Document editor did not show any editable fields".into());}
    tap(&fields[0])?;
    write_text(TITLE)?;
    let body=edit_fields()?.into_iter().last()
        .ok_or("Document body EditText missing")?;
    tap(&body)?;
    let before=grips()?.len();
    if before!=1 {return Err(format!("Expected one initial empty paragraph, found {before} grips"));}
    evidence(out,"02_blank_body");

    if !tap_label(&["插入","Insert","＋"])? {
        return Err("Insert toolbar command not accessible".into());
    }
    evidence(out,"03_insert_dialog");
    if !tap_label(&["一级标题","Heading 1","H1 标题"])? {
        return Err("H1 insertion command not accessible".into());
    }
    sleep(Duration::from_millis(950));
    let after_heading=grips()?.len();
    if after_heading!=before {
        return Err(format!("H1 generated a phantom blank paragraph: before={before}, after={after_heading}"));
    }
    evidence(out,"04_one_heading_block");

    let field=edit_fields()?.into_iter().last().ok_or("H1 field missing")?;
    tap(&field)?;
    write_text("StableHeading")?;
    if seek(&["StableHeading"])?.is_none(){
        evidence(out,"heading_not_visible");
        return Err("New heading text is not editable".into());
    }
    key("66")?; // ENTER: one new paragraph, never two.
    write_text("StableParagraph")?;
    sleep(Duration::from_millis(1000));
    let after_enter=grips()?.len();
    if after_enter!=2 {
        evidence(out,"wrong_enter_block_count");
        return Err(format!("H1 Enter must produce exactly two blocks, got {after_enter}"));
    }
    let first=seek(&["StableHeading"])?.ok_or("Heading lost after Enter")?;
    let second=seek(&["StableParagraph"])?.ok_or("Paragraph not accessible")?;
    let before_y=(center(&first).1,center(&second).1);
    evidence(out,"05_editor_before_regular_typing");

    // Same-line typing must not move the surrounding paragraph stack.
    let writer = SqliteWriter::acquire()?;
    write_text("NoJump")?;
    let pending = tree()?;
    writer.release()?;
    fs::write(out.join("receipt_while_locked.xml"), &pending).map_err(|e| e.to_string())?;
    let has_state = |xml: &str, state: &str| xml.split("<node ").any(|node|
        attr(node.split('>').next().unwrap_or(""), "resource-id").ends_with(state));
    if !has_state(&pending, "document_save_saving") || has_state(&pending, "document_save_local_saved") {
        return Err("The edited document did not show a pending receipt while SQLite was locked".into());
    }
    let mut committed = false;
    for _ in 0..12 {
        if has_state(&tree()?, "document_save_local_saved") { committed = true; break; }
        sleep(Duration::from_millis(250));
    }
    if !committed { return Err("No durable receipt after SQLite writer release".into()); }
    // Measure an actual unblocked body edit, not only a new page or a locked writer.
    // UIAutomator dumps have their own latency; the wall-clock budget includes that.
    let hot_start = std::time::Instant::now();
    command(&["shell","input","text","HotSave"])?;
    let mut hot_saved = false;
    while hot_start.elapsed() < Duration::from_secs(8) {
        let ui = tree()?;
        if ui.contains("StableParagraphNoJumpHotSave") &&
            has_state(&ui, "document_save_local_saved") {
            hot_saved = true;
            break;
        }
        sleep(Duration::from_millis(100));
    }
    let hot_ms = hot_start.elapsed().as_millis();
    fs::write(out.join("hot_body_save_ms.txt"), format!("{hot_ms}\n")).map_err(|e| e.to_string())?;
    if !hot_saved || hot_ms > 8_000 {
        evidence(out,"hot_body_save_timeout");
        return Err(format!("Uncontended body edit did not reach a local receipt within 8s: {hot_ms}ms"));
    }
    let first_after=seek(&["StableHeading"])?.ok_or("Heading lost during typing")?;
    let second_after=seek(&["StableParagraphNoJumpHotSave"])?.ok_or("Typed text lost")?;
    let after_y=(center(&first_after).1,center(&second_after).1);
    if (after_y.0-before_y.0).abs()>28 || (after_y.1-before_y.1).abs()>28 {
        evidence(out,"unexpected_scroll_movement");
        return Err(format!("Single-line typing shifted the canvas: {before_y:?} to {after_y:?}"));
    }
    evidence(out,"06_editor_after_regular_typing");

    // Reproduce a final edit followed immediately by the visible toolbar Back.
    // Do not treat an inaccessible editor (the saving barrier) as a completed exit.
    let back = seek(&["返回","Back"])?.ok_or("Document back action missing")?;
    let (back_x, back_y) = center(&back);
    command(&["shell","input","text","ExitSave"])?;
    let exit_start = std::time::Instant::now();
    command(&["shell","input","tap",&back_x.to_string(),&back_y.to_string()])?;
    let mut exited = false;
    while exit_start.elapsed() < Duration::from_secs(8) {
        let ui = tree()?;
        if ui.contains(TITLE) && !ui.contains(GRIP) &&
            !ui.contains("正在保存") && !has_state(&ui, "document_save_saving") &&
            !has_state(&ui, "document_save_local_saved") {
            exited = true;
            break;
        }
        sleep(Duration::from_millis(100));
    }
    let exit_ms = exit_start.elapsed().as_millis();
    fs::write(out.join("edit_exit_save_ms.txt"), format!("{exit_ms}\n")).map_err(|e| e.to_string())?;
    if !exited || exit_ms > 8_000 {
        evidence(out,"edit_exit_save_timeout");
        return Err(format!("Final edit followed by toolbar Back exceeded the exit budget: {exit_ms}ms"));
    }

    command(&["shell","am","force-stop",APP])?;
    command(&["shell","am","start","-W","-n",ACTIVITY])?;
    sleep(Duration::from_secs(6));
    knowledge(out)?;
    open_persisted_page(out)?;
    evidence(out,"07_reopened_saved_heading");
    if seek(&["StableHeading"])?.is_none() || seek(&["StableParagraphNoJumpHotSaveExitSave"])?.is_none() {
        return Err("Heading or paragraph disappeared after process restart".into());
    }
    if command(&["shell","pidof",APP])?.trim().is_empty() {
        return Err("App did not remain running after reopening the document".into());
    }
    let share_result = android_knowledge_share_probe::export_and_check(out)?;
    Ok(format!("PASS: H1 reused one block. Enter created one paragraph. Row centers before/after typing were {before_y:?} and {after_y:?}. SQLite contention held the visible receipt in Saving. Uncontended body edit reached a local receipt in {hot_ms} ms and final edit followed by Back exited in {exit_ms} ms (including UIAutomator). The heading and full paragraph survived force-stop after local confirmation.\n\n{share_result}"))
}
fn main(){
    let path=env::args_os().nth(1).map(PathBuf::from)
        .unwrap_or_else(||PathBuf::from("document_typing_evidence"));
    fs::create_dir_all(&path).expect("evidence directory");
    let result=run(&path);
    evidence(&path,"08_final");
    let report=match &result{
        Ok(success)=>format!("# Android document typing emulator acceptance\n\n{success}\n\nOverall: PASS\n"),
        Err(reason)=>format!("# Android document typing emulator acceptance\n\nFailure: {reason}\n\nOverall: FAILED / NOT VERIFIED\n"),
    };
    fs::write(path.join("report.md"),&report).expect("report");
    println!("{report}");
    if result.is_err(){std::process::exit(1);}
}
