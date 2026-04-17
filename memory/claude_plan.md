# 当前执行计划

说明：按要求先记录执行计划与进度。这里记录的是可审计的执行摘要、步骤和决策，不包含冗长的内部推演。

## 初始目标

本轮只完成 `TODO.md` 中第一个未完成任务；如果遇到前置缺陷、规范不匹配或任务过大，则先调整 `TODO.md` / `PLAN.md`，提交后停止。

## 执行步骤

1. 检查最新一次 Git 提交，确认是否提到需要先修复的遗留问题。
2. 阅读 `TODO.md`，定位第一个未完成任务。
3. 阅读 `PLAN.md`、相关代码与测试，判断该任务是否可直接完成。
4. 若任务过大，拆分为更小子任务，并更新 `PLAN.md` 与 `TODO.md`，然后执行新的第一个子任务。
5. 实现任务所需代码修改，确保不引入规避性方案或与规范不一致的行为。
6. 运行相关测试、`cargo fmt`、`cargo clippy --all-targets -- -D warnings`，并修复发现的问题。
7. 更新 `TODO.md`、`PLAN.md`、本文件，记录完成情况或阻塞关系。
8. 使用清晰的 Git 提交信息提交本轮改动，然后停止，不继续后续任务。

## 约束与判定

- 若最新提交提到已有问题，则这些问题优先处理，处理完后再进入 `TODO.md` 的当前任务。
- 若发现规范缺口、实现边界或缺失语言特性阻塞当前任务，必须先把该问题整理成更前置的任务并更新计划，然后提交并停止。
- 不接受临时兼容、测试特判、fixture-only 修补或其他规避实现。

## 进度记录

- 已完成：创建本计划文件。
- 已完成：检查最新提交，确认最新提交主要是追踪 blocker 的计划更新，未带出新的未修生产代码。
- 已完成：读取 `TODO.md` / `PLAN.md`，定位首个未完成任务为 `T3009b0a1d`（修正 unified `ObjectInitAccessBoundary` 的 inactive-continue / active-dispatch 合同）。
- 已完成：评估任务规模，可直接实现，无需继续拆分 `TODO.md` / `PLAN.md`。
- 已完成：用最小复现确认当前失败行为：
  - 复现程序：`handle { ReadyConfig.x + 1 } with { Raise.raise(err: RuntimeError) -> 10 }`
  - 当前输出只到 `inactive_before`、`ready.init`，说明 direct object/property access 仍被 shared `Suspend` terminator 无条件走 suspend 返回。
- 已完成：修改统一 `Suspend` terminator，让 `ObjectInitAccessBoundary` 在 TLS inactive 时回到当前 state machine 继续 caller-tail，在 TLS active 时继续走 dispatch 返回路径。
- 已完成：新增 run-pass fixture `tests/fixtures/run-pass/effect_handle_object_init_access_inactive_basic.scoop`，锁定 direct object value/property access 的 inactive-path 与 active dispatch。
- 已完成：验证 `cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/effect_handle_object_init_access_inactive_basic.scoop`、`cargo test --all`、`cargo clippy --all-targets -- -D warnings` 均通过。
- 已完成：在验证过程中顺手收回一个已恢复通过的 stale xfail：`tests/fixtures/run-pass/effect_escape_continuation_resume_cross_thread.scoop`。
- 备注：整套 `cargo run -p scoop --features llvm -- test` 当前仍会停在另一个已存在的 stale xfail `effect_handler_stack_nearest_three_levels_and_arm_outside_scope.scoop`；该收尾属于既有 `T3017` 范围，不改变本轮首个未完成主线任务已完成的结论。
- 已完成：更新任务状态与计划文件，准备提交本轮改动。
