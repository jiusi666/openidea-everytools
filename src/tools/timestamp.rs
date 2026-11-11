//! 时间戳 ↔ 日期。纯算术，不用系统时钟；「现在」由面板传入。

use crate::tools::datetime::{
    format_ms, format_parts, iso8601, local_parts, parse_datetime, relative, tz_label,
};
use crate::tools::meta::{Group, OptionSpec, ToolMeta};
use crate::tools::util::lines;
use crate::tools::{RunCtx, Tool, ToolResult};

pub struct TimestampTool;

impl Tool for TimestampTool {
    fn meta(&self) -> ToolMeta {
        ToolMeta::new(
            "timestamp",
            "时间戳 ↔ 日期",
            Group::Time,
            "秒 / 毫秒 / 微秒 / 纳秒时间戳与日期互转，支持多行批量",
            "1704083400\n2024-01-01 12:30:00",
            true,
            "text",
        )
        .with_options(vec![
            OptionSpec::number(
                "timezone",
                "时区（分钟）",
                480.0,
                -720.0,
                840.0,
                "东八区填 480，UTC 填 0",
            ),
            OptionSpec::select(
                "timeMode",
                "方向",
                "auto",
                &[
                    ("auto", "自动判断"),
                    ("to_date", "时间戳 → 日期"),
                    ("to_ms", "日期 → 时间戳"),
                ],
                "自动模式按内容是纯数字还是日期判断",
            ),
        ])
    }

    fn run(&self, input: &str, ctx: &RunCtx) -> Result<ToolResult, String> {
        let tz = ctx.i64("timezone", 480);
        let mode = ctx.str("timeMode", "auto");
        let rows = lines(input);
        if rows.is_empty() {
            return Err("粘一个时间戳或日期，一行一个".to_string());
        }
        let mut out: Vec<String> = Vec::new();
        let mut notes: Vec<String> = Vec::new();
        for raw in rows.iter().take(200) {
            let digits: String = raw.chars().filter(|c| c.is_ascii_digit()).collect();
            let looks_number = raw
                .chars()
                .all(|c| c.is_ascii_digit() || c == '-' || c == '.' || c == ',');
            let as_ms: Option<i64> = if looks_number && !digits.is_empty() {
                digits.parse::<i64>().ok().map(|n| match digits.len() {
                    10 => n * 1000,
                    13 => n,
                    16 => n / 1000,
                    19 => n / 1_000_000,
                    _ if digits.len() < 10 => n * 1000,
                    _ => n,
                })
            } else {
                None
            };
            if mode == "to_ms" || (mode == "auto" && as_ms.is_none()) {
                let ms = parse_datetime(raw, tz).ok_or_else(|| format!("看不懂这个日期：{raw}"))?;
                out.push(format!(
                    "{}\n  → {} 毫秒\n  → {} 秒\n  → {}（相对现在）",
                    raw,
                    ms,
                    ms / 1000,
                    relative(ms, ctx.now_ms)
                ));
                continue;
            }
            let ms = as_ms.ok_or_else(|| format!("看不懂这个时间戳：{raw}"))?;
            out.push(format!(
                "{}\n  → {}（{}）\n  → ISO8601 {}",
                raw,
                format_parts(&local_parts(ms, tz)),
                tz_label(tz),
                iso8601(ms)
            ));
            if digits.len() != 10 && digits.len() != 13 {
                notes.push(format!("{raw} 按 {} 位数字处理，已自动换算", digits.len()));
            }
        }
        if out.is_empty() {
            return Err("没有可处理的内容".to_string());
        }
        let mut r = ToolResult::new()
            .section("转换结果", "text", out.join("\n\n"))
            .note(format!(
                "时区：{}；当前时间：{}",
                tz_label(tz),
                format_ms(ctx.now_ms, tz)
            ));
        for n in notes {
            r = r.note(n);
        }
        Ok(r)
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
            now_ms: 1_704_083_400_000,
        }
    }

    #[test]
    fn seconds_and_millis() {
        let r = TimestampTool
            .run("1704083400\n1704083400000", &ctx(json!({})))
            .unwrap();
        assert!(r.sections[0].text.contains("2024-01-01 12:30:00"));
    }

    #[test]
    fn date_to_ms() {
        let r = TimestampTool
            .run("2024-01-01", &ctx(json!({"timeMode":"to_ms"})))
            .unwrap();
        assert!(r.sections[0].text.contains("毫秒"));
    }

    #[test]
    fn utc_timezone() {
        let r = TimestampTool
            .run("1704083400", &ctx(json!({"timezone":0})))
            .unwrap();
        assert!(r.sections[0].text.contains("04:30:00"));
    }
}
