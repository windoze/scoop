# 执行计划与决策摘要

## 约束说明

- 按用户要求，本文件在任何命令执行前创建，并持续记录计划、关键进展与调整。
- 这里记录的是可审计的决策摘要与执行步骤，不包含逐字的内部推理草稿。
- 本轮目标是：先处理最新提交中提到的既有问题；再完成 `TODO.md` 中第一个未完成任务；完成后测试、更新文档、提交并停止。

## 初始执行步骤

1. 查看最新一次 Git 提交信息，确认是否提到了待修复的既有问题。
2. 阅读 `TODO.md`，定位第一个未完成任务。
3. 如任务过大，拆分为更小子任务，并同步更新 `PLAN.md` 与 `TODO.md`。
4. 阅读相关代码、规格与测试，确认实现边界和依赖。
5. 实现当前目标任务，不引入规避性 workaround。
6. 运行相关测试；如有必要，补充或修复测试。
7. 运行格式化、`cargo clippy --all-targets -- -D warnings` 以及必要的完整测试。
8. 更新 `TODO.md`、`PLAN.md` 与本文件，记录完成状态或阻塞原因。
9. 提交 Git commit，并在完成一个任务后停止。

## 当前状态

- 已查看最新提交：`[T3016e] Track nested handler arm propagation regression`。该提交本身只更新了计划文件，没有附带生产代码修复；提交中提到的共享回归属于本轮需要优先修复的既有问题。
- 已读取 `TODO.md` / `PLAN.md`，确认第一个未完成任务是 `T3016e`，目标是修复 nested handler arm / `try-catch` 在 inner arm/catch 中再次 perform 或 rethrow non-resuming effect 后，错误继续当前 body 的回归。
- 已复现 3 条目标 fixture，现象一致：
  - inner arm/catch 内第二次 `Raise.raise(...)` / `Boom.boom(...)` 后，没有命中外层最近 handler/catch；
  - 程序继续执行 `unreachable_after_inner` / `unreachable_after_middle`，说明不是 self-capture，而是错误地把 outward propagation 当成当前 arm 正常完成。
- 已导出 failing case 的 LLVM IR 并定位根因：
  - dispatch loop / arm 执行 IR 本身没有把 active/inactive 分支写错；
  - 真正问题在 plan builder：arm body 仍作为 opaque expr lowering，inner handle 的 `HandleStateMachinePlan::may_suspend_outward()` 只检查 `suspend_sites` / nested handles，未把 arm body 内再次 outward suspend 计入；
  - 结果是外层 machine 把这类 inner handle 误判成普通 `NestedHandle`，没有生成 `NestedHandleBoundary` + `Suspend` terminator，所以在 `codegen_handle_expr_via_state_machine` 的 `handle_propagate` 分支里只是写默认值并继续当前 state。
- 已完成实现：
  1. 在 `state_machine_plan.rs` 为 arm body 增加了精确的 outward-suspend 判定，并让 `HandleStateMachinePlan::may_suspend_outward()` 消费它。
  2. 新增 unified transform 结构测试 `nested_handle_with_non_resuming_arm_rethrow_materializes_outer_boundary`；同时保留并通过了既有的 self-contained immediate-resume nested handle 测试，确认没有把自洽 nested handle 误升级成 boundary。
  3. 已将 `tests/fixtures/run-pass/effect_handler_stack_nearest_three_levels_and_arm_outside_scope.scoop` 从旧 `EXPECT: fail` 改回 `EXPECT: pass`。
- 已验证通过：
  - `cargo test -p scoopc nested_handle_ -- --nocapture`
  - `cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/effect_handler_stack_nearest_and_arm_outside_scope.scoop`
  - `cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/effect_custom_nonresuming_nested_nearest_and_arm_outside_scope.scoop`
  - `cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/effect_handler_stack_nearest_three_levels_and_arm_outside_scope.scoop`
  - `cargo test --all`
  - `cargo clippy --all-targets -- -D warnings`
- 已同步文档：
  - `TODO.md` 已将 `T3016e` 标记为 `[DONE]`，并记录 root cause、修复方式与验证命令。
  - `PLAN.md` 已新增当前轮完成更新，并把 effect 主线下一项推进到 `T3016eR`。
- 下一步：
  1. 检查最终 diff 与工作树状态。
  2. 提交本轮变更并停止，不继续处理 `T3016eR`。
