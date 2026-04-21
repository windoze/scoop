# Scoop：下一轮计划（正确的单次 delimited continuation 优先）

> 生成时间：2026-04-21  
> 历史归档：`PLAN-5.md` / `TODO-5.md`  
> 本轮主题：先修复全量回归暴露的 `@Extern` + moving-GC native-roots 既有问题，再把 `Continuation` 从当前“为 effect / async lowering 服务的 step-driving advanced API”收口为**正确的单次（one-shot）delimited continuation**，然后继续按 `SCOOP_TASK.md` 收口 core `Task` surface、最小化 task-only runtime/codegen、把 task 主体迁回 Scoop（只覆盖 phase 1-3；phase 4 executor / wake / reactor 明确延期到 stdlib）；annotation、删除 `inline` 关键字、FFI / ABI、const / comptime 顺延。  
> 设计前提：**不支持 multi-shot continuation**。Scoop 保持当前可变局部、writeback、once-init 与 GC-managed frame 的整体运行时方向，不为 continuation cloning / replay 另开一套“immutable everything”语义世界。

## 0. 工作原则

- 本轮严格按 `TODO.md` 中的顺序推进，不跨条目并行实现。
- `Continuation` 的目标语义是**单次、deep、以最近 `handle` 为 delimiter** 的 delimited continuation。
- 语言层面只保留 `Effect.op(args) -> expr` 与 `Effect.op(args), k -> expr` 两种 handler arm；`-> resume` 从用户态语法移除。若需要 immediate-resume fast path，只能作为 lowering / codegen 内部优化分类。
- `k.resume(payload)` 在 resumed computation 正常完成 delimiter 时，应返回该 delimiter 的 answer type；后续本地代码可继续执行。
- repeated resume 继续是 one-shot 违规；multi-shot、continuation cloning、resume-many replay 都不纳入本轮范围。
- `Task<T>` 仍是 general-purpose async API；raw `Continuation` 仍是 advanced API。区别在于本轮结束后，`Task` 不得再依赖“resume 后偷读 frame 前缀结果”的 runtime hack。
- 基于 `SCOOP_TASK.md`，core task 设计仍在进行中，不保留 `Poll<T>` / `poll()` 等命名的向后兼容包袱；若公开 surface 需要改名，应直接收口到最终形态。
- annotation 的方向改为**compile-time markers only**：不把 annotation 做成复杂 nominal runtime/type-system feature。
- `inline` 关键字默认从语言 surface 移除；若仍需要内联提示，由 `@Inline` 统一承担，且它只是一种 compile-time marker / 优化提示，不附带控制流语义。
- executor framework、wakeup queue、work-stealing、public `spawn/join` 调度语义继续 deferred，且明确顺延到 stdlib stage；它们不能成为本轮 core task 设计前提。
- 若实现改变公开语义，必须同步 `SCOOP_FULL_SPEC.md`；若涉及运行时合同，还要同步 `SCOOP_RUNTIME.md`、`sysroot/core.scoop` 与必要注释。

## 1. 顺序总览

1. 前置 blockers 与 continuation / `Task` review 已收口：`T1510c1`、`T1510c2`、`T4016R` 与 `T4016T1` 均已完成；基于 `SCOOP_TASK.md`，下一步继续执行 `T4016T2 -> T4016T3`，把 task 主体 Scoop 化并删除 task-only runtime/codegen surface
2. `ISSUES.md` 第 9 条：annotation markers、non-inline built-in annotations 与 `@Experimental` feature-gate marker（当前剩余顺序：`T4012b3 -> T4012c -> T4012R`）
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
  - `T4016d` / `T4016R` 收口的是 continuation answer model 与 task-hack 移除；这并不意味着 core task public naming、runtime/codegen surface 与实现落点已经最终定稿。当前已完成 `T4016T1` 的 public surface 收口，后续继续按 `SCOOP_TASK.md` 执行 `T4016T2 -> T4016T3`。
  - 当前顺序调整为：`T4016T2 -> T4016T3 -> T4012 -> T4013 -> T4014 -> T4015`。
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
    - `sysroot/core.scoop`、`SCOOP_FULL_SPEC.md`、`SCOOP_RUNTIME.md` 与 `runtime/c/scoop_runtime.c` 已统一当时的收口叙事：`Task` 只是把私有 `__TaskStepResult` continuation answer 投影回当时公开的 `Poll<T>` thin wrapper；随后已由 `T4016T1` 把 public naming 收口为 `TaskStep<T>` + `step()`，后续 `T4016T2~T4016T3` 继续把实现落点收口到 `SCOOP_TASK.md` 新设计；
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
  - 下一步按 `SCOOP_TASK.md` 继续 `T4016T2 -> T4016T3`：先把 task 主体迁回 Scoop，再删除 task-only runtime/codegen ABI；executor / wake / reactor / public `spawn/join` 的 phase 4 明确延期到 stdlib stage。

### P1.5. 最小 core Task surface 与 Scoop 化（`T4016T1 -> T4016T2 -> T4016T3`）

- `T4016d` / `T4016R` 已证明：`Task` 不再需要 task-private continuation hack，也不再需要第二套 answer model；但这只解决了 continuation 语义与 runtime hack 债务，还没有把 core task public surface、实现落点与 runtime/codegen surface 收口到最小形态。
- 基于 `SCOOP_TASK.md`，当前新增三步，只覆盖 phase 1-3：
  - `T4016T1` 已完成：
    - `sysroot/core.scoop` 已移除 `Poll<T>` / `Task.poll()`，公开 surface 只保留 `Task<T>`、`TaskStep<T>`、`Task.step()` 与 `Async.await`；
    - LLVM codegen / 诊断文案 / `SCOOP_FULL_SPEC.md` / `SCOOP_RUNTIME.md` / `SCOOP_TASK.md` / `ISSUES.md` / `STDLIB_COMPLETENESS.md` 已同步到 step-only 叙事；
    - run-pass 回归已重命名为 `task_step_manual_basic.scoop`，并新增 `task_poll_removed_is_error.scoop` / `task_poll_type_removed_is_error.scoop` 锁定移除后的诊断；
    - 已验证 `cargo run -p scoop_tools -- spec-fixtures check`、`cargo run -p scoop -- test --fixtures tests/fixtures/typecheck`、`cargo run -p scoop -- test --fixtures tests/fixtures/run-pass`、`cargo run -p scoop -- test`、`cargo test --all` 与 `cargo clippy --all-targets -- -D warnings`。
  - `T4016T2`：把 task 内部 driver / state / `step()` 主体迁回 Scoop，把 `async` / `await` lowering 改写到 ordinary Scoop helper target，并明确跨线程 drive/resume 的最小同步合同；语言 spec、runtime spec 与设计文档要同步改写；
  - `T4016T3`：删除 `scoop_task_*` task-only runtime / codegen ABI 与 `runtime/c/scoop_task.c`，让剩余底座只保留 generic continuation、GC、thread 与 sync runtime；`SCOOP_RUNTIME.md` 需同步移除 task-only ABI 叙事。
- phase 4 executor / wake / reactor / public `spawn/join` 不属于本组任务；它们明确延期到后续 stdlib stage，不作为 `scoop.core` 设计前提，也不在本轮计划内扩张 core surface。
- 当前状态：`T4016T2 -> T4016T3 -> T4012b3 -> T4012c -> T4012R -> T4013 -> T4013R`。

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
- 当前状态：`T4012b3 -> T4012c -> T4012R -> T4013 -> T4013R`。

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
