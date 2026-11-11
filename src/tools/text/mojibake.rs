//! 中文乱码修复：把「UTF-8 被当 GBK / Latin-1 读」的文本还原回来。
//!
//! 做法是把当前字符串按原来的错误编码编回字节，再按正确编码解一次，
//! 给出若干候选并打分。顺带提供全角 / 半角转换。

use crate::tools::meta::{Group, OptionSpec, ToolMeta};
use crate::tools::util::is_cjk;
use crate::tools::{RunCtx, Tool, ToolResult};
use encoding_rs::{GBK, UTF_8, WINDOWS_1252};

pub struct Mojibake;

fn decode_utf8(bytes: &[u8]) -> Option<String> {
    let (cow, _, had_errors) = UTF_8.decode(bytes);
    (!had_errors).then(|| cow.into_owned())
}

fn candidates(input: &str) -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = Vec::new();
    let (gbk, _, gbk_err) = GBK.encode(input);
    if !gbk_err && let Some(s) = decode_utf8(&gbk) {
        out.push(("UTF-8 被当成 GBK/ANSI 读取".to_string(), s));
    }
    let (latin, _, latin_err) = WINDOWS_1252.encode(input);
    if !latin_err && let Some(s) = decode_utf8(&latin) {
        out.push(("UTF-8 被当成 Latin-1 读取".to_string(), s));
    }
    let (utf8, _, _) = UTF_8.encode(input);
    let (decoded, _, _) = GBK.decode(&utf8);
    let s = decoded.into_owned();
    if s != input {
        out.push(("GBK 被当成 UTF-8 读取".to_string(), s));
    }
    out
}

fn score(input: &str, s: &str) -> i32 {
    if s == input {
        return -1000;
    }
    let bad = s.matches('\u{fffd}').count() as i32 * 30;
    let cjk = s.chars().filter(|c| is_cjk(*c)).count() as i32;
    let shrink = input.chars().count() as i32 - s.chars().count() as i32;
    cjk * 3 + shrink * 5 - bad
}

fn looks_broken(s: &str) -> bool {
    if s.contains('\u{fffd}') {
        return true;
    }
    let latin_high = s
        .chars()
        .filter(|c| ('\u{00c0}'..='\u{00ff}').contains(c))
        .count();
    let weird = [
        "鎴", "锛", "銆", "鈥", "鏄", "涓", "鐨", "浣", "璇", "鏃", "寮", "鍜", "绛", "绯", "缂",
        "鍦", "浠", "姝", "妫", "瀛", "闇", "鍔", "鎵", "鎬", "鐢", "鍗", "鍥", "鏂", "鍙", "绋",
    ]
    .iter()
    .any(|w| s.contains(w));
    latin_high >= 2 || weird
}

fn to_half(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            '\u{3000}' => ' ',
            '\u{ff01}'..='\u{ff5e}' => char::from_u32(c as u32 - 0xfee0).unwrap_or(c),
            _ => c,
        })
        .collect()
}

fn to_full(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            ' ' => '\u{3000}',
            '!'..='~' => char::from_u32(c as u32 + 0xfee0).unwrap_or(c),
            _ => c,
        })
        .collect()
}

impl Tool for Mojibake {
    fn meta(&self) -> ToolMeta {
        ToolMeta::new(
            "mojibake",
            "中文乱码修复",
            Group::Text,
            "还原「UTF-8 被当 GBK / Latin-1 读」的乱码，并给全角半角转换",
            "鎴戜滑\nä½ å¥½",
            true,
            "text",
        )
        .with_options(vec![
            OptionSpec::bool(
                "alsoWidth",
                "同时给出全角 / 半角转换",
                true,
                "只处理 ASCII 全角字符与全角空格",
            ),
            OptionSpec::bool(
                "forceFix",
                "强制尝试修复",
                false,
                "文本看起来正常时也列出候选",
            ),
        ])
    }

    fn run(&self, input: &str, ctx: &RunCtx) -> Result<ToolResult, String> {
        let broken = looks_broken(input) || ctx.bool("forceFix", false);
        let mut list = if broken {
            candidates(input)
        } else {
            Vec::new()
        };
        list.sort_by_key(|(_, s)| -score(input, s));
        let mut r = ToolResult::new();
        if let Some((kind, best)) = list.first() {
            r = r.section("修复结果", "text", best.clone()).note(format!(
                "按「{kind}」还原；如果还是不对，试试下面的其他候选"
            ));
        } else {
            r = r
                .section("修复结果", "text", input.to_string())
                .note("没有检测到典型乱码特征，原文已保留；确定是乱码时勾选「强制尝试修复」");
        }
        if list.len() > 1 {
            let others = list[1..]
                .iter()
                .map(|(k, s)| format!("【{k}】\n{s}"))
                .collect::<Vec<_>>()
                .join("\n\n");
            r = r.section("其他候选", "text", others);
        }
        if ctx.bool("alsoWidth", true) {
            r = r.section(
                "全角 / 半角",
                "text",
                format!(
                    "全角转半角：{}\n\n半角转全角：{}",
                    to_half(input),
                    to_full(input)
                ),
            );
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
            now_ms: 0,
        }
    }

    #[test]
    fn fixes_latin1_mojibake() {
        // 「你好」的 UTF-8 字节被当成 cp1252 读出：E4 BD A0 E5 A5 BD
        // 注意 A0 必须是不换行空格 U+00A0，换成普通空格后字节就不是合法 UTF-8，无法还原
        let broken = "\u{e4}\u{bd}\u{a0}\u{e5}\u{a5}\u{bd}";
        let r = Mojibake
            .run(broken, &ctx(json!({"alsoWidth": false})))
            .unwrap();
        assert!(
            r.sections[0].text.contains("你好"),
            "got {}",
            r.sections[0].text
        );
    }

    #[test]
    fn keeps_clean_text() {
        let r = Mojibake
            .run("正常中文", &ctx(json!({"alsoWidth": false})))
            .unwrap();
        assert_eq!(r.sections[0].text, "正常中文");
        assert!(r.notes[0].contains("没有检测到"));
    }

    #[test]
    fn width_conversion() {
        assert_eq!(to_half("ＡＢＣ　１２３"), "ABC 123");
        assert_eq!(to_full("ABC 123"), "ＡＢＣ　１２３");
    }
}
