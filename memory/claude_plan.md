# 执行计划记录

## 说明

用户要求在执行命令前将思路与计划写入此文件。这里记录的是可审计的执行计划、判断依据摘要和后续进展；不包含逐字逐句的内部推理。

## 初始目标

本轮只完成 `TODO.md` 中第一个未完成任务，然后停止。

## 初始计划

1. 检查最新一次 Git 提交，确认提交说明里是否提到已有问题；若有，先修复这些问题。
2. 阅读 `TODO.md`，定位第一个未完成任务。
3. 结合 `PLAN.md`、相关代码和测试，判断该任务是否可以在本轮完整落地。
4. 如果任务过大，则把它拆分为更小的子任务，并同步更新 `PLAN.md` 与 `TODO.md`；本轮只执行拆分后的第一个子任务。
5. 实现任务所需代码修改。
6. 运行相关测试，并补充必要测试；确保 `cargo clippy --all-targets -- -D warnings` 无警告。
7. 更新文档与任务状态，包括 `TODO.md`、`PLAN.md`，必要时补充 `README.md` 或代码注释。
8. 提交本轮修改，提交信息清晰描述完成内容，然后停止。

## 当前状态

- 已完成：初始化计划文件。
- 已完成：检查最新提交，未发现提交说明中提及需要优先修复的既有问题。
- 已完成：读取 `TODO.md` 与 `PLAN.md`，当前首个未完成任务为 `T2003b2`（在 `if/branch` 中支持 immediate-resume direct perform）。
- 已完成：读取 `T2003b2` 周边任务定义与实现上下文，确认无需继续拆分。
- 已完成：实现 immediate-resume 的 statement-position `if` then/else branch 恢复路径，并补充 run-pass fixtures。
- 已完成：运行验证命令并通过。

## 当前判断摘要

- 最新提交 `d4e66aa8007a51130021ed40dfda92c4e4caee38` 的说明仅为 `[T2003b1] Support immediate-resume nested block perform`，未包含需先修复的遗留问题描述。
- 结合 `TODO.md` 与 `PLAN.md`，`T2003b1` 已完成，下一项为 `T2003b2`。
- 本轮优先检查：
  1. `T2003b2` 的范围是否可以在一轮内完整实现；
  2. 现有 immediate-resume lowering 对 branch/if 的限制点在哪里；
  3. 需要新增哪些 fixtures 与诊断。

## 实施摘要

- 代码实现：
  - 为 immediate-resume 增加 `block + if-then/if-else` 路径表示；
  - 扩展扫描器，使 statement-position `if` 的 then/else block 可承载单个 direct perform；
  - 引入共享执行 plan，收口 state0/state1/no-perform continuation helper，避免 no-perform 分支污染命中 perform 路径的编译期环境；
  - 保持当前阶段约束：仍只支持单个 direct perform、单次 `resume(value)`。
- 新增回归：
  - `tests/fixtures/run-pass/effect_resume_if_then_branch_single_perform.scoop`
  - `tests/fixtures/run-pass/effect_resume_if_else_branch_single_perform.scoop`

## 验证结果

- `cargo test --all`：通过
- `cargo run -p scoop -- test`：通过（`fixtures: ok (916)`）
- `cargo run -p scoop --features llvm -- test`：通过（`fixtures: ok (916)`）
- `cargo clippy --workspace --all-targets -- -D warnings`：通过

## 结束状态

- 本轮任务 `T2003b2` 已完成。
- 下一轮首个未完成任务应为 `T2003b3`。
