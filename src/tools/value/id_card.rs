//! 证件号校验与脱敏：身份证、统一社会信用代码、手机号、银行卡。

use crate::tools::datetime::days_in_month;
use crate::tools::meta::{Group, ToolMeta};
use crate::tools::util::lines;
use crate::tools::{RunCtx, Tool, ToolResult};

pub struct IdCard;

const ID_W: [u32; 17] = [7, 9, 10, 5, 8, 4, 2, 1, 6, 3, 7, 9, 10, 5, 8, 4, 2];
const ID_CHECK: [char; 11] = ['1', '0', 'X', '9', '8', '7', '6', '5', '4', '3', '2'];

fn id18_check(s: &str) -> Result<String, String> {
    let chars: Vec<char> = s.chars().collect();
    if chars.len() != 18 {
        return Err("身份证应为 18 位".to_string());
    }
    if !chars[..17].iter().all(char::is_ascii_digit) {
        return Err("前 17 位必须是数字".to_string());
    }
    let last = chars[17].to_ascii_uppercase();
    let sum: u32 = chars[..17]
        .iter()
        .zip(ID_W)
        .map(|(c, w)| c.to_digit(10).unwrap_or(0) * w)
        .sum();
    let expect = ID_CHECK[(sum % 11) as usize];
    let ok = last == expect;
    let year: i64 = s[6..10].parse().map_err(|_| "出生年份不合法".to_string())?;
    let month: u32 = s[10..12]
        .parse()
        .map_err(|_| "出生月份不合法".to_string())?;
    let day: u32 = s[12..14].parse().map_err(|_| "出生日不合法".to_string())?;
    let date_ok = (1..=12).contains(&month) && day >= 1 && day <= days_in_month(year, month);
    let gender = if chars[16].to_digit(10).unwrap_or(0) % 2 == 1 {
        "男"
    } else {
        "女"
    };
    Ok(format!(
        "  校验位    {}（应为 {}）\n  出生日期  {:04}-{:02}-{:02} {}\n  性别      {}\n  脱敏      {}********{}",
        if ok { "正确" } else { "错误" },
        expect,
        year,
        month,
        day,
        if date_ok { "" } else { "（日期不合法）" },
        gender,
        &s[..6],
        &s[14..]
    ))
}

fn upgrade_15(s: &str) -> Result<String, String> {
    if s.len() != 15 || !s.chars().all(|c| c.is_ascii_digit()) {
        return Err("15 位身份证应全为数字".to_string());
    }
    let mut id = String::with_capacity(18);
    id.push_str(&s[..6]);
    id.push_str("19");
    id.push_str(&s[6..]);
    let chars: Vec<char> = id.chars().collect();
    let sum: u32 = chars
        .iter()
        .zip(ID_W)
        .map(|(c, w)| c.to_digit(10).unwrap_or(0) * w)
        .sum();
    id.push(ID_CHECK[(sum % 11) as usize]);
    Ok(id)
}

const USCC: &[u8] = b"0123456789ABCDEFGHJKLMNPQRTUWXY";
const USCC_W: [u32; 17] = [
    1, 3, 9, 27, 19, 26, 16, 17, 20, 29, 25, 13, 8, 24, 10, 30, 28,
];

fn uscc_check(s: &str) -> Result<String, String> {
    let chars: Vec<char> = s.chars().collect();
    if chars.len() != 18 {
        return Err("统一社会信用代码应为 18 位".to_string());
    }
    let mut sum = 0u32;
    for (i, c) in chars[..17].iter().enumerate() {
        let idx = USCC
            .iter()
            .position(|b| *b as char == *c)
            .ok_or_else(|| format!("第 {} 位字符 {c} 不在代码字符集里", i + 1))?;
        sum += idx as u32 * USCC_W[i];
    }
    let check = (31 - sum % 31) % 31;
    let expect = USCC[check as usize] as char;
    let ok = chars[17] == expect;
    Ok(format!(
        "  校验位    {}（应为 {}）\n  脱敏      {}********{}",
        if ok { "正确" } else { "错误" },
        expect,
        &s[..4],
        &s[14..]
    ))
}

