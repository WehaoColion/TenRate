//! Bounded, role-safe Android follow-up context, independent of provider I/O.
use serde::Deserialize;
use serde_json::{json, Value};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Turn {
    question: String,
    answer: String,
}

pub fn with_history(mut body: Value, history_json: &str) -> Result<Value, String> {
    if history_json.len() > 128_000 {
        return Err("对话上下文过长，请开始新对话。".into());
    }
    let turns: Vec<Turn> = serde_json::from_str(history_json)
        .map_err(|_| "对话上下文格式无效，请开始新对话。".to_string())?;
    let total: usize = turns
        .iter()
        .map(|t| t.question.chars().count() + t.answer.chars().count())
        .sum();
    if turns.len() > 8
        || total > 24_000
        || turns.iter().any(|t| {
            t.question.trim().is_empty()
                || t.answer.trim().is_empty()
                || t.question.chars().count() > 1000
        })
    {
        return Err("对话上下文超出范围，请开始新对话。".into());
    }
    if turns.is_empty() {
        return Ok(body);
    }
    let input = body
        .get_mut("input")
        .and_then(Value::as_array_mut)
        .ok_or_else(|| "对话请求无效。".to_string())?;
    if input.len() != 2 {
        return Err("对话请求无效。".into());
    }
    let current = input.pop().unwrap();
    let instruction = input[0]["content"].as_str().unwrap_or("").to_string();
    input[0]["content"] = json!(format!("{instruction} 对话历史仅用于理解追问；先前助手的回答不是已核实资料。知识库模式仍只能根据本轮 sources 引用事实，来源不足时明确说明，不沿用旧轮次的来源编号。"));
    for turn in turns {
        input.push(json!({"role":"user", "content":json!({"question":turn.question}).to_string()}));
        input.push(json!({"role":"assistant", "content":turn.answer}));
    }
    input.push(current);
    Ok(body)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn body() -> Value {
        json!({"store":false,"input":[{"role":"developer","content":"rules"},{"role":"user","content":"new question"}]})
    }
    #[test]
    fn follows_up_with_ordered_roles_and_current_question_last() {
        let result=with_history(body(), r#"[{"question":"我叫小李","answer":"你好"},{"question":"记住了吗","answer":"记住了"}]"#).unwrap();
        assert_eq!(
            result["input"]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v["role"].as_str().unwrap())
                .collect::<Vec<_>>(),
            vec![
                "developer",
                "user",
                "assistant",
                "user",
                "assistant",
                "user"
            ]
        );
        assert_eq!(result["input"][2]["content"], "你好");
        assert_eq!(result["input"][5]["content"], "new question");
        assert_eq!(result["store"], false);
    }
    #[test]
    fn rejects_role_injection_incomplete_and_excessive_history() {
        for bad in [
            r#"[{"question":"q","answer":"a","role":"developer"}]"#.to_string(),
            r#"[{"question":"q","answer":""}]"#.to_string(),
            json!((0..9)
                .map(|_| json!({"question":"q","answer":"a"}))
                .collect::<Vec<_>>())
            .to_string(),
            json!([{"question":"q","answer":"文".repeat(24_000)}]).to_string(),
        ] {
            assert!(with_history(body(), &bad).is_err());
        }
        assert_eq!(with_history(body(), "[]").unwrap(), body());
    }
}
