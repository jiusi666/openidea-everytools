//! 雪花 ID 解析：41 位时间 + 5 位数据中心 + 5 位机器 + 12 位序列。

use crate::tools::datetime::format_ms;
use crate::tools::meta::{Group, OptionSpec, ToolMeta};
use crate::tools::util::lines;
use crate::tools::{RunCtx, Tool, ToolResult};

pub struct Snowflake;

const DEFAULT_EPOCH: i64 = 1_288_834_974_657;

fn parse_one(raw: &str, epoch: i64, tz: i64) -> Result<String, String> {
    let id: i64 = raw
        .trim()
        .parse()
        .map_err(|_| format!("不是合法的 64 位整数：{raw}"))?;
    if id < 0 {
        return Err(format!("{raw} 是负数，雪花 ID 最高位应为 0"));
    }
    let bits = format!("{:064b}", id as u64);
    let ts = u64::from_str_radix(&bits[1..42], 2).unwrap_or(0);
    let dc = u64::from_str_radix(&bits[42..47], 2).unwrap_or(0);
    let worker = u64::from_str_radix(&bits[47..52], 2).unwrap_or(0);
    let seq = u64::from_str_radix(&bits[52..64], 2).unwrap_or(0);
    let ms = ts as i64 + epoch;
    Ok(format!(
        "ID          {id}\n时间        {}（{}）\n毫秒        {ms}\n数据中心    {dc}\n机器        {worker}\n序列        {seq}\n二进制      0 | {} | {} | {} | {}",
        format_ms(ms, tz),
        crate::tools::datetime::tz_label(tz),
        &bits[1..42],
        &bits[42..47],
        &bits[47..52],
        &bits[52..64]
    ))
}

impl Tool for Snowflake {
    fn meta(&self) -> ToolMeta {
        ToolMeta::new(
            "snowflake",
            "雪花 ID 解析",
            Group::Value,
            "把 64 位雪花 ID 拆成时间、数据中心、机器和序列号",
            "1234567890123456789",
            true,
            "text",
        )
        .with_options(vec![
            OptionSpec::number(
                "epoch",
                "起始纪元（毫秒）",
                DEFAULT_EPOCH as f64,
                0.0,
                4_102_444_800_000.0,
                "Twitter 默认 1288834974657；用了自定义纪元就改这里",
            ),
            OptionSpec::number(
                "timezone",
                "时区（分钟）",
                480.0,
                -720.0,
                840.0,
                "只影响显示",
            ),
        ])
    }

    fn run(&self, input: &str, ctx: &RunCtx) -> Result<ToolResult, String> {
        let epoch = ctx.i64("epoch", DEFAULT_EPOCH);
        let tz = ctx.i64("timezone", 480);
        let rows = lines(input);
        if rows.is_empty() {
            return Err("粘一个雪花 ID".to_string());
        }
        let mut out = Vec::new();
        for row in rows.iter().take(50) {
            out.push(parse_one(row, epoch, tz)?);
        }
        Ok(ToolResult::new()
            .section("解析结果", "text", out.join("\n\n"))
            .note("按 1+41+5+5+12 的标准位分配解析；位分配自定义过的话结果会偏"))
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
    fn parses_known_id() {
        // 2024-01-01 12:30:00 UTC 的毫秒 - epoch，左移 22 位，再加机器与序列
        let ms = 1_704_083_400_000i64;
        let id = ((ms - DEFAULT_EPOCH) << 22) | (3 << 17) | (7 << 12) | 123;
        let r = Snowflake.run(&id.to_string(), &ctx(json!({}))).unwrap();
        let s = &r.sections[0].text;
        assert!(s.contains("数据中心    3"));
        assert!(s.contains("机器        7"));
        assert!(s.contains("序列        123"));
    }

    #[test]
    fn rejects_text() {
        assert!(Snowflake.run("abc", &ctx(json!({}))).is_err());
    }
}
