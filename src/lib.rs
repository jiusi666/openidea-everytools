//! EveryTools — 中文开发者的效率工具合集。
//!
//! 这一层只做插件壳：命令路由、视图消息、状态编排。具体计算在 `tools/`，
//! 与宿主交互在 `actions.rs`，持久化在 `state.rs`。新增工具不用改这里。

mod actions;
mod state;
mod tools;

use openidea_plugin::{
    Error, Invocation, Plugin, StatusItem, host, register_plugin,
    views::{View, ViewEvent, Viewport, Views},
};
use serde_json::{Value, json};
use state::{HistoryItem, State};
use tools::ToolResult;

/// 与 plugin.toml 的 `[[contributes.views]] id` 一致。
const VIEW: &str = "everytools";
/// 与 plugin.toml 的 `[[contributes.statusBar]] id` 一致。
const STATUS_ID: &str = "jiusi.everytools.status";

#[derive(Default)]
pub struct EveryTools {
    state: State,
    result: ToolResult,
    source: String,
    loaded: bool,
}

impl EveryTools {
    fn load(&mut self) {
        self.state = State::load();
        self.loaded = true;
    }

    fn ensure_loaded(&mut self) {
        if !self.loaded {
            self.load();
        }
    }

    fn recompute(&mut self, now: i64) {
        self.state.last_now = now;
        self.result = tools::run(
            &self.state.tool,
            &self.state.input,
            &self.state.options,
            now,
        );
    }

    fn snapshot(&self) -> Value {
        json!({
            "type": "snapshot",
            "tool": self.state.tool,
            "input": self.state.input,
            "options": self.state.options,
            "result": self.result,
            "source": self.source,
            "history": self.state.history,
            "autoRun": self.state.auto_run,
            "maxHistory": self.state.max_history,
        })
    }

    fn push(&self) {
        let _ = host::post_view_message(VIEW, &self.snapshot().to_string());
    }

    fn push_catalog(&self) {
        let payload = json!({ "type": "catalog", "tools": tools::catalog() });
        let _ = host::post_view_message(VIEW, &payload.to_string());
    }

    fn toast(&self, text: &str) {
        let payload = json!({ "type": "toast", "text": text });
        let _ = host::post_view_message(VIEW, &payload.to_string());
    }

    fn update_status(&self) {
        if !self.state.show_status {
            host::remove_status_item(STATUS_ID);
            return;
        }
        let name = tools::catalog()
            .into_iter()
            .find(|m| m.id == self.state.tool)
            .map(|m| m.name)
            .unwrap_or_else(|| "EveryTools".to_string());
        host::set_status_item(
            STATUS_ID,
            &StatusItem {
                text: name,
                tooltip: Some("点击打开 EveryTools 面板".to_string()),
                command: Some("open".to_string()),
            },
        );
    }

    fn add_history(&mut self) {
        let input = self.state.input.clone();
        if input.trim().is_empty() {
            return;
        }
        if let Some(top) = self.state.history.first()
            && top.tool == self.state.tool
        {
            if top.input == input {
                return;
            }
            // 同一个工具连着改同一段内容（自动模式下一边打一边记）只留最后一条，
            // 否则历史会被半截输入刷满
            if input.starts_with(top.input.as_str()) || top.input.starts_with(input.as_str()) {
                self.state.history.remove(0);
            }
        }
        let first = input
            .lines()
            .find(|l| !l.trim().is_empty())
            .unwrap_or("")
            .trim();
        let mut label: String = first.chars().take(56).collect();
        if first.chars().count() > 56 {
            label.push('…');
        }
        if label.is_empty() {
            label = "（空输入）".to_string();
        }
        self.state.history.insert(
            0,
            HistoryItem {
                tool: self.state.tool.clone(),
                label,
                input,
                options: self.state.options.clone(),
            },
        );
        self.state.history.truncate(self.state.max_history);
    }

    fn set_input(&mut self, text: &str, remember: bool, source: &str, now: i64) {
        self.state.input = tools::util::truncate_utf8(text, tools::MAX_INPUT_BYTES).to_string();
        self.source = source.to_string();
        if remember {
            self.add_history();
        }
        self.recompute(now);
        self.save();
        self.update_status();
        self.push();
    }

    fn set_option(&mut self, key: &str, value: Value, now: i64) {
        if !self.state.options.is_object() {
            self.state.options = json!({});
        }
        if let Some(obj) = self.state.options.as_object_mut() {
            obj.insert(key.to_string(), value);
        }
        self.recompute(now);
        self.save();
        self.push();
    }

