# 本次执行计划

1. 先读取 `TODO.md`，确认第一个标题未带 `[DONE]` 的任务；不做开放式问题排查。
2. 检查最近提交是否直接提到与该任务相关的未完成问题；若该问题阻塞当前任务，则先在 `TODO.md` 中显式记录为前置依赖。
3. 阅读当前任务涉及的代码、测试、规格说明与约束，整理最小正确改动方案。
4. 实现当前任务，避免使用变通方案；若发现规格缺口或阻塞问题，按要求更新 `TODO.md` / `PLAN.md` 并停止在该前置问题上。
5. 运行任务要求的验证命令，以及必要的 `cargo fmt`、`cargo test`、`cargo clippy --all-targets -- -D warnings`（若适用且不与任务要求冲突）。
6. 根据执行结果更新 `memory/claude_plan.md` 进度，记录关键发现、计划调整和已完成步骤。
7. 完成后在 `TODO.md` 中将该任务标题标记为 `[DONE]`，补全完成记录；仅在阶段计划发生变化时更新 `PLAN.md`。
8. 按仓库约定创建一次 git 提交，然后停止，不继续下一个任务。

> 说明：在确认具体任务后，会把更具体的实施步骤补充到本文件。

## 当前任务：`CG-T07S0a`

目标：完成 `effect_handle_top_level_val_pattern_access_basic.scoop` 的最终验收，并在不引入变通方案的前提下解除其对默认 full-suite 的阻塞。

具体步骤：

1. 先检查当前工作树状态，避免误覆盖已有未提交修改。
2. 阅读与 `effect_handle_top_level_val_pattern_access_basic`、EffectStep lowering、top-level value ref / once-init / pattern binder 相关的实现与最近回归测试，确认该任务原始修复当前是否仍成立。
3. 先重跑任务要求的定向 build/test，确认该 fixture 当前状态；若原始问题复发，则直接修复该根因并补最小回归。
4. 在原始 fixture 通过的前提下，重跑默认 `cargo run -p scoop -- test`，确认当前阻塞是否已解除，或定位新的、与本任务直接相关的 blocker。
5. 若 full-suite 暴露新的直接前置 blocker，则按用户要求把最小 prerequisite 插入 `TODO.md` 的正确位置，更新依赖与完成记录，并停止在该 blocker 记录上。
6. 若 full-suite 通过并且当前任务完整满足验收条件，则把 `CG-T07S0a` 标记为 `[DONE]`，补全完成记录；仅在阶段计划变化时更新 `PLAN.md`。
7. 运行所需格式化/静态检查（至少包含任务要求验证，以及必要的 `cargo clippy --all-targets -- -D warnings`），确认无警告。
8. 将本次相关改动提交为单个 git commit，然后停止。

## 当前进度

- 已确认首个未完成任务为 `CG-T07S0a`。
- 已执行 `cargo run -p scoop -- build tests/fixtures/run-pass/effect_handle_top_level_val_pattern_access_basic.scoop -o /tmp/effect_handle_top_level_val_pattern_access_basic`，通过。
- 已执行 `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/effect_handle_top_level_val_pattern_access_basic.scoop`，通过。
- 已执行 `cargo run -p scoop -- test`，默认 full-suite 通过（`fixtures: ok (1271)`）。
- 已执行 `cargo clippy --all-targets -- -D warnings`，通过。
- 已更新 `TODO.md`，将 `CG-T07S0a` 标记为 `[DONE]` 并补充最终完成记录。
- 下一步：检查工作树差异，提交本轮变更后停止。
