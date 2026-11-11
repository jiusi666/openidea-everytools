//! JSON → Java 类：嵌套对象生成静态内部类，数组生成 List<T>。

use crate::tools::java::parse::{field_name, pascal};
use crate::tools::meta::{Group, OptionSpec, ToolMeta};
use crate::tools::util::java_escape;
use crate::tools::{RunCtx, Tool, ToolResult};
use serde_json::Value;
use std::collections::HashSet;

pub struct JsonToPojo;

impl Tool for JsonToPojo {
    fn meta(&self) -> ToolMeta {
        ToolMeta::new(
            "json-to-pojo",
            "JSON → POJO",
            Group::Code,
            "把 JSON 样本变成 Java 类，嵌套对象生成静态内部类，数组推断成 List<T>",
            "{\n  \"id\": 1,\n  \"user_name\": \"张三\",\n  \"tags\": [\"vip\"],\n  \"profile\": { \"age\": 18 }\n}",
            true,
            "java",
        )
        .with_options(vec![
            OptionSpec::text("className", "根类名", "", "Root / Item", "留空时对象用 Root、数组用 Item"),
            OptionSpec::text("javaPackage", "包名", "", "com.example.dto", "留空不输出 package"),
            OptionSpec::bool("lombok", "使用 Lombok @Data", true, "关闭后展开 getter / setter"),
            OptionSpec::bool("jackson", "补 @JsonProperty", false, "键名与字段名不一致时加注解"),
            OptionSpec::select(
                "fieldNaming",
                "字段命名",
                "camel",
                &[("camel", "camelCase"), ("snake", "snake_case"), ("keep", "保持原样")],
                "JSON 键转 Java 字段名的规则",
            ),
        ])
    }

    fn run(&self, input: &str, ctx: &RunCtx) -> Result<ToolResult, String> {
        generate(input, ctx)
    }
}

struct ClassDef {
    name: String,
    fields: Vec<FieldDef>,
}

struct FieldDef {
    ty: String,
    name: String,
    key: String,
}

struct Gen {
    naming: String,
    nested: Vec<ClassDef>,
    used_classes: HashSet<String>,
}

impl Gen {
    fn infer(&mut self, v: &Value, key: &str) -> String {
        match v {
            Value::Object(_) => {
                let name = self.unique_class(&pascal(key));
                let class = self.build_class(v, &name);
                self.nested.push(class);
                name
            }
            Value::Array(items) => {
                let inner = match items.first() {
                    Some(first) => self.infer(first, &singular(key)),
                    None => "Object".to_string(),
                };
                format!("List<{inner}>")
            }
            Value::String(_) => "String".to_string(),
            Value::Bool(_) => "Boolean".to_string(),
            Value::Number(n) if n.is_i64() || n.is_u64() => "Long".to_string(),
            Value::Number(_) => "Double".to_string(),
            Value::Null => "Object".to_string(),
        }
    }

    fn unique_class(&mut self, raw: &str) -> String {
        if self.used_classes.insert(raw.to_string()) {
            return raw.to_string();
        }
        let mut i = 2;
        loop {
            let name = format!("{raw}{i}");
            if self.used_classes.insert(name.clone()) {
                return name;
            }
            i += 1;
        }
    }

    fn build_class(&mut self, v: &Value, name: &str) -> ClassDef {
        let mut fields = Vec::new();
        let mut used: HashSet<String> = HashSet::new();
        if let Value::Object(map) = v {
            for (key, val) in map.iter() {
                let mut fname = field_name(key, &self.naming);
                if !used.insert(fname.clone()) {
                    let mut i = 2;
                    loop {
                        let candidate = format!("{fname}{i}");
                        if used.insert(candidate.clone()) {
                            fname = candidate;
                            break;
                        }
                        i += 1;
                    }
                }
                let ty = self.infer(val, key);
                fields.push(FieldDef {
                    ty,
                    name: fname,
                    key: key.clone(),
                });
            }
        }
        ClassDef {
            name: name.to_string(),
            fields,
        }
    }
}

fn singular(key: &str) -> String {
    let lower = key.to_ascii_lowercase();
    if let Some(base) = lower.strip_suffix("ies") {
        return format!("{base}y");
    }
    for suffix in ["ses", "xes", "zes", "ches", "shes"] {
        if let Some(base) = lower.strip_suffix(suffix) {
            return base.to_string();
        }
    }
    if lower.ends_with('s') && !lower.ends_with("ss") && lower.len() > 1 {
        return lower[..lower.len() - 1].to_string();
    }
    lower
}

fn write_fields(out: &mut String, fields: &[FieldDef], indent: usize, jackson: bool) {
    let pad = " ".repeat(indent);
    if fields.is_empty() {
        out.push_str(&format!("{pad}// 空对象\n"));
    }
    for f in fields {
        if jackson && f.key != f.name {
            out.push_str(&format!(
                "{pad}@JsonProperty(\"{}\")\n",
                java_escape(&f.key)
            ));
        }
        out.push_str(&format!("{pad}private {} {};\n", f.ty, f.name));
    }
}

fn write_accessors(out: &mut String, fields: &[FieldDef], indent: usize) {
    let pad = " ".repeat(indent);
    for f in fields {
        let cap = pascal(&f.name);
        out.push_str(&format!(
            "\n{pad}public {} get{cap}() {{\n{pad}    return {name};\n{pad}}}\n",
            f.ty,
            name = f.name
        ));
        out.push_str(&format!(
            "{pad}public void set{cap}({} {}) {{\n{pad}    this.{name} = {name};\n{pad}}}\n",
            f.ty,
            f.name,
            name = f.name
        ));
    }
}

