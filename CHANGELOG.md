# Changelog

## 0.1.0 — 首个版本

- 16 个工具，分四组：代码 5（JSON→POJO、POJO→JSON、Setter/Builder 链、DTO/VO 骨架、SQL→Mapper XML）、
  文本 3（乱码修复、字数统计、拼音首字母）、时间 2（Cron 释义、时间戳互转）、数值与校验 6
  （人民币大写、进制/单位换算、雪花 ID、颜色互转、UUID、证件号校验脱敏）。
- 面板即工具箱：粘贴即算，结果分段显示，每段可单独复制；`Ctrl+Enter` 立即计算。
- 写回编辑器：结果替换当前选区，整段算一次撤销；复制走系统剪贴板（`clipboard` 能力）。
- 三个快捷键：`Alt+Shift+E` 打开面板、`Alt+Shift+J` 进 JSON→POJO、`Alt+Shift+T` 进时间戳互转；
  每个工具同时是独立命令，执行时自动读取编辑器选中文本。
- 历史记录：最近 20 条输入连同工具与选项一起恢复；可在面板里单条删除。
- 11 项设置：默认包名、Lombok、`@JsonProperty`、字段命名、赋值风格、Cron 方言、时区、雪花纪元、UUID 个数、历史条数、自动计算与状态栏。
- 工具自描述（`Tool::meta()`）+ 注册表：面板按元数据生成工具列表与选项控件，新增工具不改前端。
  `catalog_json_shape_matches_panel_contract` 守住 Rust 序列化与面板之间的字段契约。
- 能力 4 个：`editor.read`、`storage`（Low）、`clipboard`、`editor.write`（Medium）。
  不联网、不执行程序、不读写项目文件，历史与选项只存本机。
- 70 个单元测试，clippy `-D warnings` 零告警。
