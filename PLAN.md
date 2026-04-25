# Scoop：下一轮计划（正确的单次 delimited continuation 优先）

> 生成时间：2026-04-21  
> 历史归档：`PLAN-5.md` / `TODO-5.md`  
> 本轮主题：先修复全量回归暴露的 `@Extern` + moving-GC native-roots 既有问题，再把 `Continuation` 从当前“为 effect / async lowering 服务的 step-driving advanced API”收口为**正确的单次（one-shot）delimited continuation**，然后继续按 `SCOOP_TASK.md` 收口 core `Task` surface、最小化 task-only runtime/codegen、把 task 主体迁回 Scoop，并进一步收口去除 `Task` 内建 lock 的轻量 claim single-driver 合同（只覆盖 phase 1-3；phase 4 executor / wake / reactor 明确延期到 stdlib）；随后按 `CONTINUATION.md` 把 effect / continuation 运行时从 TLS side channel 收口到显式 `EffectCtx` / `EffectOutcome` 设计，最后再回到 annotation、删除 `inline` 关键字、FFI / ABI、const / comptime。  
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

1. 前置 blockers、continuation review、core `Task` 无锁 single-driver review 与 `T4017` 显式上下文化收口均已完成；`T1510c1`、`T1510c2`、`T4016R`、`T4016T1`、`T4016T1a`、`T4016T1b`、`T4016T1c`、`T4016T1R`、`T4016T1d1`、`T4016T1d2`、`T4016T1d3`、`T4016T1d4`、`T4016T1d5`、`T4016T2`、`T4016T3`、`T4016T4`、`T4016T5`、`T4016T5a`、`T4016T6`、`T4016T7`、`T4016T7a`、`T4016T8`、`T4016T9`、`T4016T4R`、`T4017a`、`T4017b`、`T4017c`、`T4017d`、`T4017e1`、`T4017e2`、`T4017e3`、`T4017f`、`T4017R`、`T4012b3`、`T4012c`、`T4012R`、`T4013`、`T4013R`、`T4014a`、`T4014b`、`T4014R`、`T4015a1`、`T4015a2`、`T4015b`、`T4015c` 与 `T1220b` 均已完成；package-level `comptime if` 条件现已接入 compilation-unit pre-trim 的 typechecked 调用绑定主线，因此下一步回到 `T4015R`。
2. `CONTINUATION.md` 已收口为显式 `EffectCtx` / `EffectOutcome` 的实施基线，且 `T4017R` 已确认 ordinary boundary、continuation resume 与文档叙事均不再把 ambient effect TLS 当成 source of truth。
3. `ISSUES.md` 第 9 条：`@Inline` 交叉项已随 `T4013` 收口，不再构成 annotation blocker
4. `ISSUES.md` 第 10 条：legacy `inline` 关键字与 non-local return 语义残留已由 `T4013R` review 确认关闭。
5. `ISSUES.md` 第 11 条：ordinary `@Extern` 的 effect-impermeable 边界与 stable handle / `Pinned` 职责分离已由 `T4014R` 复审确认收口。
6. `ISSUES.md` 第 12 条：const / comptime 的声明级 Pure/Pure! 合同与 package-level `comptime if` 条件的调用绑定现已一并收口；剩余顺序为 `T4015R`，下一步执行 `T4015R`。

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
  - `T4016d` / `T4016R` 收口的是 continuation answer model 与 task-hack 移除；`T4016T1~T4016T7a` 现已进一步完成 public surface、ordinary Scoop task 主体、task-only ABI 删除，以及无锁 claim-bit + trap-on-contention 的执行主线。
  - `T4016T8` probing 暴露的 earlier blocker（ordinary/statepoint call 会把含 GC refs 的 by-value aggregate 实参以 stale SSA 形式直接传入 callee）已在 `T4016T7a` 收口：
    - ordinary/internal 调用里含 GC refs 的 aggregate 实参与 aggregate 返回值现统一走 hidden by-ref / hidden sret ABI，`__TaskStepResult` / `TaskStep` / effect transport 不再裸穿 stale aggregate SSA；
    - `gc-leaf-function` 误判、object/global init helper 缺失 GC strategy、operator overload 未复用统一 ordinary-call lowering 等边界也一并对齐；
    - runtime 补齐了 `InNative` caller-frame roots、`yield()` safepoint 与 collect 入口遇到已发起 STW 时的让出逻辑；全量验证中还补上 `gc_immix_compaction` test binary 的进程内串行化锁，避免共享全局 runtime 时的 STW 死锁。
  - `T4016T7a` 已通过 `cargo run -p scoop -- test`（`fixtures: ok (1168)`）、`cargo test --all` 与 `cargo clippy --all-targets -- -D warnings` 验收。
  - `T4016T1R` 期间又收口了一个必须优先修的既有缺口：boxed multi-field enum variant 经 `val Variant(...) = expr` 解构后，若 payload 含 function type 且后续直接调用，隐藏 `Raise.raise(...)` 会被 ordinary callee suspend plan 误建模成 `Ref` 型 resume slot。现已通过：
    - 为 variant pattern 的隐藏 binder 恢复真实字段类型；
    - 将 `synth_raise_null_assertion_failed()` 的隐藏 `Perform` 收口为 `Nothing` 类型，并避免与外层合成 `when` 共用完全相同的 span；
    - 新增 boxed multi-field enum function payload run-pass 回归，并同步相关 HIR golden。
  - 当前顺序调整为：`T4016T4R -> T4017 -> T4012 -> T4013 -> T4014 -> T4015`。
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

