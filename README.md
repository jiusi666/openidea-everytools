# EveryTools 中文效率小工具

中文开发者的效率工具合集：16 个常用小工具装在一个侧边面板里，粘贴或选中文本即可出结果，
一键复制到系统剪贴板或直接写回编辑器。全部在本机做文本级计算——不联网、不跑程序、不读写项目文件。

## 工具

| 分组 | 工具 | 干什么 |
| --- | --- | --- |
| 代码 | JSON → POJO | JSON 样本变 Java 类：嵌套对象生成 `public static class`，数组推断成 `List<T>`，可选 Lombok、`@JsonProperty`、字段命名规则 |
| 代码 | POJO → JSON | Java 字段声明变 JSON 骨架：按类型给默认值（`""` / `0` / `[]` / `{}` / `false`），优先认 `@JsonProperty` |
| 代码 | Setter / Builder 链 | 字段列表变赋值代码，三种风格：`setXxx` 链、`.builder().x(..).build()`、`BeanUtils.copyProperties`；可选 `!= null` 判空 |
| 代码 | DTO ↔ VO 骨架 | 按 DTO 生成 VO 类 + 拷贝代码，支持 `旧字段:新字段` 一行一条改名映射 |
| 代码 | SQL → Mapper XML | 裸 SQL 变 MyBatis 片段：`?` 按序变 `#{param}`，SELECT 生成 `<where>` + `<if test>`，UPDATE 生成 `<set>`，引号内的关键字不动 |
| 文本 | 中文乱码修复 | 「UTF-8 被当 GBK / Latin-1 读」还原回来，**逐行判断**（混合乱码也能逐行救回），并给全角 ↔ 半角转换 |
| 文本 | 中文字数统计 | 汉字、英文单词、数字、标点、空白、行数、段落、UTF-8 字节、预计阅读时长 |
| 文本 | 汉字转拼音首字母 | 按 GB2312 一级汉字区段取声母首字母，可调大小写、分隔符、是否保留非汉字 |
| 时间 | Cron 中文释义 | 5/6/7 段表达式翻成中文，支持 Spring 与 Quartz 方言、`L` / `W` / `#` / 步长 / 范围，并算出未来几次执行时间 |
| 时间 | 时间戳 ↔ 日期 | 10 / 13 / 16 / 19 位时间戳与日期双向互转，一行一个可批量，时区可选 |
| 数值 | 人民币金额大写 | `1234.56` → `壹仟贰佰叁拾肆元伍角陆分`，含零与整的处理，支持反向解析大写 |
| 数值 | 进制 / 单位换算 | 2 / 8 / 10 / 16 / 36 进制互转；字节按 1024 与 1000 双口径；时长换算成 `DD:HH:MM:SS.mmm` |
| 数值 | 雪花 ID 解析 | 拆出 1+41+5+5+12 位：时间、数据中心、机器、序列号，纪元与时区可配 |
| 数值 | 颜色互转 | HEX（3/4/6/8）/ RGB / RGBA / HSL / 颜色名互转，附带 Java 与 Android 写法、相对亮度与建议文字颜色 |
| 数值 | UUID / 随机串 | 批量生成 UUID v4（最多 100 个），四种格式；也能解析已有 UUID 的版本、变体与 v1 时间 |
| 校验 | 证件号校验 / 脱敏 | 18 位身份证校验位、生日性别、15 位升 18 位；统一社会信用代码、Luhn 银行卡、手机号；输出一律脱敏 |

## 用法

三种入口，用的是同一份状态：

1. **面板**（`Alt+Shift+E` / `Cmd+Shift+E`，或点状态栏的「工具」）：点顶部的工具名打开可搜索的列表（`↑↓` 选、`Enter` 确认、`Esc` 关），粘内容即出结果；「取选区」把编辑器里选中的文本拿进来。
2. **命令面板**：搜 `EveryTools`，每个工具都是独立命令。执行时会读取编辑器里**选中的文本**；没有选区就读整篇。
3. **快捷键**：`Alt+Shift+J` 直接进 JSON → POJO，`Alt+Shift+T` 直接进时间戳互转。

结果区每段都有「复制」按钮（走系统剪贴板；只有一段时合并到顶栏——那里复制的是主结果），「写回」把结果替换掉当前选区，一次替换、一次撤销就能回到原样。

面板里 `Ctrl+Enter`（Mac 是 `Cmd+Enter`）立即计算，不必等「输入即算」的防抖。选项默认收起，收起时右侧显示当前配置摘要，点开才占位置。

输入框默认 180px 高，拖底部那条细手柄可以改高度（双击手柄回到默认），下次打开面板还是你拖的那个高度。

## 权限

