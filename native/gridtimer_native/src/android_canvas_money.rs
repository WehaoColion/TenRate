// v2.13.5.7 - 旧客户已付与待付金额可原子整理为项目明细，禁止重复累计。
// v2.13.5.6 - 客户款项与逐笔收款独立于风控账本，金额以整数分存储。
use super::*;

const MAX_CHARGES: usize = 60;
const MAX_RECEIPTS: usize = 120;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ClientReceipt {
    pub id: String,
    pub amount_cents: i64,
    pub received_on: String,
    pub location: String,
    pub channel: String,
    pub note: String,
    #[serde(default, skip_serializing_if = "historical_false")]
    pub historical: bool,
}
fn historical_false(value: &bool) -> bool { !*value }

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ClientCharge {
    pub id: String,
    pub title: String,
    pub total_cents: i64,
    pub service_location: String,
    pub due_on: String,
    pub note: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub receipts: Vec<ClientReceipt>,
}

fn bounded_text(request: &Value, key: &str, limit: usize) -> Result<String, String> {
    let raw = text(request, key)?.trim();
    if raw.chars().count() > limit || raw.chars().any(|c| c.is_control()) {
        return Err(format!("{key} 内容超过允许范围"));
    }
    Ok(raw.to_owned())
}

fn positive_amount(request: &Value, key: &str) -> Result<i64, String> {
    client_money_yuan(text(request, key)?)?
        .filter(|v| *v > 0)
        .ok_or_else(|| format!("{key} 必须是大于零的人民币金额"))
}

