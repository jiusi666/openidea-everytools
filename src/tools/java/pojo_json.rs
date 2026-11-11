//! Java 类 → 示例 JSON：按字段类型给默认值，保持字段顺序。

use crate::tools::java::parse::{default_json, parse_fields, snake};
use crate::tools::meta::{Group, OptionSpec, ToolMeta};
use crate::tools::util::json_escape;
use crate::tools::{RunCtx, Tool, ToolResult};

pub struct PojoToJson;

impl Tool for PojoToJson {
    fn meta(&self) -> ToolMeta {
        ToolMeta::new(
            "pojo-to-json",
            "POJO → JSON",
            Group::Code,
            "按 Java 字段生成示例 JSON，类型决定默认值，@JsonProperty 决定键名",
            "public class User {\n    private Long id;\n    private String userName;\n}",
            true,
            "json",
        )
        .with_options(vec![OptionSpec::select(
            "keyStyle",
            "JSON 键名",
            "field",
            &[("field", "保持字段名"), ("snake", "snake_case")],
            "字段名转 JSON 键的规则；有 @JsonProperty 时以注解为准",
        )])
    }

    fn run(&self, input: &str, ctx: &RunCtx) -> Result<ToolResult, String> {
        let fields = parse_fields(input);
        if fields.is_empty() {
            return Err("没有识别到字段，请粘贴 Java 类定义（每行一个 `类型 名称;`）".to_string());
        }
        let key_style = ctx.str("keyStyle", "field");
        let mut rows: Vec<String> = Vec::new();
        for f in &fields {
            let key = match (&f.json_name, key_style) {
                (Some(j), _) => j.clone(),
                (None, "snake") => snake(&f.name),
                (None, _) => f.name.clone(),
            };
            rows.push(format!("  {}: {}", json_escape(&key), default_json(&f.ty)));
        }
        let body = format!("{{\n{}\n}}\n", rows.join(",\n"));
        Ok(ToolResult::new()
            .section("示例 JSON", "json", body)
            .note(format!(
                "按 {} 个字段生成；泛型与嵌套对象只给空容器，需要时手动展开",
                fields.len()
            )))
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
    fn defaults_and_order() {
        let input = "private Long id;\nprivate String userName;\nprivate List<String> tags;";
        let r = PojoToJson.run(input, &ctx(json!({}))).unwrap();
        let s = &r.sections[0].text;
        assert!(s.contains("\"id\": 0"));
        assert!(s.contains("\"userName\": \"\""));
        assert!(s.contains("\"tags\": []"));
        assert!(s.find("\"id\"").unwrap() < s.find("\"userName\"").unwrap());
    }

    #[test]
    fn json_property_wins() {
        let input = "@JsonProperty(\"user_email\")\nprivate String email;";
        let r = PojoToJson
            .run(input, &ctx(json!({"keyStyle":"snake"})))
            .unwrap();
        assert!(r.sections[0].text.contains("\"user_email\": \"\""));
    }
}