### P1.5. 最小 core Task surface、Scoop 化与无锁 single-driver 收口（`T4016T1 -> T4016T1a -> T4016T1b -> T4016T1c -> T4016T1R -> T4016T1d1 -> T4016T1d2 -> T4016T1d3 -> T4016T1d4 -> T4016T1d5 -> T4016T2 -> T4016T3 -> T4016T4 -> T4016T5 -> T4016T5a -> T4016T6 -> T4016T7 -> T4016T7a -> T4016T8 -> T4016T9 -> T4016T4R`）

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
  - `T4016T5` 已完成：
    - `crates/scoopc/src/llvm/codegen/mod.rs` 现已把 atomic 目标求址升级为可递归恢复真实槽位地址的 `AddressablePlace` 主线；`__atomicInt*` 不再只支持局部变量 / 顶层 var，ordinary class field、nested class field 与由 addressable class field 派生出的 nested struct field 都能直接在真实字段槽位上发出原子指令。
    - 在继续 probing object-field atomics 时暴露出的更基础 layout/type 恢复缺口也已同步修复：`crates/scoopc/src/hir/lower/util.rs` 现会把 `scoop.unsafe.__AtomicInt` / `scoop.core.UIntPtr` 这类 layout alias 映射回稳定的 builtin `TypeId`；`crates/scoopc/src/llvm/codegen/ty.rs` 补上了 `__AtomicInt` 的 fallback lowering 与 GC-free 分类。
    - 新增 `tests/fixtures/run-pass/unsafe_atomic_int_field_lvalue_basic.scoop` 与 `tests/fixtures/build/unsafe_atomic_int_field_lvalue_llvm.scoop`，分别锁定语义行为与 LLVM 必须直接在字段 GEP 上发出 `load atomic` / `store atomic` / `cmpxchg` 的合同。
    - 已复验 `cargo run -p scoop -- test --fixtures tests/fixtures/build`（`fixtures: ok (16)`）、`cargo run -p scoop -- test --fixtures tests/fixtures/run-pass`（`fixtures: ok (389)`）、`cargo run -p scoop -- test`（`fixtures: ok (1162)`）、`cargo test --all` 与 `cargo clippy --all-targets -- -D warnings`。
  - `T4016T5a` 已完成：
    - `codegen_class_ctor_invoke_inner(...)` 与 ctor-parameter-property 写回路径不再对“已求值/已类型对齐”的 ctor args 重新走 source-backed literal 反查；相关落槽逻辑现已收口到 `store_local_value_exact(...)`。
    - `SourceMap::slice` / `offset_to_line_col` 现会显式拒绝非 UTF-8 字符边界的 span/offset，避免同类 source mismatch 直接 panic。
    - 新增 source 单测与 LLVM 单测 `cross_file_class_ctor_literal_codegen_uses_correct_source_with_utf8_comments`，锁定“跨文件 class ctor + 整数字面量参数 + 中文注释”回归；并已复验 `cargo test -p scoopc --features llvm`、`cargo run -p scoop -- test`、`cargo test --all` 与 `cargo clippy --all-targets -- -D warnings` 通过。
  - `T4016T6` 已完成：
    - `sysroot/core.scoop` 中 `Task<T>` 的内部布局已从 `__lock: scoop.sync.Mutex` 切到 `__claim: scoop.unsafe.__AtomicInt`；task 注释也同步更新为“atomic claim 字段 + 私有 `__TaskState<T>`”的过渡实现说明。
    - `sysroot/task.scoop` 已删除 `mutexCreate()` / `lock()` / `unlock()` 路径；新增 `__task_claim_acquire()` / `__task_claim_release()`，通过 `__atomicIntCompareExchange` / `__atomicIntStore` 承担原先的短临界区串行化。
    - `__task_create()` / `__task_from_result()` 不再为每个 task 分配 sync 对象；`Task.step()`、`__task_apply_step()` 与 `__task_restore_waiting()` 已切到 atomic claim helper，同时刻意保留当前 `Running -> Pending` 的过渡语义，把 trap-on-contention 留给 `T4016T7`。
    - 新增 `tests/fixtures/build/task_atomic_claim_no_mutex_llvm.scoop`，锁定 task manual-drive 主线会发出 atomic `cmpxchg` / `store atomic`，且不再出现 `scoop_sync_mutex_{create,lock,unlock,destroy}` 调用。
    - 由于 sysroot 类型表新增/重排导致 MIR `TypeId` 稳定编号前移，已同步更新 `tests/fixtures/mir/closure_capture_val.mir` 与 `tests/fixtures/mir/closure_capture_var.mir` 两份 golden；并已复验 `cargo run -p scoop -- test --fixtures tests/fixtures/build`（`fixtures: ok (17)`）、`cargo run -p scoop -- test --fixtures tests/fixtures/run-pass`（`fixtures: ok (389)`）、`cargo run -p scoop -- test`（`fixtures: ok (1163)`）、`cargo test --all` 与 `cargo clippy --all-targets -- -D warnings`。
  - `T4016T7` 已完成：
    - `sysroot/task.scoop` 的 claim 入口已从“失败后 `yield()` 重试”收口为单次 `cmpxchg` try-claim；claim 失败直接 `exit(3)`，不再把竞争编码成阻塞式重试或 `Pending`。
    - `Task.step()` 中 `Running -> Pending()` 的过渡行为已删除；成功 claim 后若观察到 `Running`，现在稳定按 single-driver misuse trap 处理。
    - `sysroot/core.scoop` / `sysroot/task.scoop` 注释已同步到当前合同：claim 字段表达最小独占 drive ownership，claim 竞争与 reentrant drive 都直接 trap。
    - `tests/fixtures/build/task_atomic_claim_no_mutex_llvm.scoop` 已继续补锁 trap 路径：manual-drive LLVM IR 现在既保留 atomic claim 指令，也包含 `scoop_process_exit`，且不再出现 claim 竞争自旋的 `scoop_thread_yield`。
    - 新增 run-pass 回归 `task_step_cross_thread_sequential_handoff_basic.scoop`、`task_step_reentrant_trap.scoop` 与 `task_step_concurrent_running_trap.scoop`，分别锁定顺序跨线程 handoff、同线程重入 trap 与并发竞争 trap。
    - 已复验三条新回归的单独 `build + run`，以及 `cargo run -p scoop -- test --fixtures tests/fixtures/build`（`fixtures: ok (17)`）、`cargo run -p scoop -- test --fixtures tests/fixtures/run-pass`（`fixtures: ok (392)`）、`cargo run -p scoop -- test`（`fixtures: ok (1166)`）、`cargo test --all` 与 `cargo clippy --all-targets -- -D warnings`。
  - `T4016T8` 已完成：
    - `crates/scoopc/src/llvm/codegen/mod.rs` 现把 `threadSpawn` / `Thread.join` / `sleepMillis` / `yield` 统一收口到 `build_call_preserving_gc_local_roots(...)`，修复了 blocking/safepoint 线程调用在 moving GC 下遗漏 caller-frame local roots writeback 的 compiler 缺口。
    - 新增 runtime GC 回归 `tests/fixtures/runtime_gc/task_step_cross_thread_sequential_handoff_gc_stress.scoop`，覆盖 spawn pin、cross-thread sequential handoff、worker `join()` 与随后主线程再次 collect/step 的整条路径；moving GC + stress + verify-roots 模式现都稳定通过。
    - 新增 LLVM 单测 `thread_join_statepoint_preserves_live_gc_locals`，直接锁定 `@scoop_thread_join` 的 statepoint `gc-live` roots 里包含 `inner / outer / worker` keepalive，且返回后会把 relocated roots 写回真实局部槽位。
    - 在全量验收中还收口了一个既有 fixtures runner 缺口：`crates/scoop/src/fixtures/mod.rs` 现在支持把 `tests/fixtures/run_pass_cone` 根目录或单个 `tests/fixtures/run_pass_cone/<case>` 目录直接作为 `--fixtures` 输入，不再误把 cone case 名识别为未实现 phase。
    - 已复验 `cargo run -p scoop -- test --fixtures tests/fixtures/runtime_gc`（`fixtures: ok (24)`）、`cargo run -p scoop -- test --fixtures tests/fixtures/run_pass_cone`（`fixtures: ok (19)`）、`cargo run -p scoop -- test`（`fixtures: ok (1169)`）、`cargo test --all` 与 `cargo clippy --all-targets -- -D warnings` 全部通过。
  - `T4016T9` 已完成：
    - `SCOOP_TASK.md`、`SCOOP_FULL_SPEC.md`、`SCOOP_RUNTIME.md`、`ISSUES.md`、`STDLIB_COMPLETENESS.md`、`sysroot/core.scoop`、`sysroot/task.scoop` 与 `sysroot/unsafe.scoop` 已统一切到 “ordinary Scoop state + atomic claim-bit + single-driver + sequential handoff + misuse trap” 叙事，不再保留 per-task mutex / shared drive / contention-as-`Pending` 的旧说明。
    - `crates/scoopc/src/llvm/codegen/mod.rs` 注释现已明确：task-aware lowering 仅剩 erased payload transport；`runtime/c/scoop_thread.c` 注释也已同步到“cross-thread task handoff 仅依赖通用 thread substrate、无 task-specific scheduler ABI” 的实现说明。
    - 在复扫文档时还顺手清理了一处既有过期表述：`SCOOP_RUNTIME.md` 第 10 节不再把 continuation/runtime 合同写成“正在向 T4016 收口”的进行时。
    - 已复验 `cargo run -p scoop_tools -- spec-fixtures check`、`cargo test --all`、`cargo run -p scoop -- test`（`fixtures: ok (1169)`）与 `cargo clippy --all-targets -- -D warnings` 全部通过。
  - `T4016T4R` 已完成：
    - 复扫 `sysroot/task.scoop` / `sysroot/core.scoop` 后，已再次确认 `Task<T>` 只保留私有 `__claim: scoop.unsafe.__AtomicInt` 与 `__state`，不再内建 per-task `Mutex`；claim 失败和成功 claim 后观察到 `Running` 都直接 trap，`Pending` 不再承担 contention 语义。
    - 复扫 `crates/scoopc/src/llvm/codegen/mod.rs` 后，已确认 task-aware lowering 只剩 erased payload transport；对象字段 atomic intrinsic 统一经 `scoop.unsafe.__atomicInt*` 与 ordinary addressable-place 主线 lowering，不存在 task-only atomic special-case。
    - 复扫 `SCOOP_TASK.md`、`SCOOP_FULL_SPEC.md`、`SCOOP_RUNTIME.md`、`ISSUES.md`、`STDLIB_COMPLETENESS.md` 与 `sysroot/*` 注释后，已确认“single-driver + sequential cross-thread handoff + misuse trap” 叙事与实现一致；shared subtask / multi-parent / contention-as-`Pending` 旧模型未再残留于生产主线。
    - 已复验 `cargo test -p scoopc --features llvm task_step_ir_uses_seqcst_atomic_claim_and_trap_without_mutex`、`cargo run -p scoop -- test --fixtures tests/fixtures/build`（`fixtures: ok (17)`）、`cargo run -p scoop -- test --fixtures tests/fixtures/runtime_gc`（`fixtures: ok (24)`）、`cargo run -p scoop -- test --fixtures tests/fixtures/run-pass`（`fixtures: ok (392)`）、`cargo run -p scoop -- test`（`fixtures: ok (1169)`）、`cargo run -p scoop_tools -- spec-fixtures check`、`cargo test --all` 与 `cargo clippy --all-targets -- -D warnings`，均未暴露新的前置 blocker。
  - 因此当前剩余顺序为：
    - `T4017a`：先更新 `CONTINUATION.md`、spec 与 runtime 设计文档，收口显式 `EffectCtx` / `EffectOutcome` 叙事。
  - phase 4 executor / wake / reactor / public `spawn/join` 不属于本组任务；它们明确延期到后续 stdlib stage，不作为 `scoop.core` 设计前提，也不在本轮计划内扩张 core surface。
