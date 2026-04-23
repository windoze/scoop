# Scoop：下一轮计划（正确的单次 delimited continuation 优先）

> 生成时间：2026-04-21  
> 历史归档：`PLAN-5.md` / `TODO-5.md`  
> 本轮主题：先修复全量回归暴露的 `@Extern` + moving-GC native-roots 既有问题，再把 `Continuation` 从当前“为 effect / async lowering 服务的 step-driving advanced API”收口为**正确的单次（one-shot）delimited continuation**，然后继续按 `SCOOP_TASK.md` 收口 core `Task` surface、最小化 task-only runtime/codegen、把 task 主体迁回 Scoop，并进一步收口去除 `Task` 内建 lock 的轻量 claim single-driver 合同（只覆盖 phase 1-3；phase 4 executor / wake / reactor 明确延期到 stdlib）；annotation、删除 `inline` 关键字、FFI / ABI、const / comptime 顺延。  
> 设计前提：**不支持 multi-shot continuation**。Scoop 保持当前可变局部、writeback、once-init 与 GC-managed frame 的整体运行时方向，不为 continuation cloning / replay 另开一套“immutable everything”语义世界。

## 0. 工作原则

- 本轮严格按 `TODO.md` 中的顺序推进，不跨条目并行实现。
- `Continuation` 的目标语义是**单次、deep、以最近 `handle` 为 delimiter** 的 delimited continuation。
- 语言层面只保留 `Effect.op(args) -> expr` 与 `Effect.op(args), k -> expr` 两种 handler arm；`-> resume` 从用户态语法移除。若需要 immediate-resume fast path，只能作为 lowering / codegen 内部优化分类。
- `k.resume(payload)` 在 resumed computation 正常完成 delimiter 时，应返回该 delimiter 的 answer type；后续本地代码可继续执行。
- repeated resume 继续是 one-shot 违规；multi-shot、continuation cloning、resume-many replay 都不纳入本轮范围。
- `Task<T>` 仍是 general-purpose async API；raw `Continuation` 仍是 advanced API。区别在于本轮结束后，`Task` 不得再依赖“resume 后偷读 frame 前缀结果”的 runtime hack。
- 基于 `SCOOP_TASK.md`，core task 设计仍在进行中，不保留 `Poll<T>` / `poll()` 等命名的向后兼容包袱；若公开 surface 需要改名，应直接收口到最终形态。
- core `Task` 继续收口为轻量 single-driver object：不支持多个父 task / 多线程共享同一 task 驱动；public `step()` 的并发 / reentrant 误用直接 trap，`Pending` 不再承担竞争失败语义。
- annotation 的方向改为**compile-time markers only**：不把 annotation 做成复杂 nominal runtime/type-system feature。
- `inline` 关键字默认从语言 surface 移除；若仍需要内联提示，由 `@Inline` 统一承担，且它只是一种 compile-time marker / 优化提示，不附带控制流语义。
- executor framework、wakeup queue、work-stealing、public `spawn/join` 调度语义继续 deferred，且明确顺延到 stdlib stage；它们不能成为本轮 core task 设计前提。
- 若实现改变公开语义，必须同步 `SCOOP_FULL_SPEC.md`；若涉及运行时合同，还要同步 `SCOOP_RUNTIME.md`、`sysroot/core.scoop` 与必要注释。

## 1. 顺序总览

1. 前置 blockers 与 continuation / `Task` review 已收口；`T1510c1`、`T1510c2`、`T4016R`、`T4016T1`、`T4016T1a`、`T4016T1b`、`T4016T1c`、`T4016T1R`、`T4016T1d1`、`T4016T1d2`、`T4016T1d3`、`T4016T1d4`、`T4016T1d5`、`T4016T2`、`T4016T3` 与 `T4016T4` 均已完成；但 core `Task` 还需按 `T4016T5 -> T4016T6 -> T4016T7 -> T4016T8 -> T4016T9 -> T4016T4R` 收口“去掉 per-task lock / 轻量 claim / single-driver trap”主线。
2. `ISSUES.md` 第 9 条：annotation markers、non-inline built-in annotations 与 `@Experimental` feature-gate marker（依赖 `T4016T4R`；回到该组后的剩余顺序：`T4012b3 -> T4012c -> T4012R`）
3. `ISSUES.md` 第 10 条：删除 `inline` 关键字与 legacy non-local return 语义残留（`T4013 -> T4013R`）
4. `ISSUES.md` 第 11 条：FFI / ABI 的 effect-impermeable 边界与 stable handle / pin 职责分离（`T4014a -> T4014b -> T4014R`）
5. `ISSUES.md` 第 12 条：const / comptime 纯计算子集扩展（`T4015a -> T4015b -> T4015c -> T4015R`）

## 2. 分阶段目标

### P0. 前置既有问题：`@Extern` + moving GC native-roots 回归

- `cargo run -p scoop -- test` 在 `T4016R` 验证阶段暴露 `tests/fixtures/runtime_gc/extern_enter_native_roots_gc.scoop` 失败：fixture 期望在 `SCOOP_GC_MOVE=1` 下通过 `@Extern("scoop_test_gc_collect_in_native")` 触发 GC 后仍能打印 `hello 7`，实际进程 `exit(3)`。
- 已复验：
  - 单独 `cargo run -p scoop -- build tests/fixtures/runtime_gc/extern_enter_native_roots_gc.scoop -o /tmp/extern_enter_native_roots_gc.out` 成功，但 `SCOOP_GC_MOVE=1 /tmp/extern_enter_native_roots_gc.out` 仍 `exit(3)`；
  - `cargo test -p scoop_runtime --test gc_enter_native` 通过，说明 runtime 的 “InNative + native_roots 保活对象” 基本能力仍在。
- 已定位到更精确的 blocking mismatch：
  - fixture 的 LLVM IR 确实生成了 `scoop_enter_native(root_slots = 1)`，局部 `x` 的槽位也被放进 `native_root_slots`；
  - 但 extern body 的 statepoint 返回后，codegen 继续沿用 native 期间的 SSA `gc.relocate` 值，并在 `scoop_leave_native()` 之后把它写回 `%x`；
  - moving GC 若已通过 `native_roots` 把 `%x` 更新到新地址，这个“把旧 SSA 值写回局部槽位”的动作会把 stale/pre-move 指针 resurrect 回 managed frame，随后 `GC.handleNew/handleGet` 路径以 `exit(3)` 失败。
- 本轮已完成 `T1510c1`，修复点包括：
  - extern/native 三连调用改走独立 lowering，不再复用 ordinary safepoint 的 SSA keepalive spill/writeback 合同；`@Extern` / `scoop_enter_native` / `scoop_leave_native` 也已从 LLVM GC 视角标记为 leaf。
  - 新增“活动中的临时 GC root 槽位”管理：call args、class ctor params 等 pointer-shaped 中间值若要跨后续子表达式存活，会先落到受根集追踪的槽位，并同时纳入 native_roots 与 ordinary conservative root 收集。
  - class ctor 参数属性赋值不再缓存 `stored_args` SSA，而是按需从参数局部槽位 reload，避免在后续 extern/native 调用后继续沿用 stale 形参值。
  - 新增 `tests/fixtures/runtime_gc/extern_enter_native_gc_arg_spill_reload.scoop` 与 `tests/fixtures/build/extern_enter_native_no_statepoint_writeback.scoop` 两个定向回归，分别锁定“外层表达式中的 direct GC SSA reload”与“LLVM IR 不再把 extern/native 三连调用包成 statepoint”。
  - 已复验 `cargo run -p scoop -- test --fixtures tests/fixtures/build`、`cargo run -p scoop -- test --fixtures tests/fixtures/runtime_gc`、`cargo test --all` 与 `cargo clippy --all-targets -- -D warnings` 通过。
