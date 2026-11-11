//! 面板状态：当前工具、输入、选项、历史。唯一碰 storage 的地方。

use crate::tools;
use crate::tools::MAX_INPUT_BYTES;
use crate::tools::util::truncate_utf8;
use openidea_plugin::host;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

pub const KEY_STATE: &str = "jiusi.everytools:state";
pub const MAX_HISTORY: usize = 50;
const MAX_HISTORY_BYTES: usize = 32 * 1024;

fn empty_object() -> Value {
    json!({})
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
pub struct HistoryItem {
    pub tool: String,
    pub label: String,
    pub input: String,
    pub options: Value,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct State {
    pub tool: String,
    pub input: String,
    pub options: Value,
    pub history: Vec<HistoryItem>,
    pub show_status: bool,
    pub max_history: usize,
    pub auto_run: bool,
    /// 面板最近一次上报的当前时间；命令触发时先用它，面板 init 后会再校正。
    pub last_now: i64,
}

impl Default for State {
    fn default() -> Self {
        Self {
            tool: default_tool(),
            input: String::new(),
            options: empty_object(),
            history: Vec::new(),
            show_status: true,
            max_history: 20,
            auto_run: true,
            last_now: 0,
        }
    }
}

fn default_tool() -> String {
    tools::catalog()
        .first()
        .map(|m| m.id.clone())
        .unwrap_or_else(|| "json-to-pojo".to_string())
}

impl State {
    /// 首次运行把设置面板的值铺成面板选项；之后以本机 storage 为准。
    pub fn from_settings() -> Self {
        let mut s = Self::default();
        let mut opt = serde_json::Map::new();
        opt.insert(
            "javaPackage".into(),
            json!(host::setting_string("defaultPackage").unwrap_or_default()),
        );
        opt.insert(
            "lombok".into(),
            json!(host::setting_bool("lombok").unwrap_or(true)),
        );
        opt.insert(
            "jackson".into(),
            json!(host::setting_bool("jackson").unwrap_or(false)),
        );
        opt.insert(
            "fieldNaming".into(),
            json!(host::setting_string("fieldNaming").unwrap_or_else(|| "camel".into())),
        );
        opt.insert(
            "style".into(),
            json!(host::setting_string("setterStyle").unwrap_or_else(|| "setter".into())),
        );
        opt.insert(
            "cronDialect".into(),
            json!(host::setting_string("cronDialect").unwrap_or_else(|| "spring".into())),
        );
        opt.insert(
            "timezone".into(),
            json!(host::setting_number("timezone").unwrap_or(480.0)),
        );
        opt.insert(
            "epoch".into(),
            json!(host::setting_number("snowflakeEpoch").unwrap_or(1_288_834_974_657.0)),
        );
        opt.insert(
            "uuidCount".into(),
            json!(host::setting_number("uuidCount").unwrap_or(5.0)),
        );
        s.options = Value::Object(opt);
        s.show_status = host::setting_bool("showStatusBar").unwrap_or(true);
        s.auto_run = host::setting_bool("autoRun").unwrap_or(true);
        s.max_history = host::setting_number("maxHistory")
            .map(|n| n as usize)
            .unwrap_or(20);
        s.normalize()
    }

    pub fn normalize(mut self) -> Self {
        if !tools::is_tool(&self.tool) {
            self.tool = default_tool();
        }
        let input = truncate_utf8(&self.input, MAX_INPUT_BYTES).to_string();
        self.input = input;
        if !self.options.is_object() {
            self.options = empty_object();
        }
        self.max_history = self.max_history.clamp(1, MAX_HISTORY);
        for item in &mut self.history {
            if !item.options.is_object() {
                item.options = empty_object();
            }
            item.input = truncate_utf8(&item.input, MAX_HISTORY_BYTES).to_string();
        }
        self.history.truncate(self.max_history);
        self
    }

    pub fn load() -> Self {
        host::storage::get(KEY_STATE)
            .ok()
            .flatten()
            .and_then(|s| serde_json::from_str::<State>(&s).ok())
            .unwrap_or_else(Self::from_settings)
            .normalize()
    }

    pub fn save(&self) {
        if let Ok(s) = serde_json::to_string(self) {
            let _ = host::storage::set(KEY_STATE, &s);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use openidea_plugin::mock::MockHost;

    #[test]
    fn normalize_fixes_bad_values() {
        let s = State {
            tool: "nope".into(),
            max_history: 999,
            options: Value::Null,
            history: vec![HistoryItem {
                tool: "nope".into(),
                label: "x".into(),
                input: "y".into(),
                options: Value::Null,
            }],
            ..State::default()
        }
        .normalize();
        assert!(tools::is_tool(&s.tool));
        assert_eq!(s.max_history, MAX_HISTORY);
        assert!(s.options.is_object());
        assert!(s.history[0].options.is_object());
    }

    #[test]
    fn settings_seed_panel_options() {
        let _host = MockHost::install();
        let s = State::from_settings();
        assert!(s.options.get("lombok").is_some());
        assert!(s.options.get("timezone").is_some());
    }
}