- 当前状态：`T4017R`、`T4012b3`、`T4012c`、`T4012R`、`T4013`、`T4013R`、`T4014a`、`T4014b` 与 `T4014R` 已完成；后续顺序现已推进到 `T4015R`。

### P1.6. continuation / effect runtime 显式上下文化（`T4017a -> T4017b -> T4017c -> T4017d -> T4017e1 -> T4017e2 -> T4017e3 -> T4017f -> T4017R`）

- `CONTINUATION.md` 已收口为新的内部模型基线：
  - `EffectCtx*` 表示运行时动态 effect 环境；
  - `EffectOutcome<R>` 表示一次 eager 执行是 `Complete` 还是 `Propagate(signal)`；
  - continuation 捕获的是 `frame + captured ctx`，而不是“当前线程 TLS 上碰巧残留的 effect 状态”。
- 迁移不改公开语言 surface，重点是把当前 `handler stack + active flag + perform slot + callee suspend state + pending continuation replay` 这套 TLS side channel，分阶段迁到显式 compiler/runtime contract。
- 当前项目没有为兼容而保留 effect TLS 的需求；最终状态下，effect/continuation 相关 TLS 若仍存在，只能承担调试职责。
- `T4017a` 已完成：
  - `CONTINUATION.md` 已从“设计草案”收口为 `T4017` 实施基线，并将 staged rollout 细化到 `T4017a -> T4017f`；
  - `SCOOP_FULL_SPEC.md`、`SCOOP_RUNTIME.md` 与 `docs/effect_unified_state_machine.md` 已统一改写为“`EffectCtx + EffectOutcome` 是权威语义模型，TLS 仅是过渡 transport / scratch”；
  - `runtime/c/scoop_runtime.c` 的相关实现注释也已同步，不再把 `active flag + perform slot` 表述为最终 source-of-truth；
  - 已验证 `cargo run -p scoop_tools -- spec-fixtures check`、`cargo test --all` 与 `cargo clippy --all-targets -- -D warnings` 通过。
