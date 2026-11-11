//! 工具的稳定契约与注册表。
//!
//! 一个工具 = 一个实现 `Tool` 的结构体：`meta()` 自描述（名称、分组、选项），
//! `run()` 做纯计算。新增工具只需要新建模块 + 在 `registry()` 里加一行。

pub mod cron;
pub mod datetime;
pub mod java;
pub mod meta;
pub mod sql_mapper;
pub mod text;
pub mod timestamp;
pub mod util;
pub mod value;

use meta::ToolMeta;
use serde::Serialize;
use serde_json::Value;
use std::sync::OnceLock;

pub use util::MAX_INPUT_BYTES;

#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Section {
    pub title: String,
    pub text: String,
    pub language: String,
}

#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolResult {
    pub sections: Vec<Section>,
    pub notes: Vec<String>,
    pub primary: String,
}

impl ToolResult {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn section(mut self, title: &str, language: &str, text: impl Into<String>) -> Self {
        self.sections.push(Section {
            title: title.to_string(),
            text: text.into(),
            language: language.to_string(),
        });
        self
    }

    pub fn note(mut self, note: impl Into<String>) -> Self {
        self.notes.push(note.into());
        self
    }

    /// 收尾：没显式指定 primary 时用第一段。
    pub fn done(mut self) -> Self {
        if self.primary.is_empty() {
            self.primary = self
                .sections
                .first()
                .map(|s| s.text.clone())
                .unwrap_or_default();
        }
        self
    }
}

/// 一次计算用到的上下文：合并后的选项 + 面板给的当前时间。
pub struct RunCtx<'a> {
    pub options: &'a Value,
    pub now_ms: i64,
}

impl<'a> RunCtx<'a> {
    pub fn str(&self, key: &str, default: &'a str) -> &'a str {
        util::opt_str(self.options, key, default)
    }

    pub fn bool(&self, key: &str, default: bool) -> bool {
        util::opt_bool(self.options, key, default)
    }

    pub fn i64(&self, key: &str, default: i64) -> i64 {
        util::opt_i64(self.options, key, default)
    }

    pub fn usize(&self, key: &str, default: usize, min: usize, max: usize) -> usize {
        util::opt_usize(self.options, key, default, min, max)
    }
}

pub trait Tool: Send + Sync {
    fn meta(&self) -> ToolMeta;
    fn run(&self, input: &str, ctx: &RunCtx) -> Result<ToolResult, String>;
}

fn registry() -> &'static [Box<dyn Tool>] {
    static REG: OnceLock<Vec<Box<dyn Tool>>> = OnceLock::new();
    REG.get_or_init(|| {
        vec![
            Box::new(java::JsonToPojo),
            Box::new(java::PojoToJson),
            Box::new(java::SetterChain),
            Box::new(java::DtoVo),
            Box::new(sql_mapper::SqlMapper),
            Box::new(text::Mojibake),
            Box::new(text::Count),
            Box::new(text::Pinyin),
            Box::new(cron::CronExplain),
            Box::new(timestamp::TimestampTool),
            Box::new(value::Rmb),
            Box::new(value::Unit),
            Box::new(value::Snowflake),
            Box::new(value::Color),
            Box::new(value::Uuid),
            Box::new(value::IdCard),
        ]
    })
}

pub fn catalog() -> Vec<ToolMeta> {
    registry().iter().map(|t| t.meta()).collect()
}

pub fn is_tool(id: &str) -> bool {
    registry().iter().any(|t| t.meta().id == id)
}

pub fn run(tool_id: &str, input: &str, options: &Value, now_ms: i64) -> ToolResult {
    let input = util::truncate_utf8(input, MAX_INPUT_BYTES);
    let Some(tool) = registry().iter().find(|t| t.meta().id == tool_id) else {
        return ToolResult::new()
            .note(format!("未知工具：{tool_id}"))
            .done();
    };
    let meta = tool.meta();
    if meta.needs_input && input.trim().is_empty() {
        return ToolResult::new()
            .note("先粘贴内容，或选中文本后执行命令")
            .done();
    }
    match tool.run(input, &RunCtx { options, now_ms }) {
        Ok(r) => r.done(),
        Err(e) => ToolResult::new().note(e).done(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn catalog_is_non_empty_and_unique() {
        let all = catalog();
        assert_eq!(all.len(), 16);
        let mut ids: Vec<&str> = all.iter().map(|m| m.id.as_str()).collect();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), 16);
    }

    #[test]
    fn every_tool_has_meta_and_runs_without_panic() {
        for meta in catalog() {
            assert!(!meta.name.is_empty());
            assert!(!meta.summary.is_empty());
            let _ = run(&meta.id, "", &json!({}), 1_700_000_000_000);
        }
    }

    #[test]
    fn unknown_tool_reports_note() {
        let r = run("nope", "x", &json!({}), 0);
        assert!(r.notes[0].contains("未知工具"));
    }

    /// 面板是按这份 JSON 动态建控件的：字段名或 serde tag 一改，界面就白屏。
    /// 这里把契约钉死，改字段名必须同步改 web/index.html。
    #[test]
    fn catalog_json_shape_matches_panel_contract() {
        let v = serde_json::to_value(catalog()).unwrap();
        let first = &v[0];
        for key in [
            "id",
            "name",
            "group",
            "groupLabel",
            "summary",
            "placeholder",
            "needsInput",
            "options",
            "language",
        ] {
            assert!(first.get(key).is_some(), "catalog 缺字段 {key}");
        }

        let mut kinds: Vec<String> = Vec::new();
        for tool in v.as_array().unwrap() {
            for spec in tool["options"].as_array().unwrap() {
                for key in ["key", "label", "kind", "default", "hint"] {
                    assert!(spec.get(key).is_some(), "option 缺字段 {key}");
                }
                kinds.push(spec["kind"]["kind"].as_str().unwrap().to_string());
                if spec["kind"]["kind"] == "select" {
                    let choices = spec["kind"]["choices"].as_array().unwrap();
                    assert!(!choices.is_empty());
                    assert!(choices[0].get("value").is_some());
                    assert!(choices[0].get("label").is_some());
                }
            }
        }
        for kind in ["bool", "text", "number", "select"] {
            assert!(
                kinds.iter().any(|k| k == kind),
                "16 个工具里应该至少有一个 {kind} 选项，面板靠它验证四种控件"
            );
        }

        // 结果段同样是面板直读的
        let r = run("text-count", "你好", &json!({}), 0);
        let rv = serde_json::to_value(&r).unwrap();
        for key in ["sections", "notes", "primary"] {
            assert!(rv.get(key).is_some(), "result 缺字段 {key}");
        }
        for key in ["title", "text", "language"] {
            assert!(rv["sections"][0].get(key).is_some(), "section 缺字段 {key}");
        }
    }
}