- `T4016R` 在重新执行全量 `cargo run -p scoop -- test` 时又暴露出新的前置 blocker：`tests/fixtures/run-pass/stackmap_registry_statepoint_smoke.scoop` 期望输出 `1`，实际输出 `-3`。
- 已复验：
  - 单独 `cargo run -p scoop -- build tests/fixtures/run-pass/stackmap_registry_statepoint_smoke.scoop -o /tmp/stackmap_registry_statepoint_smoke.out` 成功；
  - 直接执行 `/tmp/stackmap_registry_statepoint_smoke.out` 输出 `-3`，对应 `scoop_test_stackmap_statepoint_smoke()` 中“registry 非空，但 `__builtin_return_address(0)` lookup 未命中 record”的失败分支。
- 已定位到更精确的 blocking mismatch：
  - `T1510c1` 已通过 `tests/fixtures/build/extern_enter_native_no_statepoint_writeback.scoop` 锁定 extern/native 三连调用不再被 statepoint 包裹；
  - 但 `stackmap_registry_statepoint_smoke.scoop` 仍把 `@Extern("scoop_test_stackmap_statepoint_smoke")` 调用点当作真实 statepoint smoke 的载体，默认这个 caller return address 能在 registry 中查到 record；
  - 因此当前失败并不说明 registry 解析/索引逻辑整体失效，而是 smoke fixture 继续依赖已经被 `T1510c1` 明确移除的调用点形状。
- 本轮已完成 `T1510c2`：
  - `stackmap_registry_statepoint_smoke.scoop` 已改走 sysroot 内部 helper `__scoop_stackmap_statepoint_smoke()`；codegen 会把它 lowering 到 ordinary managed runtime call `scoop_test_stackmap_statepoint_smoke()`，从而让调用点重新保留真实的 statepoint / stackmap record。
  - `runtime/c/scoop_test.c` 注释已明确：stackmap smoke 必须走 ordinary managed runtime call；`@Extern` + `enter_native/leave_native` leaf lowering 明确不再作为 smoke 载体。
  - 新增 `tests/fixtures/build/stackmap_registry_statepoint_smoke_managed_call.scoop`，锁定 smoke 调用点会生成 `@scoop_test_stackmap_statepoint_smoke` 对应的 statepoint；原 `extern_enter_native_no_statepoint_writeback.scoop` 继续锁定 extern/native 三连调用不生成 statepoint。
  - 已复验手动 `build + run` 的 smoke 产物输出 `1`，并通过 `cargo run -p scoop -- test`、`cargo test --all` 与 `cargo clippy --all-targets -- -D warnings`。
- `T4016R` 已完成：在前置 blockers 收口后重新审查 continuation / `Task` 主线，结论如下：
  - parser / AST / HIR 里 `-> resume` 只剩 removed-syntax diagnostic；生产 surface 只保留 `->` 与 `, k ->` 两种 arm。
  - `Continuation.resume(...): Answer` 的静态模型、LLVM lowering 与 runtime 都统一走 `scoop_continuation_resume_with(...)` 共享 payload+answer helper。
  - `Task` 继续以私有 `__TaskStepResult` 作为 delimiter answer，但 runtime 只通过共享 continuation helper 消费它，不再依赖 task-private frame-peek / 旁路 ABI。
  - 已复验 `cargo run -p scoop -- test`、`cargo test --all` 与 `cargo clippy --all-targets -- -D warnings` 通过。

### P1. 正确的单次 delimited continuation 与 `Task` 去 hack

- continuation model 要从当前 `Continuation<T, eff E>` + `resume(...): Unit` 的 step-driving 形态，收口为带**显式 answer type** 的真正 continuation model。
- 默认语义固定为 deep handler：`k` 捕获从 effect 点到最近 `handle` delimiter 的剩余计算；执行 arm body 时 handler 自身 inactive；`k.resume(...)` 时在 captured handler stack 下恢复执行；若 resumed computation 再次通过 escape continuation suspend，则捕获 fresh continuation。
- 语言层面固定只保留两种 handler arm：`Effect.op(args) -> expr` 与 `Effect.op(args), k -> expr`；`-> resume` 从用户态语法移除。
- 若编译器仍需要 stack-local fast path / immediate-resume 分类，只能作为 lowering / codegen 内部优化，不得再暴露为独立语义或语法。
- 单次约束维持不变：不引入 frame clone、continuation copy、可重复 resume，也不把语言整体改造成“全部不可变以支持 multi-shot”。
- `Task` 需要真正成为“ordinary object + private continuation / step-result carrier”的薄封装：内部 continuation 的 answer type 由 task step driver 显式建模，而不是通过 runtime 私有 frame-layout 旁路回读。
- 为了把设计收口和主线实现分开推进，`T4016a` 进一步拆成两步：
  - `T4016a1`：先在 spec / runtime 设计文档中定稿 answer-returning continuation、deep handler、`-> resume` 移除与迁移叙事。
  - `T4016a2`：再把 sysroot / 内部注释对齐到同一套过渡合同，为 `T4016b` 的 parser / typecheck / HIR / lowering 实装清障。
- 随着代码盘点，`T4016b` 再拆成三步并与 `T4016c` 交错推进：
  - `T4016b1`：先移除用户态 `-> resume` 语法，并把原先 immediate-resume 的 tail 形态收口为 lowering / codegen 内部分类。
  - `T4016b2`：把 continuation answer type 接入 binder 静态模型与显式 `Continuation<Resume, Answer, eff E>` surface。
  - `T4016c`：再收口 runtime / ABI 的 answer-return channel，避免前端静态模型与底层 `void scoop_continuation_resume(...)` 继续错位。
  - `T4016b3`：最后基于统一 answer-return 通道，把 `Continuation.resume(...): Answer` 的 typecheck / lowering / codegen 主线彻底接通。
  - `T4016b4a`：先移除 legacy `Continuation<Resume, eff E>` shorthand，并收口最先暴露出来的 answer-hole codegen blocker。
  - `T4016b4a0`：已完成 object property / top-level immutable backing globals 的永久 GC roots 合同，显式 GC 后的模块级引用悬挂/错指问题已收口。
  - `T4016b4b0` 已完成：
    - `gc_continuation_multi_thread_concurrent_alloc_resume.scoop` 已恢复为真实的 stress-mode `run-pass` 回归：fixture 头部现为 `EXPECT: pass`，并通过 `ENV: SCOOP_GC_STRESS=1` 固定开启压力路径；
    - 已复验隔离的 fixtures runner 子集、手动 `build + SCOOP_GC_STRESS=1` 执行路径、`cargo test --all` 与 `cargo clippy --all-targets -- -D warnings` 全部通过；
    - 结论：此前在 `workerA_resuming` 后异常退出的 blocker 已随 `T4016b4a0` 的 GC roots 修复实质消失，本轮已把它正式收口为可持续回归的验收项。
  - `T4016b4b` 已完成：已重新盘点剩余 pure `Continuation<Resume>` shorthand 残留，并以全量 `run-pass` 完成最终验收。
  - `T4016R` 已完成：
    - 生产代码与文档中，continuation answer model、one-shot deep 语义、`-> resume` 移除与 `Task` 的私有 answer carrier 叙事现已一致。
    - 对仓库残留文本的机械复核显示：legacy continuation 简写仅剩 removed-diagnostic fixtures / 报错文本；`-> resume` 仅剩文档说明、removed diagnostic 与迁移回归。
  - `T4016d` / `T4016R` 收口的是 continuation answer model 与 task-hack 移除；`T4016T1~T4016T3` 又进一步完成了 public surface、ordinary Scoop task 主体与 task-only ABI 删除。但 `Task` 仍保留 per-task `Mutex` 与“共享/竞争 `step()` 可被 `Pending` 吸收”的过渡合同，因此还要继续前插 `T4016T4 -> T4016T5 -> T4016T6 -> T4016T7 -> T4016T8 -> T4016T9 -> T4016T4R`，再回到 annotation 主线。
  - `T4016T1R` 期间又收口了一个必须优先修的既有缺口：boxed multi-field enum variant 经 `val Variant(...) = expr` 解构后，若 payload 含 function type 且后续直接调用，隐藏 `Raise.raise(...)` 会被 ordinary callee suspend plan 误建模成 `Ref` 型 resume slot。现已通过：
    - 为 variant pattern 的隐藏 binder 恢复真实字段类型；
    - 将 `synth_raise_null_assertion_failed()` 的隐藏 `Perform` 收口为 `Nothing` 类型，并避免与外层合成 `when` 共用完全相同的 span；
    - 新增 boxed multi-field enum function payload run-pass 回归，并同步相关 HIR golden。
  - 当前顺序调整为：`T4016T4 -> T4016T5 -> T4016T6 -> T4016T7 -> T4016T8 -> T4016T9 -> T4016T4R -> T4012 -> T4013 -> T4014 -> T4015`。
