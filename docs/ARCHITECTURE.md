# EveryTools 架构

这份文档说明**为什么代码是这么分的**，以及加东西的时候该动哪一层。目标只有一个：16 个工具之后还要能继续加，
加第 17 个时不希望有人去改 `lib.rs`、更不希望去改 `web/index.html`。

## 分层

```
        ┌──────────────────────────────────────────────────┐
        │  web/index.html   沙箱面板（零依赖单文件）          │
        │  · 按 catalog 渲染工具列表与选项控件               │
        │  · 只发消息、只显示快照，不含任何业务规则           │
        └───────────────▲──────────────────┬───────────────┘
              snapshot / │                  │ selectTool / run
              catalog /  │                  │ setOption / copy / apply
              toast      │                  ▼
        ┌───────────────┴──────────────────────────────────┐
        │  src/lib.rs   插件壳                               │
        │  · 命令路由（run_command）                         │
        │  · 视图消息路由（handle → on_message）              │
        │  · 状态编排：ensure_loaded → recompute → push      │
        └───┬───────────────┬───────────────────┬──────────┘
            │               │                   │
            ▼               ▼                   ▼
   ┌────────────────┐ ┌─────────────┐ ┌────────────────────┐
   │ state.rs       │ │ actions.rs  │ │ tools/             │
   │ 状态 + 持久化   │ │ 剪贴板 +    │ │ 16 个工具，纯函数   │
   │ 唯一碰 storage │ │ 写回编辑器   │ │ 不碰 host           │
   └────────────────┘ └─────────────┘ └────────────────────┘
```

依赖方向是单向的：`web` → `lib` → (`state` | `actions` | `tools`)，反过来不行。
`tools/` 里**不允许**出现 `openidea_plugin::host`——这条规则让所有工具逻辑都能用普通单元测试跑，
不需要 `MockHost`，也让「计算」和「副作用」的边界一眼可见。

## 一个工具 = 一个结构体

```rust
pub trait Tool: Send + Sync {
    fn meta(&self) -> ToolMeta;
    fn run(&self, input: &str, ctx: &RunCtx) -> Result<ToolResult, String>;
}
```

`meta()` 是**自描述**：名称、分组、摘要、占位符、要不要输入、有哪些选项、结果语言。
面板拿到的不是一份手写的界面配置，而是运行时从 `meta()` 序列化出来的 JSON，据此生成工具列表、
输入框、复选框、数字框和下拉框。所以：

- 加工具**不改前端**；
- 改一个工具的选项**不改前端**；
- 前端不可能出现「界面有这个开关但后端不认识」的漂移。

`plugin.toml` 里的 `[[contributes.commands]]` 是唯一需要在 Rust 之外同步的地方——因为命令面板的命令表是 IDE 读清单得到的，
不是运行时注册的。测试 `catalog_is_non_empty_and_unique` 守住 id 唯一性。

### 注册表

```rust
fn registry() -> &'static [Box<dyn Tool>] {
    static REG: OnceLock<Vec<Box<dyn Tool>>> = OnceLock::new();
    REG.get_or_init(|| vec![ /* 一行一个 */ ])
}
```

`OnceLock` 而不是每次 `vec![]`：`meta()` 会被面板、状态栏、命令路由反复调用，装一次就够。
顺序即面板里的显示顺序（同一分组内按注册顺序）。

## 数据流

两个入口，最后都落到同一个 `recompute()`：

**命令触发**（`Alt+Shift+J` 之类）

```
run_command("json-to-pojo")
  → read_editor_text()        有选区读选区，没选区读整篇
  → state.tool = "json-to-pojo"
  → recompute(now)            tools::run(...)
  → state.save()              写本机 storage
  → reveal_view()             把面板拉出来
  → push()                    推一份完整快照
```

**面板触发**

```
面板 input 事件 → {type:"run", text, now}（自动计算，不记历史）
  → on_message
  → set_input（截断 → recompute → save → 更新状态栏 → push）
  → 停手 800ms 后 {type:"remember"} → 只补一条历史（不重算）

点「运行」/ Ctrl+Enter → {type:"run", remember:true} → 同一条路径顺手记历史
```

`recompute()` 是唯一的计算入口，`push()` 是唯一的出站通道；推的永远是**完整快照**而不是增量，
面板因此可以是无状态的——收到什么就画什么，不存在「少收到一条消息导致界面错位」这种问题。

## 消息协议

面板 → 插件：

| type | 字段 | 作用 |
| --- | --- | --- |
| `init` | `now` | 面板加载完成：载入 storage、回 catalog + 第一帧 |
| `selectTool` | `tool` | 换工具，**清空输入**（选项保留，各工具的 key 不重叠） |
| `run` | `tool?` `text?` `options?` `remember?` `source?` | 跑一次；`text` 缺省表示只重算；`remember` 为真时顺带记历史 |
| `remember` | — | 自动模式下输入停稳后补记一条历史，不重算 |
| `setOption` | `key` `value` | 改一个选项并重算 |
| `copy` | `text?` | 复制（缺省复制主结果） |
| `apply` | — | 写回编辑器 |
| `readEditor` | — | 读编辑器**选区**当输入 |
| `clear` | — | 只清输入与结果，选项不动 |
| `reset` | — | 输入、结果、选项全部回初始（选项取设置里的默认值） |
| `history` / `deleteHistory` | `index` | 恢复 / 删除一条历史 |
| `setAutoRun` | `value` | 自动计算开关 |

