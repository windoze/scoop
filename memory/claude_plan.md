## 当前轮次计划

说明：
- 按要求先记录执行计划，再进行仓库检查与实现工作。
- 不写入不可审计的内部思维链；这里只记录可复核的步骤、判断依据和进展。

### 执行步骤

1. 检查最新一次 Git 提交，确认提交说明里是否提到已知问题、遗留修复或需要优先处理的事项。
2. 阅读 `TODO.md`，定位第一个未完成任务。
3. 阅读 `PLAN.md`，核对该任务的上下文、依赖和预期范围。
4. 判断该任务是否可在当前轮次完整完成：
   - 如果可完成：直接实现、补测试、验证、更新文档、提交。
   - 如果过大或被前置缺陷阻塞：把任务拆成更小子任务或新增前置修复任务，并更新 `TODO.md` / `PLAN.md`，本轮只处理新的第一个未完成项。
5. 在实现前后持续更新本文件，记录：
   - 当前锁定的任务
   - 是否发现阻塞
   - 已完成的关键步骤
   - 运行过的验证命令及结果摘要
6. 完成本轮首个任务后：
   - 更新 `TODO.md`
   - 更新 `PLAN.md`
   - 提交 Git commit
   - 停止，不继续做下一个任务

### 初始状态

- 尚未检查最新提交
- 尚未读取 `TODO.md`
- 尚未读取 `PLAN.md`
- 尚未确定本轮任务

## 当前进展

- 已检查最新提交：`[T3009b0a1eR] Review NestedHandleBoundary inactive-path contract`
- 已确认最新提交说明本身没有额外 commit body，也没有单独点名需要先修的遗留问题。
- 已读取 `TODO.md` / `PLAN.md`，当前首个未完成任务为 `T3009b0a1cR`：
  - `Review：确认 unified SuspendCall 的 inactive-path 已回到单一 state-machine 合同`
- 当前判断：该任务是定向复审任务，范围明确，可在本轮直接完成；暂不需要拆分子任务。

### 本轮复审重点

1. 检查 `UnifiedStateTerminator::Suspend` 对 `SuspendSiteKind::CallMaySuspend` / `CallStateMachineCallee` / `ClassCtorInit` 的 inactive/active 分流是否仍只依赖 `SuspendSiteKind` 与 TLS active。
2. 检查 `resume_path` / synthetic resume slot 是否仍是 post-call caller-tail 的唯一 authoritative 数据通路。
3. 检查 `expr.rs`、普通 call codegen、hidden-suspend helper 相关生产入口，确认没有为 `SuspendCall` inactive-path 回流出 call-site 特判、callee 名称特判、shape-based 分流或 ordinary helper 专用补丁。
4. 若复审发现问题，直接修复并补测试；若未发现问题，则跑定向与全量验收后更新 `TODO.md` / `PLAN.md`。

## 本轮结果

- 已完成任务：`T3009b0a1cR`
- 结论：未发现需要修复的生产代码问题。

### 复审结论摘要

1. `SuspendCall` 的 inactive/active 分流仍只由 shared `UnifiedStateTerminator::Suspend` + TLS active 驱动；`HandleStateOp::SuspendCall` 自身只负责求值调用表达式。
2. `resume_path` + synthetic resume slot 仍是 post-call caller-tail 的唯一 authoritative 数据通路；`state_machine_plan.rs`、`state_machine_segments.rs`、`state_machine_transform.rs` 都要求 call-like suspend site 保留这份合同元数据。
3. `expr.rs` 仍统一把调用表达式导向 `codegen_call`；`Continuation.resume(...)` 只依赖 `continuation_resume_call_sites` side table；ordinary call 的 TLS active 检查仍只服务普通 frame，step function 生成时清空了 `current_fun_return_ty` / `return_context`，没有 ordinary-helper 旁路。

### 已执行验证

- `cargo test -p scoopc resume_path_is_preserved_from_plan_to_segments_to_unified_machine -- --nocapture`
- `cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/effect_handle_suspend_call_inactive_helper_basic.scoop`
- `cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/effect_handle_hidden_suspend_local_closure_helper_basic.scoop`
- `cargo test --all`
- `cargo clippy --all-targets -- -D warnings`

### 收尾步骤

1. 更新 `TODO.md`：将 `T3009b0a1cR` 标记为完成并记录复审结论。
2. 更新 `PLAN.md`：记录本轮进展，并把下一项推进到 `T3009b0a2`。
3. 提交本轮变更后停止。