- `T4017b` 已完成：
  - `llvm/codegen/effect/state_machine_plan.rs` 已把 `declared_effectful` 与 `body_may_outward_effect` 分离：whole-function fixpoint、closure/local function value 传播与 `handle` 分析都改成只追踪真正会向外传播的 effect，而不是简单把 non-`Pure` row 视为 outward-effect；
  - ordinary direct call、closure/function-value call 与 `FunPtr` call 已仅在 `body_may_outward_effect == true` 时发射 TLS propagation check；`vtable` / `itable` 调用暂时保守保留 `declared_effectful` 决策，避免在动态分派目标未知时做不 sound 的去分流；
  - 已补 `llvm` / state-machine regression，覆盖 latent effect、未调用的 higher-order effectful 参数、局部 `handle` 吃掉 helper effect，以及真实 outward-effect 仍保留 TLS 检查的边界；
  - 已额外把 callable-value / funptr 调用元数据整理为 `CallableValueCallSpec`，解决新增 `call_may_suspend` 参数触发的 `clippy::too_many_arguments`，并复验 `cargo run -p scoop -- test`、`cargo test --all` 与 `cargo clippy --all-targets -- -D warnings` 通过。
- `T4017c` 已完成：
  - `crates/scoopc/src/llvm/codegen/effect/contract.rs` 已引入显式 `ValueTransport` / `EffectSignal` / `EffectOutcome` helper，ordinary propagation check、`perform` lowering、`Continuation.resume(...)` active fallback 与 handler dispatch 统一改走这层 contract，不再在新增主线上散落手写 TLS 协议。
  - `crates/scoopc/src/llvm/codegen/runtime_abi.rs` 已补 `ScoopEffectCtx` / `ScoopValueTransport` / `ScoopEffectSignal` / `ScoopEffectOutcome` 的 LLVM struct 类型，给后续 `T4017d/e` 的显式 ABI/continuation 迁移提供稳定命名与布局入口。
  - `runtime/c/scoop_runtime.c` 已新增同名内部结构与 helper；runtime-originated propagate/clear path 现经由 `ScoopEffectOutcome` helper 收口，continuation alloc/resume 也改为围绕显式 `ScoopEffectCtx` 组织 captured/restored handler context 叙事。
  - 已新增 LLVM 回归 `effect_contract_struct_types_are_registered_for_effect_codegen`，并同步更新因为 contract 命名收口而变化的 state-machine / ordinary-call LLVM 单测断言。
  - 已复验 `cargo fmt --check`、`cargo test -p scoopc --features llvm effect_contract_struct_types_are_registered_for_effect_codegen`、`cargo test -p scoop_runtime --test effect_tls`、`cargo test -p scoop_runtime --test continuation_one_shot continuation_double_resume_uses_shared_runtime_error_transport_contract`、`cargo run -p scoop -- test`、`cargo test --all` 与 `cargo clippy --all-targets -- -D warnings` 通过。
