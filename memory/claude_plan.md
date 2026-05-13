# Claude Plan

## 约束说明

按要求先写入计划文件，再执行其余检查与实现步骤。此文件记录可执行计划、决策摘要和进度更新；不包含不可公开的逐字内部推理。

## 初始执行计划

1. 读取 `TODO.md`，定位第一个标题未带 `[DONE]` 的任务。
2. 查看最近提交信息，确认是否有与当前任务直接相关且明确未完成的问题；若有，按要求视为当前任务的一部分或在 `TODO.md` 中补成前置任务。
3. 读取当前任务条目全文，确认需求、依赖、验证要求与完成记录。
4. 仅在必要时读取相关代码与测试，建立最小充分上下文，不做开放式排查。
5. 实现当前任务；若遇到阻塞当前任务的真实缺口或缺陷，则先修复，或把最小必要前置任务写回 `TODO.md` 并停止。
6. 运行与当前任务直接相关的验证；若任务完成需要，更广泛运行 `cargo test --all`、`cargo clippy --all-targets -- -D warnings` 等必要命令，并修复发现的问题。
7. 更新 `memory/claude_plan.md` 记录关键进展；完成后在 `TODO.md` 中将该任务标题标记为 `[DONE]` 并补全完成记录。
8. 仅在阶段级计划发生变化时更新 `PLAN.md`。
9. 按要求创建一次 git 提交，提交信息使用当前任务编号，随后停止，不继续下一个任务。

## 进度日志

- 已创建计划文件，下一步读取 `TODO.md` 与最近提交信息。
- 已确认第一个未完成任务为 `P5-T02R`：Review dump / fixture 迁移，检查 HIR / MIR / IR / effect facts / effect lowered 五类 active textual surface 是否全部脱离 raw `Debug` 协议。
- 已检查最近提交 `8ce8e1ea [P5-T02] Stabilize effect dump surfaces`，未发现提交说明中额外声明且会改变当前任务顺序的未完 blocker。
- 下一步：
  1. 审计 `crates/scoop/src/commands/*`、`crates/scoop/src/fixtures/mod.rs` 与各 `*/dump.rs` 是否仍存在 active surface 走 raw `Debug` 或“先 `Debug` 再补丁”路径。
  2. 复跑 P5-T01 / P5-T02 相关测试与文本搜索。
  3. 若审计通过，则回填 `TODO.md` 完成记录并提交；若发现 blocker，则先修复或补前置任务。
- 审计结论：当前 active CLI/fixture/stage dump 入口均已复用 `stable_dump()` / dedicated renderer；未发现需要新增前置任务的 blocker。
- 已完成验证矩阵：
  - `cargo test -p scoopc`
  - `cargo test -p scoop`
  - `cargo run -p scoop -- test --fixtures tests/fixtures/hir`
  - `cargo run -p scoop -- test --fixtures tests/fixtures/mir`
  - `cargo run -p scoop -- test --fixtures tests/fixtures/mir_refactor`
  - `cargo run -p scoop -- test --fixtures tests/fixtures/effect_facts`
  - `cargo run -p scoop -- test --fixtures tests/fixtures/effect_lowered`
  - `cargo clippy -p scoopc --all-targets -- -D warnings`
  - `cargo clippy -p scoop --all-targets -- -D warnings`
- 文本搜索结论：相关 fixture 已无 `TypeId(`/`bb0`/`site0`/`step_schema#0` 等旧协议命中；`crates/scoop/src` 中仅剩 `dump_ast` 与 `SCOOP_FIXTURE_REPRO_DIR` raw MIR repro 使用 `format!("{:#?}")`，属于允许保留的内部/非目标 surface。
- 已将 `P5-T02R` 标记为 `[DONE]` 并回填完成记录；下一步检查工作树并提交本次 review 结果。
