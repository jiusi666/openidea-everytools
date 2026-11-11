//! SQL → MyBatis Mapper XML。
//!
//! 纯文本解析：按顶层 WHERE / SET / VALUES 切分，括号和引号内的 and/or
//! 不会被误切。生成的 `<if>` 是常见的「按条件拼 where」形态，复杂动态 SQL
//! 仍需要手改。

use crate::tools::meta::{Group, OptionSpec, ToolMeta};
use crate::tools::util::{lines, xml_escape};
use crate::tools::{RunCtx, Tool, ToolResult};

pub struct SqlMapper;

const KEYWORDS: &[&str] = &[
    "select", "from", "where", "and", "or", "order", "group", "by", "having", "limit", "offset",
    "insert", "into", "values", "update", "set", "delete", "join", "left", "right", "inner",
    "outer", "on", "as", "like", "in", "not", "null", "is", "desc", "asc", "union", "all",
    "distinct", "case", "when", "then", "else", "end", "between",
];

impl Tool for SqlMapper {
    fn meta(&self) -> ToolMeta {
        ToolMeta::new(
            "sql-mapper",
            "SQL → Mapper XML",
            Group::Code,
            "把一条普通 SQL 变成带 <where> / <if> / <set> 的 MyBatis 片段",
            "SELECT id, name FROM user\nWHERE id = ? AND name = ?\nORDER BY id DESC",
            true,
            "xml",
        )
        .with_options(vec![
            OptionSpec::text(
                "statementId",
                "语句 id",
                "selectByCondition",
                "selectById",
                "生成 <select id=\"…\">",
            ),
            OptionSpec::text(
                "params",
                "参数名",
                "id, name",
                "id, name, status",
                "按 ? 的顺序填，逗号或空格分隔",
            ),
            OptionSpec::text(
                "resultType",
                "resultType",
                "",
                "com.example.User",
                "留空不写该属性",
            ),
            OptionSpec::text(
                "parameterType",
                "parameterType",
                "map",
                "map",
                "留空不写该属性",
            ),
            OptionSpec::bool(
                "smartString",
                "字符串条件补 != ''",
                false,
                "只对 = 条件加，数字字段请关闭",
            ),
            OptionSpec::bool(
                "uppercaseSql",
                "SQL 关键字大写",
                true,
                "只影响输出，不改语义",
            ),
        ])
    }

    fn run(&self, input: &str, ctx: &RunCtx) -> Result<ToolResult, String> {
        generate(input, ctx)
    }
}

fn is_word(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_'
}

fn find_top(sql: &str, kw: &str) -> Option<usize> {
    let bytes = sql.as_bytes();
    let lower = sql.to_ascii_lowercase();
    let lower = lower.as_bytes();
    let mut depth = 0i32;
    let mut quote: Option<u8> = None;
    let mut i = 0;
    while i < bytes.len() {
        let c = bytes[i];
        if let Some(q) = quote {
            if c == q {
                quote = None;
            }
            i += 1;
            continue;
        }
        match c {
            b'\'' | b'"' | b'`' => {
                quote = Some(c);
                i += 1;
            }
            b'(' => {
                depth += 1;
                i += 1;
            }
            b')' => {
                depth -= 1;
                i += 1;
            }
            _ => {
                if depth == 0 && lower[i..].starts_with(kw.as_bytes()) {
                    let before_ok = i == 0 || !is_word(bytes[i - 1]);
                    let after = i + kw.len();
                    let after_ok = after >= bytes.len() || !is_word(bytes[after]);
                    if before_ok && after_ok {
                        return Some(i);
                    }
                }
                i += 1;
            }
        }
    }
    None
}

/// 按顶层 and/or 切分 where 子句，保留连接词。
fn split_conditions(clause: &str) -> Vec<(String, String)> {
    let bytes = clause.as_bytes();
    let lower = clause.to_ascii_lowercase();
    let lower = lower.as_bytes();
    let mut depth = 0i32;
    let mut quote: Option<u8> = None;
    let mut out = Vec::new();
    let mut last = 0usize;
    let mut connector = String::new();
    let mut i = 0;
    while i < bytes.len() {
        let c = bytes[i];
        if let Some(q) = quote {
            if c == q {
                quote = None;
            }
            i += 1;
            continue;
        }
        match c {
            b'\'' | b'"' | b'`' => {
                quote = Some(c);
                i += 1;
            }
            b'(' => {
                depth += 1;
                i += 1;
            }
            b')' => {
                depth -= 1;
                i += 1;
            }
            _ => {
                if depth == 0 && clause.is_char_boundary(i) {
                    for kw in ["and", "or"] {
                        if lower[i..].starts_with(kw.as_bytes()) {
                            let before_ok = i == 0 || !is_word(bytes[i - 1]);
                            let after = i + kw.len();
                            let after_ok = after >= bytes.len() || !is_word(bytes[after]);
                            if before_ok && after_ok {
                                let part = clause[last..i].trim();
                                if !part.is_empty() {
                                    out.push((connector.clone(), part.to_string()));
                                }
                                connector = kw.to_uppercase();
                                last = after;
                                i = after;
                                break;
                            }
                        }
                    }
                }
                i += 1;
            }
        }
    }
    let part = clause[last..].trim();
    if !part.is_empty() {
        out.push((connector, part.to_string()));
    }
    out
}

