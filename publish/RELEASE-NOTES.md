# EveryTools 0.1.0

16 个中文开发者常用小工具装在一个侧边面板里。粘贴即算，结果可一键复制或写回编辑器。

**代码**：JSON → POJO（嵌套生成内部类，数组推断 `List<T>`）、POJO → JSON 骨架、Setter / Builder / `BeanUtils` 三种赋值链、DTO → VO 骨架（支持字段改名映射）、裸 SQL → MyBatis Mapper XML（`?` 变 `#{param}`，自动套 `<where>` + `<if>` 与 `<set>`）。

**文本**：中文乱码修复（UTF-8 被当 GBK / Latin-1 读，多候选按可信度排序）+ 全角半角互转、中文字数统计（汉字/单词/标点/行段/字节/阅读时长）、汉字转拼音首字母。

**时间**：Cron 中文释义（5/6/7 段，Spring 与 Quartz 方言，`L` / `W` / `#` / 步长 / 范围，附未来执行时间）、时间戳 ↔ 日期（10/13/16/19 位，可批量）。

**数值与校验**：人民币金额大写（含反向解析）、进制 / 字节 / 时长换算、雪花 ID 拆解、颜色互转（HEX / RGB / RGBA / HSL / 颜色名 + Java 与 Android 写法）、UUID v4 批量生成与解析、身份证 / 统一社会信用代码 / Luhn 银行卡 / 手机号校验并脱敏。

- 三个快捷键：`Alt+Shift+E` 面板、`Alt+Shift+J` JSON→POJO、`Alt+Shift+T` 时间戳；每个工具同时是独立命令，执行时读取编辑器选中文本。
- 写回编辑器整段算一次撤销；复制走系统剪贴板；最近 20 条输入连同工具与选项可一键恢复。
- 11 项设置：默认包名、Lombok、`@JsonProperty`、字段命名、赋值风格、Cron 方言、时区、雪花纪元、UUID 个数、历史条数、自动计算与状态栏。
- 能力 4 个：`editor.read`、`storage`（Low）、`clipboard`、`editor.write`（Medium）。不联网、不执行程序、不读写项目文件，历史与选项只存本机。
- 工具由元数据自描述，面板按元数据生成界面；新增工具不需要改前端。

构建：`wasm32-wasip2` + component model，`engines.openidea = "^1.4"`，源码 Apache-2.0，包由本仓库源码构建、无压缩混淆。
