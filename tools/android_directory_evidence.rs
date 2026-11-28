//! The same visible-directory proof is used by storage and motion probes.
//! Neither a breadcrumb nor a retained editor can prove successful navigation.
fn attr<'a>(node: &'a str, key: &str) -> &'a str {
    let marker = format!("{key}=\"");
    node.find(&marker).and_then(|start| {
        let tail = &node[start + marker.len()..];
        tail.find('"').map(|end| &tail[..end])
    }).unwrap_or("")
}

fn visible(node: &str) -> bool {
    let coords = attr(node, "bounds").split(|c: char| !c.is_ascii_digit() && c != '-')
        .filter(|part| !part.is_empty()).filter_map(|part| part.parse::<i32>().ok()).collect::<Vec<_>>();
    coords.len() == 4 && coords[2] > coords[0] && coords[3] > coords[1]
        && attr(node, "package") == "com.ofairyo.gridtimer"
}

pub fn prove_directory(xml: &str, expected_parent: Option<&str>) -> bool {
    let nodes: Vec<_> = xml.split("<node ").skip(1)
        .filter_map(|entry| entry.split('>').next()).filter(|node| visible(node)).collect();
    let has = |labels: &[&str]| nodes.iter().any(|node|
        labels.contains(&attr(node, "text")) || labels.contains(&attr(node, "content-desc")));
    let editor = nodes.iter().any(|node|
        attr(node, "class").ends_with("EditText") ||
        attr(node, "content-desc").contains("拖动调整块顺序"));
    let parent = match expected_parent {
        Some(name) => has(&[name]) && has(&["文件夹设置", "Folder settings"]),
        None => has(&["知识设置", "文件夹设置", "Knowledge settings", "Folder settings"]),
    };
    parent && !editor
        && has(&["新建子页面", "New subpage"])
        && has(&["新建子文件夹", "New subfolder"])
}