    fn select_tool(&mut self, tool: &str, now: i64) {
        if !tools::is_tool(tool) {
            return;
        }
        self.state.tool = tool.to_string();
        // 换工具就换一份输入：上一段 SQL 留在 POJO 工具里没有意义，还容易被误算
        self.state.input.clear();
        self.source.clear();
        self.recompute(now);
        self.save();
        self.update_status();
        self.push();
    }

    /// 重置：输入、结果、选项都回到初始状态（选项取设置里的默认值）。
    fn reset(&mut self, now: i64) {
        self.state.input.clear();
        self.source.clear();
        self.state.options = State::from_settings().options;
        self.recompute(now);
        self.save();
        self.update_status();
        self.push();
    }

    fn clear(&mut self, now: i64) {
        self.state.input.clear();
        self.source.clear();
        self.recompute(now);
        self.save();
        self.push();
    }

    fn save(&self) {
        self.state.save();
    }

    /// `ctx` 只有写回编辑器时要；测试传 None。
    fn on_message(&mut self, text: &str, ctx: Option<&Invocation>) {
        let msg: Value = match serde_json::from_str(text) {
            Ok(v) => v,
            Err(_) => return,
        };
        let kind = msg.get("type").and_then(Value::as_str).unwrap_or("");
        if kind != "init" {
            self.ensure_loaded();
        }
        let now = msg
            .get("now")
            .and_then(Value::as_i64)
            .unwrap_or(self.state.last_now)
            .max(0);
        match kind {
            "init" => {
                self.load();
                self.recompute(now);
                self.update_status();
                self.push_catalog();
                self.push();
            }
            "selectTool" => {
                if let Some(tool) = msg.get("tool").and_then(Value::as_str) {
                    self.select_tool(tool, now);
                }
            }
            "run" => {
                if let Some(tool) = msg.get("tool").and_then(Value::as_str) {
                    self.state.tool = tool.to_string();
                }
                if let Some(opts) = msg.get("options")
                    && opts.is_object()
                {
                    self.state.options = opts.clone();
                }
                let remember = msg
                    .get("remember")
                    .and_then(Value::as_bool)
                    .unwrap_or(false);
                match msg.get("text").and_then(Value::as_str) {
                    Some(input) => {
                        let source = msg
                            .get("source")
                            .and_then(Value::as_str)
                            .unwrap_or("面板输入");
                        self.set_input(input, remember, source, now);
                    }
                    None => {
                        self.recompute(now);
                        self.save();
                        self.push();
                    }
                }
            }
            "setOption" => {
                if let Some(key) = msg.get("key").and_then(Value::as_str) {
                    let value = msg.get("value").cloned().unwrap_or(Value::Null);
                    self.set_option(key, value, now);
                }
            }
            "copy" => {
                // 面板可以带 text 复制某一段结果，不带就复制主结果。
                let text = msg
                    .get("text")
                    .and_then(Value::as_str)
                    .map(str::to_string)
                    .unwrap_or_else(|| self.result.primary.clone());
                match actions::copy(&text) {
                    Ok(()) => self.toast("已复制到剪贴板"),
                    Err(e) => self.toast(&format!("{e}")),
                }
            }
            "apply" => match ctx {
                Some(ctx) => match actions::write_back(ctx, &self.result.primary) {
                    Ok(()) => self.toast("已写回编辑器"),
                    Err(e) => self.toast(&format!("{e}")),
                },
                None => self.toast("写回需要在编辑器里执行"),
            },
            "readEditor" => match ctx {
                Some(ctx) => match read_editor_text(ctx, true) {
                    Ok(text) if text.trim().is_empty() => self.toast("编辑器里没有选中内容"),
                    Ok(text) => self.set_input(&text, true, "编辑器选中", now),
                    Err(e) => self.toast(&format!("{e}")),
                },
                None => self.toast("读取编辑器需要在编辑器里执行"),
            },
            "clear" => self.clear(now),
            "reset" => self.reset(now),
            // 自动模式下没有「运行」这个动作，面板等输入停稳后补记一条历史，
            // 输入和结果都算过了，这里只落历史不再重算
            "remember" => {
                self.add_history();
                self.save();
                self.push();
            }
            "refresh" => {
                self.recompute(now);
                self.push();
            }
            "setAutoRun" => {
                self.state.auto_run = msg
                    .get("value")
                    .and_then(Value::as_bool)
                    .unwrap_or(self.state.auto_run);
                self.save();
                self.push();
            }
            "history" => {
                if let Some(i) = msg.get("index").and_then(Value::as_u64)
                    && let Some(item) = self.state.history.get(i as usize).cloned()
                {
                    self.state.tool = item.tool;
                    self.state.input = item.input;
                    self.state.options = item.options;
                    self.source = "历史记录".to_string();
                    self.recompute(now);
                    self.save();
                    self.push();
                }
            }
            "deleteHistory" => {
                if let Some(i) = msg.get("index").and_then(Value::as_u64) {
                    let i = i as usize;
                    if i < self.state.history.len() {
                        self.state.history.remove(i);
                        self.save();
                        self.push();
                    }
                }
            }
            _ => {}
        }
    }
}

