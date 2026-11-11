//! 汉字 → 拼音首字母。
//!
//! 用 GBK 编码把汉字映射到 GB2312 一级汉字区（按拼音排序），查表得到声母。
//! 只做首字母，不做全拼：全拼需要几万字的字典，会把 wasm 包撑大。

use crate::tools::meta::{Group, OptionSpec, ToolMeta};
use crate::tools::util::is_cjk;
use crate::tools::{RunCtx, Tool, ToolResult};
use encoding_rs::GBK;

pub struct Pinyin;

const RANGES: [(u16, u16, char); 23] = [
    (0xB0A1, 0xB0C4, 'A'),
    (0xB0C5, 0xB2C0, 'B'),
    (0xB2C1, 0xB4ED, 'C'),
    (0xB4EE, 0xB6E9, 'D'),
    (0xB6EA, 0xB7A1, 'E'),
    (0xB7A2, 0xB8C0, 'F'),
    (0xB8C1, 0xB9FD, 'G'),
    (0xB9FE, 0xBBF6, 'H'),
    (0xBBF7, 0xBFA5, 'J'),
    (0xBFA6, 0xC0AB, 'K'),
    (0xC0AC, 0xC2E7, 'L'),
    (0xC2E8, 0xC4C2, 'M'),
    (0xC4C3, 0xC5B5, 'N'),
    (0xC5B6, 0xC5BD, 'O'),
    (0xC5BE, 0xC6D9, 'P'),
    (0xC6DA, 0xC8BA, 'Q'),
    (0xC8BB, 0xC8F5, 'R'),
    (0xC8F6, 0xCBF9, 'S'),
    (0xCBFA, 0xCDDA, 'T'),
    (0xCDDA, 0xCEF3, 'W'),
    (0xCEF4, 0xD1B8, 'X'),
    (0xD1B9, 0xD4D0, 'Y'),
    (0xD4D1, 0xD7F9, 'Z'),
];

fn initial_of(c: char) -> Option<char> {
    let buf = c.to_string();
    let (bytes, _, had_errors) = GBK.encode(&buf);
    if had_errors || bytes.len() != 2 {
        return None;
    }
    let code = (bytes[0] as u16) << 8 | bytes[1] as u16;
    RANGES
        .iter()
        .find(|(lo, hi, _)| code >= *lo && code <= *hi)
        .map(|(_, _, ch)| *ch)
}

fn convert(input: &str, upper: bool, sep: &str, keep_other: bool) -> String {
    let mut out = String::new();
    for c in input.chars() {
        if is_cjk(c) {
            if let Some(initial) = initial_of(c) {
                let ch = if upper {
                    initial
                } else {
                    initial.to_ascii_lowercase()
                };
                out.push(ch);
                if !sep.is_empty() && !out.ends_with(sep) {
                    out.push_str(sep);
                }
            } else if keep_other {
                out.push(c);
            }
            continue;
        }
        if c == '\n' {
            out.push('\n');
        } else if keep_other {
            out.push(c);
        }
    }
    if !sep.is_empty() {
        while out.ends_with(sep) {
            out.truncate(out.len() - sep.len());
        }
    }
    out
}

impl Tool for Pinyin {
    fn meta(&self) -> ToolMeta {
        ToolMeta::new(
            "pinyin",
            "汉字转拼音首字母",
            Group::Text,
            "把汉字转成声母首字母，常用于给中文名起英文变量名 / 常量名",
            "用户名\n订单状态",
            true,
            "text",
        )
        .with_options(vec![
            OptionSpec::select(
                "pinyinCase",
                "大小写",
                "upper",
                &[("upper", "大写 YHM"), ("lower", "小写 yhm")],
                "常量名用大写，变量名用小写",
            ),
            OptionSpec::select(
                "pinyinSep",
                "分隔符",
                "none",
                &[("none", "不加"), ("space", "空格"), ("dot", "·")],
                "逐字加分隔符，便于阅读",
            ),
            OptionSpec::bool(
                "pinyinKeepOther",
                "保留非汉字",
                false,
                "关闭后只输出首字母，英文数字会被丢掉",
            ),
        ])
    }

    fn run(&self, input: &str, ctx: &RunCtx) -> Result<ToolResult, String> {
        let upper = ctx.str("pinyinCase", "upper") != "lower";
        let sep = match ctx.str("pinyinSep", "none") {
            "space" => " ",
            "dot" => "·",
            _ => "",
        };
        let keep = ctx.bool("pinyinKeepOther", false);
        let out = convert(input, upper, sep, keep);
        Ok(ToolResult::new()
            .section("首字母", "text", out)
            .note("只取声母首字母；多音字按 GB2312 排序取默认读音，二级汉字会保留原字"))
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
    fn common_chars() {
        let out = convert("用户名", true, "", false);
        assert_eq!(out, "YHM");
        assert_eq!(convert("订单状态", true, "", false), "DDZT");
    }

    #[test]
    fn separator_and_keep_other() {
        assert_eq!(convert("用户", true, " ", false), "Y H");
        assert_eq!(convert("用户A", true, "", true), "YHA");
        assert_eq!(convert("用户\u{3000}名", true, "", true), "YH\u{3000}M");
    }

    #[test]
    fn tool_returns_section() {
        let r = Pinyin
            .run("用户", &ctx(json!({"pinyinCase":"lower"})))
            .unwrap();
        assert_eq!(r.sections[0].text, "yh");
    }
}
