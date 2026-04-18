# 本轮执行计划（可公开摘要）

说明：用户要求记录“完整思考过程”。我不会记录逐字内部推理，但会完整记录可公开的决策依据、执行步骤、风险判断与状态变化，便于审计和追踪。

## 本轮目标

- 只完成 `TODO.md` 中首个未完成任务，然后停止。
- 本轮定位到的任务是 `T3016cR`：复审 outer-body `Continuation.resume(...)` 的生产 lowering，确认它没有回落到 generic call、nested-frame seeding 缺口或 silent exit。

## 初始检查

1. 已检查最新提交 `fd53dd9cc76d40be00d8c1980f1338be7d3aedca`（`[T3016c] Fix outer-body continuation replay in nested handle resumes`）。
2. 该提交主题没有额外声明“尚未修复、必须先独立处理”的既有问题，因此按要求继续读取 `TODO.md` / `PLAN.md`。
3. 已确认 `TODO.md` 的首个未完成任务是 `T3016cR`，无需再拆分子任务。

## 本轮复审重点

1. 确认 `Continuation.resume(...)` 的 codegen 入口是否仍只依赖 typecheck side table：
   - `crates/scoopc/src/llvm/codegen/mod.rs`
   - `crates/scoopc/src/llvm/codegen/effect/mod.rs`
2. 确认 outer-body / nested-handle resume 是否仍完全收口在 unified state-machine 合同内：
   - `crates/scoopc/src/llvm/codegen/effect/state_machine_plan.rs`
   - `crates/scoopc/src/llvm/codegen/effect/state_machine_segments.rs`
   - `crates/scoopc/src/llvm/codegen/effect/state_machine_transform.rs`
   - `crates/scoopc/src/llvm/codegen/effect/state_machine_emitter.rs`
3. 若复审中发现新的真实生产缺口，需要先修复并重新复审；若未发现，则只更新文档状态并提交。

## 复审结论摘要

- `codegen_call()` 仍只在 `continuation_resume_call_sites` 命中时调用 `codegen_continuation_resume_builtin()`；没有按成员名、receiver 类型或源码形状推断 builtin resume。
- `state_machine_plan::classify_builtin_suspend_call()` 也只按同一 side table 把 builtin `Continuation.resume(...)` 建模为 `RuntimeRaise("Continuation.resume")`，不存在第二条旁路。
- 自洽 nested handle 通过 `may_suspend_outward()` 留在自身 state machine 内，不再在 outer machine 中物化 `NestedHandleBoundary`；这意味着 outer `when` arm / nested handle / `Continuation.resume(...)` 路径里的 binder scope 可以直接被 inner handle seeding 消费，不再触发 `effect frame seed outer-scope local`。
- escaped continuation replay 仅对 call-like / boundary site 分配 `escape_resume_target`；direct `perform` / `runtime-raise` continuation 继续停在各自的 post-site resume state，不会被错误重定向回旧 owner-state replay，因此不会再回到错误的 earlier perform 之后路径。
- 本轮复审未发现需要继续修复的新生产问题，因此本轮改动只涉及 `TODO.md` / `PLAN.md` / 本文件的状态同步。

## 已执行验证

- `cargo test -p scoopc when_arm_try_resume_nested_handle_ir_keeps_binder_scope_for_inner_resume -- --nocapture`
- `cargo test -p scoopc source_plan_does_not_assign_escape_replay_target_for_later_perform_site -- --nocapture`
- `cargo test -p scoopc self_contained_nested_handle_does_not_materialize_outer_boundary_resume_path -- --nocapture`
- `cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/effect_escape_continuation_nested_outer_resume_inner_multi.scoop`
- `cargo test -p scoop_runtime continuation_resume_ -- --nocapture`
- `cargo test --all`
- `cargo clippy --all-targets -- -D warnings`

## 当前状态

- `T3016cR` 已完成，结论已经写回 `TODO.md`、`PLAN.md` 与本文件。
- 当前只剩创建本轮 Git 提交，然后停止，不进入 `T3016d`。
