//! 中文乱码修复：把「UTF-8 被当 GBK / Latin-1 读」的文本还原回来。
//!
//! 做法是把当前字符串按原来的错误编码编回字节，再按正确编码解一次。
//!
//! 两个关键点，都是踩过的坑：
//!
//! 1. **逐行判断**。`encoding_rs` 的 `encode()` 遇到编不了的字符不会失败返回空，
//!    而是塞一个 `&#228;` 这样的 HTML 数字实体并把 `had_errors` 置 true。
//!    整段一起编码时，只要有一行编不了，另一行本来能还原的结果也会被一起丢掉。
//! 2. **只接受变短的候选**。还原一定会让字符数减少（多字节合并）；字符数持平和膨胀
//!    都是把正常中文再转一遍的误判，靠这条把它们挡在门外。

use crate::tools::meta::{Group, OptionSpec, ToolMeta};
use crate::tools::util::is_cjk;
use crate::tools::{RunCtx, Tool, ToolResult};
use encoding_rs::{GBK, UTF_8, WINDOWS_1252};

pub struct Mojibake;

const K_GBK: &str = "UTF-8 被当成 GBK/ANSI 读取";
const K_LATIN: &str = "UTF-8 被当成 Latin-1 读取";
const K_UTF8: &str = "GBK 被当成 UTF-8 读取";

fn decode_utf8(bytes: &[u8]) -> Option<String> {
    let (cow, _, had_errors) = UTF_8.decode(bytes);
    (!had_errors).then(|| cow.into_owned())
}

/// 一行文本的三个还原方向。
fn line_candidates(line: &str) -> Vec<(&'static str, String)> {
    let mut out: Vec<(&'static str, String)> = Vec::new();
    let (gbk, _, gbk_err) = GBK.encode(line);
    if !gbk_err && let Some(s) = decode_utf8(&gbk) {
        out.push((K_GBK, s));
    }
    let (latin, _, latin_err) = WINDOWS_1252.encode(line);
    if !latin_err && let Some(s) = decode_utf8(&latin) {
        out.push((K_LATIN, s));
    }
    let (utf8, _, _) = UTF_8.encode(line);
    let (decoded, _, _) = GBK.decode(&utf8);
    let s = decoded.into_owned();
    if s != line {
        out.push((K_UTF8, s));
    }
    out
}

fn cjk_count(s: &str) -> i32 {
    s.chars().filter(|c| is_cjk(*c)).count() as i32
}

/// 候选得分：小于等于 0 表示「这不是还原，是另一种破坏」。
fn score_line(line: &str, cand: &str) -> i32 {
    if cand.contains('\u{fffd}') {
        return i32::MIN;
    }
    let shrink = line.chars().count() as i32 - cand.chars().count() as i32;
    if shrink <= 0 {
        return i32::MIN;
    }
    shrink * 5 + (cjk_count(cand) - cjk_count(line)) * 3
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
    ];
    latin_high >= 2 || weird.iter().any(|w| s.contains(w))
}

