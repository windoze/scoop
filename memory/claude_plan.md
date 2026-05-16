# 执行计划

说明：本文件记录可审阅的执行计划、关键决策、进度和验证结果；不记录私有逐步推理细节。

当前任务：`P1-T02` - 自动 prelude：`scoop.core.*` + `scoop.lang.string.*` 注入 `ImportTable`。

最新提交检查：最新提交为 `[P1-T01] Add lang string sysroot cone`，是当前任务的显式依赖；未发现额外未完成前置项。

执行计划：

1. 读取 `TODO.md` 和 `TODO-1.md`，确认首个未完成任务及验证要求。
2. 检查 `ImportTable::build`、`SourceFile::is_sysroot`、`Span` 与 resolve/typecheck 的 import 消费路径。
3. 在用户源文件构建 `ImportTable` 时注入 `scoop.core.*` 与 `scoop.lang.string.*`；sysroot 文件不注入。
4. 保持显式 import 与合成 import 等价，并去重。
5. 补齐空 package 的 star import 识别，确保 `scoop.lang.string` placeholder cone 可被 import。
6. 添加 owner 测试和 fixture，运行指定验证与全量回归。
7. 更新 `TODO.md` / `TODO-1.md` 完成记录；`PLAN.md` 仅在阶段计划变化时更新。
8. 提交本任务相关变更并停止。

进度记录：

- 已确认首个未完成任务为 `P1-T02`。
- 已实现自动 prelude：用户文件先注入 `scoop.core.*` 与 `scoop.lang.string.*`，sysroot 文件跳过，显式重复 star import 去重。
- 已让 `Index` 记录 package 前缀，star import validation 能接受空 package，因此 P1-T01 的空 `scoop.lang.string` cone 在 P5 前也是有效 import 目标。
- 已让 header type path、direct supertype best-effort 解析与 `TypeEnv` best-effort import table 复用自动 prelude，避免只在 value resolve 中生效。
- 已新增 `tests/fixtures/run-pass/auto_prelude_core_basic.scoop` / `.stdout` 覆盖无显式 `import scoop.core.*` 的 `println` 使用。
- 已完成验证：`cargo test -p scoopc resolve::imports -- --nocapture`、owner fixture、5 条显式 core import 抽样 fixture、`cargo run -p scoop -- test`、`cargo clippy --all-targets -- -D warnings`、`cargo test --all --all-targets`、触及 Rust 文件的 `rustfmt --edition 2024 --check` 均通过。
- 已处理一次误触发的 workspace-wide `cargo fmt` 噪声：撤回无关格式化改动后重新验证通过。
- 已在 `TODO.md` 与 `TODO-1.md` 将 `P1-T02` 标记为 `[DONE]`；`PLAN.md` 未变化。
