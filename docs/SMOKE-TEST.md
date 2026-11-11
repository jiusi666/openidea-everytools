# 怎么测 EveryTools

分三层，从快到慢。日常改代码走 A + C，交付前补 B。

## A. 面板预览（10 秒，不装插件）

直接用浏览器打开 `web/index.html`。没有宿主时会进「预览模式」：内置两条假工具，
能看工具下拉、选项控件、输入区、结果区、历史、明暗主题的完整排版，但**不会真的计算**。

适合改 CSS / 调布局时反复看，不适合验证功能。

## B. 装进 IDE 测真功能

两种装法，选一种。

### B1. 装安装包（模拟用户，验证打包没问题）

`Settings › Plugins › Install from Disk`，选：

```
dist/jiusi.everytools-0.1.0.oiplugin
```

装完 `Alt+Shift+E`（macOS `⌘⇧E`）打开面板，或点状态栏右侧的「工具」。
装完不需要重启，但面板第一次打开才会 `init`，所以先关掉旧面板再开。

卸载/排查装了几份：

```sh
python scripts/local_install.py status
python scripts/local_install.py clean            # 试算，不写文件
python scripts/local_install.py clean --apply    # 真解除（IDE 要先退出）
```

### B2. 挂成开发插件（改代码不用重装）

```sh
# 1) 先退出 OpenIDEA（脚本会拒绝在 IDE 运行时改状态文件）
python scripts/local_install.py link --apply
# 2) 重新打开 IDE，插件从本目录加载
```

之后：

- 改 `web/index.html` → 关掉面板再打开就生效。
- 改 Rust → 必须重新构建，因为宿主加载的是 `dist/plugin.wasm`：

```sh
cargo +stable-x86_64-pc-windows-gnu build --release --target wasm32-wasip2
cp target/wasm32-wasip2/release/openidea_plugin_everytools.wasm dist/plugin.wasm
```

解除：`python scripts/local_install.py clean --apply`。

## C. 自动化

```sh
cargo +stable-x86_64-pc-windows-gnu test                    # 77 项
cargo +stable-x86_64-pc-windows-gnu clippy --all-targets -- -D warnings
python scripts/oiplugin.py check                            # 清单 + 能力 + 产物
python scripts/oiplugin.py verify dist/jiusi.everytools-0.1.0.oiplugin
```

测试盯的是「逻辑和契约」：16 个工具的边界、面板与 Rust 之间的 JSON 字段名、
每个工具的示例输入必须真的跑得出东西。它盯不住的是**真实宿主里的交互**——
能力授权、写回的光标行为、剪贴板、面板在窄侧栏里的实际观感，这些只能手工过一遍。

## D. 手工验收清单

打开面板，按表点一遍。输入粘进输入框，自动算（关掉自动运行就按 `运行` 或 `Ctrl+Enter`）。

### 代码

| 工具 | 输入 | 应该看到 |
| --- | --- | --- |
| JSON → POJO | `{"id":1,"user_name":"张三","tags":["vip"],"profile":{"age":18}}` | `private String userName;`、`private List<String> tags;`、`public static class Profile` |
| POJO → JSON | `private Long id;`<br>`private String userName;` | `"id": 0`、`"userName": ""` |
| Setter / Builder 链 | `private Long id;`<br>`private String name;` | `dto.setId(src.getId());`；把「风格」换成 builder 应变成 `.id(src.getId())` |
| DTO ↔ VO 骨架 | 同上两个字段，映射写 `name:userName` | 生成 `TargetVO` 类，拷贝那行是 `vo.setUserName(src.getName())` |
| SQL → Mapper XML | `select id, name from user where id = ? and name = ? order by id desc` | `<where>` 里两个 `<if test="p1 != null">`，`?` 变成 `#{p1}` / `#{p2}`，`order by` 留在外面 |

### 文本

| 工具 | 输入 | 应该看到 |
| --- | --- | --- |
| 中文乱码修复 | `鎴戜滑`（或直接用示例的两行） | `我们`；示例两行都应还原成中文，不该出现 `閹存垳` 这类新乱码 |
| 中文字数统计 | `你好 world，测试。` | 汉字 4、英文单词 1、标点 2、空白 1、行数 1 |
| 汉字转拼音首字母 | `你好吗` | `NHM` |

### 时间

| 工具 | 输入 | 应该看到 |
| --- | --- | --- |
| Cron 中文释义 | `0 0 9 * * 1-5` | `每周一至周五 09:00 执行` + 未来 5 次时间 |
| 时间戳 ↔ 日期 | `1704083400`<br>`2024-01-01 12:30:00` | 前者 → `2024-01-01 12:30:00 周一`，后者 → `1704083400000 毫秒` |

### 数值与校验

| 工具 | 输入 | 应该看到 |
| --- | --- | --- |
| 人民币金额大写 | `1234.56` | `壹仟贰佰叁拾肆元伍角陆分` |
| 进制 / 单位换算 | `255`<br>`1024 KB`<br>`90 min` | `0xFF` / `0b11111111`；`1.00 MiB` 与 `1.05 MB` 两种口径；`01:30:00` |
| 雪花 ID 解析 | 任意雪花 ID（点示例即可） | 拆出时间、数据中心、机器、序列号 |
| 颜色互转 | `#2563eb` 或 `rgb(37, 99, 235)` | HEX / RGB / HSL 互转，外加 Java 与 Android 写法 |
| UUID / 随机串 | 留空点运行 | 一次 5 个 v4；把「模式」改成解析、粘一个 UUID 能读出格式 |

### 跨工具的行为

这几条是工具本身，最容易被忽略但最容易出问题：

1. **写回**：在编辑器里选中几行 → 面板算完点「写回」→ 选区被替换 → `Ctrl+Z` **一次**就应该整段还原。
2. **取选区**：编辑器里选中一段 → 面板点「取选区」→ 输入框出现那段文字，来源标签显示「编辑器选中」。
3. **剪贴板**：点结果段右上角的「复制」→ 粘到别处；再点底部「复制」应该复制的是主结果段。
4. **命令入口**：选中 Java 字段声明 → 命令面板执行 `EveryTools: 生成 Setter / Builder 链` → 面板自动弹出并切到该工具。
5. **历史**：切到另一个工具、改几个选项、再点历史里那条 → 工具、输入、选项应该一起恢复。
6. **设置生效**：`Settings › Plugins › EveryTools` 关掉 Lombok → 用 `local_install.py clean --apply` 清掉本地状态（或换个没装过的用户目录）→ 重新打开，JSON→POJO 应展开 getter/setter。**注意**：设置只决定首次默认值，之后以面板里改过的为准。
7. **不做的事**：面板和命令都不应该在任何时候联网、读写项目里的其它文件、或启动进程。

## 出问题怎么查

| 现象 | 先看这里 |
| --- | --- |
| 面板一片空白 | 面板必须靠 `init` 拿数据；看 `src/lib.rs` 的 `on_message` 是否被调用（`handle` 里打日志），以及 `plugin.toml` 的 `views[].web` 路径 |
| 面板打开是旧样子 | 实例被缓存了：关掉视图重开；开发模式下改 `web/index.html` 也必须重开 |
| 工具下拉是空的 | `catalog` 没发出去；`push_catalog` 只在 `init` 里调用 |
| 改 Rust 没反应 | 忘了重新 build + 复制到 `dist/plugin.wasm` |
| 报「缺少 xxx 权限」 | `plugin.toml` 的 `[capabilities]` 没写，或用户在设置里关掉了 `optional` |
| 时间类工具算错 | 面板没带 `now`；用 `snapshot` 里 `last_now` 对一下 |
