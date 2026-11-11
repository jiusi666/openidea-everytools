//! 人民币金额大写，以及大写反向解析。

use crate::tools::meta::{Group, OptionSpec, ToolMeta};
use crate::tools::{RunCtx, Tool, ToolResult};

pub struct Rmb;

const DIGITS: [&str; 10] = ["零", "壹", "贰", "叁", "肆", "伍", "陆", "柒", "捌", "玖"];
const UNITS: [&str; 16] = [
    "", "拾", "佰", "仟", "万", "拾", "佰", "仟", "亿", "拾", "佰", "仟", "万亿", "拾", "佰", "仟",
];

pub fn to_upper(input: &str) -> Result<String, String> {
    let s = input.trim().replace([',', ' ', '¥', '￥'], "");
    if s.is_empty() {
        return Err("输入一个金额，例如 1234.56".to_string());
    }
    let neg = s.starts_with('-');
    let s = s.trim_start_matches('-');
    let (int_str, frac_str) = match s.split_once('.') {
        Some((a, b)) => (a, b),
        None => (s, ""),
    };
    let int_str = if int_str.is_empty() { "0" } else { int_str };
    if !int_str.chars().all(|c| c.is_ascii_digit()) {
        return Err("整数部分只能是数字".to_string());
    }
    if int_str.len() > 16 {
        return Err("整数部分最多 16 位".to_string());
    }
    let frac: Vec<u32> = frac_str
        .chars()
        .filter(|c| c.is_ascii_digit())
        .filter_map(|c| c.to_digit(10))
        .collect();
    let jiao = frac.first().copied().unwrap_or(0);
    let fen = frac.get(1).copied().unwrap_or(0);
    let trimmed = int_str.trim_start_matches('0');
    let digits: Vec<u32> = if trimmed.is_empty() {
        vec![0]
    } else {
        trimmed.chars().filter_map(|c| c.to_digit(10)).collect()
    };
    let len = digits.len();
    let mut int_part = String::new();
    let mut zero = false;
    for (i, d) in digits.iter().enumerate() {
        let pos = len - 1 - i;
        if *d == 0 {
            zero = true;
            continue;
        }
        if zero && !int_part.is_empty() {
            int_part.push('零');
        }
        zero = false;
        int_part.push_str(DIGITS[*d as usize]);
        int_part.push_str(UNITS[pos]);
    }
    let mut out = String::new();
    if neg {
        out.push('负');
    }
    if int_part.is_empty() {
        out.push_str("零元");
    } else {
        out.push_str(&int_part);
        out.push('元');
    }
    if jiao == 0 && fen == 0 {
        out.push('整');
    } else {
        if jiao == 0 {
            out.push('零');
        } else {
            out.push_str(DIGITS[jiao as usize]);
            out.push('角');
        }
        if fen != 0 {
            out.push_str(DIGITS[fen as usize]);
            out.push('分');
        }
    }
    Ok(out)
}

fn digit_value(c: char) -> Option<i128> {
    DIGITS
        .iter()
        .position(|d| d.starts_with(c))
        .map(|i| i as i128)
}