- `T4017d` 已完成：
  - ordinary direct / closure / funptr outward-effect call 已统一切到显式 `EffectCtx + EffectOutcome` boundary；direct top-level callee 通过 `__scoop_effect_call_wrapper__*` 安装 caller ctx、调用 legacy body、consume 当前 outcome，再由 caller 读取 outcome tag 决定继续 / 传播。
  - runtime / ABI helper 已补齐 `scoop_effect_handler_stack_top`、`scoop_effect_handler_stack_swap_top`、`scoop_effect_outcome_consume_current`、`scoop_effect_outcome_publish`，ordinary boundary 现可在显式 outcome 与 legacy TLS transport 之间显式往返。
  - state-machine `SuspendCall` fresh path 已同步切到显式 outcome：`state_machine_emitter` 现在会捕获当前 site 的 outcome slot，terminator 读取 outcome tag 判断 active/inactive，并在 active 分支先 publish 回 TLS 再沿既有 suspend / dispatch 主线继续。
  - 已新增/更新 runtime、LLVM 与 state-machine 回归，覆盖 direct / closure / funptr ordinary call 的显式 outcome boundary，以及 state-machine fresh/replay path 不再回退到 post-call TLS probing。
  - 已复验 `cargo fmt --all`、`cargo run -p scoop -- test`（`fixtures: ok (1169)`）、`cargo test --all` 与 `cargo clippy --all-targets -- -D warnings` 通过。
- 具体顺序固定为：
  - `T4017a`：先做文档更新，收口 `CONTINUATION.md`、`SCOOP_FULL_SPEC.md`、`SCOOP_RUNTIME.md` 与 `docs/effect_unified_state_machine.md` 的叙事。
  - `T4017b`：把 `declared_effectful` / `body_may_outward_effect` / `needs_resumable_frame` 的区分接入编译器主线，只在真正可能 outward-effect 的调用点保留 TLS propagation check。
  - `T4017c`：在 compiler/runtime contract 中引入显式 `EffectCtx` / `EffectOutcome` / `EffectSignal` 抽象，并从这一层开始停止新增任何依赖 effect TLS 语义的路径。
  - `T4017d`：把 ordinary direct / closure / funptr effectful call 切到显式 `ctx + outcome` internal ABI。
  - `T4017e1` 已完成：
    - `runtime/c/scoop_runtime.c` 已引入 `ScoopContinuationResumeScope`，并删除 `__scoop_continuation_resume_pending_continuation` / `__scoop_continuation_resume_active` 两个原始 TLS 槽位；当前线程只保留一个 active resume-scope 指针作为局部 bookkeeping。
    - `scoop_continuation_resume_publish_pending_continuation()` 现在只向当前 active scope 写入 pending continuation；`scoop_continuation_resume_common()` 通过 scope 链隔离 nested resume。
    - `crates/scoop_runtime/tests/continuation_one_shot.rs` 已新增 `continuation_publish_pending_continuation_is_scoped_to_active_resume_driver`，锁定“scope 外 publish 为 no-op，scope 内 publish 会被包装成 replay-state，而不是泄漏 raw continuation 指针”。
    - 已复验 `cargo test --all`、`cargo run -p scoop -- test`（`fixtures: ok (1169)`）与 `cargo clippy --all-targets -- -D warnings` 通过。
  - `T4017e2` 已完成：
    - `runtime/c/scoop_runtime.c` 中的 `scoop_continuation_resume_with(...)` 已增加 `ScoopEffectOutcome *outcome` 参数；continuation propagation 现在显式填充 `outcome->signal.resume_token = pending_continuation`，而不是要求 caller 再从 TLS replay-state 取回 inner continuation。
    - 兼容层仍允许 `scoop_continuation_resume()` / `scoop_continuation_resume_u64()` 与 `resume_with(..., outcome = NULL)` 回退到 legacy TLS replay-state 安装路径，确保未迁移边界不被提前打坏；但 `Continuation.resume(...)` 的 fresh / replay 主线已不再依赖 TLS 作为 source of truth。
    - `crates/scoopc/src/llvm/codegen/effect/mod.rs` 与 `crates/scoopc/src/llvm/codegen/effect/state_machine_emitter.rs` 已把 `Continuation.resume(...)` 切到显式 outcome + frame replay-token 槽位：fresh path 调用 `scoop_continuation_resume_with(..., outcome_slot)`，replay path 从 unified state-machine frame 读取 `continuation_resume_replay_token` 与 payload。
    - `SuspendCall` fresh path 捕获 propagation outcome 后会把 `effect_outcome.signal.resume_token` 写入 frame，resume replay 时再读出并清空；相关 IR 断言已锁定 `continuation_resume_replay_token` 出现且 `continuation_resume_replay_state_raw` 不再出现。
    - 复验通过：`cargo test --all`、`cargo run -p scoop -- test`（`fixtures: ok (1169)`）、`cargo clippy --all-targets -- -D warnings`、`cargo test -p scoop_runtime --test continuation_one_shot -- --test-threads=1`、`cargo test -p scoopc --features llvm continuation_resume -- --nocapture`、`cargo test -p scoopc --features llvm async_task_resume_replay_ir_terminates_step_fn_on_active_effect -- --nocapture`、`cargo test -p scoopc --features llvm when_arm_try_resume_nested_handle_ir_keeps_binder_scope_for_inner_resume -- --nocapture`，以及 `tests/fixtures/run-pass/continuation_resume_answer_replay_basic.scoop` 的构建/运行 stdout 比对。
  - `T4017e3` 已完成：
    - `crates/scoopc/src/llvm/codegen/effect/state_machine_emitter.rs` 的 `HandleStateOp::SuspendCall` 已为 ordinary callee 区分 fresh / replay：frame 上已有 `resume_token` 时直接走显式 replay，不再重新 fresh 调用 ordinary callee。
    - ordinary-callee replay 路径现会从 frame token 槽位取回 callee token，恢复 `resume_word` / `resume_gc_ref` 与保存的 resume thunk，replay 完成后把结果写回 caller resume slot，并清掉 fresh-path explicit outcome tag，避免 suspend terminator 读取脏 outcome。
    - fresh continuation materialization 已把 ordinary callee token 捕获到 continuation metadata；ordinary indirect callee resume 的 authoritative state 现已收口到显式 `frame + continuation + resume token`，不再依赖 TLS resume 入口。
    - 已复验 `cargo run -p scoop -- run tests/fixtures/run-pass/effect_escape_continuation_indirect_perform_multi_site_callee_branch.scoop`、`cargo run -p scoop -- test`、`cargo test --all`、`cargo clippy --all-targets -- -D warnings` 与 `cargo fmt` 通过。
  - `T4017f` 已完成：
    - `crates/scoopc/src/llvm/codegen/effect/contract.rs` / `crates/scoopc/src/llvm/codegen/mod.rs` 已新增统一 `LegacyEffectBoundary` helper，把 legacy callee boundary 收口到显式 `EffectCtx + EffectOutcome` contract；vtable / itable / object init / top-level init 现已共享同一条 boundary 流程。其间曾临时把 extern-native boundary 也纳入同一路径，但该部分后续已在 `T4014a` 随 ordinary `@Extern` effect-impermeable 合同一并移除。
    - `crates/scoopc/src/llvm/codegen/effect/state_machine_emitter.rs` 已补 hidden-suspend boundary 的 explicit outcome 捕获；`ObjectInitAccessBoundary` / `RuntimeRaiseBoundary` 现在会把 propagation outcome 正确写回 unified state-machine，不再误吞 active path。
    - LLVM IR 回归与 run-pass fixtures 已覆盖 virtual call、interface call、object value init 与 top-level immutable init 的显式 outcome 行为，并锁定这些路径不再依赖 `@scoop_effect_is_active`；当时新增的 extern/native outward-propagation coverage 现已由 `T4014a` 删除。
    - 当时为验证 extern/native outward propagation 临时补入的 runtime test-only helper `scoop_test_raise_null_assertion_failed_in_native` 与 allowlist，现已在 `T4014a` 随 contract 反转删除。
    - 复验通过：`cargo fmt --all`、`cargo test -p scoopc --features llvm explicit_outcome_boundary -- --nocapture`、`cargo run -p scoop -- test --fixtures tests/fixtures/run-pass`（`fixtures: ok (397)`）、`cargo run -p scoop -- test`（`fixtures: ok (1174)`）、`cargo test --all` 与 `cargo clippy --all-targets -- -D warnings`。
  - `T4017R` 已完成：
    - 代码搜索与 review 已确认：ordinary direct / closure / funptr / vtable / itable / object init / top-level init 统一走显式 `EffectCtx + EffectOutcome` contract，production IR 不再在这些 boundary 后 probing `@scoop_effect_is_active`。ordinary `@Extern` 当时仍复用该 contract 的部分，现已由 `T4014a` 明确撤回为 effect-impermeable native leaf boundary。
    - continuation resume 的权威恢复状态已收口为 captured handler context、continuation/frame metadata 与显式 resume token；`scoop_callee_suspend_state_get()` 不再被生产 codegen 当作恢复入口。
    - `state_machine_emitter` 中保留的 TLS handler-stack / perform-slot / active 读取与 `CONTINUATION.md`、`docs/effect_unified_state_machine.md` 的叙事一致，只承担 direct `perform` / hidden-suspend / arm-cleanup 的局部 transport，不再把 ambient TLS 当成语义 source of truth。
    - 已复验 `cargo run -p scoop -- test`、`cargo test --all` 与 `cargo clippy --all-targets -- -D warnings` 通过。
