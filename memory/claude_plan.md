# Claude Execution Plan

## Current Objective

Complete exactly the first incomplete task listed in `TODO.md`, then stop after marking it done and committing the completed work. This file records the execution plan and progress notes without exposing private chain-of-thought.

## Step-by-Step Plan

1. Read `TODO.md` first and identify the first task whose title is not prefixed with `[DONE]`.
2. Review the selected task body, dependencies, validation requirements, and completion-record expectations.
3. Inspect only the code, fixtures, and documentation needed to implement that task correctly.
4. If a concrete blocking prerequisite is discovered, update `TODO.md` with the minimum prerequisite task in the correct order, keep the current task incomplete, update this plan, commit the bookkeeping change, and stop.
5. Otherwise implement the selected task with minimal, spec-correct changes and no workarounds.
6. Add or update targeted tests/fixtures required by the task.
7. Run the task-specified validation and any relevant focused checks; fix failures caused by this task.
8. Update `TODO.md` by prefixing the task title with `[DONE]` and filling in the completion record.
9. Update this plan with completed key steps and validation results.
10. Commit all relevant changes with a clear task-scoped commit message.
11. Stop without starting the next task.

## Progress

- Plan initialized before repository inspection.
- Read `TODO.md`; first incomplete task is `P8-T02` in `TODO-4.md`.
- Latest commit is `a26aa96b [P8-T01] Document scalar operator baseline`; it is directly relevant as the required baseline, but does not mention an unfinished blocker.
- Implemented the scalar named intrinsic registry additions, FQN fallback helpers, LLVM lowering paths, and focused owner tests for representative integer, float, compareTo, and bool entries.
- Validation passed: `cargo test -p scoopc named_intrinsic -- --nocapture`, `cargo build`, `cargo clippy --all-targets -- -D warnings`, and `cargo test --all --all-targets` (rerun with longer timeout after the first full-suite command hit the tool's 120s total timeout; the stopped GC test passed individually in 0.06s).
- Updated `TODO.md` and `TODO-4.md` to mark `P8-T02` as `[DONE]` with a completion record.
- Re-ran `cargo test -p scoopc named_intrinsic -- --nocapture` after the final Char fallback mapping adjustment; it passed.
## 执行计划

1. 读取 `TODO.md`，按文档顺序找出第一个标题未带 `[DONE]` 的任务，并确认其要求、依赖和验证方式。
2. 只围绕该任务收集必要上下文；如最新提交明确提到与该任务直接相关的未完成问题，将其纳入当前任务或作为前置项记录到 `TODO.md`。
3. 实现该任务要求的最小正确改动，避免 workaround、夹具专用逻辑或偏离规范的替代方案。
4. 运行任务要求的验证命令和相关测试；若发现直接阻塞当前任务的规范/实现缺口，优先修复，或把最小前置任务插入 `TODO.md` 后停止。
5. 完成后更新 `TODO.md`：在该任务标题前加 `[DONE]`，填写完成记录；仅当阶段级计划改变时才更新 `PLAN.md`。
6. 提交全部相关改动，提交信息使用任务编号前缀；提交后停止，不继续下一个任务。

## 当前状态

- 已记录初始计划。
- 已确认首个未完成任务为 `P8-T03`。
- 发现直接阻塞点：`P8-T03` 需要 `unaryPlus` method 声明，但 P8-T02 尚未注册 `int_unary_plus` / `float_unary_plus` named intrinsic。将把这两个 entry 作为当前任务的前置修复一起完成。
- 已补齐 `int_unary_plus` / `float_unary_plus`，并将 `Int.hash` 收口为 named IR intrinsic，避免旧 retained member-call 路径阻塞 HIR typed call contract。
- 已在 `sysroot/core.scoop` 添加标量 type body method 声明，删除迁移后的顶层 numeric scalar extension intrinsic 声明，并新增 `tests/fixtures/run-pass/scalar_method_intrinsic_basic.scoop`。
- 已单跑 `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/scalar_method_intrinsic_basic.scoop`，结果通过。
- 已运行 `cargo test -p scoopc named_intrinsic -- --nocapture`、`cargo clippy --all-targets -- -D warnings`，均通过。
- 已运行 `cargo run -p scoop -- test`，全量 baseline 仍有 7 个非 scalar-method 新增 fixture 的失败项；已写入 `TODO-4.md` 完成记录并指向后续 `P13-T04` fixture 收尾。
- 已更新 `TODO.md` / `TODO-4.md` 将 `P8-T03` 标记为 `[DONE]`。
- 下一步：检查 git diff/status，提交本任务改动后停止。