插件 → 面板：

| type | 内容 |
| --- | --- |
| `catalog` | `tools`: `ToolMeta` 数组（面板的全部界面来源） |
| `snapshot` | `tool` `input` `options` `result` `source` `history` `autoRun` |
| `toast` | 一行一次性提示 |

每条消息都带 `now`（毫秒 UTC）。这不是冗余：WASM 侧拿不到可靠时钟，时间类工具需要一个可信的「现在」。

**契约测试** `catalog_json_shape_matches_panel_contract` 把这份 JSON 的字段名、`serde` tag
（`kind: bool|text|number|select`）、`sections` 的形状钉住。改字段名会先挂测试，而不是先白屏。

## 状态与生命周期

插件实例会被回收重建，入口处必须 `ensure_loaded()`：

```rust
fn ensure_loaded(&mut self) { if !self.loaded { self.load(); } }
```

真值只在本机 storage 的 `jiusi.everytools:state` 这一个键里，结构见 `state.rs::State`：
当前工具、输入、选项（扁平的一个 object，键名全局唯一）、历史、显示偏好、上次的 `now`。

- `from_settings()` 只在**没有存档时**跑一次，把设置面板的 12 项铺成初始选项；
- `normalize()` 兜住脏数据：工具 id 不存在就回默认、`options` 不是对象就重置、历史按上限截断；
- 输入在进入状态时截断到 256 KiB，历史单条截断到 32 KiB——不让一份大文件把 storage 撑爆。
- 历史用前缀合并：同工具下新输入是上一条的前缀延伸（或反过来），就替换那一条。自动模式下边打边记，
  没有这条规则历史会被半截输入刷满；换了内容才新增。

选项是**扁平共享**的：`timezone` 同时被时间戳和雪花 ID 用，`lombok` 同时被 JSON→POJO 和 DTO/VO 用，
语义一致所以故意共享；不共享的用不同 key（`style` vs `keyStyle`）。加工具时如果语义和已有的不一样，
**新起一个 key**，不要复用。

## 几个绕不开的约束

| 约束 | 应对 |
| --- | --- |
| 没有 Java AST / PSI | `tools/java/parse.rs` 是唯一的解析入口，做行级状态机（认注解、跳过 `static final`、处理 `名称: 类型` 与 `类型 名称;` 两种写法）。复杂声明认不出时降级而不是 panic |
| 时钟不可靠 | 日期算法自己实现（`civil_from_days` / `days_from_civil`），「现在」由面板传 |
| 没有随机数 | UUID 用 xorshift，种子取 `now_ms ^ fnv1a(input)`；不引 `rand` |
| 剪贴板只写不读 | 读方向的能力干脆不申请；「取选区」走 `editor.read`，而不是从剪贴板偷 |
| 包体积 | 拼音只做首字母（GBK 编码后查 GB2312 区段表），不引字典；`encoding_rs` 是唯一的大依赖（乱码修复必需） |
| UTF-8 边界 | 截断走 `truncate_utf8`（按 `is_char_boundary` 回退）；`sql_mapper` 的占位符替换按 `char_indices` 推进，不做字节切片 |

## 改哪里

| 想做的事 | 动哪 |
| --- | --- |
| 加一个工具 | 新建 `tools/<领域>/<名字>.rs` + `registry()` 加一行 + `plugin.toml` 加命令 |
| 加一个工具分组 | `tools/meta.rs` 的 `Group` 加变体 + `label()` 给中文名（面板自动出现新分组） |
| 加一种选项控件 | `OptionKind` 加变体 + `web/index.html` 的 `buildOptions` / `readOption` 各加一个分支 + 契约测试 |
| 改结果展示 | `ToolResult` / `Section`，以及面板的 `renderResult` |
| 改持久化 | `state.rs`，注意 `#[serde(default)]` 保证旧存档能读 |
| 换面板样式 | 只动 `web/index.html` 的 CSS |

## 测试策略

- `tools/` 里的纯函数：直接单测，不需要宿主。目前 81 个用例覆盖 16 个工具的主要分支。
- `meta` / `catalog`：唯一性、非空、每个工具都能在空输入下跑完不 panic。
- 契约：字段名与 `serde` tag 对齐面板假设。
- `lib.rs`：`MockHost::install()` 之后走真实消息，断言状态变化（init / run / setOption / selectTool / clear / 非法消息）。
- 面板：`web/index.html` 在浏览器里直接打开会进「预览模式」（内置两条假工具），可离线调样式；
  截图走 Edge/Chromium 的 `prefers-color-scheme` 双主题各看一遍。
