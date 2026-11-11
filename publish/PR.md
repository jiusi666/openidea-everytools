# PR：新增插件 jiusi.everytools

> 提给 [l0ng-ai/openidea-plugins](https://github.com/l0ng-ai/openidea-plugins) 的 PR 正文草稿。
> publisher `jiusi` 已在 registry 中，本地 `publishers/jiusi.toml` 与官方条目逐字一致，
> 所以 PR 的实际差异只有新增的 `plugins/jiusi.everytools.toml`。
> 顺序：先把源码推到 `jiusi666/openidea-everytools`，发 `v0.1.0` Release 并上传
> `jiusi.everytools-0.1.0.oiplugin`，再用附件核对出来的 sha256 填好
> `publish/plugins/jiusi.everytools.toml`，最后跑 `python scripts/open_pr.py`。

## 标题

```
Add jiusi.everytools — 16 Chinese-developer utility tools in one side panel
```

## 正文

```markdown
### What it is

`jiusi.everytools` packs 16 small tools Chinese developers reach for every day into one
side panel: JSON ⇄ POJO, setter / builder chains, DTO→VO skeletons, raw SQL → MyBatis
Mapper XML, cron expressions explained in Chinese, timestamps, RMB amount in Chinese
uppercase, mojibake repair, CJK text statistics, pinyin initials, radix / byte / duration
conversion, snowflake ID decoding, colour conversion, UUID v4, and ID-card / USCC / bank-card
validation with masking.

Paste text into the panel, or select code in any editor and run the matching command
(`Alt+Shift+E` opens the panel, `Alt+Shift+J` goes straight to JSON→POJO, `Alt+Shift+T`
to timestamps). Every result section has its own copy button that writes to the system
clipboard, and "write back" replaces the current selection in one undoable step.

Everything is text-level computation inside the WASM component. No network, no child
processes, no project files, no telemetry.

Source: https://github.com/jiusi666/openidea-everytools (Apache-2.0).

### Capabilities

- `editor.read` — read the current selection (or the whole document when there is no
  selection) as tool input, exactly what the panel shows.
- `editor.write` — the explicit "write back" action replaces the current selection with the
  result. Nothing is written to disk; it only edits the in-memory buffer you are looking at.
- `clipboard` — the copy buttons write the selected result section to the system clipboard.
- `storage` — remembers the current tool, input, options and the last 20 inputs locally under
  `jiusi.everytools:state`; removed together with plugin data on uninstall.

`project.read`, `project.write`, `net:*`, `process:*`, `fs.*`, `vcs.*`, `secrets` and
`ide.commands` are **not** requested.

### Verification

- `cargo test` — 70 unit tests: all 16 tools' core branches (Java field parsing, JSON→POJO
  nesting and arrays, three assignment styles, DTO field mapping, SQL placeholder filling
  with quoting/comment awareness, cron field parsing and next-run calculation, mojibake
  candidates and scoring, RMB uppercase round-trip, radix/byte/duration, snowflake bit
  layout, colour parsing, UUID generation and parsing, ID-card checksum / USCC / Luhn / phone
  masking), plus catalog uniqueness, "every tool runs on empty input without panicking",
  the panel/Rust serialization contract, and the message handlers via `MockHost`.
- `cargo clippy --all-targets -- -D warnings` — clean.
- `python scripts/oiplugin.py verify dist/jiusi.everytools-0.1.0.oiplugin` — recomputes the
  digest and checks the signature.

### Notes

- Built from this source with `wasm32-wasip2` + component model
  (`engines.openidea = "^1.4"`); no minified or generated-only code. `web/index.html` is a
  single hand-written file with no dependencies.
- The panel UI is generated from tool metadata (`Tool::meta()`), so the tool list and option
  controls come from the same Rust code that does the computation; a contract test pins the
  JSON field names between the two sides.
- Scope overlap: the bundled `openidea.string-tools` already covers case conversion, encoding
  escapes, line sorting/dedup, so none of those are duplicated here. My own `jiusi.mybatis-log`
  restores console logs into SQL — this plugin does the opposite direction (SQL → Mapper XML)
  and shares no code.
- Package: `jiusi.everytools-0.1.0.oiplugin`, sha256 `b133b92ef58d5217467588ff14244b134b9c3bdc85fd94d494f7e104d07d1c1f`.
```