/// 读编辑器内容：有选区读选区；`only_selection` 为 false 时没选区就退回整篇。
fn read_editor_text(ctx: &Invocation, only_selection: bool) -> Result<String, Error> {
    let editor = ctx
        .editor()
        .ok_or_else(|| Error::Failed("没有打开的编辑器".to_string()))?;
    let doc = editor.document()?;
    let selections = editor.selections()?;
    let mut parts: Vec<String> = Vec::new();
    for range in &selections {
        if let Some(text) = doc.text_in(*range)
            && !text.is_empty()
        {
            parts.push(text.to_string());
        }
    }
    Ok(if parts.is_empty() && !only_selection {
        doc.text
    } else {
        parts.join("\n")
    })
}

impl Plugin for EveryTools {
    fn run_command(
        &mut self,
        id: &str,
        ctx: &Invocation,
        _arg: Option<&str>,
    ) -> Result<Option<String>, Error> {
        self.ensure_loaded();
        match id {
            "open" => {
                let _ = host::reveal_view(VIEW);
                Ok(None)
            }
            "copy-result" => {
                actions::copy(&self.result.primary)?;
                Ok(Some("已复制当前结果".to_string()))
            }
            "apply-result" => {
                actions::write_back(ctx, &self.result.primary)?;
                Ok(Some("已写回编辑器".to_string()))
            }
            "clear" => {
                self.clear(self.state.last_now);
                Ok(None)
            }
            other if tools::is_tool(other) => {
                let text = match read_editor_text(ctx, false) {
                    Ok(t) => t,
                    Err(Error::Denied(_)) => {
                        return Err(Error::Failed(
                            "缺少 editor.read 权限，请在 Settings › Plugins › EveryTools 里开启"
                                .to_string(),
                        ));
                    }
                    Err(_) => String::new(),
                };
                self.state.tool = other.to_string();
                self.state.input = text;
                self.source = "编辑器选中".to_string();
                self.recompute(self.state.last_now);
                self.save();
                self.update_status();
                let _ = host::reveal_view(VIEW);
                self.push();
                Ok(Some("EveryTools：已切到该工具".to_string()))
            }
            other => Err(Error::NotFound(format!("command {other}"))),
        }
    }
}

impl Views for EveryTools {
    fn render(
        &mut self,
        _view: &str,
        _viewport: Option<Viewport>,
        _ctx: &Invocation,
    ) -> Result<View, Error> {
        Ok(View::message("正在加载 EveryTools…"))
    }

    fn handle(&mut self, _view: &str, event: ViewEvent, ctx: &Invocation) -> Result<(), Error> {
        if let ViewEvent::Message(text) = event {
            self.on_message(&text, Some(ctx));
        }
        Ok(())
    }
}

register_plugin!(EveryTools, views);

#[cfg(test)]
mod tests {
    use super::*;
    use openidea_plugin::mock::MockHost;

    fn plugin() -> EveryTools {
        EveryTools {
            state: State::default(),
            loaded: true,
            ..EveryTools::default()
        }
    }

    #[test]
    fn init_pushes_catalog_and_snapshot() {
        let _host = MockHost::install();
        let mut p = plugin();
        p.on_message(
            &json!({"type":"init","now":1_704_083_400_000i64}).to_string(),
            None,
        );
        assert_eq!(p.state.last_now, 1_704_083_400_000);
    }

