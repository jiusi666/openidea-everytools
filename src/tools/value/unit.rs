//! 进制与单位换算：字节、时长、2/8/10/16/36 进制。

use crate::tools::meta::{Group, ToolMeta};
use crate::tools::util::lines;
use crate::tools::{RunCtx, Tool, ToolResult};

pub struct Unit;

fn parse_int(s: &str) -> Option<u128> {
    let t = s.trim().replace('_', "");
    let lower = t.to_ascii_lowercase();
    if let Some(hex) = lower.strip_prefix("0x") {
        u128::from_str_radix(hex, 16).ok()
    } else if let Some(bin) = lower.strip_prefix("0b") {
        u128::from_str_radix(bin, 2).ok()
    } else if let Some(oct) = lower.strip_prefix("0o") {
        u128::from_str_radix(oct, 8).ok()
    } else if t.chars().all(|c| c.is_ascii_digit()) {
        t.parse().ok()
    } else {
        None
    }
}

fn split_num_unit(s: &str) -> Option<(f64, &str)> {
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() && (bytes[i].is_ascii_digit() || bytes[i] == b'.') {
        i += 1;
    }
    if i == 0 {
        return None;
    }
    let num: f64 = s[..i].parse().ok()?;
    Some((num, s[i..].trim()))
}

fn byte_factor(unit: &str) -> Option<f64> {
    Some(match unit {
        "b" | "byte" | "bytes" | "字节" => 1.0,
        "kb" | "k" | "kib" => 1024.0,
        "mb" | "m" | "mib" => 1024.0 * 1024.0,
        "gb" | "g" | "gib" => 1024.0f64.powi(3),
        "tb" | "t" | "tib" => 1024.0f64.powi(4),
        "pb" | "pib" => 1024.0f64.powi(5),
        _ => return None,
    })
}

fn duration_factor(unit: &str) -> Option<f64> {
    Some(match unit {
        "ms" | "毫秒" => 1.0,
        "s" | "sec" | "secs" | "秒" => 1000.0,
        "min" | "mins" | "分" | "分钟" => 60_000.0,
        "h" | "hr" | "hour" | "小时" => 3_600_000.0,
        "d" | "day" | "天" => 86_400_000.0,
        _ => return None,
    })
}

fn human_bytes(bytes: u128) -> String {
    const UNITS: [&str; 6] = ["B", "KiB", "MiB", "GiB", "TiB", "PiB"];
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit < UNITS.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{bytes} B")
    } else {
        format!("{value:.2} {}", UNITS[unit])
    }
}

fn human_decimal_bytes(bytes: u128) -> String {
    const UNITS: [&str; 6] = ["B", "KB", "MB", "GB", "TB", "PB"];
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1000.0 && unit < UNITS.len() - 1 {
        value /= 1000.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{bytes} B")
    } else {
        format!("{value:.2} {}", UNITS[unit])
    }
}

fn human_duration(ms: u128) -> String {
    let days = ms / 86_400_000;
    let rest = ms % 86_400_000;
    let h = rest / 3_600_000;
    let m = (rest % 3_600_000) / 60_000;
    let s = (rest % 60_000) / 1000;
    let milli = rest % 1000;
    let mut out = String::new();
    if days > 0 {
        out.push_str(&format!("{days} 天 "));
    }
    out.push_str(&format!("{h:02}:{m:02}:{s:02}"));
    if milli > 0 {
        out.push_str(&format!(".{milli:03}"));
    }
    out
}

fn radix_block(n: u128) -> String {
    [
        format!("十进制    {n}"),
        format!("十六进制  0x{n:X}"),
        format!("八进制    0o{n:o}"),
        format!("二进制    0b{n:b}"),
        format!("36 进制   {}", base36(n)),
    ]
    .join("\n")
}

fn base36(mut n: u128) -> String {
    if n == 0 {
        return "0".to_string();
    }
    const CHARS: &[u8] = b"0123456789abcdefghijklmnopqrstuvwxyz";
    let mut out = Vec::new();
    while n > 0 {
        out.push(CHARS[(n % 36) as usize]);
        n /= 36;
    }
    out.reverse();
    String::from_utf8(out).unwrap_or_default()
}

fn convert_line(line: &str) -> Result<String, String> {
    let s = line.trim();
    if s.is_empty() {
        return Err("空行".to_string());
    }
    if let Some(n) = parse_int(s) {
        let mut blocks = vec![radix_block(n)];
        if n <= u128::from(u64::MAX) {
            blocks.push(format!(
                "字节      {}（{}，按 1000 算是 {}）",
                n,
                human_bytes(n),
                human_decimal_bytes(n)
            ));
            if n <= 10_000_000_000_000 {
                blocks.push(format!("时长      {} 毫秒 = {}", n, human_duration(n)));
            }
        }
        return Ok(blocks.join("\n"));
    }
    if let Some((num, unit)) = split_num_unit(s) {
        let unit_lower = unit.to_ascii_lowercase();
        if let Some(factor) = byte_factor(&unit_lower) {
            let bytes = (num * factor).round().max(0.0) as u128;
            return Ok(format!(
                "{} {} = {} 字节\n  = {}（1024 进制）\n  = {}（1000 进制）",
                num,
                unit,
                bytes,
                human_bytes(bytes),
                human_decimal_bytes(bytes)
            ));
        }
        if let Some(factor) = duration_factor(&unit_lower) {
            let ms = (num * factor).round().max(0.0) as u128;
            return Ok(format!(
                "{} {} = {} 毫秒\n  = {}",
                num,
                unit,
                ms,
                human_duration(ms)
            ));
        }
    }
    Err(format!("看不懂：{s}（示例：0xFF / 1024KB / 90min / 1.5h）"))
}

impl Tool for Unit {
    fn meta(&self) -> ToolMeta {
        ToolMeta::new(
            "unit",
            "进制 / 单位换算",
            Group::Value,
            "数字进制互转、字节大小与时长换算，一行一个",
            "0xFF\n1024KB\n90min",
            true,
            "text",
        )
    }

    fn run(&self, input: &str, _ctx: &RunCtx) -> Result<ToolResult, String> {
        let rows = lines(input);
        if rows.is_empty() {
            return Err("一行一个值，例如 0xFF / 1024KB / 90min".to_string());
        }
        let mut out = Vec::new();
        for row in rows.iter().take(100) {
            match convert_line(row) {
                Ok(s) => out.push(s),
                Err(e) => out.push(e),
            }
        }
        Ok(ToolResult::new()
            .section("换算结果", "text", out.join("\n\n"))
            .note("字节同时给 1024 与 1000 两种口径；时长按毫秒展开"))
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
    fn radix_conversion() {
        let out = convert_line("0xFF").unwrap();
        assert!(out.contains("十进制    255"));
        assert!(out.contains("0b11111111"));
    }

    #[test]
    fn bytes_and_duration() {
        assert!(convert_line("1024KB").unwrap().contains("1.00 MiB"));
        assert!(convert_line("90min").unwrap().contains("01:30:00"));
    }

    #[test]
    fn tool_runs_each_line() {
        let r = Unit.run("0x10\n1.5h", &ctx()).unwrap();
        assert!(r.sections[0].text.contains("十六进制"));
        assert!(r.sections[0].text.contains("5400000 毫秒"));
    }
}
