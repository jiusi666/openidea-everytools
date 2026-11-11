//! 与宿主交互的动作：系统剪贴板、写回编辑器。插件里只有这里碰这两个能力。

use openidea_plugin::{Error, Invocation, host};

fn friendly(e: Error, capability: &str) -> Error {
    match e {
        Error::Denied(_) => Error::Failed(format!(
            "缺少 {capability} 权限，请在 Settings › Plugins › EveryTools 里开启"
        )),
        other => other,
    }
}

pub fn copy(text: &str) -> Result<(), Error> {
    if text.trim().is_empty() {
        return Err(Error::Invalid("还没有结果可复制".to_string()));
    }
    host::copy_to_clipboard(text).map_err(|e| friendly(e, "clipboard"))
}

/// 有选区就替换，只有光标就插入。整段算一次 undo。
pub fn write_back(ctx: &Invocation, text: &str) -> Result<(), Error> {
    if text.is_empty() {
        return Err(Error::Invalid("还没有结果可写回".to_string()));
    }
    let editor = ctx
        .editor()
        .ok_or_else(|| Error::Failed("请先打开一个编辑器".to_string()))?;
    editor
        .transform_selections(|_| text.to_string())
        .map_err(|e| friendly(e, "editor.write"))?;
    Ok(())
}