    #[test]
    fn run_message_updates_input_and_history() {
        let _host = MockHost::install();
        let mut p = plugin();
        p.on_message(
            &json!({"type":"run","tool":"pojo-to-json","text":"private Long id;","now":1,"remember":true})
                .to_string(),
            None,
        );
        assert_eq!(p.state.tool, "pojo-to-json");
        assert!(p.result.sections[0].text.contains("\"id\": 0"));
        assert_eq!(p.state.history.len(), 1);
    }

    #[test]
    fn remember_message_records_without_recomputing() {
        let _host = MockHost::install();
        let mut p = plugin();
        // 自动模式：面板只推 run，不带 remember
        p.on_message(
            &json!({"type":"run","tool":"pojo-to-json","text":"private Long id;","now":1})
                .to_string(),
            None,
        );
        assert!(p.state.history.is_empty(), "自动推送不该自己记历史");
        // 输入停稳后补一条
        p.on_message(&json!({"type":"remember","now":1}).to_string(), None);
        assert_eq!(p.state.history.len(), 1);
        assert_eq!(p.state.history[0].input, "private Long id;");
        assert_eq!(p.state.history[0].tool, "pojo-to-json");
    }

    #[test]
    fn typing_the_same_input_collapses_into_one_entry() {
        let _host = MockHost::install();
        let mut p = plugin();
        for text in ["private", "private Long", "private Long id;"] {
            p.on_message(
                &json!({"type":"run","tool":"pojo-to-json","text":text,"now":1,"remember":true})
                    .to_string(),
                None,
            );
        }
        assert_eq!(p.state.history.len(), 1, "同一段内容的中间态要合并");
        assert_eq!(p.state.history[0].input, "private Long id;");
        // 换成另一段内容才新增一条
        p.on_message(
            &json!({"type":"run","tool":"pojo-to-json","text":"private String name;","now":1,"remember":true})
                .to_string(),
            None,
        );
        assert_eq!(p.state.history.len(), 2);
        assert_eq!(p.state.history[0].input, "private String name;");
    }

    #[test]
    fn set_option_recomputes() {
        let _host = MockHost::install();
        let mut p = plugin();
        p.on_message(
            &json!({"type":"run","tool":"setter-chain","text":"private Long id;","now":1})
                .to_string(),
            None,
        );
        p.on_message(
            &json!({"type":"setOption","key":"style","value":"builder","now":1}).to_string(),
            None,
        );
        assert_eq!(p.state.options["style"], "builder");
        assert!(p.result.sections[0].text.contains(".build()"));
    }

    #[test]
    fn select_tool_and_clear() {
        let _host = MockHost::install();
        let mut p = plugin();
        p.on_message(
            &json!({"type":"selectTool","tool":"cron","now":1}).to_string(),
            None,
        );
        assert_eq!(p.state.tool, "cron");
        p.on_message(
            &json!({"type":"run","text":"0 9 * * *","now":1}).to_string(),
            None,
        );
        p.on_message(&json!({"type":"clear","now":1}).to_string(), None);
        assert!(p.state.input.is_empty());
        p.on_message("not json", None);
        p.on_message(&json!({"type":"unknown"}).to_string(), None);
    }

    #[test]
    fn switching_tool_drops_previous_input() {
        let _host = MockHost::install();
        let mut p = plugin();
        p.on_message(
            &json!({"type":"run","tool":"sql-mapper","text":"select 1","now":1}).to_string(),
            None,
        );
        assert_eq!(p.state.input, "select 1");
        p.on_message(
            &json!({"type":"selectTool","tool":"cron","now":1}).to_string(),
            None,
        );
        assert!(p.state.input.is_empty(), "换工具后不该留着上一份输入");
        assert!(p.source.is_empty());
        assert_eq!(p.state.tool, "cron");
        assert!(p.result.sections.is_empty());
    }

    #[test]
    fn reset_clears_input_and_restores_option_defaults() {
        let _host = MockHost::install();
        let mut p = plugin();
        p.on_message(
            &json!({"type":"run","tool":"setter-chain","text":"private Long id;","now":1})
                .to_string(),
            None,
        );
        p.on_message(
            &json!({"type":"setOption","key":"style","value":"builder","now":1}).to_string(),
            None,
        );
        assert_eq!(p.state.options["style"], "builder");
        p.on_message(&json!({"type":"reset","now":1}).to_string(), None);
        assert!(p.state.input.is_empty());
        assert!(p.result.sections.is_empty());
        assert_eq!(
            p.state.options["style"], "setter",
            "重置要回到设置里的默认风格，而不是保留面板上改过的值"
        );
    }
}