- 当前状态：
  - `T4016a1` 已完成：`SCOOP_FULL_SPEC.md` / `SCOOP_RUNTIME.md` 已把 continuation answer model、deep handler、one-shot 与 `-> resume` 移除的迁移叙事收口到同一口径。
  - `T4016a2` 已完成：`sysroot/core.scoop`、`runtime/c/scoop_runtime.c` 与 `runtime/c/scoop_task.c` 的注释现已明确：
    - `Continuation<T, eff E>` 仍只是过渡中的 sysroot surface，answer type 尚待 `T4016b` 接入主线；
    - 用户态 handler surface 只保留 `Effect.op(args) -> expr` 与 `Effect.op(args), k -> expr`；
    - 当时 `Task` 仍保留“resume 后回读 frame 前缀得到 `__TaskStepResult`”的过渡债务；该债务已在 `T4016c` 收口进共享 helper。
  - `T4016a` 设计/注释收口阶段已完成；围绕旧 `void scoop_continuation_resume(void*)` 的 runtime ABI 错位也已在 `T4016c` 开始拆开。
  - `T4016b1` 已完成：
    - parser / AST / HIR / resolver / typecheck 已移除用户态 `-> resume` surface，并改为 removed-syntax diagnostic；
    - AST / HIR 级别的 `ImmediateResume` arm kind 已删除；tail `k.resume(...)` 仅作为 lowering / codegen 内部分类保留；
    - 相关 parse / HIR / typecheck / run-pass fixtures 已迁移到 `, k ->` + `k.resume(...)`，并同步了必要的 golden / 预期；
    - 已验证 `cargo test --all`、受影响 fixture 子集（38 个）以及 `cargo clippy --all-targets -- -D warnings` 通过。
  - `T4016b2` 已完成：
    - `sysroot/core.scoop` 已切到显式 `Continuation<Resume, Answer, eff E = Pure>`，并把 task step continuation 的 answer type 明确写成 `__TaskStepResult`；
    - escape continuation binder 的静态类型现已显式携带 delimiter answer type；当 answer type 在首轮进入 arm body 前尚未确定时，typecheck 会先放入内部 answer-hole，并在 handle 结果类型确定后回填/复验 binder；
    - type lowering / pretty-print / diagnostics / HIR 都已能消费 `Continuation<Resume, Answer, eff E>` surface；`Continuation.resume` 与 LLVM payload 读取路径也已切到从新 surface 的第一个普通类型参数读取 payload；
    - 为避免一次性迁移大量旧 fixture/source，前端暂时保留 `Continuation<Resume, eff E>` / `Continuation<Resume>` 的过渡 lowering 兼容；内部 answer-hole 只服务旧注解兼容，不改变推导出的 binder 类型与新显式 surface；
    - 已补充并验证 answer type mismatch、escape binder answer/effect 推导、显式 continuation answer 注解、HIR 参数类型显示，以及相关 shorthand 兼容 run-pass。
    - 已验证 `cargo run -p scoop -- test --fixtures tests/fixtures/typecheck`、`cargo run -p scoop -- test --fixtures tests/fixtures/hir`、选定 continuation `run-pass` 用例、`cargo test --all` 与 `cargo clippy --all-targets -- -D warnings` 通过。
  - `T4016c` 已完成：
    - runtime 新增共享 helper `scoop_continuation_resume_into(...)`：负责 one-shot 检查、执行 resume，并在 resumed computation 正常完成 delimiter 时把 answer transport 通过显式 ABI 写回 caller；
    - LLVM `Continuation.resume` lowering 与 state-machine tail-resume fast path 已切到该 helper，避免继续依赖旧的 void-only resume ABI；
    - `runtime/c/scoop_task.c` 已改为通过共享 helper 取得 `__TaskStepResult`，不再直接回读 continuation heap frame 前缀；
    - 已同步 `sysroot/core.scoop` / `SCOOP_RUNTIME.md` 的过渡叙事，并补 runtime `continuation_one_shot` 回归、两个 LLVM IR 定向测试，以及 `task_step_manual_basic.scoop`、`continuation_resume_surface_named_tuple_and_unit_basic.scoop` 的端到端运行验证；
    - 已复验 `cargo test --all` 与 `cargo clippy --all-targets -- -D warnings` 通过。
  - `T4016b3` 已完成：
    - `Continuation.resume(...)` 的 typecheck 已改为返回 continuation 的 answer type，safe-call `receiver?.resume(...)` 也会相应返回 `Option<Answer>`；
    - escape continuation arm 的 tail-resume 过渡路径不再绕开 handle result / answer-hole 推导，answer type 会在 handle 结果确定后回填并重新校验 arm body；
    - LLVM lowering 已把 fresh-path / replay-path 的 `Continuation.resume(...): Answer` 都接到共享 answer-return helper，并在需要时解码 answer transport；
    - 已补 typecheck / run-pass 回归，覆盖 expression-position resume、safe-call，以及 resumed computation 再次 suspend 的 replay-path answer-return。
    - 已验证 `cargo test --all`、`cargo clippy --all-targets -- -D warnings`、定向 continuation 回归与新增 fixture 子集通过。
  - `T4016d` 已完成：
    - async HIR lowering 生成的私有 task-step continuation 已显式写成 `Continuation<Any, __TaskStepResult>`，不再把 answer type 藏回旧的一参 continuation 形状；
    - runtime 新增共享 helper `scoop_continuation_resume_with(...)`，把“写 payload + resume + 读 answer”收口为统一 continuation ABI；当时公开 surface 里的 `Task.poll()/step()` 与 expression-position `Continuation.resume(...)` 已共用这一入口，而不是各自窥视 continuation payload 字段；
    - `runtime/c/scoop_task.c` 已删除本地 `ScoopContinuation` payload 布局镜像，pending task 恢复路径改为完全走共享 helper；
    - LLVM `Continuation.resume(...)` fresh-path / replay-path / tail-resume fast path 已切到共享 helper，不再由 caller 直接 GEP 写 `resume_word` / `resume_gc_ref`；
    - `scoop_continuation_resume_u64` 已保留旧的“只驱动 resume、不要求 delimiter answer”兼容语义，避免 cross-thread resume helper 被错误收口进 answer-required 路径；
    - `sysroot/core.scoop`、`SCOOP_FULL_SPEC.md`、`SCOOP_RUNTIME.md` 与 `runtime/c/scoop_runtime.c` 已统一当时的收口叙事：`Task` 只是把私有 `__TaskStepResult` continuation answer 投影回当时公开的 `Poll<T>` thin wrapper；随后已由 `T4016T1` 把 public naming 收口为 `TaskStep<T>` + `step()`，后续 `T4016T1b~T4016T3` 继续把 function-type 边界与 task 实现落点一并收口到 `SCOOP_TASK.md` 新设计；
    - 已补 runtime 回归 `continuation_resume_with_returns_answer_transport_and_clears_outputs_on_failure` 与 `task_poll_resumes_pending_task_via_shared_continuation_helper`，并更新 LLVM IR 断言以锁定共享 helper 路径；
    - 已验证 `cargo run -p scoop -- build tests/fixtures/run-pass/task_step_manual_basic.scoop -o /tmp/task_step_manual_basic.out`、执行 `/tmp/task_step_manual_basic.out`、`cargo run -p scoop -- test --fixtures tests/fixtures/run-pass`（`fixtures: ok (375)`）、`cargo test --all` 与 `cargo clippy --all-targets -- -D warnings` 通过。
  - `T4016b4a` 已完成：
    - type lowering 现已移除 legacy `Continuation<Resume, eff E>` shorthand，并新增早期 removed/compatibility diagnostic：要求显式写成 `Continuation<Resume, Answer, eff E>`；
    - `continuation_escape_binder_resume_effect_row_runtime_basic.scoop`、`continuation_resume_from_escape_binder_requires_step_effect.scoop` 与 `non_pure_continuation_resume_classifies_as_call_suspend_site` 单测已迁到显式 answer type；
    - 同时先把一批显然 answer=`Unit` 的 resume payload fixtures 迁到 `Continuation<Payload, Unit>`，避免继续混用旧 shorthand 叙事；
    - 已验证 `cargo run -p scoop -- build tests/fixtures/run-pass/continuation_escape_binder_resume_effect_row_runtime_basic.scoop -o /tmp/cont-shorthand.out && /tmp/cont-shorthand.out`、`cargo run -p scoop -- test --fixtures tests/fixtures/typecheck`、`cargo test --all` 与 `cargo clippy --all-targets -- -D warnings` 通过。
  - `T4016b4b` 已完成：
    - type lowering 已移除 legacy pure `Continuation<Resume>` 自动补 answer-hole 的兼容路径，并新增 removed diagnostic，要求显式写成 `Continuation<Resume, Answer>`；
    - 构造器参数路径现已支持 expected-type placeholder 回填，`Cell(None())` 这类场景不再阻断 continuation fixture build；
    - 相关 run-pass / typecheck fixtures 与 LLVM 内嵌测试源码已大批迁移到显式 answer type；
    - 已重新盘点仓库内剩余 `Continuation<Resume>` 文本匹配；除文档、计划/TODO 记录与 removed-diagnostic fixture 外，不再有会进入生产/codegen 主线的 legacy pure shorthand；
    - 已验证 `cargo run -p scoop -- test --fixtures tests/fixtures/typecheck`、`cargo run -p scoop -- test --fixtures tests/fixtures/run-pass`（`fixtures: ok (375)`）、`cargo test --all` 与 `cargo clippy --all-targets -- -D warnings` 通过。
  - 本轮已完成此前前置出的 `T4016b4a0`：
    - runtime 新增 `scoop_gc_register_global_root(void *base, const ScoopTypeDescriptor *type_desc)`，并在 baseline / immix / minimal / hosted 四个 backend 中接入 permanent global roots 扫描、verify 与 moving update；
    - LLVM codegen 改为在 object/top-level immutable 的 once-init 函数内就地注册 `__scoop_object_prop__*`、`__scoop_top_level_val__*` 与 `__scoop_object_instance__*`，同时为 backing global 生成可递归描述 nested GC refs 的 type descriptor；
    - 在修复注册路径时，还顺手收口了 ordinary pointer-shaped locals keepalive 的两个编译器回归：frame-slot GEP 现会在当前 block 重新物化，dispatch loop / task body wrapper 的嵌套函数 codegen 也不再泄漏 `env`；
    - 已验证 `cargo test -p scoopc --lib`、`cargo test --all`、`cargo clippy --all-targets -- -D warnings`、`SCOOP_GC_MOVE=1 SCOOP_GC_VERIFY_ROOTS=1 /tmp/gc_module_global_roots_move_basic.out` 与 `SCOOP_GC_STRESS=1 /tmp/gc_continuation_multi_thread_concurrent_alloc_resume.out` 通过。
  - 本轮已完成 `T4016T1`：core public surface 已收口为 `TaskStep<T>` + `step()`，相关 spec/runtime/design/doc/fixtures/diagnostics 已同步。
  - 本轮已完成 `T4016T1a`：rich enum / struct layout 现可保留 function-type payload 的真实 `TypeId`；member value 为函数值时也会继续走统一 callable-value 主线。新增 `enum_function_payload_basic.scoop`、`task_state_function_payload_basic.scoop`、`struct_function_field_call_basic.scoop` 及对应 typecheck 回归已锁定 custom enum payload、`Task` 目标形状与字段函数值调用。
  - `T4016T1b` 已完成：cast 前端现已拒绝 function runtime cast；`Pure! -> Any` 的显式擦除保留，`Any -> pure function` 与 non-`Pure` function-type cast 均在 typecheck 阶段给出稳定诊断，避免再把未定义语义漏到 LLVM。
  - `T4016T1c` 已完成：opaque callable 现已按静态 function type 的 effect row 上界决定 may-suspend 编译；state-machine planner / suspend analysis / callable-value codegen 的 concrete-type 恢复已对齐到 member access、`Block`/`If`/`When`、higher-order 返回值与 object/struct/class field 路径，`wrapper.f()` 与 `choose(mode)()` 这类调用点不再漏掉 outward suspend。
  - 为完成全量验收，本轮同时修复了 fixture runner 在 `cargo run -p scoop -- test` 中通过 `current_exe()` 拿到 `.../scoop (deleted)` 路径而导致 run-pass 自调用失败的既有问题；runner 现会回退到去掉 `(deleted)` 后缀的真实路径，`cargo run -p scoop -- test` 已恢复通过。
  - 在准备进入 `T4016T2` 时，本轮通过最小 probe 复现出新的 blocking mismatch：ordinary Scoop 定义的 generic task-state object model 还不能稳定落到当前 LLVM / typecheck 主线，因此不能继续把 task 主体从 `runtime/c/scoop_task.c` 迁回 Scoop。
  - 已复现的 blocker 包括：
    - `class Task<T>(..., var state: __TaskState<T>)` 这类 generic rich enum / continuation 状态字段会在 LLVM 报 `unsupported_main_body: struct field type`；
    - `T?` / `Option<T>` 或 `Option<Nominal<T>>` 槽位会在 codegen 中漏出 `TypeKind::Param(T)`，分别触发 `unsupported_main_body: Option<T> inner type` / `class field type`；
    - `Any?` 槽位方案里，`as` / `as?` 会让 `Task.step()` 引入 `Raise<RuntimeError>`，而 `is` smart-cast + generic member access 当前又报 `unsupported_expr: member access（未 resolve）`；
    - generic state carrier 的普通 ctor 路径在类型参数只经包装状态对象暴露时，会落到 `no_matching_overload` / `class ctor call overload mismatch/ambiguous`。
  - 本轮继续实现和验证后确认，原始 `T4016T1d` 实际包含两层不同 blocker：
    - `T4016T1d1`：先把 concrete-instance generic task-state carrier/object model 的 LLVM / typecheck 主线打通，覆盖 `Option<TaskState<T>>`、`Continuation<Any, DriverStep<T>>`、generic class instantiation、`Any -> Box<Int>` smart-cast member access 等 concrete-instance 路径；
    - `T4016T1d2`：再收口 generic helper / method body 内的 monomorph/type-param leak，覆盖 `fun <T> drive(...)`、`if (x is Box<T>) x.value` 与 `carrier.lock.destroy()` 这类仍会把 `TypeKind::Param(T)` 泄漏进 codegen 的路径。
  - `T4016T1d1` 本轮已完成：
    - `hir/lower/util.rs` 已把 generic class instantiation 的类型替换升级为递归替换，并补齐带 `type_kinds` 的 layout type lowering / nominal interning；
    - `resolve/scopes.rs` 已允许 `Any` receiver 成员访问延后到 typecheck，`hir/lower/expr.rs` / `llvm/codegen/mod.rs` 也已改为保留并优先使用 smart-cast 后的具体类型；
    - 新增 `task_generic_state_object_model_basic.scoop` 与 `smart_cast_any_member_access_generic_class_basic.scoop` 两个 run-pass 回归，前者锁定 ordinary Scoop concrete-instance state carrier/object model，后者锁定 `Any` smart-cast generic class field access；
    - 已复验 `cargo run -p scoop -- test`、`cargo test --all` 与 `cargo clippy --all-targets -- -D warnings` 通过。
  - `T4016T1d2` 已完成，但继续推进 `T4016T2` 时又暴露出三个新的 blocker：
    - 限定 payload enum ctor / `when` pattern 仍不完整：`TaskStep.Ready(value)` 报 `unresolved_member`，`TaskStep.Ready(v)` 仍在 parser 阶段卡在 `.`；
    - `emit_minimal_main_ir(...)` / single-file LLVM 测试路径没有跟随 `scoop build` 一起纳入 `sysroot/task.scoop` 这类可编译 sysroot 源，导致 async/task helper 仍在最小 IR 路径上报 `UnsupportedMainBody { kind: "call callee type" }`；
    - 普通 Scoop `Task` 若直接持有 `Mutex`，当前 sync runtime 仍只有显式 `destroy()` 合同，没有能覆盖 task 生命周期的无泄漏 release path。
  - 因此 `T4016T2` 必须再次前插三个更窄的前置项：`T4016T1d3 -> T4016T1d4 -> T4016T1d5`。

