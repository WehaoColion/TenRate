// v2.13.5.13 - Read-only, provenance-bearing canvas facts for AI questions.
// Customer money is computed locally in cents; nothing here edits the board or risk ledger.
use super::*;
use std::collections::{HashMap, HashSet, VecDeque};

fn short(value: &str, limit: usize) -> String {
    value.chars().take(limit).collect()
}

fn label(node: &Node) -> String {
    short(node.text.lines().next().unwrap_or("").trim(), 80)
}

fn selected_ids(canvas: &Canvas, scope: &str, focus: &str) -> Result<HashSet<String>, String> {
    if scope == "all" {
        return Ok(canvas.nodes.iter().map(|n| n.id.clone()).collect());
    }
    if !matches!(scope, "card" | "network") {
        return Err("未知的画布提问范围".into());
    }
    if !canvas.nodes.iter().any(|n| n.id == focus) {
        return Err("选中的卡片已不存在".into());
    }
    let mut selected = HashSet::from([focus.to_owned()]);
    if scope == "network" {
        let mut queue = VecDeque::from([focus.to_owned()]);
        while let Some(id) = queue.pop_front() {
            for edge in canvas.edges.iter() {
                let neighbor = if edge.from == id.as_str() {
                    Some(&edge.to)
                } else if edge.to == id.as_str() {
                    Some(&edge.from)
                } else {
                    None
                };
                if let Some(next) = neighbor {
                    if selected.insert(next.clone()) {
                        queue.push_back(next.clone());
                    }
                }
            }
        }
    }
    Ok(selected)
}