- 当前状态：`T4017a`、`T4017b`、`T4017c`、`T4017d`、`T4017e1`、`T4017e2`、`T4017e3`、`T4017f`、`T4017R`、`T4012b3`、`T4012c`、`T4012R`、`T4013`、`T4013R`、`T4014a`、`T4014b` 与 `T4014R` 已完成，`T4017e` 已整体收口。下一步执行 `T4015R`。

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
  - `T4012b3` 已完成：
    - `warnings.rs` 已建立结构化 warning code / suppression 基础设施，`deprecated`、`enum-size-disparity` 与 `redundant-when-else` 统一走 `warn[code]` 输出并支持按 span 抑制。
    - built-in `@Suppress` 已接入 parser / annotation typecheck / warning collection 主线，要求至少一个字符串位置参数，并对 named args、非字符串参数与未知 warning code 给出稳定诊断。
    - expression annotation AST 与 annotated local declaration 解析已补齐，`@Suppress` 现可覆盖 expression / declaration / file 三类 suppression surface。
    - 额外修复了 class self-type 与 `@CLayout` 内部 lowering 错误发出 deprecated use-site warning 的既有问题，避免 declaration 内部辅助路径污染用户 stderr。
    - 已验证 `cargo run -p scoop -- run tests/fixtures/run-pass/suppress_deprecated_declaration_basic.scoop`、`cargo run -p scoop -- test`（`fixtures: ok (1185)`）、`cargo test --all`、`cargo clippy --all-targets -- -D warnings` 与 `cargo run -p scoop_tools -- spec-fixtures check` 通过。
- `T4012c` 已完成：
  - 编译器现已把 `@Experimental` 识别为 built-in annotation，并将合法 target 收口为函数 / 类型 / 属性 / 文件。
  - use-site surface 已固定为 `@Experimental(feature = "...")`；typecheck 会对缺少 `feature`、参数非字符串字面量、非法 arg shape 与非法 target 给出稳定 diagnostics。
  - `sysroot/core.scoop`、`SCOOP_FULL_SPEC.md` 与 `ISSUES.md` 已同步写清：当前只保留 built-in marker 与参数校验，不接入 feature gating framework。
  - 新增 parse/typecheck fixtures，并已验证 `cargo run -p scoop -- test --fixtures tests/fixtures/parse`、`cargo run -p scoop -- test --fixtures tests/fixtures/typecheck`、`cargo run -p scoop -- test`（`fixtures: ok (1191)`）、`cargo test --all`、`cargo run -p scoop_tools -- spec-fixtures check` 与 `cargo clippy --all-targets -- -D warnings` 通过。
