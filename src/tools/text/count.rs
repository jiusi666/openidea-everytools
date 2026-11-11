//! 中文字数统计：字符、汉字、英文单词、行数、段落、阅读时长。

use crate::tools::meta::{Group, ToolMeta};
use crate::tools::util::is_cjk;
use crate::tools::{RunCtx, Tool, ToolResult};

pub struct Count;

impl Tool for Count {
    fn meta(&self) -> ToolMeta {
        ToolMeta::new(
            "text-count",
            "中文字数统计",
            Group::Text,
            "统计字符、汉字、英文单词、标点、行数、段落和阅读时长",
            "把要统计的文案粘进来",
            true,
            "text",
        )
    }

    fn run(&self, input: &str, _ctx: &RunCtx) -> Result<ToolResult, String> {
        Ok(ToolResult::new().section("统计结果", "text", statistics(input)))
    }
}

fn is_cn_punct(c: char) -> bool {
    matches!(
        c,
        '。' | '，'
            | '、'
            | '；'
            | '：'
            | '？'
            | '！'
            | '“'
            | '”'
            | '‘'
            | '’'
            | '（'
            | '）'
            | '《'
            | '》'
            | '【'
            | '】'
            | '—'
            | '…'
            | '·'
    )
}

pub fn statistics(input: &str) -> String {
    let mut cjk = 0usize;
    let mut words = 0usize;
    let mut digits = 0usize;
    let mut punct = 0usize;
    let mut spaces = 0usize;
    let mut other = 0usize;
    let mut in_word = false;
    for c in input.chars() {
        if is_cjk(c) {
            cjk += 1;
            in_word = false;
        } else if c.is_ascii_alphabetic() {
            if !in_word {
                words += 1;
                in_word = true;
            }
        } else if c.is_ascii_digit() {
            digits += 1;
            in_word = false;
        } else if c.is_whitespace() {
            spaces += 1;
            in_word = false;
        } else if c.is_ascii_punctuation() || is_cn_punct(c) {
            punct += 1;
            in_word = false;
        } else {
            other += 1;
            in_word = false;
        }
    }
    let total = input.chars().count();
    let line_count = input.lines().count();
    let non_empty = input.lines().filter(|l| !l.trim().is_empty()).count();
    let paragraphs = input.split("\n\n").filter(|p| !p.trim().is_empty()).count();
    let minutes = (cjk + words) as f64 / 300.0;
    let reading = if minutes < 1.0 {
        format!(
            "约 {} 秒",
            ((cjk + words) as f64 / 5.0).ceil().max(1.0) as i64
        )
    } else {
        format!("约 {:.1} 分钟", minutes)
    };
    let rows = [
        ("总字符", total.to_string()),
        ("汉字", cjk.to_string()),
        ("英文单词", words.to_string()),
        ("数字", digits.to_string()),
        ("标点", punct.to_string()),
        ("空白", spaces.to_string()),
        ("其他", other.to_string()),
        ("行数", format!("{line_count}（非空 {non_empty}）")),
        ("段落", paragraphs.to_string()),
        ("UTF-8 字节", input.len().to_string()),
        ("阅读时长", reading),
    ];
    rows.iter()
        .map(|(k, v)| format!("{k}：{v}"))
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{Value, json};

    fn ctx() -> RunCtx<'static> {
        let leaked: &'static Value = Box::leak(Box::new(json!({})));
        RunCtx {
            options: leaked,
            now_ms: 0,
        }
    }

    #[test]
    fn counts_mixed_text() {
        let s = statistics("你好 world 2024，测试。\n\n第二段 hello");
        assert!(s.contains("汉字：7"));
        assert!(s.contains("英文单词：2"));
        assert!(s.contains("行数：3（非空 3）") || s.contains("行数：3"));
        assert!(s.contains("段落：2"));
    }

    #[test]
    fn tool_runs() {
        let r = Count.run("测试", &ctx()).unwrap();
        assert!(r.sections[0].text.contains("汉字：2"));
    }
}