pub(super) fn build(
    canvas: &Canvas, revision: u64, question: &str, scope: &str, focus: &str,
) -> Result<Value, String> {
    if question.trim().is_empty() || question.chars().count() > 1000 {
        return Err("问题不能为空或超过 1000 字".into());
    }
    let selected = selected_ids(canvas, scope, focus)?;
    let client_ids: HashSet<&str> = canvas.nodes.iter()
        .filter(|n| n.kind == "client").map(|n| n.id.as_str()).collect();
    let names: HashMap<&str, String> = canvas.nodes.iter()
        .map(|n| (n.id.as_str(), label(n))).collect();
    let mut total_paid = 0_i64;
    let mut total_pending = 0_i64;
    let mut due_count = 0_usize;
    let mut client_count = 0_usize;
    for node in canvas.nodes.iter().filter(|n| selected.contains(&n.id)) {
        if node.kind != "client" { continue; }
        client_count += 1;
        let (paid, pending) = money::totals(node);
        total_paid = total_paid.checked_add(paid).ok_or("画布金额超出范围")?;
        total_pending = total_pending.checked_add(pending).ok_or("画布金额超出范围")?;
        if pending > 0 { due_count += 1; }
    }
    let referral_edges: Vec<&Edge> = canvas.edges.iter()
        .filter(|e| selected.contains(&e.from) && selected.contains(&e.to)
            && is_referral(e, &client_ids))
        .collect();
    let link_edges: Vec<&Edge> = canvas.edges.iter()
        .filter(|e| selected.contains(&e.from) && selected.contains(&e.to)
            && !is_referral(e, &client_ids))
        .collect();

    // Ranking selects illustrative evidence, not the population used in exact totals.
    let payment_question = ["待付", "待收", "欠", "收款", "付款", "金额", "收入"]
        .iter().any(|word| question.contains(word));
    let referral_question = ["推荐", "关系", "下级", "上级", "网络", "谁"]
        .iter().any(|word| question.contains(word));
    let mut ordered: Vec<&Node> = canvas.nodes.iter()
        .filter(|n| selected.contains(&n.id)).collect();
    ordered.sort_by_key(|node| {
        let (paid, due) = if node.kind == "client" { money::totals(node) } else { (0, 0) };
        let named = !label(node).is_empty() && question.contains(label(node).as_str());
        let related = referral_edges.iter().any(|e| e.from == node.id || e.to == node.id);
        let score = (if named { 1_000_000_i64 } else { 0 })
            + (if payment_question && due > 0 { 100_000 } else { 0 })
            + (if referral_question && related { 10_000 } else { 0 })
            + (if payment_question && paid > 0 { 1_000 } else { 0 });
        std::cmp::Reverse((score, if payment_question { due } else { 0 }))
    });
    const MAX_SOURCES: usize = 20;
    let truncated = ordered.len() > MAX_SOURCES;
    let sources: Vec<Value> = ordered.into_iter().take(MAX_SOURCES).map(|node| {
        let title = if node.kind == "page" {
            "知识页面（正文在设备端权限检查后补充）".to_owned()
        } else { label(node) };
        let mut excerpt = String::new();
        if node.kind == "client" {
            let (paid, pending) = money::totals(node);
            excerpt.push_str(&format!("已支付：{paid} 分；待支付：{pending} 分。"));
            if !node.charges.is_empty() {
                excerpt.push_str("款项及收款明细：");
                for charge in node.charges.iter().take(8) {
                    let received = money::charge_paid(charge);
                    excerpt.push_str(&format!("{}：总额{}分，已收{}分，剩余{}分；",
                        short(&charge.title, 30), charge.total_cents, received,
                        charge.total_cents - received));
                }
            }
        }
        if node.kind != "page" {
            excerpt.push_str(&short(node.text.trim(), 900));
        }
        let mut edges = Vec::new();
        for edge in canvas.edges.iter()
            .filter(|e| selected.contains(&e.from) && selected.contains(&e.to)
                && (e.from == node.id || e.to == node.id)).take(12)
        {
            edges.push(format!("{} {} {}",
                names.get(edge.from.as_str()).map(String::as_str).unwrap_or("未命名"),
                if is_referral(edge, &client_ids) { "推荐→" } else { "连线→" },
                names.get(edge.to.as_str()).map(String::as_str).unwrap_or("未命名")));
        }
        if !edges.is_empty() { excerpt.push_str("\n关系："); excerpt.push_str(&edges.join("；")); }
        json!({
            "nodeId":node.id, "pageId":node.page_id, "kind":node.kind,
            "title": title, "excerpt": short(&excerpt, 1350)
        })
    }).collect();

    // Exact census and totals are always computed over the entire requested scope.
    // An LLM can explain them, but must not recompute money from truncated examples.
    let mut facts = format!(
        "此处为本地 Rust 计算的权威统计，金额单位为人民币分，不得更改或自行推算：范围卡片{}张，客户{}名，已支付合计{}分，待支付合计{}分，仍有待付款客户{}名，推荐关系{}条，普通连线{}条。 ",
        selected.len(), client_count, total_paid, total_pending, due_count,
        referral_edges.len(), link_edges.len()
    );
    if payment_question {
        facts.push_str("待付款客户及对应金额（按金额降序；仅列示前 20 名）：");
        let mut dues: Vec<(&str, i64)> = canvas.nodes.iter()
            .filter(|n| selected.contains(&n.id) && n.kind == "client")
            .map(|n| (n.id.as_str(), money::totals(n).1))
            .filter(|(_, due)| *due > 0).collect();
        dues.sort_by(|a, b| b.1.cmp(&a.1));
        for (id, due) in dues.iter().take(20) {
            facts.push_str(&format!("{} {}分；",
                names.get(id).map(String::as_str).unwrap_or("未命名"), due));
        }
        if dues.len() > 20 { facts.push_str("其余客户未逐个列出，合计仍包含全部客户。"); }
    }
    if referral_question {
        facts.push_str("推荐边方向为推荐人→被推荐人。部分关系：");
        for edge in referral_edges.iter().take(20) {
            facts.push_str(&format!("{}→{}；",
                names.get(edge.from.as_str()).map(String::as_str).unwrap_or("未命名"),
                names.get(edge.to.as_str()).map(String::as_str).unwrap_or("未命名")));
        }
        if referral_edges.len() > 20 { facts.push_str("还有其他关系未展开。"); }
    }
    Ok(json!({
        "ok":true, "canvasId":canvas.id, "revision":revision, "scope":scope,
        "nodeCount":selected.len(), "clientCount":client_count,
        "totalPaidCents":total_paid, "totalPendingCents":total_pending,
        "pendingClientCount":due_count,
        "summary":short(&facts, 4000), "sources":sources, "truncated":truncated
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn client(id: &str, paid: i64, due: i64) -> Node {
        Node { id:id.into(), kind:"client".into(), page_id:String::new(),
            text:format!("{}\n备注",id), x:0.0, y:0.0, width:192.0, height:132.0,
            color:"blue".into(), paid_cents:Some(paid), pending_cents:Some(due),
            charges:vec![] }
    }
    #[test]
    fn scope_and_exact_money_do_not_mutate_original_canvas() {
        let mut board = Canvas::new("客户网络");
        board.nodes = vec![client("甲", 56000, 66000), client("乙", 44000, 66000),
            client("丙", 150000, 0)].into();
        board.edges = vec![Edge {id:"e".into(), from:"甲".into(), to:"乙".into(),
            relation:Some("referral".into())}].into();
        let unchanged = board.clone();
        let all = build(&board, 7, "哪些客户待收款", "all", "").unwrap();
        assert_eq!(all["totalPendingCents"], 132000);
        assert_eq!(all["totalPaidCents"], 250000);
        assert_eq!(all["pendingClientCount"], 2);
        let network = build(&board, 7, "推荐和待收", "network", "甲").unwrap();
        assert_eq!(network["nodeCount"], 2);
        assert_eq!(network["totalPendingCents"], 132000);
        let card = build(&board, 7, "金额", "card", "丙").unwrap();
        assert_eq!(card["totalPaidCents"], 150000);
        assert_eq!(board, unchanged);
    }
    #[test]
    fn invalid_focus_is_rejected_and_large_boards_are_bounded() {
        let mut board = Canvas::new("画布");
        board.nodes = (0..60).map(|n| client(&format!("客户{n}"), 0, 100)).collect::<Vec<_>>().into();
        assert!(build(&board, 0, "问题", "card", "missing").is_err());
        let all = build(&board, 0, "哪些客户待收", "all", "").unwrap();
        assert_eq!(all["pendingClientCount"], 60);
        assert_eq!(all["sources"].as_array().unwrap().len(), 20);
        assert_eq!(all["truncated"], true);
    }
}
