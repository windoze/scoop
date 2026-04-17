# 执行计划（细化中）

## 已完成

1. 已检查最近一次 git 提交：`[T3014] Register all handler op tags in runtime stack`。
   - 提交说明本身没有新增一个独立、必须先于 `TODO.md` 处理的“既有问题”条目。
2. 已阅读 `TODO.md` / `PLAN.md`，确认首个未完成任务是 `T3014R`：
   - `Review：确认 multi-op handler registration 与 unmatched propagation 已与合同一致`

## 当前任务：T3014R

1. 审查 `crates/scoopc/src/llvm/codegen/effect/state_machine_emitter.rs` 中与下列路径相关的生产代码：
   - `allocate_registered_handler_frames`
   - `pop_registered_handler_frames`
   - `dispatch_arm` / `dispatch_unmatched`
   - `handle_propagate` / `handle_done`
2. 审查 `crates/scoopc/src/llvm/codegen/effect/state_machine_transform.rs` 中 `dispatch_entries()` 合同来源，确认 runtime 注册是否与 lowering 合同一一对应。
3. 审查 `crates/scoopc/src/llvm/codegen/runtime_abi.rs` 与 `runtime/c/scoop_runtime.c` 中 handler stack ABI，确认不存在为某些 fixture 特判的外传路径。
4. 特别甄别一个新增发现的合同风险：
   - `dispatch_entry` 结构允许同一 `op_fqn` 下存在多个 arm；
   - emitter 当前 dispatch 代码只取 `arms().first()`；
   - 需要确认这是否已被上游 typecheck 保证“不可能进入生产路径”，否则必须在本轮直接修复，不能带着静默假设收口 review。
5. 根据审查结果分支处理：
   - 若发现真实生产缺口：直接修复、补测试、重新验证。
   - 若未发现需要落地的代码修复：更新 `TODO.md` / `PLAN.md` / 本文件，记录复审结论与验证结果。
6. 执行验证，至少覆盖：
   - 与 `T3014` 相关的定向 IR / fixture 测试
   - `cargo test --all`
   - `cargo clippy --all-targets -- -D warnings`
7. 完成后提交本轮结果并停止。

## 当前结论（暂存）

- `dispatch_unmatched` 当前直接跳往 outward propagation 路径，不再穿过 `handle_done`。
- multi-op registration 的 push/pop 已集中到 helper 中，但继续复查后确认这不是当前 review 的唯一合同点。

## 本轮阻塞结论

1. 已确认一个未被显式跟踪的前置缺口：
   - `DispatchPlan` / `UnifiedDispatchEntry` / `SuspendSite.matching_arms()` 都保留“同一 `op_fqn` 可能关联多个 arm”的 lowering 合同；
   - `state_machine_emitter.rs` 的 dispatch 主路径却仍在命中 entry 后静默取 `dispatch_entry.arms().first()`；
   - runtime `op_tag` 也只按原始 `op_fqn` 分派，说明 same-op multi-arm dispatch 合同尚未真正闭环。
2. 这会阻塞 `T3014R`：
   - 在该缺口补齐前，不能把当前生产路径描述为“已经按统一合同收口”；
   - 按仓库工作流，需要先把缺口补成新的前置任务，再移动 `T3014R` 依赖。
3. 已执行的计划调整：
   - 在 `TODO.md` 中新增 `T3014a`：补齐同一 `op_fqn` 下多 arm 的 unified handler dispatch 合同。
   - 将 `T3014R` 依赖改为 `T3014a`。
   - 在 `PLAN.md` 中记录阻塞原因，并把当前执行顺序更新为 `T3014a -> T3014R -> ...`。

## 本轮收尾

1. 不继续修改生产代码，也不进入下一个实现任务。
2. 提交 `TODO.md` / `PLAN.md` / `memory/claude_plan.md` 的阻塞与重排更新。
3. 停止，等待下一轮从新的首个未完成任务 `T3014a` 开始。
