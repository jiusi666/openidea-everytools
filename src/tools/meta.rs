//! 工具的自描述元数据。
//!
//! 面板不硬编码任何工具：它在 init 时拿到 `catalog`，按 `group` 分组、按
//! `options[].kind` 动态渲染控件。新增工具只要实现 `Tool::meta()`，前端零改动。

use serde::Serialize;
use serde_json::Value;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Group {
    Code,
    Text,
    Time,
    Value,
    Check,
}

impl Group {
    pub fn label(self) -> &'static str {
        match self {
            Group::Code => "代码",
            Group::Text => "文本",
            Group::Time => "时间",
            Group::Value => "数值",
            Group::Check => "校验",
        }
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Choice {
    pub value: String,
    pub label: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum OptionKind {
    Bool,
    Text { placeholder: String },
    Number { min: f64, max: f64, step: f64 },
    Select { choices: Vec<Choice> },
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OptionSpec {
    pub key: String,
    pub label: String,
    pub kind: OptionKind,
    pub default: Value,
    pub hint: String,
}

impl OptionSpec {
    pub fn bool(key: &str, label: &str, default: bool, hint: &str) -> Self {
        Self {
            key: key.to_string(),
            label: label.to_string(),
            kind: OptionKind::Bool,
            default: Value::Bool(default),
            hint: hint.to_string(),
        }
    }

    pub fn text(key: &str, label: &str, default: &str, placeholder: &str, hint: &str) -> Self {
        Self {
            key: key.to_string(),
            label: label.to_string(),
            kind: OptionKind::Text {
                placeholder: placeholder.to_string(),
            },
            default: Value::String(default.to_string()),
            hint: hint.to_string(),
        }
    }

    pub fn number(key: &str, label: &str, default: f64, min: f64, max: f64, hint: &str) -> Self {
        Self {
            key: key.to_string(),
            label: label.to_string(),
            kind: OptionKind::Number {
                min,
                max,
                step: 1.0,
            },
            default: Value::from(default),
            hint: hint.to_string(),
        }
    }

    pub fn select(
        key: &str,
        label: &str,
        default: &str,
        choices: &[(&str, &str)],
        hint: &str,
    ) -> Self {
        Self {
            key: key.to_string(),
            label: label.to_string(),
            kind: OptionKind::Select {
                choices: choices
                    .iter()
                    .map(|(value, text)| Choice {
                        value: value.to_string(),
                        label: text.to_string(),
                    })
                    .collect(),
            },
            default: Value::String(default.to_string()),
            hint: hint.to_string(),
        }
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolMeta {
    pub id: String,
    pub name: String,
    pub group: Group,
    /// 分组的中文名，面板直接用，不用自己维护一份映射。
    pub group_label: String,
    pub summary: String,
    pub placeholder: String,
    /// 不填输入也能用（UUID、颜色这类）。
    pub needs_input: bool,
    pub options: Vec<OptionSpec>,
    /// 结果的主语言，前端只用来选等宽字体/高亮开关。
    pub language: String,
}

impl ToolMeta {
    pub fn new(
        id: &str,
        name: &str,
        group: Group,
        summary: &str,
        placeholder: &str,
        needs_input: bool,
        language: &str,
    ) -> Self {
        Self {
            id: id.to_string(),
            name: name.to_string(),
            group_label: group.label().to_string(),
            group,
            summary: summary.to_string(),
            placeholder: placeholder.to_string(),
            needs_input,
            options: Vec::new(),
            language: language.to_string(),
        }
    }

    pub fn with_options(mut self, options: Vec<OptionSpec>) -> Self {
        self.options = options;
        self
    }
}
