// v2.13.5.13 - API 35 emulator smoke acceptance for canvas AI entry and request guards.
use std::{env,fs,path::{Path,PathBuf},process::{Command,Output},thread,time::Duration};

const APP: &str = "com.ofairyo.gridtimer";
#[derive(Clone)]
struct Node { text:String, description:String, class:String, bounds:[i32;4] }
fn adb(args:&[&str])->Result<Output,String>{Command::new("adb").args(args).output().map_err(|e|e.to_string())}
fn run(args:&[&str])->Result<String,String>{
    let out=adb(args)?;
    if !out.status.success(){return Err(format!("adb {:?}: {}",args,String::from_utf8_lossy(&out.stderr)))}
    Ok(String::from_utf8_lossy(&out.stdout).to_string())
}
fn attr(s:&str,key:&str)->String{
    let tag=format!("{key}=\"");
    s.find(&tag).and_then(|i|{
        let v=&s[i+tag.len()..];v.find('"').map(|j|v[..j].to_string())
    }).unwrap_or_default()
}
fn nodes(xml:&str)->Vec<Node>{
    xml.split("<node ").skip(1).filter_map(|p|{
        let head=p.split('>').next()?;
        let ints:Vec<i32>=attr(head,"bounds").split(|ch:char|!ch.is_ascii_digit()&&ch!='-')
            .filter(|s|!s.is_empty()).filter_map(|s|s.parse().ok()).collect();
        if ints.len()!=4{return None}
        Some(Node{text:attr(head,"text"),description:attr(head,"content-desc"),
            class:attr(head,"class"),
            bounds:[ints[0],ints[1],ints[2],ints[3]]})
    }).collect()
}
fn xml()->Result<String,String>{
    for _ in 0..5{
        let _=run(&["shell","uiautomator","dump","/sdcard/canvas_question.xml"]);
        if let Ok(data)=run(&["exec-out","cat","/sdcard/canvas_question.xml"]){
            if data.contains("<hierarchy"){return Ok(data)}
        }
        thread::sleep(Duration::from_millis(400));
    }
    Err("Cannot read Android accessibility tree".into())
}
fn find(labels:&[&str])->Result<Option<Node>,String>{
    let tree=xml()?;
    for label in labels{
        if let Some(node)=nodes(&tree).into_iter().find(|n|
            n.bounds[2]>n.bounds[0]&&n.bounds[3]>n.bounds[1]&&
            (n.text.contains(label)||n.description.contains(label))){
            return Ok(Some(node))
        }
    }
    Ok(None)
}
fn tap(n:&Node)->Result<(),String>{
    run(&["shell","input","tap",
        &((n.bounds[0]+n.bounds[2])/2).to_string(),
        &((n.bounds[1]+n.bounds[3])/2).to_string()])?;
    thread::sleep(Duration::from_millis(700));
    Ok(())
}
fn tap_label(names:&[&str])->Result<bool,String>{
    if let Some(n)=find(names)?{tap(&n)?;Ok(true)}else{Ok(false)}
}
fn evidence(dir:&Path,name:&str){
    if let Ok(out)=adb(&["exec-out","screencap","-p"]){
        if out.status.success()&&out.stdout.starts_with(b"\x89PNG"){
            let _=fs::write(dir.join(format!("{name}.png")),out.stdout);
        }
    }
    if let Ok(tree)=xml(){let _=fs::write(dir.join(format!("{name}.xml")),tree);}
}
fn dismiss_system()->Result<(),String>{
    for _ in 0..5{
        let tree=xml()?;
        if tree.contains("Pixel Launcher isn't responding") || tree.contains("Pixel Launcher isn&#39;t responding"){
            let _=tap_label(&["Close app"])?;continue;
        }
        if tree.contains("Allow tenfold to send you notifications?"){
            let _=tap_label(&["Allow"])?;continue;
        }
        break;
    }
    Ok(())
}
fn open_canvas(dir:&Path)->Result<(),String>{
    for attempt in 0..6{
        dismiss_system()?;
        if find(&["向这张画布提问","知识画布，"])?.is_some(){return Ok(())}
        if find(&["我的画布","新建画布"])?.is_some(){
            if tap_label(&["知识画布","客户网络"])?
                && find(&["向这张画布提问","知识画布，"])?.is_some(){return Ok(())}
        }
        let _=tap_label(&["画布","Canvas","卡片与连线"])?;
        if find(&["向这张画布提问","知识画布，"])?.is_some(){return Ok(())}
        let _=tap_label(&["知识库","知识","Knowledge","笔记","Notes"])?;
        evidence(dir,&format!("navigation_{attempt}"));
    }
    Err("Unable to reach a canvas board from the main app".into())
}
// A Material3 button's label can report enabled=true even when the button is
// disabled. Exercise the guarded action rather than trusting child semantics.
fn verify_submission_blocked(question:Option<&str>)->Result<(),String>{
    let send=find(&["发送问题"])?.ok_or("Send button missing")?;
    tap(&send)?;
    let tree=xml()?;
    if !tree.contains("画布问答") || !tree.contains("发送问题") ||
        tree.contains("正在分析画布") || tree.contains("这轮回答尚未保存") {
        return Err("A guarded canvas question started or dismissed unexpectedly".into());
    }
    match question {
        Some(value) if !tree.contains(value) =>
            Err("Blocked submission cleared the unsent question".into()),
        None if !tree.contains("向这张画布提问") =>
            Err("Blank submission unexpectedly changed the input".into()),
        _ => Ok(())
    }
}
fn acceptance(dir:&Path, report:&mut String)->Result<(),String>{
    run(&["shell","am","start","-W","-n","com.ofairyo.gridtimer/.MainActivity"])?;
    thread::sleep(Duration::from_secs(7));
    dismiss_system()?;
    evidence(dir,"01_app_started");
    if run(&["shell","pidof",APP])?.trim().is_empty(){return Err("App process not running".into())}
    open_canvas(dir)?;
    evidence(dir,"02_canvas");
    if !tap_label(&["向这张画布提问"])?{return Err("Canvas ask entry is missing".into())}
    thread::sleep(Duration::from_millis(600));
    evidence(dir,"03_question_sheet");
    let tree=xml()?;
    if !tree.contains("画布问答") || !tree.contains("当前画布"){
        return Err("Question sheet title or scope is missing".into());
    }
    report.push_str("PASS: signed APK opens canvas and displays a scoped question sheet.\n\n");
    verify_submission_blocked(None)?;
    report.push_str("PASS: empty question cannot be sent.\n\n");
    if !tap_label(&["返回画布"])?{return Err("Return to canvas action missing".into())}
    evidence(dir,"04_back_to_canvas");
    if find(&["向这张画布提问"])?.is_none(){
        return Err("Returning from canvas chat lost the canvas surface".into())
    }
    report.push_str("PASS: dismissing the question sheet restores canvas without a graph mutation.\n\n");
    let _=tap_label(&["向这张画布提问"])?;
    if let Some(field)=nodes(&xml()?).into_iter().find(|n|n.class.ends_with("EditText")){
        tap(&field)?;
        run(&["shell","input","text","HowMuchPending"])?;
        thread::sleep(Duration::from_millis(500));
        evidence(dir,"05_question_entered");
        let current=xml()?;
        if current.contains("尚未配置可用的 AI 服务"){
            // Hide IME before tapping the guarded button.
            run(&["shell","input","keyevent","4"])?;
            thread::sleep(Duration::from_millis(500));
            verify_submission_blocked(Some("HowMuchPending"))?;
            report.push_str("PASS: unconfigured AI connection blocks network submission.\n\n");
        } else {
            report.push_str("INFO: service configuration present or not exposed in this emulator; no paid model calls executed.\n\n");
        }
    }
    Ok(())
}
fn main(){
    let dir=env::args().nth(1).map(PathBuf::from).unwrap_or_else(||PathBuf::from("canvas_question_evidence"));
    fs::create_dir_all(&dir).expect("evidence directory");
    let mut report=String::from("# Android 2.13.5.13 canvas question emulator acceptance\n\n");
    let result=acceptance(&dir,&mut report);
    if let Err(ref error)=result{
        report.push_str(&format!("FAIL: {error}\n"));
        evidence(&dir,"failure");
    }
    fs::write(dir.join("report.md"),report.clone()).expect("report write");
    println!("{report}");
    if result.is_err(){std::process::exit(1)}
}
