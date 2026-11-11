//! 赋值代码与 DTO/VO 骨架：setter 链、Builder 链、BeanUtils 拷贝。
//!
//! 三个风格共用同一份字段列表，DTO/VO 只是多生成一个目标类。

use crate::tools::java::parse::{
    Field, apply_mapping, getter_name, is_primitive, parse_fields, pascal,
};
use crate::tools::meta::{Group, OptionSpec, ToolMeta};
use crate::tools::{RunCtx, Tool, ToolResult};

pub struct SetterChain;
pub struct DtoVo;

fn assign_options(default_target: &str) -> Vec<OptionSpec> {
    vec![
        OptionSpec::text(
            "targetClass",
            "目标类",
            default_target,
            "UserDTO / UserVO",
            "生成的对象类型",
        ),
        OptionSpec::text("varName", "目标变量名", "dto", "dto", ""),
        OptionSpec::text("srcName", "源变量名", "src", "src", ""),
        OptionSpec::select(
            "style",
            "输出风格",
            "setter",
            &[
                ("setter", "setter 链"),
                ("builder", "Builder 链"),
                ("copy", "BeanUtils 拷贝"),
            ],
            "Builder 需要目标类有 Lombok @Builder",
        ),
        OptionSpec::bool(
            "nullCheck",
            "包装类型加 null 判断",
            false,
            "基本类型不加，避免拆箱空指针",
        ),
        OptionSpec::text(
            "mapping",
            "字段映射",
            "",
            "userName:name",
            "一行一条 old:new，用于源字段改名后赋值",
        ),
    ]
}

fn assign_code(fields: &[Field], ctx: &RunCtx, default_target: &str) -> Result<String, String> {
    if fields.is_empty() {
        return Err("没有识别到字段，请粘贴 Java 类定义".to_string());
    }
    let target = {
        let t = ctx.str("targetClass", default_target).trim();
        if t.is_empty() {
            default_target.to_string()
        } else {
            pascal(t)
        }
    };
    let var = ctx.str("varName", "dto").trim();
    let src = ctx.str("srcName", "src").trim();
    let var = if var.is_empty() { "dto" } else { var };
    let src = if src.is_empty() { "src" } else { src };
    let style = ctx.str("style", "setter");
    let null_check = ctx.bool("nullCheck", false);
    let mut out = String::new();
    match style {
        "builder" => {
            out.push_str(&format!("{target} {var} = {target}.builder()\n"));
            for f in fields {
                out.push_str(&format!(
                    "        .{}({src}.{}())\n",
                    f.name,
                    getter_name(f)
                ));
            }
            out.push_str("        .build();\n");
        }
        "copy" => {
            out.push_str(&format!("{target} {var} = new {target}();\n"));
            out.push_str(&format!("BeanUtils.copyProperties({src}, {var});\n"));
        }
        _ => {
            out.push_str(&format!("{target} {var} = new {target}();\n"));
            for f in fields {
                let setter = format!("{var}.set{}({src}.{}());", pascal(&f.name), getter_name(f));
                if null_check && !is_primitive(&f.ty) {
                    out.push_str(&format!(
                        "if ({src}.{}() != null) {{\n    {setter}\n}}\n",
                        getter_name(f)
                    ));
                } else {
                    out.push_str(&setter);
                    out.push('\n');
                }
            }
        }
    }
    Ok(out)
}

impl Tool for SetterChain {
    fn meta(&self) -> ToolMeta {
        ToolMeta::new(
            "setter-chain",
            "Setter / Builder 链",
            Group::Code,
            "从字段列表生成目标对象的赋值代码，支持 setter、Builder、BeanUtils 三种风格",
            "private Long id;\nprivate String userName;",
            true,
            "java",
        )
        .with_options(assign_options("TargetDTO"))
    }

    fn run(&self, input: &str, ctx: &RunCtx) -> Result<ToolResult, String> {
        let fields = apply_mapping(&parse_fields(input), ctx.options);
        let code = assign_code(&fields, ctx, "TargetDTO")?;
        let mut r = ToolResult::new().section("赋值代码", "java", code);
        if ctx.str("style", "setter") == "copy" {
            r = r.note("BeanUtils 要求字段名完全一致；有差异时改用 setter 或 builder");
        }
        Ok(r)
    }
}