/// 逐行还原。返回（还原后的全文、各方向命中行数、未能识别的行号）。
fn fix_lines(input: &str) -> (String, Vec<(&'static str, usize)>, Vec<usize>) {
    let mut out: Vec<String> = Vec::new();
    let mut kinds: Vec<(&'static str, usize)> = Vec::new();
    let mut missed: Vec<usize> = Vec::new();
    for (i, line) in input.split('\n').enumerate() {
        let mut best: Option<(i32, &'static str, String)> = None;
        for (kind, cand) in line_candidates(line) {
            if cand == line {
                continue;
            }
            let score = score_line(line, &cand);
            if score > 0 && best.as_ref().is_none_or(|(b, _, _)| score > *b) {
                best = Some((score, kind, cand));
            }
        }
        match best {
            Some((_, kind, text)) => {
                match kinds.iter_mut().find(|(k, _)| *k == kind) {
                    Some((_, n)) => *n += 1,
                    None => kinds.push((kind, 1)),
                }
                out.push(text);
            }
            None => {
                if !line.trim().is_empty() && looks_broken(line) {
                    missed.push(i + 1);
                }
                out.push(line.to_string());
            }
        }
    }
    (out.join("\n"), kinds, missed)
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
            "还原「UTF-8 被当 GBK / Latin-1 读」的乱码，逐行判断，并给全角半角转换",
            "鎴戜滑\n浠婂ぉ澶╂皵寰堝ソ",
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
                "文本看起来正常时也逐行尝试",
            ),
        ])
    }

    fn run(&self, input: &str, ctx: &RunCtx) -> Result<ToolResult, String> {
        let force = ctx.bool("forceFix", false);
        let mut r = ToolResult::new();

        if !looks_broken(input) && !force {
            r = r
                .section("修复结果", "text", input.to_string())
                .note("没有检测到典型乱码特征，原文已保留；确定是乱码时勾选「强制尝试修复」");
        } else {
            let (fixed, kinds, missed) = fix_lines(input);
            let total: usize = kinds.iter().map(|(_, n)| *n).sum();
            if total == 0 {
                r = r.section("修复结果", "text", input.to_string()).note(
                    "认出了乱码特征但还原不出可读文本——常见于字节已残损的文本（例如多字节里的 0xA0 被换成了普通空格），原始字节丢了，没法逆推",
                );
            } else {
                r = r.section("修复结果", "text", fixed);
                let mut detail: Vec<String> = kinds
                    .iter()
                    .map(|(k, n)| format!("{n} 行按「{k}」还原"))
                    .collect();
                if !missed.is_empty() {
                    detail.push(format!(
                        "第 {} 行没认出来，已保留原文",
                        missed
                            .iter()
                            .map(|i| i.to_string())
                            .collect::<Vec<_>>()
                            .join("、")
                    ));
                }
                r = r.note(detail.join("；"));
            }
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

    fn run(input: &str) -> ToolResult {
        Mojibake
            .run(input, &ctx(json!({"alsoWidth": false})))
            .unwrap()
    }

    #[test]
    fn fixes_gbk_misread() {
        // 「我们」的 UTF-8 字节被当成 GBK 读
        assert_eq!(run("鎴戜滑").sections[0].text, "我们");
    }

    #[test]
    fn fixes_latin1_mojibake() {
        // 「你好」的 UTF-8 字节被当成 cp1252 读出：E4 BD A0 E5 A5 BD
        // 注意 A0 必须是不换行空格 U+00A0，换成普通空格后字节就不是合法 UTF-8，无法还原
        let broken = "\u{e4}\u{bd}\u{a0}\u{e5}\u{a5}\u{bd}";
        assert_eq!(run(broken).sections[0].text, "你好");
    }

    /// 这是改逐行判断的直接原因：第二行编不了 GBK，不能连累第一行。
    #[test]
    fn fixes_each_line_independently() {
        let r = run("鎴戜滑\n\u{e4}\u{bd}\u{a0}\u{e5}\u{a5}\u{bd}");
        assert_eq!(r.sections[0].text, "我们\n你好");
    }

    #[test]
    fn keeps_clean_text() {
        let r = run("正常中文");
        assert_eq!(r.sections[0].text, "正常中文");
        assert!(r.notes[0].contains("没有检测到"));
    }

    /// 已经正常的中文不能被「修」成乱码：三个方向要么编不了，要么只会让字变多。
    #[test]
    fn never_expands_clean_text() {
        for text in ["正常中文", "你好世界", "这是一段中文说明"] {
            let out = fix_lines(text);
            assert_eq!(out.0, text, "{text} 被改成了 {:?}", out.0);
        }
    }

    /// 缺了字节的残损文本不要瞎猜，要说清楚救不回来。
    #[test]
    fn reports_unrecoverable() {
        let broken = "\u{e4}\u{bd} \u{e5}\u{a5}\u{bd}";
        let r = run(broken);
        assert_eq!(r.sections[0].text, broken);
        assert!(r.notes[0].contains("没法逆推"), "notes: {:?}", r.notes);
    }

    #[test]
    fn width_conversion() {
        assert_eq!(to_half("ＡＢＣ　１２３"), "ABC 123");
        assert_eq!(to_full("ABC 123"), "ＡＢＣ　１２３");
    }
}