### P1.5. 最小 core Task surface、Scoop 化与无锁 single-driver 收口（`T4016T1 -> T4016T1a -> T4016T1b -> T4016T1c -> T4016T1R -> T4016T1d1 -> T4016T1d2 -> T4016T1d3 -> T4016T1d4 -> T4016T1d5 -> T4016T2 -> T4016T3 -> T4016T4 -> T4016T5 -> T4016T6 -> T4016T7 -> T4016T8 -> T4016T9 -> T4016T4R`）

- `T4016d` / `T4016R` 已证明：`Task` 不再需要 task-private continuation hack，也不再需要第二套 answer model；`T4016T1~T4016T3` 又进一步完成了 public surface、ordinary Scoop 主体与 task-only ABI 删除。但这还没有把 core task 的 drive ownership 合同收口到最终形态。
- 基于 `SCOOP_TASK.md`，当前 task 主线只覆盖 phase 1-3，并在 `T4016T3` 之后新增一段“去掉 per-task lock、改用轻量 claim bit、收口为 single-driver/trap-on-contention”的后续任务：
  - `T4016T1` 已完成：
    - `sysroot/core.scoop` 已移除 `Poll<T>` / `Task.poll()`，公开 surface 只保留 `Task<T>`、`TaskStep<T>`、`Task.step()` 与 `Async.await`；
    - LLVM codegen / 诊断文案 / `SCOOP_FULL_SPEC.md` / `SCOOP_RUNTIME.md` / `SCOOP_TASK.md` / `ISSUES.md` / `STDLIB_COMPLETENESS.md` 已同步到 step-only 叙事；
    - run-pass 回归已重命名为 `task_step_manual_basic.scoop`，并新增 `task_poll_removed_is_error.scoop` / `task_poll_type_removed_is_error.scoop` 锁定移除后的诊断；
    - 已验证 `cargo run -p scoop_tools -- spec-fixtures check`、`cargo run -p scoop -- test --fixtures tests/fixtures/typecheck`、`cargo run -p scoop -- test --fixtures tests/fixtures/run-pass`、`cargo run -p scoop -- test`、`cargo test --all` 与 `cargo clippy --all-targets -- -D warnings`。
  - `T4016T1a` 已完成：
    - `hir/lower/util.rs` 已补齐 `TypeRef::Function` 的 layout `TypeId` / effect row 恢复，以及 generic struct/enum layout 收集里的函数字段 substitution，不再把函数字段降成“无布局类型”；
    - `typecheck/expr/call.rs` / `typecheck/expr/member.rs` / `llvm/codegen/mod.rs` 已把 member value 为函数值/`FunPtr` 的路径接回统一 callable-value 主线，struct/class 风格字段上的函数值调用不再在 build 阶段丢失 resolution 或 concrete type；
    - 新增 `enum_function_payload_basic.scoop`、`task_state_function_payload_basic.scoop`、`struct_function_field_call_basic.scoop` 与对应 typecheck fixtures，最小回归已覆盖 custom enum payload、`Created(val start: () -> __TaskStepResult)` 目标形状，以及 `receiver.f()` 字段函数值调用；
    - 为保证 `cargo run -p scoop -- test` 可持续复验，本轮同时修复了 fixture runner 对 `current_exe()` 返回 `.../scoop (deleted)` 的自调用失败。
    - 已验证 `cargo run -p scoop -- test --fixtures tests/fixtures/typecheck`、`cargo run -p scoop -- test --fixtures tests/fixtures/run-pass`、`cargo run -p scoop -- test`、`cargo test --all` 与 `cargo clippy --all-targets -- -D warnings`。
  - `T4016T1b` 已完成：
    - `typecheck/expr/infer.rs` 新增 function cast boundary gate，direct function `as/as?` 不再漏到 LLVM `type check target type` 报错；`Pure! -> Any` 的显式擦除保持可用。
    - 新增 `function_type_cast_not_supported` / `effectful_function_type_cast_not_supported` 诊断，明确函数类型 runtime cast 未定义，且 non-`Pure` effect row 不具备 runtime-checkable semantics。
    - `SCOOP_FULL_SPEC.md` 与 typecheck fixtures 已同步到同一叙事：函数子类型 / coercion 才是合法路径；若要跨 runtime nominal 边界，应使用 wrapper。
    - 已验证 `cargo run -p scoop -- test --fixtures tests/fixtures/typecheck`、`cargo run -p scoop -- test`、`cargo test --all`、`cargo run -p scoop_tools -- spec-fixtures check` 与 `cargo clippy --all-targets -- -D warnings`。
  - `T4016T1c` 已完成：
    - `typecheck/expr/call.rs` 已补齐 “callee 是普通表达式且其类型为 function/FunPtr” 的调用类型推导，higher-order 返回值直接调用不再被 `UnsupportedExpr { kind: "call" }` 拒绝；
    - `llvm/codegen/effect/state_machine_plan.rs` 与 `llvm/codegen/mod.rs` 已统一 opaque callable 的 concrete-type 恢复逻辑：`MemberAccess`、`Call`、`Block`、`If`、`When`、object property、struct/class field 上的函数值都能按静态 function type 的 effect row 决定 may-suspend；
    - 新增 `effect_indirect_perform_nonresuming_function_value_wrapper_member_direct*` 与 `effect_indirect_perform_nonresuming_function_value_higher_order_when_direct*` run-pass 回归，以及对应的 state-machine dump 单测，覆盖 wrapper member direct call、same-site pure/effectful actual values、`when` LUB 与 higher-order 返回值直调；
    - 已验证 `cargo test -p scoopc segment_dump_classifies_ -- --nocapture`、`cargo test -p scoopc unified_state_machine_transforms_all_segment_kinds_from_feature_matrix -- --nocapture`、`cargo run -p scoop -- test --fixtures tests/fixtures/typecheck`、`cargo run -p scoop -- test --fixtures tests/fixtures/run-pass`、`cargo run -p scoop -- test`、`cargo test --all`、`cargo run -p scoop_tools -- spec-fixtures check` 与 `cargo clippy --all-targets -- -D warnings`。
  - `T4016T1R` 已完成：
    - 复审 rich enum / callable-value / state-machine planner / LLVM 主线过程中，确认并修复了 boxed multi-field enum function payload 在 `val Variant(...) = expr` 解构调用路径上的既有缺口；
    - `enum_function_payload_boxed_multi_field_basic.scoop` 已把 boxed ctor、`when` 解构调用与 `val` 解构调用一并锁进 run-pass 回归；
    - 与 null-assert lowering 相关的 HIR golden 已同步到 “hidden `Raise.raise(...)` 为 `Nothing` 类型 + 零宽 span” 的新合同。
  - `T4016T1d1` 已完成：
    - generic class instantiation 的类型替换现已递归穿透 nominal args、`Option<T>`、tuple、function 与 effect row；generic state carrier 的字段/ctor 参数不再只在顶层替换 `Param("T")`；
    - layout type lowering / nominal interning 现可恢复只出现在字段类型里的带 type args nominal，`TaskState<Int>` / `DriverStep<Int>` / `Continuation<Any, DriverStep<Int>>` 这类布局键不再缺少 `TypeId`；
    - `Any` receiver 的成员访问现会延后到 typecheck，并由 HIR lowering / LLVM codegen 保留 smart-cast 后的具体 receiver 类型；`if (x is Box<Int>) x.value` 已回到统一主线；
    - 新增 `task_generic_state_object_model_basic.scoop` 与 `smart_cast_any_member_access_generic_class_basic.scoop` 两个 run-pass 回归，锁定 concrete-instance object model 与 smart-cast generic member access；
    - 已复验 `cargo run -p scoop -- test`、`cargo test --all` 与 `cargo clippy --all-targets -- -D warnings` 通过。
  - `T4016T1d2` 已完成：
    - 已确认真正根因不是单独的 `sync.destroy` 或 class-layout 特判缺口，而是 monomorphized generic fun/member/getter 重新做 HIR lowering 时没有复用 typecheck side table，导致依赖 smart-cast / late member resolution 的路径（例如 `if (x is Box<T>) x.value`）退回到 base generic class `Box`，再在 LLVM class field lookup 上看到残留的 `field_ty = T`。
    - `hir/lower/mod.rs` 的 `LoweringInputs` 现可携带 `typecheck_types`；`lower_fun_with_type_bindings`、`lower_member_fun_with_type_bindings` 与 `lower_value_property_getter_with_type_bindings` 在 compilation-unit lowering 主线中会复用原始 typecheck side table，并继续通过 active type-param bindings 把 `T` 替换为 concrete type。
    - `hir/lower/util.rs` 的 generic fun/member instantiation 现会把 `Some(typecheck_types)` 传入上述 lowering helper；`monomorph/lower.rs` 与 `cone/pre_specialize.rs` 则继续显式传 `None`，保持 dump / 预专门化入口的现状不变。
    - `llvm/codegen/mod.rs` 的 `sync.destroy` receiver 类型恢复已切到统一的 `resolve_expr_concrete_type(...)` 主线，`carrier.lock.destroy()` 这类 generic receiver 字段上的 concrete nominal 调用不再退回旧的 local-var-only 路径。
    - 新增 run-pass 回归 `task_generic_state_generic_helper_method_basic.scoop`，同一用例同时锁定 `fun <T> drive(...)`、generic method body 中的 `if (x is Box<T>) x.value` 与 `carrier.lock.destroy()`。
    - 已复验 `cargo fmt --check`、`cargo run -p scoop -- test --fixtures tests/fixtures/run-pass`（`fixtures: ok (387)`）、`cargo run -p scoop -- test`（`fixtures: ok (1157)`）、`cargo test --all` 与 `cargo clippy --all-targets -- -D warnings` 通过。
  - `T4016T1d3` 已完成：
    - `when` pattern AST / parser 已支持 `Enum.Variant(...)` 与 `Enum.Variant` 的限定写法，相关 parser 单测已覆盖；
    - resolver / typecheck / LLVM codegen 已把 qualified enum variant ctor 接回统一主线，不再只支持 unqualified ctor 或 unit variant 值；
    - generic enum 的 qualified pattern 前缀现按 FQN 匹配，不再因省略 type args 而误报 `type_arity_mismatch`；
    - `task_step_manual_basic.scoop` 已切到 `TaskStep.Pending` / `TaskStep.Ready(value)` 的 sysroot regression，另补了多文件 `typecheck_multi` cross-file 回归；
    - 已复验 `cargo run -p scoop -- test`（`fixtures: ok (1159)`）、`cargo test --all` 与 `cargo clippy --all-targets -- -D warnings` 通过。
  - `T4016T1d4` 已完成：
    - 新增 `crates/scoopc/src/llvm/frontend.rs`，把单文件 LLVM 路径的 support-source 装配、完整 frontend、typecheck side table 与 monomorph key 收集独立成专用 helper；`emit_minimal_main_ir(...)` / `build_minimal_main_module(...)` 不再直接依赖 `hir::lower_for_dump(...)`。
    - single-file 路径现在会像 `scoop build` 一样把 `stdlib/*.scoop` 与 `session.sysroot().compilable_source_paths` 一并纳入 resolve/typecheck/lowering/source-map；此前 `async_await_minimal_int_basic.scoop` 的 `state machine perform effect instance key` 与 `stdlib_string_basic.scoop` 的 `unresolved_member: scoop.core.String.substring` 已恢复为正常产出 LLVM IR。
    - 新增 LLVM 单测 `single_file_minimal_ir_supports_handled_async_await` 与 `single_file_minimal_ir_includes_compilable_sysroot_string_helpers`；同时把既有 `@CLayout` / `@Extern` IR 单测更新为符合 build 路径前端约束的合法输入，避免测试继续依赖旧 minimal path 的绕过行为。
    - 已复验 `cargo test -p scoopc --features llvm`、`cargo run -p scoop -- test`、`cargo test --all` 与 `cargo clippy --all-targets -- -D warnings` 通过。
  - `T4016T1d5` 已完成：
    - `runtime/c/scoop_sync.c` 现已把 `Mutex` / `CondVar` / `Once` 统一切到 `scoop_alloc_typed(...)` + `release_fn`；显式 `destroy()` 与 sweep cleanup 已收口为同一套内部 helper。
    - `Mutex` / `CondVar` 新增 `initialized` 防护，`Once` 新增初始化 flag，确保 create 失败与 sweep cleanup 路径都不会误销毁未完成初始化的底层平台资源。
    - `runtime/c/scoop_runtime_api.h` 已补 allowlist，`sysroot/sync.scoop` 注释也已同步到“显式 destroy + GC sweep cleanup”的统一合同。
    - 新增 run-pass 回归 `sync_gc_release_task_like_object_basic.scoop`，用 ordinary Scoop task-like object 直接锁定 sync 资源在丢弃 / GC / 显式 destroy 边界上的释放与 no-double-destroy 合同。
    - 已复验 `cargo fmt`、`cargo run -p scoop -- test --fixtures tests/fixtures/run-pass`（`fixtures: ok (388)`）、`cargo run -p scoop -- test`（`fixtures: ok (1160)`）、`cargo test --all` 与 `cargo clippy --all-targets -- -D warnings` 通过。
  - `T4016T2` 已完成：
    - `sysroot/task.scoop` 已承载 ordinary Scoop task driver/state/sync 主体，`Task.step()` / `__task_join()` / `__task_drive_*()` 与私有 `__TaskStepResult<T>` / `__TaskState<T>` 均已落到普通 Scoop 定义；
    - async lowering 与 single-file/full-build LLVM 路径统一落到 `scoop.core.__task_*` helper；ordinary path 的 LLVM 回归改为断言“不再直接依赖 legacy `scoop_task_*` runtime ABI”；
    - `SCOOP_RUNTIME.md` 已同步为当前 task stepping layer contract；runtime 仅保留 continuation / GC / thread / sync substrate，task-only ABI 明确留给 `T4016T3` 删除；
    - 为通过全量验收，本轮补齐了跨文件成员 mutability、monomorphized task driver 的 resume-slot rewrite，以及跨包 bare enum variant ctor 对 internal helper enum 的可见性过滤。
  - `T4016T3` 已完成：
    - 删除 `runtime/c/scoop_task.c`、`runtime/c/scoop_runtime_api.h` 对应 allowlist、`crates/scoop_runtime/build.rs` 里的 task-only 编译入口，以及 `crates/scoop_runtime/tests/task_spawn_join.rs` 这类直接依赖旧 ABI 的 runtime integration test。
    - 删除 `sysroot/core.scoop` 中 legacy `__scoop_task_*` 声明；`Task.step()` 与 `__task_*` helper 只保留 ordinary Scoop 定义，task transport 仅剩 `__task_transport_pack()` / `__task_transport_unpack()` intrinsic。
    - 删除 LLVM codegen 中 `scoop.core.step` / `__scoop_task_*` special-case，以及 `runtime_symbols.rs` / `runtime_abi.rs` 里的 `scoop_task_*` 符号声明；新增 LLVM 回归 `task_step_ir_uses_ordinary_scoop_definition_not_legacy_poll_abi`，补锁 `Task.step()` 也不会再走 `scoop_task_poll`。
    - `SCOOP_RUNTIME.md`、`SCOOP_TASK.md`、`ISSUES.md`、`STDLIB_COMPLETENESS.md`、`sysroot/core.scoop` 与 `sysroot/task.scoop` 已同步到最终合同：`Task` 只依赖 generic continuation、GC、thread 与 sync substrate，不再存在 task-only C ABI / LLVM intrinsic 分支。
    - 已复验 `cargo fmt`、`cargo test -p scoopc --features llvm`、`cargo run -p scoop -- test`（`fixtures: ok (1160)`）、`cargo run -p scoop_tools -- spec-fixtures check`、`cargo test --all` 与 `cargo clippy --all-targets -- -D warnings` 通过。
  - 在 `T4016T3` 之后，本轮又收敛出新的 core task 设计前提：
    - `Task` 不是 thread-safe shared object；结构化并发中的 task 保持树状层级，不支持 shared subtask / multiple parents。
    - `Pending` 不再承担 drive contention 语义；public `step()` 观察到 `Running` 或 claim 竞争一律视为 executor bug 并直接 trap。
    - 最终目标不是“纯文档约束版无锁 Task”，而是“轻量 claim bit 版”。
  - `T4016T4` 已完成：
    - `SCOOP_TASK.md` 已把 task design、step algorithm 与 synchronization design 全部改写到 single-driver / trap-on-contention 合同，并明确 shared subtask / multiple parents 仍不在 core 范围内。
    - `SCOOP_FULL_SPEC.md`、`SCOOP_RUNTIME.md`、`sysroot/core.scoop` 与 `sysroot/task.scoop` 已同步最小规格草案与实现注释：`Pending` 只表示真实 not-ready，cross-thread 只允许顺序 handoff，public `step()` 观察到 `Running` / concurrent / reentrant misuse 必须 trap。
    - 同时保留了“当前 per-task `Mutex` 只是 `T4016T3` checkpoint 细节”的说明，为后续 claim-bit 实装清障，而不再把旧的 contention-as-`Pending` 语义写成稳定 contract。
    - 已复验 `cargo run -p scoop_tools -- spec-fixtures check`、`cargo run -p scoop -- test`（`fixtures: ok (1160)`）、`cargo test --all` 与 `cargo clippy --all-targets -- -D warnings`。
  - 因此当前剩余顺序为：
    - `T4016T5`：补齐 object-field atomic intrinsic 的编译器 blocker，保证 claim bit 可以作为普通 `Task` 字段承载。
    - `T4016T6`：把 `Task` object model 从 per-task `Mutex` 改为 atomic claim field。
    - `T4016T7`：重写 `Task.step()` 为 claim-bit 驱动，并把 concurrent/reentrant `step()` 误用收口为 trap。
    - `T4016T8`：清理 compiler/runtime/substrate 中残留的 mutex / contention-is-pending 假设，并确认 cross-thread sequential handoff 合同。
    - `T4016T9`：全量同步设计文档、规范、sysroot 注释与实现说明。
    - `T4016T4R`：review 全链路，确认无锁 single-driver 合同、trap 语义与回归一致。
  - phase 4 executor / wake / reactor / public `spawn/join` 不属于本组任务；它们明确延期到后续 stdlib stage，不作为 `scoop.core` 设计前提，也不在本轮计划内扩张 core surface。