- `T4012R` 已完成：
  - 复扫 annotation declaration / built-in marker / use-site 与普通 type/expr lowering 路径后，已确认 annotation system 保持 compile-time markers only，不再把 annotation class 当成一般 nominal/runtime feature。
  - review 期间额外修复了 annotation class 泄漏到 runtime nominal/type position 的既有缺陷：普通 ctor overload 收集、runtime field 收集、普通函数签名/属性类型/type annotation lowering 现已统一拒绝 annotation class；它仅允许用于 `@Name(...)` use-site 与其他 annotation class 的 payload 类型。
  - `@Experimental(feature = "...")` 仍只保留 reserved compile-time marker / gate surface；`@Inline` 的剩余交叉项已明确移交给 `T4013`。
  - 已验证 `cargo fmt --all`、`cargo run -p scoop -- test --fixtures tests/fixtures/typecheck`（`fixtures: ok (392)`）、`cargo run -p scoop -- test`（`fixtures: ok (1194)`）、`cargo test --all` 与 `cargo clippy --all-targets -- -D warnings` 通过。
- `T4013` 已完成：
  - parser 已移除合法的 `inline` modifier，并新增 `scoop::parse::inline_modifier_removed` 兼容诊断，把旧写法指向 `@Inline fun ...`。
  - typecheck 已删除 inline-specific 的 lambda non-local return 例外，`return` 在 lambda 中统一报 `scoop::typecheck::return_not_in_function_body`。
  - `@Inline` 已纳入 built-in annotation 识别，并收口为“仅函数目标、无参数”的 compile-time marker；`sysroot/core.scoop` 与 `SCOOP_FULL_SPEC.md` 已同步切到 `@Inline` surface。
  - 已新增 `inline_modifier_removed.scoop`、`inline_annotation_fun_ok.scoop`、`inline_annotation_invalid_target_is_error.scoop` 与 `return_in_inline_annotation_lambda_arg_is_error.scoop` 等回归，并复验 `cargo run -p scoop -- test --fixtures tests/fixtures/parse`（`fixtures: ok (123)`）、`cargo run -p scoop -- test --fixtures tests/fixtures/typecheck`（`fixtures: ok (394)`）、`cargo run -p scoop -- test`（`fixtures: ok (1197)`）、`cargo test --all`、`cargo run -p scoop_tools -- spec-fixtures check` 与 `cargo clippy --all-targets -- -D warnings` 通过。
- `T4013R` 已完成：
  - 复扫 parser / lexer / AST 后，已确认 `Keyword::Inline` 仅作为 removed-syntax 诊断入口保留；生产声明主线不会再接受 `inline` modifier，也不存在写回旧 modifier 语义的旁路。
  - 复扫 annotation / typecheck / lowering / codegen 后，已确认 `@Inline` 只参与 built-in annotation 识别与 target/参数校验；表达式、局部绑定等非函数目标会稳定报错，生产 lowering/runtime 中不存在 `@Inline` 控制流或 ABI special-case。
  - 复扫 `return` 规则与文档后，已确认 lambda 中的 `return` 统一只允许离开立即包裹它的命名函数体；`SCOOP_FULL_SPEC.md`、`sysroot/core.scoop` 与 `ISSUES.md` 叙事一致，若未来要重引 non-local control，只能另立 deferred 设计任务。
  - 已验证 `cargo run -p scoop -- test --fixtures tests/fixtures/parse`、`cargo run -p scoop -- test --fixtures tests/fixtures/typecheck`、`cargo run -p scoop -- test`、`cargo test --all`、`cargo clippy --all-targets -- -D warnings` 与 `cargo run -p scoop_tools -- spec-fixtures check` 通过。
- 当前状态：annotation marker / inline keyword cleanup 与 FFI / ABI 边界收口主线已完成，`T4012R`、`T4013`、`T4013R`、`T4014a`、`T4014b` 与 `T4014R` 已完成；下一步进入 `T4015R`。

### P3. FFI / ABI 边界收口

- 聚焦普通 `@Extern` 的 effect-impermeable 边界，以及 stable handle / `Pinned` 的职责分离：stable handle 负责 long-lived identity / wake token，`Pinned` 只负责短时裸地址借出。
- `T4014a` 已完成：ordinary `@Extern` 现已要求 Pure（或省略 effect row）、禁止 `eff` 参数、继续拒绝 GC-managed control object 直接过 ABI，并移除了 extern-native outward-effect lowering/test-only helper。
- `T4014b` 已完成：
  - `crates/scoopc/src/typecheck/annotations.rs` 的 extern ABI 诊断已显式区分“长期 token 用 `GcHandle.raw: UIntPtr`”与“短时裸地址借出用 `GC.pin/unpin` + `scoop.unsafe.Ptr<T>`”；`crates/scoopc/src/typecheck/expr/error.rs` 里过时的 `Pinned<引用类型>` 文案也已修正为 `Pinned` handle。
  - `SCOOP_FULL_SPEC.md`、`SCOOP_RUNTIME.md`、`sysroot/core.scoop` 与 `ISSUES.md` 已统一到同一叙事：ordinary `@Extern` / reactor / callback 的长期 round-trip 走 `GcHandle.raw: UIntPtr`；`Pinned` 只保留 Scoop 侧短时 pin handle 语义，不是 ordinary `@Extern` ABI token。
  - 新增 typecheck 回归 `extern_fun_gc_handle_raw_token_roundtrip_ok.scoop` 与 `extern_fun_signature_with_pinned_is_error.scoop`；结合既有 runtime GC 回归 `gc_handle_token_roundtrip_callback_basic.scoop`、`gc_handle_stale_callback_token_is_error.scoop`、`gc_pin_unpin_move_stress_matrix.scoop`，已把 handle round-trip / drop / stale token / pin-unpin 边界锁回自动回归。
  - 已验证 `cargo fmt --check`、`cargo run -p scoop_tools -- spec-fixtures check`、`target/debug/scoop test`（`fixtures: ok (1202)`）、`cargo test --all` 与 `cargo clippy --all-targets -- -D warnings` 通过。
