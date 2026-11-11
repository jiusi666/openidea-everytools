//! Java 类的文本级解析：字段、命名转换、字段映射。
//!
//! 没有 AST，只认 `[修饰符] 类型 名称;` 和 `名称: 类型` 两种声明，
//! 跳过方法、常量、注解行。所有 Java 相关工具都从这里拿字段。

use crate::tools::util::opt_str;
use serde_json::Value;

#[derive(Clone, Debug, PartialEq)]
pub struct Field {
    pub ty: String,
    pub name: String,
    /// 字段上方 @JsonProperty / @JSONField / @SerializedName 里的名字。
    pub json_name: Option<String>,
    /// 字段映射前的原名，生成 getter 时用它。
    pub src_name: Option<String>,
}

pub fn is_ident(s: &str) -> bool {
    let mut chars = s.chars();
    match chars.next() {
        Some(c) if c.is_ascii_alphabetic() || c == '_' || c == '$' => {}
        _ => return false,
    }
    chars.all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '$')
}

pub fn parse_fields(input: &str) -> Vec<Field> {
    let mut out: Vec<Field> = Vec::new();
    let mut pending_json: Option<String> = None;
    for raw in input.lines() {
        let line = raw.trim();
        if line.is_empty() {
            continue;
        }
        if line.starts_with('@') {
            if let Some(name) = annotation_name(line) {
                pending_json = Some(name);
            }
            continue;
        }
        if line.starts_with("//") || line.starts_with('*') || line.starts_with("/*") {
            continue;
        }
        let line = match line.find("//") {
            Some(i) => line[..i].trim(),
            None => line,
        };
        if line.is_empty()
            || line.contains('(')
            || line.contains(')')
            || line.contains('{')
            || line.contains('}')
            || line.contains("...")
        {
            pending_json = None;
            continue;
        }
        let lower = line.to_ascii_lowercase();
        if lower.starts_with("package ")
            || lower.starts_with("import ")
            || lower.contains(" class ")
            || lower.starts_with("class ")
            || lower.contains(" interface ")
            || lower.contains(" enum ")
            || lower.contains(" record ")
        {
            pending_json = None;
            continue;
        }
        let mut decl = line.trim_end_matches(';').trim();
        if decl
            .split_whitespace()
            .any(|w| matches!(w, "static" | "final" | "transient" | "volatile"))
        {
            pending_json = None;
            continue;
        }
        if let Some(i) = decl.find('=') {
            decl = decl[..i].trim();
        }
        if decl.is_empty() {
            continue;
        }
        let (ty, name) = match decl.split_once(':') {
            Some((a, b)) if !a.trim().contains(char::is_whitespace) => {
                (b.trim().to_string(), a.trim().to_string())
            }
            _ => split_decl(decl),
        };
        let name = name.trim().to_string();
        let ty = ty.trim().to_string();
        if !is_ident(&name) || ty.is_empty() {
            pending_json = None;
            continue;
        }
        if ty.split_whitespace().any(|w| {
            matches!(
                w,
                "static" | "final" | "transient" | "volatile" | "return" | "new" | "throw"
            )
        }) || ty.contains(';')
            || ty.contains('=')
        {
            pending_json = None;
            continue;
        }
        out.push(Field {
            ty,
            name,
            json_name: pending_json.take(),
            src_name: None,
        });
    }
    out
}

fn split_decl(decl: &str) -> (String, String) {
    let mut words: Vec<&str> = decl.split_whitespace().collect();
    if words.len() < 2 {
        return (String::new(), String::new());
    }
    let name = words.pop().unwrap_or("").to_string();
    let ty = words
        .into_iter()
        .filter(|w| {
            !matches!(
                *w,
                "private" | "public" | "protected" | "static" | "final" | "transient" | "volatile"
            )
        })
        .collect::<Vec<_>>()
        .join(" ");
    (ty, name)
}

fn annotation_name(line: &str) -> Option<String> {
    for marker in ["JsonProperty(", "JSONField(", "SerializedName("] {
        if let Some(i) = line.find(marker) {
            let rest = &line[i + marker.len()..];
            let start = rest.find('"')?;
            let after = &rest[start + 1..];
            let end = after.find('"')?;
            return Some(after[..end].to_string());
        }
    }
    None
}

pub fn pascal(s: &str) -> String {
    let mut out = String::new();
    let mut up = true;
    for c in s.chars() {
        if c.is_ascii_alphanumeric() {
            if up {
                out.push(c.to_ascii_uppercase());
                up = false;
            } else {
                out.push(c);
            }
        } else {
            up = true;
        }
    }
    if out.is_empty() {
        out.push_str("Item");
    }
    if out.chars().next().is_some_and(|c| c.is_ascii_digit()) {
        out.insert(0, 'C');
    }
    out
}

pub fn camel(s: &str) -> String {
    let cleaned: String = s
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { ' ' })
        .collect();
    let mut words = cleaned.split_whitespace();
    let mut out = String::new();
    if let Some(first) = words.next() {
        out.push_str(&first.to_ascii_lowercase());
    }
    for w in words {
        out.push_str(&pascal(w));
    }
    let mut out: String = out
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '_' || *c == '$')
        .collect();
    if out.is_empty() {
        out.push_str("field");
    }
    if out.chars().next().is_some_and(|c| c.is_ascii_digit()) {
        out.insert(0, '_');
    }
    out
}