- 当前状态：`T4016T5 -> T4016T6 -> T4016T7 -> T4016T8 -> T4016T9 -> T4016T4R -> T4012b3 -> T4012c -> T4012R -> T4013 -> T4013R`。

### P2. annotation markers 与 `inline` 关键字清理

- annotation 保持 compile-time markers only，不进入复杂 nominal / runtime 语义。
- `T4012a` 已完成：
  - typecheck 已把 annotation declaration model 收口为 compile-time markers only，并显式拒绝 `annotation` modifier 的非法目标、annotation class 的 nominal modifier、type/effect params、`where`、supertypes 与 type body；
  - `SCOOP_FULL_SPEC.md`、`sysroot/core.scoop`、`ISSUES.md` 与 parser / AST 注释已同步到同一叙事：annotation 只承载编译期 marker payload，不再保留“未来要扩成复杂 nominal feature”的错误方向；
  - 已新增 8 个 annotation 定向 typecheck fixtures，并复验 `cargo run -p scoop -- test --fixtures tests/fixtures/typecheck`、`cargo run -p scoop -- test`、`cargo test --all`、`cargo run -p scoop_tools -- spec-fixtures check` 与 `cargo clippy --all-targets -- -D warnings` 通过。
- `T4012b` 已拆成三步：
  - `T4012b1` 已完成：
    - `BuiltinAnnotationKind` 已新增 `AllowIntrinsic`，并在 file-level annotations 中强制“仅 file/module target、且无参数”的最小合同；
    - `check_file_annotations` 已把 `@file:AllowIntrinsic` 收口为当前文件 intrinsic gate；用户源码中的 `@Intrinsic` 函数 / 类型声明若未开门，会给出稳定的 `intrinsic_user_decl_requires_allow_intrinsic` 诊断；
    - `stdlib/mutable_array.scoop` 与新增的 typecheck fixtures 已同步迁移到这一 gate 合同；
    - 已复验 `cargo run -p scoop -- test --fixtures tests/fixtures/typecheck`、`cargo run -p scoop -- test`、`cargo test --all`、`cargo run -p scoop_tools -- spec-fixtures check` 与 `cargo clippy --all-targets -- -D warnings` 通过。
  - `T4012b2` 已完成：
    - `sysroot/core.scoop` 已补齐 `Deprecated` 注解声明面，built-in `@Deprecated(message, replaceWith?)` 的 target/参数规则已由 annotation typecheck 收口。
    - `TypeEnv` 现可跨文件 / sysroot 收集类型、顶层属性/值与函数的 deprecation 元数据；type lowering、顶层值读取、函数调用与顶层函数值引用路径都能在 use-site 发出结构化 warning。
    - `scoop build/run` 已安装 warning capture，并以 `path:line:col: warn[deprecated]: ...` 的稳定格式输出到 stderr，供 run-pass fixtures 断言。
    - 已新增 typecheck / run-pass fixtures，覆盖非法 file target、第二个位置参数非法、参数类型不匹配，以及函数/类型/顶层属性 use-site warning。
    - 已验证 `cargo run -p scoop -- test --fixtures tests/fixtures/typecheck`、`cargo run -p scoop -- test --fixtures tests/fixtures/run-pass`（`fixtures: ok (378)`）、`cargo test --all` 与 `cargo clippy --all-targets -- -D warnings` 通过。
  - `T4012b3`：最后为 `@Suppress` 建立 warning-code 与 suppression surface。由于 spec 还举了 expression annotation 例子，这一步需要连同表达式注解语义一起收口，不能只做声明头占位。