fn valid_date(value: &str) -> bool {
    if value.is_empty() {
        return true;
    }
    if value.len() != 10 {
        return false;
    }
    let bytes = value.as_bytes();
    if bytes[4] != b'-' || bytes[7] != b'-'
        || bytes.iter().enumerate().any(|(i, b)| i != 4 && i != 7 && !b.is_ascii_digit())
    {
        return false;
    }
    let year: usize = value[0..4].parse().unwrap_or(0);
    let month: usize = value[5..7].parse().unwrap_or(0);
    let day: usize = value[8..10].parse().unwrap_or(0);
    if !(1900..=2100).contains(&year) || !(1..=12).contains(&month) {
        return false;
    }
    let leap = year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
    let days = [31, if leap { 29 } else { 28 }, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
    (1..=days[month - 1]).contains(&day)
}

pub(super) fn charge_paid(charge: &ClientCharge) -> i64 {
    charge.receipts.iter().map(|p| p.amount_cents).sum()
}

pub(super) fn totals(node: &Node) -> (i64, i64) {
    let mut paid = node.paid_cents.unwrap_or(0);
    let mut pending = node.pending_cents.unwrap_or(0);
    for charge in &node.charges {
        let received = charge_paid(charge);
        paid += received;
        pending += charge.total_cents - received;
    }
    (paid, pending)
}

pub(super) fn validate(node: &Node) -> Result<(), String> {
    if node.kind != "client" {
        return if node.charges.is_empty() { Ok(()) } else { Err("非客户卡片不能记录款项".into()) };
    }
    if node.charges.len() > MAX_CHARGES {
        return Err("单个客户款项项目过多".into());
    }
    let mut ids = HashSet::new();
    let mut paid = node.paid_cents.unwrap_or(0);
    let mut pending = node.pending_cents.unwrap_or(0);
    for charge in &node.charges {
        if !valid_id(&charge.id) || !ids.insert(charge.id.as_str())
            || charge.title.trim().is_empty() || charge.title.chars().count() > 80
            || charge.service_location.chars().count() > 100
            || charge.note.chars().count() > 500
            || !(1..=MAX_CLIENT_MONEY_CENTS).contains(&charge.total_cents)
            || !valid_date(&charge.due_on)
            || charge.receipts.len() > MAX_RECEIPTS
        {
            return Err("客户款项项目无效".into());
        }
        let mut received = 0_i64;
        for receipt in &charge.receipts {
            if !valid_id(&receipt.id) || !ids.insert(receipt.id.as_str())
                || !(1..=MAX_CLIENT_MONEY_CENTS).contains(&receipt.amount_cents)
                || !valid_date(&receipt.received_on)
                || (receipt.received_on.is_empty() && !receipt.historical)
                || receipt.location.chars().count() > 100
                || receipt.channel.chars().count() > 40
                || receipt.note.chars().count() > 500
            {
                return Err("客户收款记录无效".into());
            }
            received = received.checked_add(receipt.amount_cents).ok_or("收款金额累计溢出")?;
        }
        if received > charge.total_cents {
            return Err("实收金额不能超过项目应付金额".into());
        }
        paid = paid.checked_add(received).ok_or("收款累计金额溢出")?;
        pending = pending.checked_add(charge.total_cents - received).ok_or("待付款累计金额溢出")?;
    }
    if paid > MAX_CLIENT_MONEY_CENTS || pending > MAX_CLIENT_MONEY_CENTS {
        return Err("客户汇总金额超过允许范围".into());
    }
    Ok(())
}

pub(super) fn edit(canvas: &mut Canvas, request: &Value) -> Result<(), String> {
    let op = text(request, "op")?;
    let node = canvas.nodes.iter_mut()
        .find(|n| n.kind == "client" && n.id == text(request, "id").unwrap_or(""))
        .ok_or("客户卡片不存在")?;
    match op {
        "importClientHistory" => {
            let charge_id = bounded_text(request, "chargeId", 256)?;
            if !valid_id(&charge_id) { return Err("款项标识不能为空".into()); }
            if node.charges.iter().any(|c| c.id == charge_id) { return Ok(()); }
            if node.charges.len() >= MAX_CHARGES { return Err("该客户款项项目已达上限".into()); }
            let previously_paid = node.paid_cents.unwrap_or(0);
            let previously_due = node.pending_cents.unwrap_or(0);
            let total = previously_paid.checked_add(previously_due)
                .ok_or("历史汇总金额溢出")?;
            if total <= 0 || total > MAX_CLIENT_MONEY_CENTS {
                return Err("没有可整理的历史金额".into());
            }
            let title = bounded_text(request, "title", 80)?;
            if title.is_empty() { return Err("请填写历史款项名称".into()); }
            let due_on = bounded_text(request, "dueOn", 10)?;
            if !valid_date(&due_on) { return Err("预计付款日期无效".into()); }
            let receipts = if previously_paid > 0 {
                let receipt_id = bounded_text(request, "receiptId", 256)?;
                if !valid_id(&receipt_id) { return Err("收款标识不能为空".into()); }
                let received_on = bounded_text(request, "receivedOn", 10)?;
                if !valid_date(&received_on) { return Err("实际收款日期无效".into()); }
                vec![ClientReceipt {
                    id: receipt_id,
                    amount_cents: previously_paid,
                    received_on,
                    location: bounded_text(request, "location", 100)?,
                    channel: bounded_text(request, "channel", 40)?,
                    note: bounded_text(request, "receiptNote", 500)?,
                    historical: true,
                }]
            } else { Vec::new() };
            node.charges.push(ClientCharge {
                id: charge_id,
                title,
                total_cents: total,
                service_location: bounded_text(request, "serviceLocation", 100)?,
                due_on,
                note: bounded_text(request, "note", 500)?,
                receipts,
            });
            // This is a reclassification, never a second charge. Undo restores the original summary.
            node.paid_cents = None;
            node.pending_cents = None;
        }
        "addClientCharge" => {
            let charge_id = bounded_text(request, "chargeId", 256)?;
            if !valid_id(&charge_id) {
                return Err("款项标识不能为空".into());
            }
            if node.charges.iter().any(|c| c.id == charge_id) {
                return Ok(()); // Replaying the same saved request is idempotent.
            }
            if node.charges.len() >= MAX_CHARGES {
                return Err("该客户款项项目已达上限".into());
            }
            let title = bounded_text(request, "title", 80)?;
            if title.is_empty() { return Err("请填写款项名称".into()); }
            let due_on = bounded_text(request, "dueOn", 10)?;
            if !valid_date(&due_on) { return Err("预计收款日期无效".into()); }
            node.charges.push(ClientCharge {
                id: charge_id, title,
                total_cents: positive_amount(request, "totalYuan")?,
                service_location: bounded_text(request, "serviceLocation", 100)?,
                due_on,
                note: bounded_text(request, "note", 500)?,
                receipts: Vec::new(),
            });
        }
        "updateClientCharge" | "removeClientCharge" | "addClientReceipt" | "updateClientReceipt" | "removeClientReceipt" => {
            let charge_id = text(request, "chargeId")?;
            let charge = node.charges.iter_mut()
                .find(|c| c.id == charge_id)
                .ok_or("款项项目不存在")?;
            match op {
                "updateClientCharge" => {
                    let title = bounded_text(request, "title", 80)?;
                    if title.is_empty() { return Err("请填写款项名称".into()); }
                    let due_on = bounded_text(request, "dueOn", 10)?;
                    if !valid_date(&due_on) { return Err("预计收款日期无效".into()); }
                    let total = positive_amount(request, "totalYuan")?;
                    if total < charge_paid(charge) {
                        return Err("项目总额不能低于已经收到的金额".into());
                    }
                    charge.title = title;
                    charge.total_cents = total;
                    charge.due_on = due_on;
                    charge.service_location = bounded_text(request, "serviceLocation", 100)?;
                    charge.note = bounded_text(request, "note", 500)?;
                }
                "removeClientCharge" => {
                    if !charge.receipts.is_empty() {
                        return Err("请先核对和移除该项目的收款记录".into());
                    }
                    node.charges.retain(|c| c.id != charge_id);
                }
                "addClientReceipt" => {
                    let receipt_id = bounded_text(request, "receiptId", 256)?;
                    if !valid_id(&receipt_id) { return Err("收款标识不能为空".into()); }
                    if charge.receipts.iter().any(|p| p.id == receipt_id) {
                        return Ok(()); // Prevent double-crediting a retried save.
                    }
                    if charge.receipts.len() >= MAX_RECEIPTS {
                        return Err("该项目收款笔数已达上限".into());
                    }
                    let received_on = bounded_text(request, "receivedOn", 10)?;
                    if received_on.is_empty() || !valid_date(&received_on) {
                        return Err("请选择有效的实际收款日期".into());
                    }
                    charge.receipts.push(ClientReceipt {
                        id: receipt_id,
                        amount_cents: positive_amount(request, "amountYuan")?,
                        received_on,
                        location: bounded_text(request, "location", 100)?,
                        channel: bounded_text(request, "channel", 40)?,
                        note: bounded_text(request, "note", 500)?,
                        historical: false,
                    });
                }
                "updateClientReceipt" => {
                    let receipt = charge.receipts.iter_mut()
                        .find(|p| p.id == text(request, "receiptId").unwrap_or(""))
                        .ok_or("收款记录不存在")?;
                    let received_on = bounded_text(request, "receivedOn", 10)?;
                    if !valid_date(&received_on) || (received_on.is_empty() && !receipt.historical) {
                        return Err("请选择有效的实际收款日期".into());
                    }
                    receipt.amount_cents = positive_amount(request, "amountYuan")?;
                    receipt.received_on = received_on;
                    receipt.location = bounded_text(request, "location", 100)?;
                    receipt.channel = bounded_text(request, "channel", 40)?;
                    receipt.note = bounded_text(request, "note", 500)?;
                }
                "removeClientReceipt" => {
                    let receipt_id = text(request, "receiptId")?;
                    if !charge.receipts.iter().any(|p| p.id == receipt_id) {
                        return Err("收款记录不存在".into());
                    }
                    charge.receipts.retain(|p| p.id != receipt_id);
                }
                _ => unreachable!(),
            }
        }
        _ => return Err("不支持的客户收付款操作".into()),
    }
    validate(node)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn leap_day_and_amount_validation() {
        assert!(valid_date("2024-02-29"));
        assert!(!valid_date("2025-02-29"));
        assert!(!valid_date("2026-13-01"));
        assert!(!valid_date("2026-02-31"));
        assert_eq!(client_money_yuan("0.01").unwrap(), Some(1));
    }
}