pub fn snake(s: &str) -> String {
    let mut out = String::new();
    for (i, c) in s.chars().enumerate() {
        if c.is_ascii_uppercase() {
            if i > 0 {
                out.push('_');
            }
            out.push(c.to_ascii_lowercase());
        } else if c.is_ascii_alphanumeric() || c == '_' {
            out.push(c);
        } else {
            out.push('_');
        }
    }
    out
}

pub fn field_name(key: &str, naming: &str) -> String {
    match naming {
        "snake" => snake(key),
        "keep" => {
            let s: String = key
                .chars()
                .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
                .collect();
            let s = s.trim_matches('_').to_string();
            if s.is_empty() {
                "field".to_string()
            } else if s.chars().next().is_some_and(|c| c.is_ascii_digit()) {
                format!("_{s}")
            } else {
                s
            }
        }
        _ => camel(key),
    }
}

pub fn is_primitive(ty: &str) -> bool {
    matches!(
        ty.trim(),
        "int" | "long" | "short" | "byte" | "double" | "float" | "boolean" | "char"
    )
}

pub fn getter_name(f: &Field) -> String {
    let base = f.src_name.as_deref().unwrap_or(&f.name);
    let is_bool = matches!(f.ty.trim(), "boolean" | "Boolean");
    format!("{}{}", if is_bool { "is" } else { "get" }, pascal(base))
}

/// 把 JSON 里的默认值按 Java 类型给出来。泛型只给空容器。
pub fn default_json(ty: &str) -> String {
    let t: String = ty.chars().filter(|c| !c.is_whitespace()).collect();
    let base = t
        .split('<')
        .next()
        .unwrap_or(&t)
        .rsplit('.')
        .next()
        .unwrap_or(&t)
        .trim_end_matches("[]");
    if t.contains("List<") || t.contains("Set<") || t.contains("Collection<") || t.ends_with("[]") {
        return "[]".to_string();
    }
    if t.contains("Map<") {
        return "{}".to_string();
    }
    match base {
        "String" | "CharSequence" | "char" | "Character" => "\"\"".to_string(),
        "boolean" | "Boolean" => "false".to_string(),
        "double" | "Double" | "float" | "Float" | "BigDecimal" => "0.0".to_string(),
        "int" | "Integer" | "long" | "Long" | "short" | "Short" | "byte" | "Byte"
        | "BigInteger" => "0".to_string(),
        "Date" | "LocalDate" | "LocalDateTime" | "Instant" | "Timestamp" => {
            "\"2024-01-01 00:00:00\"".to_string()
        }
        "Optional" => "null".to_string(),
        _ => "{}".to_string(),
    }
}

/// 解析 `旧:新` / `旧=新` 形式的字段映射，返回改名后的字段。
pub fn apply_mapping(fields: &[Field], opt: &Value) -> Vec<Field> {
    let mapping = opt_str(opt, "mapping", "");
    let mut pairs: Vec<(String, String)> = Vec::new();
    for line in mapping.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if let Some((a, b)) = line.split_once(':').or_else(|| line.split_once('=')) {
            let a = a.trim();
            let b = b.trim();
            if !a.is_empty() && !b.is_empty() && is_ident(b) {
                pairs.push((a.to_string(), b.to_string()));
            }
        }
    }
    fields
        .iter()
        .map(|f| match pairs.iter().find(|(from, _)| *from == f.name) {
            Some((_, to)) => Field {
                ty: f.ty.clone(),
                name: to.clone(),
                json_name: f.json_name.clone(),
                src_name: Some(f.name.clone()),
            },
            None => f.clone(),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    const SRC: &str = r#"
public class User {
    private Long id;
    private String userName;
    private List<String> tags;
    private static final long serialVersionUID = 1L;
    public Long getId() { return id; }
    // 注释
    @JsonProperty("user_email")
    private String email;
}
"#;

    #[test]
    fn parses_fields_and_skips_noise() {
        let f = parse_fields(SRC);
        assert_eq!(f.len(), 4);
        assert_eq!(f[0].name, "id");
        assert_eq!(f[1].ty, "String");
        assert_eq!(f[2].ty, "List<String>");
        assert_eq!(f[3].json_name.as_deref(), Some("user_email"));
    }

    #[test]
    fn name_styles() {
        assert_eq!(camel("user_name"), "userName");
        assert_eq!(camel("User-Name"), "userName");
        assert_eq!(camel("2fa"), "_2fa");
        assert_eq!(snake("userName"), "user_name");
        assert_eq!(pascal("user_name"), "UserName");
    }

    #[test]
    fn default_values_follow_type() {
        assert_eq!(default_json("String"), "\"\"");
        assert_eq!(default_json("List<Item>"), "[]");
        assert_eq!(default_json("Map<String, Object>"), "{}");
        assert_eq!(default_json("BigDecimal"), "0.0");
        assert_eq!(default_json("Integer"), "0");
        assert_eq!(default_json("Item"), "{}");
    }

    #[test]
    fn mapping_renames_fields() {
        let f = parse_fields("private String userName;");
        let mapped = apply_mapping(&f, &json!({"mapping": "userName:name"}));
        assert_eq!(mapped[0].name, "name");
        assert_eq!(getter_name(&mapped[0]), "getUserName");
    }
}