- 在 `T4012b*` 收口后，再补入 `@Experimental(feature = "...")` 这一保留的 built-in feature-gate marker；具体 feature gating wiring 后续再做。
- 再删除 `inline` 关键字与 legacy non-local return 语义残留；若未来仍需内联提示，统一由 `@Inline` 作为纯优化 marker 承担。
- 当前状态：依赖 `T4016T4R`；回到本组后的顺序为 `T4012b3 -> T4012c -> T4012R -> T4013 -> T4013R`。

### P3. FFI / ABI 边界收口

- 聚焦普通 `@Extern` 的 effect-impermeable 边界，以及 stable handle / `Pinned` 的职责分离：stable handle 负责 long-lived identity / wake token，`Pinned` 只负责短时裸地址借出。
- 当前状态：`T4014a -> T4014b -> T4014R` 待开始；依赖 `T4013R`。

### P4. const / comptime 扩展

- 在保持纯计算模型前提下，扩展 const/comptime 的解析、控制流与 effect-row 合同，避免继续停留在“同文件 + 名字/参数个数 + 字面量求值”的最小子集。
- 当前状态：`T4015a -> T4015b -> T4015c -> T4015R` 待开始；依赖 `T4014R`。

## 3. 各阶段完成标准

### C1. delimited continuation / `Task`

