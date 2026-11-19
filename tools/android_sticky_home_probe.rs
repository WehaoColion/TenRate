// v3.0.2.3 - Exercise real long-note home layout with Chinese and English UI.
use std::{env,fs,path::{Path,PathBuf},process::{Command,Output},thread,time::Duration};

const APP:&str="com.ofairyo.gridtimer";
const VERSION:&str="3.0.2.3";
const TITLE:&str="StickyLayoutProbe";
#[derive(Clone)]
struct Node{text:String,desc:String,class:String,bounds:[i32;4]}
fn adb(args:&[&str])->Result<Output,String>{
    Command::new("adb").args(args).output().map_err(|e|e.to_string())
}
fn run(args:&[&str])->Result<String,String>{
    let o=adb(args)?;
    if !o.status.success(){return Err(format!("adb {:?}: {}",args,String::from_utf8_lossy(&o.stderr)))}
    Ok(String::from_utf8_lossy(&o.stdout).into_owned())
}
fn attr(s:&str,key:&str)->String{
    let marker=format!("{key}=\"");
    s.find(&marker).and_then(|i|{
        let rest=&s[i+marker.len()..];
        rest.find('"').map(|stop|rest[..stop].to_owned())
    }).unwrap_or_default()
}
fn nodes(xml:&str)->Vec<Node>{
    xml.split("<node ").skip(1).filter_map(|entry|{
        let head=entry.split('>').next()?;
        let values:Vec<i32>=attr(head,"bounds").split(|c:char|!c.is_ascii_digit()&&c!='-')
            .filter(|v|!v.is_empty()).filter_map(|v|v.parse().ok()).collect();
        if values.len()!=4{return None}
        Some(Node{text:attr(head,"text"),desc:attr(head,"content-desc"),
            class:attr(head,"class"),bounds:[values[0],values[1],values[2],values[3]]})
    }).collect()
}
fn hierarchy()->Result<String,String>{
    for _ in 0..6{
        let _=run(&["shell","uiautomator","dump","/sdcard/sticky_summary.xml"]);
        if let Ok(xml)=run(&["exec-out","cat","/sdcard/sticky_summary.xml"]){
            if xml.contains("<hierarchy"){return Ok(xml)}
        }
        thread::sleep(Duration::from_millis(550));
    }
    Err("Android UI hierarchy unavailable".into())
}
fn find(labels:&[&str])->Result<Option<Node>,String>{
    let list=nodes(&hierarchy()?);
    for exact in [true,false]{
        for name in labels{
            if let Some(n)=list.iter().find(|n|{
                n.bounds[2]>n.bounds[0]&&n.bounds[3]>n.bounds[1]&&
                if exact{n.text==*name||n.desc==*name}
                else{n.text.contains(name)||n.desc.contains(name)}
            }) {return Ok(Some(n.clone()))}
        }
    }
    Ok(None)
}
fn tap(n:&Node)->Result<(),String>{
    run(&["shell","input","tap",&((n.bounds[0]+n.bounds[2])/2).to_string(),
        &((n.bounds[1]+n.bounds[3])/2).to_string()])?;
    thread::sleep(Duration::from_millis(800));
    Ok(())
}
fn tap_any(names:&[&str])->Result<bool,String>{
    if let Some(n)=find(names)?{tap(&n)?;Ok(true)}else{Ok(false)}
}
fn capture(dir:&Path,name:&str){
    if let Ok(xml)=hierarchy(){let _=fs::write(dir.join(format!("{name}.xml")),xml);}
    if let Ok(o)=adb(&["exec-out","screencap","-p"]){
        if o.status.success()&&o.stdout.starts_with(b"\x89PNG"){
            let _=fs::write(dir.join(format!("{name}.png")),o.stdout);
        }
    }
}
fn dismiss()->Result<(),String>{
    for _ in 0..3{
        let xml=hierarchy()?;
        if xml.contains("Allow tenfold to send you notifications"){
            if tap_any(&["Allow","允许"])?{continue}
        }
        if xml.contains("Pixel Launcher")&&tap_any(&["Close app"])?{continue}
        break;
    }
    Ok(())
}
fn home(dir:&Path)->Result<(),String>{
    for pass in 0..9{
        dismiss()?;
        if find(&["最近便签","Recent notes"])?.is_some()&&find(&["回收站","Trash"])?.is_some(){return Ok(())}
        if pass==0{capture(dir,"01_launched");}
        if tap_any(&["便签","Quick notes","Sticky notes"])?{
            capture(dir,&format!("02_notes_opened_{pass}"));
            continue
        }
        thread::sleep(Duration::from_millis(900));
    }
    capture(dir,"02_home_not_found");
    Err("Could not navigate to the real sticky-note home screen".into())
}
fn visible_buttons(dir:&Path,label:&str)->Result<(),String>{
    let title=find(&["最近便签","Recent notes"])?.ok_or("Recent note header not visible")?;
    let create=find(&["新建","New"])?.ok_or("Create action not visible")?;
    let folder=find(&["文件夹","Folders"])?.ok_or("Folder action not visible")?;
    let trash=find(&["回收站","Trash"])?.ok_or("Trash action not visible")?;
    let row_top=create.bounds[1].min(folder.bounds[1]).min(trash.bounds[1]);
    let row_bottom=create.bounds[3].max(folder.bounds[3]).max(trash.bounds[3]);
    let title_bottom=title.bounds[3];
    if row_top<=title_bottom {
        return Err(format!("{label}: actions overlap header or lie above the card content"));
    }
    let display_height=run(&["shell","wm","size"])?.split_whitespace().last()
        .and_then(|x|x.split('x').nth(1))
        .and_then(|x|x.trim().parse::<i32>().ok()).unwrap_or(2400);
    if row_bottom-title.bounds[1]>display_height/3 {
        return Err(format!("{label}: summary grew beyond one third of screen: {} px",
            row_bottom-title.bounds[1]));
    }
    capture(dir,label);
    Ok(())
}
fn acceptance(dir:&Path)->Result<String,String>{
    run(&["wait-for-device"])?;
    let package=run(&["shell","dumpsys","package",APP])?;
    if !package.contains(&format!("versionName={VERSION}")){
        return Err("Installed APK version does not match source commit".into());
    }
    let _=run(&["shell","cmd","locale","set-app-locales",APP,"--locales","zh-CN"]);
    run(&["shell","am","start","-W","-n","com.ofairyo.gridtimer/.MainActivity"])?;
    thread::sleep(Duration::from_secs(7));
    home(dir)?;
    visible_buttons(dir,"02_before_long_note")?;
    if !tap_any(&["新建","New"])?{return Err("Cannot create a sticky note from home".into())}
    let mut fields=Vec::new();
    for _ in 0..8{
        fields=nodes(&hierarchy()?).into_iter().filter(|n|
            n.class.ends_with("EditText")&&n.bounds[2]>n.bounds[0]
            &&n.bounds[3]>n.bounds[1]).collect();
        if fields.len()>=2{break}
        thread::sleep(Duration::from_millis(700));
    }
    if fields.len()<2{
        capture(dir,"03_editor_missing");
        return Err("Actual note editor did not expose both title and body fields".into());
    }
    fields.sort_by_key(|n|n.bounds[1]);
    tap(&fields[0])?;
    run(&["shell","input","text",TITLE])?;
    let mut body=None;
    for _ in 0..5{
        let list=nodes(&hierarchy()?).into_iter().filter(|n|
            n.class.ends_with("EditText")&&n.bounds[2]>n.bounds[0]
            &&n.bounds[3]>n.bounds[1]).collect::<Vec<_>>();
        body=list.into_iter().max_by_key(|n|n.bounds[1]);
        if body.is_some(){break}
        thread::sleep(Duration::from_millis(350));
    }
    let body=body.ok_or("Note body input missing after naming the note")?;
    tap(&body)?;
    for _ in 0..12{
        run(&["shell","input","text","LongPreviewABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789"])?;
    }
    capture(dir,"03_long_note_in_editor");
    run(&["shell","input","keyevent","4"])?;
    thread::sleep(Duration::from_millis(500));
    if !tap_any(&["完成","Done"])?{
        capture(dir,"04_save_missing");
        return Err("Note editor has no accessible finish/save button".into());
    }
    for _ in 0..24{
        if find(&["最近便签","Recent notes"])?.is_some() && find(&["新建","New"])?.is_some() {break}
        thread::sleep(Duration::from_secs(1));
    }
    home(dir)?;
    visible_buttons(dir,"04_long_note_home")?;
    let current=hierarchy()?;
    if !current.contains("LongPreview") {
        capture(dir,"05_preview_missing");
        return Err("Newly saved long note is not the home summary".into());
    }
    let search=find(&["搜索便签","Search notes","搜索","Search","标题、正文、文件夹都能搜","Search titles, body text, and folders"])?;
    if search.is_none(){
        capture(dir,"05_search_offscreen");
        return Err("Search was pushed out of the first viewport by summary".into());
    }
    capture(dir,"05_search_visible");
    let _=run(&["shell","settings","put","system","font_scale","1.3"]);
    thread::sleep(Duration::from_secs(2));
    home(dir)?;
    visible_buttons(dir,"06_large_font_summary")?;
    Ok(format!("PASS: signed Android {VERSION} launched, created a long real sticky note, saved it, and kept header, actions and search accessible; 1.3 font scale captured."))
}
fn main(){
    let dir=env::args_os().nth(1).map(PathBuf::from).unwrap_or_else(||PathBuf::from("sticky_summary_evidence"));
    fs::create_dir_all(&dir).expect("evidence directory");
    let result=acceptance(&dir);
    if result.is_err(){capture(&dir,"99_failure");}
    let report=match &result{
        Ok(message)=>format!("# Android {VERSION} sticky-summary emulator acceptance\n\n{message}\n\nOverall: PASS\n"),
        Err(error)=>format!("# Android {VERSION} sticky-summary emulator acceptance\n\n{error}\n\nOverall: FAILED / UNVERIFIED\n"),
    };
    fs::write(dir.join("report.md"),&report).expect("report write");
    println!("{report}");
    if result.is_err(){std::process::exit(1)}
}