impl Tool for DtoVo {
    fn meta(&self) -> ToolMeta {
        let mut options = assign_options("TargetVO");
        options.push(OptionSpec::bool(
            "lombok",
            "目标类使用 Lombok @Data",
            true,
            "关闭后展开 getter / setter",
        ));
        options.push(OptionSpec::text(
            "javaPackage",
            "包名",
            "",
            "com.example.vo",
            "留空不输出 package",
        ));
        ToolMeta::new(
            "dto-vo",
            "DTO ↔ VO 骨架",
            Group::Code,
            "按源类字段生成目标类骨架和赋值代码，支持字段改名",
            "private Long id;\nprivate String userName;",
            true,
            "java",
        )
        .with_options(options)
    }

    fn run(&self, input: &str, ctx: &RunCtx) -> Result<ToolResult, String> {
        let fields = apply_mapping(&parse_fields(input), ctx.options);
        if fields.is_empty() {
            return Err("没有识别到字段，请粘贴源类字段".to_string());
        }
        let target = {
            let t = ctx.str("targetClass", "TargetVO").trim();
            pascal(if t.is_empty() { "TargetVO" } else { t })
        };
        let lombok = ctx.bool("lombok", true);
        let package = ctx.str("javaPackage", "").trim();
        let mut body = String::new();
        if !package.is_empty() {
            body.push_str(&format!("package {package};\n\n"));
        }
        if lombok {
            body.push_str("import lombok.Data;\n\n@Data\n");
        }
        body.push_str(&format!("public class {target} {{\n"));
        for f in &fields {
            body.push_str(&format!("    private {} {};\n", f.ty, f.name));
        }
        if !lombok {
            for f in &fields {
                let cap = pascal(&f.name);
                body.push_str(&format!(
                    "\n    public {} get{cap}() {{\n        return {};\n    }}\n",
                    f.ty, f.name
                ));
                body.push_str(&format!(
                    "    public void set{cap}({} {}) {{\n        this.{} = {};\n    }}\n",
                    f.ty, f.name, f.name, f.name
                ));
            }
        }
        body.push_str("}\n");
        let code = assign_code(&fields, ctx, "TargetVO")?;
        let mut r =
            ToolResult::new()
                .section("目标类", "java", body)
                .section("赋值代码", "java", code);
        let mapping = ctx.str("mapping", "").trim();
        if !mapping.is_empty() {
            r = r.note(format!("已应用 {} 条字段映射", mapping.lines().count()));
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

    const SRC: &str = "private Long id;\nprivate String userName;";

    #[test]
    fn setter_style() {
        let r = SetterChain.run(SRC, &ctx(json!({}))).unwrap();
        let s = &r.sections[0].text;
        assert!(s.contains("TargetDTO dto = new TargetDTO();"));
        assert!(s.contains("dto.setUserName(src.getUserName());"));
    }

    #[test]
    fn builder_style() {
        let r = SetterChain
            .run(SRC, &ctx(json!({"style":"builder"})))
            .unwrap();
        let s = &r.sections[0].text;
        assert!(s.contains(".userName(src.getUserName())"));
        assert!(s.contains(".build();"));
    }

    #[test]
    fn null_check_skips_primitive() {
        let input = "private long id;\nprivate String name;";
        let r = SetterChain
            .run(input, &ctx(json!({"nullCheck":true})))
            .unwrap();
        let s = &r.sections[0].text;
        assert!(s.contains("dto.setId(src.getId());"));
        assert!(s.contains("if (src.getName() != null)"));
    }

    #[test]
    fn dto_vo_applies_mapping() {
        let r = DtoVo
            .run(
                "private String userName;",
                &ctx(json!({"targetClass":"user vo","mapping":"userName:name"})),
            )
            .unwrap();
        assert!(r.sections[0].text.contains("public class UserVo"));
        assert!(r.sections[0].text.contains("private String name;"));
        assert!(
            r.sections[1]
                .text
                .contains("dto.setName(src.getUserName());")
        );
    }
}