- `Continuation` 的静态模型必须显式承载 answer type，或给出等价但同样显式的语言级表示；不得继续把 answer type 藏在 task-private runtime 旁路中。
- `k.resume(...)` 最终必须成为真正返回表达式值的 primitive，而不是仅“触发 resumed step 后返回 `Unit`”的 builtin call。
- 语言层面只允许 `Effect.op(args) -> expr` 与 `Effect.op(args), k -> expr` 两种 arm；`-> resume` 必须作为已移除语法报错，而不是继续作为隐藏 special form 存活。
- core task public surface 必须收口为 `TaskStep<T>` + `step()`；`Poll<T>` / `poll()` 不再保留在生产 surface 中，也不引入 alias / compatibility 层。
- fixtures / tests 需覆盖：
  - `-> resume` removed-syntax diagnostics，以及迁移到 `, k ->` + `k.resume(...)` 后的等价行为；
  - arm 内 `k.resume(...)` 之后继续执行本地代码；
  - nested handle / `finally` / early return；
  - resumed computation 再次 suspend 时捕获 fresh continuation；
  - `Task.step()` 在新语义下的公开 drive 合同，以及跨线程 drive/resume 的最小同步语义。
- `Task` 必须不再依赖“调用 `scoop_continuation_resume(...)` 后再偷读 heap frame 前缀”的 runtime hack；最终 task 主体实现应主要驻留在 Scoop，而不是 `runtime/c/scoop_task.c`。
- `SCOOP_FULL_SPEC.md`、`SCOOP_RUNTIME.md`、`SCOOP_TASK.md`、`sysroot/core.scoop` 与实现注释必须对 core task public surface、内部 driver model 与 runtime substrate 保持一致。