- `T4014R` 已完成：
  - 复扫 `SCOOP_FULL_SPEC.md`、`SCOOP_RUNTIME.md`、`sysroot/core.scoop`、`sysroot/unsafe.scoop`、`crates/scoopc/src/typecheck/annotations.rs`、`crates/scoopc/src/typecheck/expr/error.rs`、`crates/scoopc/src/llvm/codegen/mod.rs` 与 `crates/scoopc/src/llvm/mod.rs` 后，ordinary `@Extern` 的 effect-impermeable 边界、stable handle 的长期 token 合同以及 `Pinned` 的短时借址语义仍保持一致，不存在继续隐含 GC / effect 语义的生产旁路。
  - 复验 `cargo run -p scoop_tools -- spec-fixtures check`、`cargo test -p scoopc pure_extern_call_does_not_install_effect_boundary --features llvm`、`cargo run -p scoop -- test --fixtures tests/fixtures/typecheck`、`cargo run -p scoop -- test --fixtures tests/fixtures/runtime_gc`、`cargo run -p scoop -- test`、`cargo test --all` 与 `cargo clippy --all-targets -- -D warnings`，均未暴露新的前置 blocker。
- 当前状态：`T4014a`、`T4014b`、`T4014R`、`T4015a1`、`T4015a2`、`T4015b`、`T4015c` 与 `T1220b` 已完成；`T4015` 已完成调用绑定 / generic 实例化 / ordinary control flow / 声明级 effect contract 主线，并已把 package-level `comptime if` 条件接到 compilation-unit 调用绑定主线，因此下一步回到 `T4015R`。

### P4. const / comptime 扩展

- 在保持纯计算模型前提下，继续扩展 const/comptime 的 generic 调用、控制流与 effect-row 合同，避免继续停留在“仅 non-generic 调用已接通、纯计算子集仍偏窄”的早期状态。
- 当前状态：`T4014R` 已完成；经代码核对，`T4015a` 已按两步收口完成：
  - `T4015a1`：先让 const/comptime 接入 compilation-unit resolve/typecheck 绑定，并按 typechecked 目标执行跨文件 / overload 的 non-generic 顶层 `const fun` 调用；
  - `T4015a2`：再支持 generic `const fun` 的实例化与 type-substitution，移除解释器对 `generic type params` 的剩余门禁。
- 当前状态补充：
  - `T4015a2` 已完成后，const/comptime 解释器现已复用 typecheck 选定的 generic type args、活动类型实参环境与 reflection/type-substitution 支撑；跨文件 generic const 调用、显式/推断类型实参，以及 nested generic const 调用现已统一复用 typechecked 绑定主线。
  - `T4015b` 已完成：ordinary `if` / block / `do`、局部 `val/var`、assignment、`while` / 普通 `for`、`break/continue` 与 const val initializer 中的 block 表达式现已可解释执行；同时默认递归门限已从 `64` 收紧到 `48`，恢复“先报 `recursion_limit_exceeded`、不先撞宿主线程栈”的稳定合同。
  - `T4015c` 已完成：`const fun` 的声明级 effect contract 现已在 typecheck / comptime 注释 / spec / README / `ISSUES.md` 中统一为“仅允许省略 effect row，或显式 `/ Pure` / `/ Pure!`，且不允许 `<eff ...>`”；新增 `const_fun_closed_pure_basic` 回归锁定显式 `/ Pure!` 仍能走 comptime 主线。
  - `T1220b` 已完成：`trim_package_level_comptime_ifs_in_compilation_unit(...)` 会在 pre-trim 阶段为“当前可见前缀 + 条件 probe”构造临时 compilation unit，复用 resolve/typecheck 主线刷新 `TopLevelFunCallBinding`，并把 probe `TypeStore` 回填给解释器，因此 package-level `comptime if` 条件中的 overloaded / generic explicit type args / imported cross-file `const fun` 调用不再退回 simple-name + arity fallback。
  - `crates/scoop/src/commands/build.rs`、`crates/scoop/src/fixtures/mod.rs`、`crates/scoopc/src/llvm/frontend.rs` 与 `eval_const_bindings_in_compilation_unit(...)` 已统一切到 compilation-unit trim 路径；`crates/scoopc/src/comptime/tests.rs` 与 `tests/fixtures/run_pass_cone/package_level_comptime_if_cross_file_const_fun/` 也已补齐同文件 overload、显式类型实参与跨文件 import 三类回归。
  - 因此当前主线顺序更新为 `T4015R`；下一步应复审整个 const/comptime 主线是否还残留类似旧旁路。

## 3. 各阶段完成标准

### C1. delimited continuation / `Task` / 显式 effect runtime

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
- effect propagation 的 source of truth 必须从 `TLS active + perform slot` 收口为显式 `EffectCtx` / `EffectOutcome` internal contract；TLS 若仍保留，只能承担调试职责。
- ordinary call sites 若静态证明不会 outward-effect，不得继续无差别支付 effect TLS 分流成本。
- continuation capture / resume 的 authoritative state 必须可解释为 `frame + captured ctx (+ signal/resume token)`，而不是 resuming thread 当前 TLS。

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