struct Cursor {
    names: Vec<String>,
    idx: usize,
}

impl Cursor {
    fn next(&mut self) -> String {
        let name = self
            .names
            .get(self.idx)
            .cloned()
            .unwrap_or_else(|| format!("p{}", self.idx + 1));
        self.idx += 1;
        name
    }
}

/// 把条件里的 ? 换成 #{x}，返回新条件和用到的参数名。
/// 按字符推进，字符串字面量里的中文不会被拆坏。
fn fill_params(cond: &str, cursor: &mut Cursor) -> (String, Vec<String>) {
    let chars: Vec<(usize, char)> = cond.char_indices().collect();
    let mut out = String::new();
    let mut used: Vec<String> = Vec::new();
    let mut quote: Option<char> = None;
    let mut k = 0usize;
    while k < chars.len() {
        let (i, c) = chars[k];
        if let Some(q) = quote {
            out.push(c);
            if c == q {
                quote = None;
            }
            k += 1;
            continue;
        }
        match c {
            '\'' | '"' | '`' => {
                quote = Some(c);
                out.push(c);
                k += 1;
            }
            '?' => {
                let name = cursor.next();
                out.push_str(&format!("#{{{name}}}"));
                used.push(name);
                k += 1;
            }
            '#' if cond.get(i + 1..).is_some_and(|rest| rest.starts_with('{')) => {
                match cond[i + 2..].find('}') {
                    Some(rel) => {
                        let end = i + 2 + rel;
                        let name = cond[i + 2..end].trim().to_string();
                        used.push(name);
                        out.push_str(&cond[i..=end]);
                        while k < chars.len() && chars[k].0 <= end {
                            k += 1;
                        }
                    }
                    None => {
                        out.push('#');
                        k += 1;
                    }
                }
            }
            _ => {
                out.push(c);
                k += 1;
            }
        }
    }
    (out, used)
}

fn test_expr(used: &[String], cond: &str, smart_string: bool) -> String {
    let mut parts: Vec<String> = Vec::new();
    for name in used.iter().take(1) {
        parts.push(format!("{name} != null"));
    }
    if smart_string && used.len() == 1 && cond.contains('=') && !cond.contains("<") {
        let name = &used[0];
        parts.push(format!("{name} != ''"));
    }
    if parts.is_empty() {
        "true".to_string()
    } else {
        parts.join(" and ")
    }
}

fn render_if(connector: &str, cond: &str, used: &[String], smart_string: bool) -> String {
    let test = test_expr(used, cond, smart_string);
    let prefix = if connector.is_empty() {
        "AND"
    } else {
        connector
    };
    format!(
        "        <if test=\"{}\">\n            {} {}\n        </if>\n",
        xml_escape(&test),
        prefix,
        xml_escape(cond)
    )
}

fn uppercase_keywords(sql: &str) -> String {
    let mut out = String::new();
    let mut word = String::new();
    let mut quote: Option<char> = None;
    let flush = |word: &mut String, out: &mut String| {
        if word.is_empty() {
            return;
        }
        let lower = word.to_ascii_lowercase();
        if KEYWORDS.contains(&lower.as_str()) {
            out.push_str(&word.to_ascii_uppercase());
        } else {
            out.push_str(word);
        }
        word.clear();
    };
    for c in sql.chars() {
        if let Some(q) = quote {
            out.push(c);
            if c == q {
                quote = None;
            }
            continue;
        }
        if matches!(c, '\'' | '"' | '`') {
            flush(&mut word, &mut out);
            quote = Some(c);
            out.push(c);
        } else if c.is_ascii_alphanumeric() || c == '_' {
            word.push(c);
        } else {
            flush(&mut word, &mut out);
            out.push(c);
        }
    }
    flush(&mut word, &mut out);
    out
}