### C2. annotation / `inline` / FFI / const/comptime

- 对应 `ISSUES.md` 条目已关闭，或至少收缩为新的、更窄的剩余 blocker。
- 新增或更新的 fixtures 覆盖 typecheck、HIR / MIR / LLVM lowering、run-pass 或相关 regression。
- `@Experimental(feature = "...")` 若按计划加入，必须先作为 built-in compile-time marker 被编译器识别并校验参数形状；具体语言特性接线可以继续 deferred。
- `inline` 关键字若按计划删除，parser / typecheck / spec / sysroot 叙事必须同步切到 `@Inline`；`@Inline` 不能继续携带任何控制流语义。
- 若规范文字被实现改变或澄清，需同步 `SCOOP_FULL_SPEC.md`，必要时同步 runtime / sysroot 文档。

## 4. 非目标

- 本轮不实现 multi-shot continuation，不定义 continuation cloning / replay 的语言级合同。
- 本轮不引入 undelimited continuation / `call/cc` 风格控制操作。
- 本轮不完成 executor framework，不定义 wake queue、event loop、I/O driver、work-stealing 或 public `spawn` 调度语义；这部分 phase 4 明确顺延到 stdlib stage。
- 本轮不为了支持 continuation 而把 Scoop 改造成“整体不可变、禁止写回”的另一种语言模型。
- 本轮不把 annotation 扩展成复杂 nominal / runtime feature。
- 本轮不扩展与 `TODO.md` 当前条目无直接关系的 stdlib / runtime surface。

## 5. 最终验收

- `PLAN.md` 与 `TODO.md` 中本轮任务已按顺序推进并留下明确结论。
- `Continuation` / `Task` / `async` / effect 文档叙事一致：spec、runtime 文档、sysroot surface 与实现不再对 continuation answer/result model 各说各话。
- 相关实现通过必要的定向测试；阶段收口时复验 `cargo test --all` 与 `cargo run -p scoop -- test`。
- 若修改了 `SCOOP_FULL_SPEC.md` 中带 fixture 的代码块，还需执行 `cargo run -p scoop_tools -- spec-fixtures check`。