fn render_nested(def: &ClassDef, indent: usize, lombok: bool, jackson: bool, out: &mut String) {
    let pad = " ".repeat(indent);
    if lombok {
        out.push_str(&format!("{pad}@Data\n"));
    }
    out.push_str(&format!("{pad}public static class {} {{\n", def.name));
    write_fields(out, &def.fields, indent + 4, jackson);
    if !lombok {
        write_accessors(out, &def.fields, indent + 4);
    }
    out.push_str(&format!("{pad}}}\n"));
}

fn render_root(def: &ClassDef, nested: &[ClassDef], lombok: bool, jackson: bool) -> String {
    let mut out = String::new();
    if lombok {
        out.push_str("@Data\n");
    }
    out.push_str(&format!("public class {} {{\n", def.name));
    write_fields(&mut out, &def.fields, 4, jackson);
    if !lombok {
        write_accessors(&mut out, &def.fields, 4);
    }
    for c in nested {
        out.push('\n');
        render_nested(c, 4, lombok, jackson, &mut out);
    }
    out.push_str("}\n");
    out
}

fn has_list(def: &ClassDef) -> bool {
    def.fields.iter().any(|f| f.ty.contains("List<"))
}

fn generate(input: &str, ctx: &RunCtx) -> Result<ToolResult, String> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err("粘一段 JSON 进来，对象或对象数组都行".to_string());
    }
    let value: Value = serde_json::from_str(trimmed).map_err(|e| format!("JSON 解析失败：{e}"))?;
    let package = ctx.str("javaPackage", "").trim().to_string();
    let class_opt = ctx.str("className", "").trim();
    let lombok = ctx.bool("lombok", true);
    let jackson = ctx.bool("jackson", false);
    let naming = ctx.str("fieldNaming", "camel");
    let (root, default_name) = match &value {
        Value::Array(items) => match items.first() {
            Some(first) if first.is_object() => (first.clone(), "Item"),
            _ => return Err("顶层数组里需要有对象".to_string()),
        },
        Value::Object(_) => (value.clone(), "Root"),
        _ => return Err("顶层需要是 JSON 对象或对象数组".to_string()),
    };
    let root_name = if class_opt.is_empty() {
        default_name.to_string()
    } else {
        pascal(class_opt)
    };
    let mut generator = Gen {
        naming: naming.to_string(),
        nested: Vec::new(),
        used_classes: HashSet::new(),
    };
    generator.used_classes.insert(root_name.clone());
    let root_class = generator.build_class(&root, &root_name);
    let nested = generator.nested;
    let code = render_root(&root_class, &nested, lombok, jackson);

    let mut out = String::new();
    if !package.is_empty() {
        out.push_str(&format!("package {package};\n\n"));
    }
    if has_list(&root_class) || nested.iter().any(has_list) {
        out.push_str("import java.util.List;\n");
    }
    if jackson {
        out.push_str("import com.fasterxml.jackson.annotation.JsonProperty;\n");
    }
    if lombok {
        out.push_str("import lombok.Data;\n");
    }
    if !out.is_empty() {
        out.push('\n');
    }
    out.push_str(&code);

    let field_count: usize =
        root_class.fields.len() + nested.iter().map(|c| c.fields.len()).sum::<usize>();
    let mut r = ToolResult::new()
        .section("Java 类", "java", out)
        .note(format!(
            "{} 个字段 / {} 个嵌套类；数字按 Long 或 Double 推断，需要 int、BigDecimal 时手改",
            field_count,
            nested.len()
        ));
    if lombok {
        r = r.note("用了 Lombok @Data，确认项目里有 lombok 依赖");
    }
    Ok(r)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn ctx(opt: Value) -> RunCtx<'static> {
        let leaked: &'static Value = Box::leak(Box::new(opt));
        RunCtx {
            options: leaked,
            now_ms: 0,
        }
    }

    #[test]
    fn generates_nested_and_list() {
        let input = r#"{"id":1,"user_name":"张三","tags":["a"],"profile":{"age":18}}"#;
        let r = generate(input, &ctx(json!({"lombok": true}))).unwrap();
        let code = &r.sections[0].text;
        assert!(code.contains("public class Root"));
        assert!(code.contains("private Long id;"));
        assert!(code.contains("private String userName;"));
        assert!(code.contains("private List<String> tags;"));
        assert!(code.contains("public static class Profile"));
        assert!(code.contains("import java.util.List;"));
        assert!(code.contains("@Data"));
    }

    #[test]
    fn array_uses_item_and_jackson() {
        let r = generate(r#"[{"user_name":"x"}]"#, &ctx(json!({"jackson": true}))).unwrap();
        let code = &r.sections[0].text;
        assert!(code.contains("public class Item"));
        assert!(code.contains("@JsonProperty(\"user_name\")"));
    }

    #[test]
    fn package_and_custom_name() {
        let r = generate(
            r#"{"a":1}"#,
            &ctx(json!({"javaPackage": "com.demo", "className": "demo dto"})),
        )
        .unwrap();
        let code = &r.sections[0].text;
        assert!(code.starts_with("package com.demo;"));
        assert!(code.contains("public class DemoDto"));
    }
}