| 能力 | 风险 | 用途 |
| --- | --- | --- |
| `editor.read` | Low | 读当前编辑器的选区或整篇文本，作为工具输入 |
| `storage` | Low | 本机保存当前工具、输入、选项与最近 20 条历史，随插件卸载一起删除 |
| `clipboard` | Medium | 「复制」按钮写入系统剪贴板 |
| `editor.write` | Medium | 「写回」把结果替换当前选区，只改你正在编辑的那篇的内存内容 |

**不申请**：`project.read` / `project.write`、`net:*`、`process:*`、`fs.*`、`vcs.*`、`secrets`、`ide.commands`。
所以插件读不到项目里其它文件、不联网、不启动任何进程、不碰磁盘上的文件，也没有账号和后端。

## 设置

`Settings › Plugins › EveryTools`：

| 设置 | 说明 |
| --- | --- |
| 默认包名 | 生成 Java 类时用，留空不输出 `package` |
| Lombok / `@JsonProperty` | 生成 POJO 时是否用 `@Data`、是否补注解 |
| 字段命名 | JSON 键转 Java 字段名：`camel` / `snake` / `keep` |
| 赋值风格 | `setter` / `builder` / `copy` |
| Cron 方言 | `spring` 或 `quartz` |
| 时区 | 分钟数，东八区 480 |
| 雪花纪元 | 毫秒，Twitter 默认 `1288834974657` |
| UUID 个数 | 一次生成几个，1–100 |
| 历史条数 | 保留最近几条输入，1–50 |
| 自动计算 / 状态栏 | 输入停下就出结果；在状态栏显示当前工具 |

设置只决定**首次**的默认值；之后以面板里改过的选项为准（存在本机 storage）。

## 构建

```sh
rustup target add wasm32-wasip2        # 只需一次
cargo +stable-x86_64-pc-windows-gnu build --release --target wasm32-wasip2
mkdir -p dist && cp target/wasm32-wasip2/release/openidea_plugin_everytools.wasm dist/plugin.wasm
cargo +stable-x86_64-pc-windows-gnu test
python scripts/oiplugin.py check .
python scripts/oiplugin.py pack . --key .keys/jiusi.key --publisher jiusi \
  --out dist/jiusi.everytools-0.1.0.oiplugin
python scripts/oiplugin.py verify dist/jiusi.everytools-0.1.0.oiplugin
```

Windows + Git Bash 下如果报 `link: extra operand`，是 Git 自带的 `link` 抢在 MSVC 链接器前面，
用上面的 `+stable-x86_64-pc-windows-gnu` 就行。

安装：`Settings › Plugins › Install from Disk`，选 `jiusi.everytools-<version>.oiplugin`。

## 目录

| 路径 | 说明 |
| --- | --- |
| `plugin.toml` | 清单：id、版本、能力、命令、快捷键、视图、设置 |
| `src/lib.rs` | 插件壳：命令路由、视图消息、状态编排 |
| `src/state.rs` | 面板状态与持久化，唯一碰 `storage` 的地方 |
| `src/actions.rs` | 剪贴板与写回，唯一碰 `clipboard` / `editor.write` 的地方 |
| `src/tools/` | 16 个工具的实现，纯函数，不碰宿主 |
| `src/tools/meta.rs` | 工具自描述：分组、选项、占位符——面板据此生成界面 |
| `web/index.html` | 面板界面，跑在沙箱视图里，零依赖单文件 |
| `settings.schema.json` | 设置面板的字段 |
| `docs/ARCHITECTURE.md` | 分层、消息协议、为什么这么分 |
| `vendor/openidea-plugin` | SDK 源码快照，保证离线可构建 |
| `scripts/oiplugin.py` | 打包、签名、校验 |

## 加一个工具

1. 在 `src/tools/` 下按领域新建模块，实现 `Tool`：`meta()` 描述自己，`run()` 做纯计算。
2. 在 `src/tools/mod.rs` 的 `registry()` 里加一行 `Box::new(xxx::Xxx)`。
3. 在 `plugin.toml` 加一条 `[[contributes.commands]]`（id 与 `meta()` 的 id 一致）。
4. 跑测试——`catalog` 唯一性、契约字段、`run` 不 panic 都是自动覆盖的。

面板不用改：工具列表、选项控件、结果区全部按 `meta()` 生成。

## 边界

- Java 解析是**文本级**的：WASM 侧没有 AST/PSI，泛型、注解、内部类这些复杂声明不保证解析正确。
- 单次输入上限 256 KiB，超出按 UTF-8 边界截断。
- 「现在」由面板传入（WASM 侧时钟不可靠）；面板没发过时间时用上次记住的值。
- 拼音只做首字母（用 GB2312 区段表），不做全拼，为的是不把包撑大。
- 乱码修复只能救「字节完整、只是被用错编码读过」的文本。字节已经残损的（例如多字节里的 `0xA0` 被换成普通空格）无法逆推，工具会明说而不是瞎猜。

## 许可

Apache-2.0。
