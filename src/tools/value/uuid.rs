//! UUID 生成与解析。随机数用时间 + 输入做种子的 xorshift，不追求密码学安全。

use crate::tools::datetime::format_ms;
use crate::tools::meta::{Group, OptionSpec, ToolMeta};
use crate::tools::{RunCtx, Tool, ToolResult};

pub struct Uuid;

const UUID_EPOCH_MS: i64 = 12_219_292_800_000;

fn fnv1a(s: &str) -> u64 {
    let mut h = 0xcbf2_9ce4_8422_2325u64;
    for b in s.bytes() {
        h ^= b as u64;
        h = h.wrapping_mul(0x100_0000_01b3);
    }
    h
}

fn xorshift(state: &mut u64) -> u64 {
    if *state == 0 {
        *state = 0x9E37_79B9_7F4A_7C15;
    }
    let mut x = *state;
    x ^= x << 13;
    x ^= x >> 7;
    x ^= x << 17;
    *state = x;
    x
}

fn make(state: &mut u64) -> [u8; 16] {
    let a = xorshift(state).to_le_bytes();
    let b = xorshift(state).to_le_bytes();
    let mut out = [0u8; 16];
    out[..8].copy_from_slice(&a);
    out[8..].copy_from_slice(&b);
    out[6] = (out[6] & 0x0f) | 0x40;
    out[8] = (out[8] & 0x3f) | 0x80;
    out
}

fn format(bytes: &[u8; 16], mode: &str) -> String {
    let hex: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
    let dashed = format!(
        "{}-{}-{}-{}-{}",
        &hex[..8],
        &hex[8..12],
        &hex[12..16],
        &hex[16..20],
        &hex[20..]
    );
    match mode {
        "upper" => dashed.to_uppercase(),
        "nohyphen" => hex,
        "braces" => format!("{{{dashed}}}"),
        _ => dashed,
    }
}

fn hex_bytes(s: &str) -> Option<Vec<u8>> {
    let t = s
        .trim()
        .trim_matches(|c| c == '{' || c == '}')
        .replace('-', "")
        .to_lowercase();
    if t.len() != 32 || !t.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    (0..16)
        .map(|i| u8::from_str_radix(&t[i * 2..i * 2 + 2], 16).ok())
        .collect()
}

fn describe(bytes: &[u8], tz: i64) -> String {
    let version = bytes[6] >> 4;
    let variant = bytes[8] >> 6;
    let variant_text = match variant {
        0b10 => "RFC 4122",
        0b11 => "Microsoft",
        _ => "NCS / 保留",
    };
    let mut out = format!("版本      {version}\n变体      {variant_text}");
    if version == 1 {
        let time_low = u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]) as u64;
        let time_mid = u16::from_be_bytes([bytes[4], bytes[5]]) as u64;
        let time_hi = (u16::from_be_bytes([bytes[6], bytes[7]]) & 0x0FFF) as u64;
        let ticks = (time_hi << 48) | (time_mid << 32) | time_low;
        let ms = ticks as i64 / 10_000 - UUID_EPOCH_MS;
        out.push_str(&format!("\n时间      {}（v1 时间戳）", format_ms(ms, tz)));
    }
    out
}

impl Tool for Uuid {
    fn meta(&self) -> ToolMeta {
        ToolMeta::new(
            "uuid",
            "UUID / 随机串",
            Group::Value,
            "批量生成 UUID v4，或解析已有 UUID 的版本与时间",
            "留空直接生成；也可以粘一个 UUID 解析",
            false,
            "text",
        )
        .with_options(vec![
            OptionSpec::select(
                "uuidMode",
                "模式",
                "generate",
                &[("generate", "生成"), ("parse", "解析")],
                "解析模式读输入里的第一个 UUID",
            ),
            OptionSpec::number("uuidCount", "生成个数", 5.0, 1.0, 100.0, "一次最多 100 个"),
            OptionSpec::select(
                "uuidFormat",
                "格式",
                "standard",
                &[
                    ("standard", "标准小写"),
                    ("upper", "大写"),
                    ("nohyphen", "无横线"),
                    ("braces", "带大括号"),
                ],
                "只影响输出",
            ),
        ])
    }

    fn run(&self, input: &str, ctx: &RunCtx) -> Result<ToolResult, String> {
        let mode = ctx.str("uuidMode", "generate");
        let format_mode = ctx.str("uuidFormat", "standard");
        if mode == "parse" {
            let Some(bytes) = hex_bytes(input) else {
                return Err("没找到合法 UUID，需要 32 位十六进制（可带横线）".to_string());
            };
            return Ok(ToolResult::new()
                .section(
                    "UUID 解析",
                    "text",
                    describe(&bytes, ctx.i64("timezone", 480)),
                )
                .section("标准形式", "text", format(&make_from(&bytes), "standard")));
        }
        let count = ctx.usize("uuidCount", 5, 1, 100);
        let mut state = ctx.now_ms as u64 ^ fnv1a(input) ^ 0x9E37_79B9_7F4A_7C15;
        let mut out = Vec::with_capacity(count);
        for _ in 0..count {
            out.push(format(&make(&mut state), format_mode));
        }
        Ok(ToolResult::new()
            .section("UUID", "text", out.join("\n"))
            .note("随机数由当前时间与输入派生，适合测试数据，不要用于安全令牌"))
    }
}

fn make_from(bytes: &[u8]) -> [u8; 16] {
    let mut out = [0u8; 16];
    for (i, b) in bytes.iter().take(16).enumerate() {
        out[i] = *b;
    }
    out
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
    fn generates_unique_uuids() {
        let r = Uuid.run("", &ctx(json!({"uuidCount": 10}))).unwrap();
        let lines: Vec<&str> = r.sections[0].text.lines().collect();
        assert_eq!(lines.len(), 10);
        assert!(lines[0].len() == 36);
        assert_ne!(lines[0], lines[1]);
        assert_eq!(&lines[0][14..15], "4");
    }

    #[test]
    fn formats() {
        let r = Uuid
            .run(
                "seed",
                &ctx(json!({"uuidCount": 1, "uuidFormat": "nohyphen"})),
            )
            .unwrap();
        assert_eq!(r.sections[0].text.len(), 32);
    }

    #[test]
    fn parses_version() {
        let r = Uuid
            .run(
                "550e8400-e29b-41d4-a716-446655440000",
                &ctx(json!({"uuidMode": "parse"})),
            )
            .unwrap();
        assert!(r.sections[0].text.contains("版本      4"));
        assert!(r.sections[0].text.contains("RFC 4122"));
    }
}
