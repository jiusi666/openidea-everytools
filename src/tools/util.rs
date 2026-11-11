//! 工具之间共用的小工具函数。这里不放业务逻辑。

use serde_json::Value;

/// 单次输入上限，避免把整份文件塞进来把 WASM 撑爆。
pub const MAX_INPUT_BYTES: usize = 256 * 1024;

pub fn truncate_utf8(s: &str, max: usize) -> &str {
    if s.len() <= max {
        return s;
    }
    let mut end = max;
    while end > 0 && !s.is_char_boundary(end) {
        end -= 1;
    }
    &s[..end]
}

/// 按行拆开并丢弃空行，绝大多数工具都是这个口径。
pub fn lines(input: &str) -> Vec<&str> {
    input
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .collect()
}

pub fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

pub fn java_escape(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

pub fn json_escape(s: &str) -> String {
    serde_json::to_string(s).unwrap_or_else(|_| "\"\"".to_string())
}

pub fn is_cjk(c: char) -> bool {
    matches!(c, '\u{3400}'..='\u{4dbf}' | '\u{4e00}'..='\u{9fff}' | '\u{f900}'..='\u{faff}')
}

pub fn opt_str<'a>(opt: &'a Value, key: &str, default: &'a str) -> &'a str {
    opt.get(key).and_then(Value::as_str).unwrap_or(default)
}

pub fn opt_bool(opt: &Value, key: &str, default: bool) -> bool {
    opt.get(key).and_then(Value::as_bool).unwrap_or(default)
}

pub fn opt_i64(opt: &Value, key: &str, default: i64) -> i64 {
    opt.get(key)
        .and_then(|v| v.as_i64().or_else(|| v.as_f64().map(|f| f as i64)))
        .unwrap_or(default)
}

pub fn opt_usize(opt: &Value, key: &str, default: usize, min: usize, max: usize) -> usize {
    (opt_i64(opt, key, default as i64).max(min as i64) as usize).min(max)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn truncate_keeps_char_boundary() {
        assert_eq!(truncate_utf8("中文abc", 4), "中");
        assert_eq!(truncate_utf8("abc", 10), "abc");
    }

    #[test]
    fn escapes() {
        assert_eq!(xml_escape("a<b&c"), "a&lt;b&amp;c");
        assert_eq!(java_escape("a\"b"), "a\\\"b");
        assert_eq!(json_escape("中"), "\"中\"");
    }
}