fn table_name(sql: &str) -> Option<String> {
    let from = find_top(sql, "from")?;
    let rest = sql[from + 4..].trim_start();
    let name: String = rest
        .chars()
        .take_while(|c| c.is_ascii_alphanumeric() || *c == '_' || *c == '.' || *c == '`')
        .collect();
    let name = name.trim_matches('`').to_string();
    if name.is_empty() { None } else { Some(name) }
}

fn parse_names(raw: &str) -> Vec<String> {
    raw.split([',', ' ', '\n', '\t'])
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|s| {
            s.trim_start_matches('#')
                .trim_start_matches('{')
                .trim_end_matches('}')
                .to_string()
        })
        .collect()
}

fn attr(name: &str, value: &str) -> String {
    if value.trim().is_empty() {
        String::new()
    } else {
        format!(" {}=\"{}\"", name, xml_escape(value.trim()))
    }
}

fn generate(input: &str, ctx: &RunCtx) -> Result<ToolResult, String> {
    let raw = lines(input).join("\n");
    if raw.trim().is_empty() {
        return Err("粘一条 SQL 进来".to_string());
    }
    let sql = if ctx.bool("uppercaseSql", true) {
        uppercase_keywords(&raw)
    } else {
        raw.clone()
    };
    let upper = sql.to_ascii_uppercase();
    let kind = if upper.trim_start().starts_with("SELECT") {
        "select"
    } else if upper.trim_start().starts_with("UPDATE") {
        "update"
    } else if upper.trim_start().starts_with("INSERT") {
        "insert"
    } else if upper.trim_start().starts_with("DELETE") {
        "delete"
    } else {
        "select"
    };
    let id = {
        let v = ctx.str("statementId", "selectByCondition").trim();
        if v.is_empty() {
            "selectByCondition".to_string()
        } else {
            v.to_string()
        }
    };
    let mut cursor = Cursor {
        names: parse_names(ctx.str("params", "")),
        idx: 0,
    };
    let smart = ctx.bool("smartString", false);
    let mut notes: Vec<String> = Vec::new();

    let mut body = String::new();
    let mut condition_count = 0usize;

    if kind == "insert" {
        let values_at = find_top(&sql, "values").ok_or("INSERT 里没找到 VALUES".to_string())?;
        let head = sql[..values_at].trim();
        let values = sql[values_at + 6..].trim();
        let cols = head
            .find('(')
            .and_then(|start| head[start..].find(')').map(|end| (start, start + end)))
            .map(|(s, e)| {
                head[s + 1..e]
                    .split(',')
                    .map(|c| c.trim().trim_matches('`').to_string())
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        let mut inner = String::new();
        let mut idx = 0usize;
        for ch in values.chars() {
            if ch == '?' {
                let name = cols
                    .get(idx)
                    .cloned()
                    .unwrap_or_else(|| format!("p{}", idx + 1));
                inner.push_str(&format!("#{{{name}}}"));
                idx += 1;
            } else {
                inner.push(ch);
            }
        }
        body.push_str(&format!(
            "    {}\n    VALUES {}\n",
            xml_escape(head),
            xml_escape(&inner)
        ));
        cursor.idx = idx;
        notes.push(format!("按列名生成了 {} 个 #{{}} 占位符", idx));
    } else if kind == "update" {
        let set_at = find_top(&sql, "set").ok_or("UPDATE 里没找到 SET".to_string())?;
        let head = sql[..set_at].trim();
        let rest = &sql[set_at + 3..];
        let where_at = find_top(rest, "where");
        let (set_clause, where_clause, tail) = match where_at {
            Some(w) => (&rest[..w], Some(&rest[w + 5..]), ""),
            None => (rest, None, ""),
        };
        let mut set_body = String::new();
        for item in set_clause.split(',') {
            let item = item.trim();
            if item.is_empty() {
                continue;
            }
            let (cond, used) = fill_params(item, &mut cursor);
            let test = test_expr(&used, &cond, false);
            set_body.push_str(&format!(
                "        <if test=\"{}\">\n            {},\n        </if>\n",
                xml_escape(&test),
                xml_escape(&cond)
            ));
            condition_count += 1;
        }
        body.push_str(&format!("    {}\n", xml_escape(head)));
        body.push_str("    <set>\n");
        body.push_str(&set_body);
        body.push_str("    </set>\n");
        if let Some(wc) = where_clause {
            let (conds, tail2) = split_tail(wc);
            if !conds.trim().is_empty() {
                body.push_str("    <where>\n");
                for (conn, cond) in split_conditions(conds) {
                    let (filled, used) = fill_params(&cond, &mut cursor);
                    body.push_str(&render_if(&conn, &filled, &used, smart));
                    condition_count += 1;
                }
                body.push_str("    </where>\n");
            }
            if !tail2.trim().is_empty() {
                body.push_str(&format!("    {}\n", xml_escape(tail2.trim())));
            }
        }
        let _ = tail;
    } else if kind == "delete" {
        let head = sql.trim();
        body.push_str(&format!("    {}\n", xml_escape(head)));
        notes.push("DELETE 已原样输出，需要按条件拼 where 时请手动改写".to_string());
    } else {
        let where_at = find_top(&sql, "where");
        if let Some(w) = where_at {
            let head = sql[..w].trim();
            let rest = &sql[w + 5..];
            let (where_clause, tail) = split_tail(rest);
            body.push_str(&format!("    {}\n", xml_escape(head)));
            body.push_str("    <where>\n");
            for (conn, cond) in split_conditions(where_clause) {
                let (filled, used) = fill_params(&cond, &mut cursor);
                body.push_str(&render_if(&conn, &filled, &used, smart));
                condition_count += 1;
            }
            body.push_str("    </where>\n");
            if !tail.trim().is_empty() {
                body.push_str(&format!("    {}\n", xml_escape(tail.trim())));
            }
        } else {
            body.push_str(&format!("    {}\n", xml_escape(sql.trim())));
        }
    }

    let result_type = ctx.str("resultType", "");
    let parameter_type = ctx.str("parameterType", "map");
    let xml = format!(
        "<{kind} id=\"{id}\"{}{}>\n{body}</{kind}>\n",
        attr("parameterType", parameter_type),
        attr("resultType", result_type)
    );
    if let Some(t) = table_name(&sql) {
        notes.insert(0, format!("表：{t}；生成 {condition_count} 个条件分支"));
    }
    if condition_count == 0 && kind != "insert" {
        notes.push("没有识别到 where 条件，已原样输出 SQL".to_string());
    }
    Ok(ToolResult::new()
        .section("Mapper XML", "xml", xml)
        .note(notes.join("；")))
}

/// 把 where 之后的内容拆成「条件部分」和「group by / order by / limit 尾巴」。
fn split_tail(rest: &str) -> (&str, &str) {
    let mut cut = rest.len();
    for kw in ["group by", "order by", "limit", "having"] {
        if let Some(i) = find_top(rest, kw)
            && i < cut
        {
            cut = i;
        }
    }
    (&rest[..cut], &rest[cut..])
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
    fn select_generates_where_and_if() {
        let sql = "select id, name from user where id = ? and name = ? order by id desc";
        let r = SqlMapper
            .run(sql, &ctx(json!({"params":"id, name"})))
            .unwrap();
        let x = &r.sections[0].text;
        assert!(x.contains("<select id=\"selectByCondition\""));
        assert!(x.contains("<where>"));
        assert!(x.contains("<if test=\"id != null\">"));
        assert!(x.contains("AND id = #{id}"));
        assert!(x.contains("AND name = #{name}"));
        assert!(x.contains("ORDER BY"));
        assert!(x.contains("</select>"));
    }

    #[test]
    fn nested_parens_are_not_split() {
        let sql = "select * from t where (a = ? or b = ?) and c = ?";
        let r = SqlMapper.run(sql, &ctx(json!({"params":"a,b,c"}))).unwrap();
        assert_eq!(r.sections[0].text.matches("<if test=").count(), 2);
        assert!(r.sections[0].text.contains("(a = #{a} OR b = #{b})"));
    }

    #[test]
    fn insert_uses_column_names() {
        let sql = "insert into user (name, age) values (?, ?)";
        let r = SqlMapper.run(sql, &ctx(json!({}))).unwrap();
        let x = &r.sections[0].text;
        assert!(x.contains("<insert"));
        assert!(x.contains("#{name}"));
        assert!(x.contains("#{age}"));
    }

    #[test]
    fn update_uses_set() {
        let sql = "update user set name = ?, age = ? where id = ?";
        let r = SqlMapper
            .run(sql, &ctx(json!({"params":"name,age,id"})))
            .unwrap();
        let x = &r.sections[0].text;
        assert!(x.contains("<update"));
        assert!(x.contains("<set>"));
        assert!(x.contains("<where>"));
        assert!(x.contains("name = #{name}"));
    }

    #[test]
    fn smart_string_adds_empty_check() {
        let sql = "select * from user where name = ?";
        let r = SqlMapper
            .run(sql, &ctx(json!({"params":"name","smartString":true})))
            .unwrap();
        assert!(r.sections[0].text.contains("name != ''"));
    }
}
