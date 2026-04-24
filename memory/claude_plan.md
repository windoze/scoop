# 本轮执行计划（T4017e2）

## 当前判断

- 最新提交 `937bfe44 [T4017e1] Localize continuation resume pending scope` 未记录需要先处理的新遗留问题。
- `TODO.md` 当前主线目标已经从 `T4017e2` 推进到 `T4017e3`。
- `T4017e2` 的实现、验证与任务文档同步均已完成；当前剩余工作是检查变更范围、提交并停止。

## 已完成的关键实现

- runtime 新增显式 propagation outcome 路径：
  - `scoop_continuation_resume_with(...)` 增加 `ScoopEffectOutcome *outcome` 参数。
  - continuation propagation 时显式写入 `outcome->signal.resume_token = pending_continuation`。
- 保留兼容回退路径：
  - `scoop_continuation_resume()` / `scoop_continuation_resume_u64()` 以及 `resume_with(..., outcome = NULL)` 仍可回退到既有 TLS replay-state 逻辑，避免破坏尚未迁移完成的调用方和 runtime 测试。
- LLVM codegen 中 `Continuation.resume(...)` 已切换为显式 outcome / frame replay-token 槽位方案：
  - fresh path 调用新的 `scoop_continuation_resume_with(..., outcome_slot)`；
  - replay path 不再从 TLS replay-state 读取，而是从当前 state-machine frame 的显式 replay-token 槽位读取 token 与 payload。
- state-machine frame 已新增按 suspend-site 分配的 `Continuation.resume` replay token 槽位。
- `SuspendCall` fresh path 捕获到 `Continuation.resume` propagation outcome 时，会把 `effect_outcome.signal.resume_token` 写入 frame；replay path 再读出并清空。
- IR 断言已纠偏：
  - 不再错误要求 async-task 普遍消除 `@scoop_callee_suspend_state_get`；
  - 真正针对 `Continuation.resume` replay 的 IR 断言集中在 `when_arm_try_resume_nested_handle_ir_keeps_binder_scope_for_inner_resume`，验证：
    - 出现 `continuation_resume_replay_token`
    - 不再出现 `continuation_resume_replay_state_raw`

## 已完成验证

- `cargo fmt`
- `cargo test -p scoop_runtime --test continuation_one_shot -- --test-threads=1`
- `cargo test -p scoopc --features llvm continuation_resume -- --nocapture`
- `cargo test -p scoopc --features llvm async_task_resume_replay_ir_terminates_step_fn_on_active_effect -- --nocapture`
- `cargo test -p scoopc --features llvm when_arm_try_resume_nested_handle_ir_keeps_binder_scope_for_inner_resume -- --nocapture`
- `cargo build tests/fixtures/run-pass/continuation_resume_answer_replay_basic.scoop -o /tmp/continuation_resume_answer_replay_basic.out`
- 运行 `/tmp/continuation_resume_answer_replay_basic.out`，输出与 `tests/fixtures/run-pass/continuation_resume_answer_replay_basic.stdout` 一致
- `cargo test --all`
- `cargo run -p scoop -- test`（`fixtures: ok (1169)`）
- `cargo clippy --all-targets -- -D warnings`

## 中途发现并已处理的问题

- `cargo clippy --all-targets -- -D warnings` 最初报告 `crates/scoopc/src/llvm/codegen/effect/mod.rs` 中两个 continuation resume helper 触发 `clippy::too_many_arguments`。
- 已通过引入 `ContinuationResumeResultSlots` 结构体收口三个输出槽位参数，并同步更新 `effect/mod.rs` 与 `effect/state_machine_emitter.rs` 的调用点。
- 修复后已重新通过 `cargo fmt`、`cargo clippy --all-targets -- -D warnings`、`cargo test -p scoopc --features llvm continuation_resume -- --nocapture`、`cargo test -p scoopc --features llvm when_arm_try_resume_nested_handle_ir_keeps_binder_scope_for_inner_resume -- --nocapture` 与 `cargo test -p scoop_runtime --test continuation_one_shot -- --test-threads=1`。

## 接下来的执行步骤

1. 检查工作区变更范围，确认只包含 `T4017e2` 及其文档同步所需内容。
2. 提交本轮变更，提交消息使用 `T4017e2`。
3. 提交后立即停止，不继续执行 `T4017e3`。

## 约束提醒

- 不以 workaround 作为完成标准。
- 只完成一个任务后停止。
- 所有输出与说明使用中文。