/// 反向解析：只认标准大写写法，遇到「零」按 0 处理。
pub fn parse_upper(input: &str) -> Result<String, String> {
    let s: String = input.chars().filter(|c| !c.is_whitespace()).collect();
    if s.is_empty() {
        return Err("输入大写金额，例如 壹佰贰拾叁元肆角伍分".to_string());
    }
    let neg = s.starts_with('负');
    let s = s.trim_start_matches('负');
    let mut total: i128 = 0;
    let mut section: i128 = 0;
    let mut number: i128 = 0;
    let mut jiao: i128 = 0;
    let mut fen: i128 = 0;
    for c in s.chars() {
        match c {
            '零' => number = 0,
            '壹' | '贰' | '叁' | '肆' | '伍' | '陆' | '柒' | '捌' | '玖' => {
                number = digit_value(c).ok_or_else(|| format!("看不懂的字符：{c}"))?;
            }
            '拾' => {
                section += if number == 0 { 1 } else { number } * 10;
                number = 0;
            }
            '佰' => {
                section += if number == 0 { 1 } else { number } * 100;
                number = 0;
            }
            '仟' => {
                section += if number == 0 { 1 } else { number } * 1000;
                number = 0;
            }
            '万' => {
                section += number;
                number = 0;
                total += section * 10_000;
                section = 0;
            }
            '亿' => {
                section += number;
                number = 0;
                total += section * 100_000_000;
                section = 0;
            }
            '元' | '圆' => {
                total += section + number;
                section = 0;
                number = 0;
            }
            '角' => {
                jiao = number;
                number = 0;
            }
            '分' => {
                fen = number;
                number = 0;
            }
            '整' => {}
            _ => return Err(format!("看不懂的字符：{c}")),
        }
    }
    total += section + number;
    let mut out = format!("{total}.{:02}", jiao * 10 + fen);
    if neg {
        out.insert(0, '-');
    }
    Ok(out)
}

impl Tool for Rmb {
    fn meta(&self) -> ToolMeta {
        ToolMeta::new(
            "rmb",
            "人民币金额大写",
            Group::Value,
            "数字金额转财务大写，也可以把大写反过来解析成数字",
            "1234.56\n壹佰贰拾叁元肆角伍分",
            true,
            "text",
        )
        .with_options(vec![OptionSpec::select(
            "rmbMode",
            "方向",
            "toUpper",
            &[("toUpper", "数字 → 大写"), ("toNumber", "大写 → 数字")],
            "自动识别不了混在一起的输入",
        )])
    }

    fn run(&self, input: &str, ctx: &RunCtx) -> Result<ToolResult, String> {
        let mode = ctx.str("rmbMode", "toUpper");
        let line = input.lines().find(|l| !l.trim().is_empty()).unwrap_or("");
        if line.trim().is_empty() {
            return Err("输入一个金额".to_string());
        }
        let result = if mode == "toNumber" {
            parse_upper(line)?
        } else {
            to_upper(line)?
        };
        Ok(ToolResult::new()
            .section("结果", "text", result.clone())
            .section(
                "说明",
                "text",
                if mode == "toNumber" {
                    "反向解析按标准大写规则，遇到不规范写法可能偏差。".to_string()
                } else {
                    format!(
                        "共 {} 个字符；支持负数、角分与万亿级金额。",
                        result.chars().count()
                    )
                },
            ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{Value, json};

    fn ctx(opt: Value) -> RunCtx<'static> {
        let leaked: &'static Value = Box::leak(Box::new(opt));
        RunCtx {
            options: leaked,
            now_ms: 0,
        }
    }

    #[test]
    fn common_amounts() {
        assert_eq!(to_upper("1234.56").unwrap(), "壹仟贰佰叁拾肆元伍角陆分");
        assert_eq!(to_upper("10001").unwrap(), "壹万零壹元整");
        assert_eq!(to_upper("100000000").unwrap(), "壹亿元整");
        assert_eq!(to_upper("0.5").unwrap(), "零元伍角");
        assert_eq!(to_upper("-10").unwrap(), "负壹拾元整");
    }

    #[test]
    fn roundtrip() {
        assert_eq!(parse_upper("壹仟贰佰叁拾肆元伍角陆分").unwrap(), "1234.56");
        assert_eq!(parse_upper("壹亿零壹万元整").unwrap(), "100010000.00");
    }

    #[test]
    fn tool_modes() {
        let r = Rmb.run("1234.56", &ctx(json!({}))).unwrap();
        assert!(r.sections[0].text.contains("壹仟贰佰叁拾肆元"));
        let r = Rmb
            .run("壹佰贰拾叁元肆角伍分", &ctx(json!({"rmbMode":"toNumber"})))
            .unwrap();
        assert_eq!(r.sections[0].text, "123.45");
    }
}
