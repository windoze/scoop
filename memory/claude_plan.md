# 本轮执行计划（继续 T3009b2a）

## 当前判断

- 已检查上一轮留下的上下文：最新提交 `095ff94f6130527f9b47443ecf0154f6d77940aa` 未暴露需要先于 `TODO.md` 继续处理的额外既有问题。
- `TODO.md` 的首个未完成项已经被拆分，当前应执行的是 `T3009b2a`：把 `callee_suspend_state` 纳入 continuation/runtime ABI 捕获合同。
- 上一轮已经完成了 LLVM 侧的一部分接线：
  - `runtime_symbols.rs` 新增 `SCOOP_CALLEE_SUSPEND_STATE_GET` / `SCOOP_CALLEE_SUSPEND_STATE_CLEAR`
  - `runtime_abi.rs` 新增对应 runtime 声明，并把 LLVM continuation struct 扩展到第 8 个字段 `captured_callee_suspend_state`
  - `state_machine_emitter.rs` 在 `UnifiedStateTerminator::Suspend` 路径里把 `scoop_callee_suspend_state_get()` 的结果写入 continuation，并在捕获后清空 TLS suspend state
- 这说明本轮的主要工作不再是重新拆任务，而是把 runtime/C 侧结构与恢复路径补齐，并做最小而充分的验证，完成 `T3009b2a` 后立即停止，不推进到 `T3009b2b`。

## 本轮目标

完成 `T3009b2a`，确保 continuation/runtime ABI 正式携带并恢复 `callee_suspend_state`，使之后的 ordinary indirect callee resumed-body restore 拥有稳定、可验证的承载合同。

## 约束

- 只做 `T3009b2a`，不提前实现 `T3009b2b` 或最终 shared 验收。
- 不能恢复已删除的 shape-based 路线，也不能做 fixture-only workaround。
- 必须持续更新本文件记录关键进展。
- 修改文件时使用 `apply_patch`。
- 完成后必须更新 `TODO.md` / `PLAN.md`，运行测试与 `clippy`，提交 git commit，然后停止。

## 具体执行步骤

1. 检查当前工作区改动，确认上一轮对 LLVM 侧文件的未提交修改仍然存在且没有冲突。
2. 阅读以下关键文件，确认 C runtime 中 continuation 结构、trace、resume、alloc 路径的当前实现与字段顺序：
   - `runtime/c/scoop_runtime.c`
   - `crates/scoopc/src/llvm/codegen/runtime_abi.rs`
   - `crates/scoopc/src/llvm/codegen/effect/state_machine_emitter.rs`
3. 在 C runtime 中实现 ABI 对齐：
   - 给 `ScoopContinuation` 增加 `captured_callee_suspend_state`
   - 更新布局断言、必要的初始化与 trace/diagnostic 辅助
   - 在 `scoop_continuation_resume_common()` 中把 continuation 中捕获的 suspend state 恢复回 TLS
4. 检查 LLVM 侧结构字段索引与 C runtime 顺序是否完全一致；若有偏差，修正索引或布局声明。
5. 补测试：
   - 优先增加一个 LLVM/IR 定向测试，验证 suspend 路径会调用 `scoop_callee_suspend_state_get` / `clear`，并把值写入 continuation 的新增字段
   - 如果现有测试框架更适合做布局或声明断言，则在对应模块添加断言测试
6. 运行验证：
   - 先跑与本任务直接相关的测试
   - 再跑 `cargo test --all`
   - 再跑 `cargo clippy --all-targets -- -D warnings`
7. 若验证通过：
   - 更新本文件记录完成情况
   - 把 `TODO.md` 中 `T3009b2a` 标记为完成
   - 更新 `PLAN.md` 反映当前进度与后续依赖
   - 生成一次提交，只包含本轮逻辑必要变更
8. 若验证过程中暴露出更前置、真实且未跟踪的规范缺口：
   - 停止继续实现下游逻辑
   - 在 `TODO.md` / `PLAN.md` 中新增或重排依赖任务
   - 更新本文件说明阻塞原因
   - 提交这些计划调整后停止

## 完成判据

- continuation 的 LLVM 布局与 C runtime 布局一致
- suspend 捕获时会把 `callee_suspend_state` 持久化到 continuation 中
- continuation resume 时会把该状态恢复回 runtime TLS
- 新增或更新的测试能覆盖这条 ABI 合同
- `cargo test --all` 与 `cargo clippy --all-targets -- -D warnings` 通过
- `TODO.md` / `PLAN.md` / 本文件已同步更新，并完成 git commit

## 当前进展（2026-04-18 本轮执行中）

- 已完成 runtime/C 侧接线：
  - `ScoopContinuation` 新增 `captured_callee_suspend_state`
  - continuation trace 现会追踪该字段
  - `scoop_continuation_alloc()` 会把该字段初始化为 `0`
  - `scoop_continuation_resume_common()` 现会在 step_fn 动态范围内恢复 captured callee state 到 TLS，并在返回后恢复调用方原 TLS 值
  - 为避免 moving GC 在 resumed body 消费前让 TLS raw 指针失效，resume 动态范围内对 captured callee state 做了 pin/unpin
  - `scoop_thread_unregister()` 现会清空 `__scoop_callee_suspend_state`
- 已完成测试补充：
  - `state_machine_emitter.rs` 新增 IR 定向测试，锁定 suspend 路径会调用 `scoop_callee_suspend_state_get` / `clear` 并写入 continuation 新字段
  - `continuation_one_shot.rs` 新增 runtime 行为测试，锁定 continuation resume 会临时恢复 captured callee state，并在返回后恢复调用方原 TLS
  - `effect_tls.rs` 新增 TLS 可观测测试，锁定 `clear` 与 `thread_unregister` 都会清空 callee suspend TLS
- 已完成定向验证：
  - `cargo test -p scoop_runtime --test continuation_one_shot`
  - `cargo test -p scoop_runtime --test effect_tls`
  - `cargo test -p scoopc suspend_ir_captures_callee_suspend_state_into_continuation -- --nocapture`
- 已完成全量验证：
  - `cargo fmt`
  - `cargo test --all`
  - `cargo clippy --all-targets -- -D warnings`
- 已完成任务状态同步：
  - `TODO.md` 已把 `T3009b2a` 标记为完成
  - `PLAN.md` 已记录本轮完成情况，并将下一项推进到 `T3009b2aR`
- 剩余步骤：
  - git commit 并停止