fn luhn(s: &str) -> bool {
    let mut sum = 0u32;
    let mut double = false;
    for c in s.chars().rev() {
        let mut d = match c.to_digit(10) {
            Some(d) => d,
            None => return false,
        };
        if double {
            d *= 2;
            if d > 9 {
                d -= 9;
            }
        }
        sum += d;
        double = !double;
    }
    sum.is_multiple_of(10)
}

fn classify(s: &str) -> String {
    let chars: Vec<char> = s.chars().collect();
    if chars.len() == 18 && chars[..17].iter().all(char::is_ascii_digit) {
        return format!(
            "18 位身份证\n{}",
            id18_check(s).unwrap_or_else(|e| format!("  {e}"))
        );
    }
    if chars.len() == 15 && chars.iter().all(char::is_ascii_digit) {
        return match upgrade_15(s) {
            Ok(id) => format!(
                "15 位旧身份证\n  升级为 18 位  {id}\n{}",
                id18_check(&id).unwrap_or_default()
            ),
            Err(e) => format!("15 位旧身份证\n  {e}"),
        };
    }
    if chars.len() == 18 {
        return format!(
            "统一社会信用代码\n{}",
            uscc_check(s).unwrap_or_else(|e| format!("  {e}"))
        );
    }
    if chars.len() == 11 && s.starts_with('1') {
        let second = chars.get(1).and_then(|c| c.to_digit(10)).unwrap_or(0);
        let ok = (3..=9).contains(&second) && chars.iter().all(char::is_ascii_digit);
        return format!(
            "手机号\n  格式      {}\n  脱敏      {}****{}",
            if ok {
                "正确"
            } else {
                "可疑（第二位应为 3–9）"
            },
            &s[..3],
            &s[7..]
        );
    }
    if (12..=19).contains(&chars.len()) && chars.iter().all(char::is_ascii_digit) {
        return format!(
            "银行卡号\n  Luhn      {}\n  脱敏      {} **** **** {}",
            if luhn(s) { "通过" } else { "不通过" },
            &s[..4],
            &s[s.len() - 4..]
        );
    }
    "  没认出类型：支持 18/15 位身份证、统一社会信用代码、11 位手机号、12–19 位银行卡".to_string()
}

impl Tool for IdCard {
    fn meta(&self) -> ToolMeta {
        ToolMeta::new(
            "id-card",
            "证件号校验 / 脱敏",
            Group::Check,
            "校验身份证与统一社会信用代码的校验位，解析生日性别，一键脱敏",
            "110101199003071234\n13812345678",
            true,
            "text",
        )
    }

    fn run(&self, input: &str, _ctx: &RunCtx) -> Result<ToolResult, String> {
        let rows = lines(input);
        if rows.is_empty() {
            return Err("一行一个号码".to_string());
        }
        let mut out = Vec::new();
        for row in rows.iter().take(50) {
            out.push(format!("{row}\n{}", classify(row)));
        }
        Ok(ToolResult::new()
            .section("校验结果", "text", out.join("\n\n"))
            .note("全部在本机计算，不会把号码发到任何地方；校验位正确不代表号码真实存在"))
    }
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
    fn valid_id_card() {
        let id = upgrade_15("110101900307123").unwrap();
        let out = id18_check(&id).unwrap();
        assert!(out.contains("校验位    正确"));
        assert!(out.contains("1990-03-07"));
    }

    #[test]
    fn invalid_id_card_detected() {
        let out = id18_check("110101199003071230").unwrap();
        assert!(out.contains("错误"));
    }

    #[test]
    fn phone_masking() {
        let out = classify("13812345678");
        assert!(out.contains("138****5678"));
    }

    #[test]
    fn luhn_checks_cards() {
        assert!(luhn("4111111111111111"));
        assert!(!luhn("4111111111111112"));
    }

    #[test]
    fn tool_runs() {
        let r = IdCard.run("13812345678", &ctx()).unwrap();
        assert!(r.sections[0].text.contains("手机号"));
    }
}
