# Scoop：下一轮计划（正确的单次 delimited continuation 优先）

> 生成时间：2026-04-21  
> 历史归档：`PLAN-5.md` / `TODO-5.md`  
> 本轮主题：先把 `Continuation` 从当前“为 effect / async lowering 服务的 step-driving advanced API”收口为**正确的单次（one-shot）delimited continuation**，再让 `Task` 真正退化为其上的薄封装；annotation、删除 `inline` 关键字、FFI / ABI、const / comptime 顺延。  
> 设计前提：**不支持 multi-shot continuation**。Scoop 保持当前可变局部、writeback、once-init 与 GC-managed frame 的整体运行时方向，不为 continuation cloning / replay 另开一套“immutable everything”语义世界。

## 0. 工作原则

- 本轮严格按 `TODO.md` 中的顺序推进，不跨条目并行实现。
- `Continuation` 的目标语义是**单次、deep、以最近 `handle` 为 delimiter** 的 delimited continuation。
- 语言层面只保留 `Effect.op(args) -> expr` 与 `Effect.op(args), k -> expr` 两种 handler arm；`-> resume` 从用户态语法移除。若需要 immediate-resume fast path，只能作为 lowering / codegen 内部优化分类。
- `k.resume(payload)` 在 resumed computation 正常完成 delimiter 时，应返回该 delimiter 的 answer type；后续本地代码可继续执行。
- repeated resume 继续是 one-shot 违规；multi-shot、continuation cloning、resume-many replay 都不纳入本轮范围。
- `Task<T>` 仍是 general-purpose async API；raw `Continuation` 仍是 advanced API。区别在于本轮结束后，`Task` 不得再依赖“resume 后偷读 frame 前缀结果”的 runtime hack。
- annotation 的方向改为**compile-time markers only**：不把 annotation 做成复杂 nominal runtime/type-system feature。
- `inline` 关键字默认从语言 surface 移除；若仍需要内联提示，由 `@Inline` 统一承担，且它只是一种 compile-time marker / 优化提示，不附带控制流语义。
- executor framework、wakeup queue、work-stealing、public `spawn/join` 调度语义继续 deferred；它们不能成为本轮设计前提。
- 若实现改变公开语义，必须同步 `SCOOP_FULL_SPEC.md`；若涉及运行时合同，还要同步 `SCOOP_RUNTIME.md`、`sysroot/core.scoop` 与必要注释。

## 1. 顺序总览

1. 新增优先项：正确的单次 delimited continuation 与 `Task` 去 hack（`T4016a1 -> T4016a2 -> T4016b1 -> T4016b2 -> T4016c -> T4016b3 -> T4016d -> T4016R`）
2. `ISSUES.md` 第 9 条：annotation markers、non-inline built-in annotations 与 `@Experimental` feature-gate marker（`T4012a -> T4012b -> T4012c -> T4012R`）
3. `ISSUES.md` 第 10 条：删除 `inline` 关键字与 legacy non-local return 语义残留（`T4013 -> T4013R`）
4. `ISSUES.md` 第 11 条：FFI / ABI 的 effect-impermeable 边界与 stable handle / pin 职责分离（`T4014a -> T4014b -> T4014R`）
5. `ISSUES.md` 第 12 条：const / comptime 纯计算子集扩展（`T4015a -> T4015b -> T4015c -> T4015R`）

## 2. 分阶段目标

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
- 当前状态：
  - `T4016a1` 已完成：`SCOOP_FULL_SPEC.md` / `SCOOP_RUNTIME.md` 已把 continuation answer model、deep handler、one-shot 与 `-> resume` 移除的迁移叙事收口到同一口径。
  - `T4016a2` 已完成：`sysroot/core.scoop`、`runtime/c/scoop_runtime.c` 与 `runtime/c/scoop_task.c` 的注释现已明确：
    - `Continuation<T, eff E>` 仍只是过渡中的 sysroot surface，answer type 尚待 `T4016b` 接入主线；
    - 用户态 handler surface 只保留 `Effect.op(args) -> expr` 与 `Effect.op(args), k -> expr`；
    - `Task` 当前“resume 后回读 frame 前缀得到 `__TaskStepResult`”的路径只是待 `T4016c/d` 移除的 task-only 实现债务。
  - `T4016a` 设计/注释收口阶段已完成；由于当前 runtime ABI 仍是 `void scoop_continuation_resume(void*)`，而前端 / HIR / LLVM 又把 `-> resume` immediate-resume 与 `Continuation.resume(...): Unit` 绑在一起，`T4016b` 已拆成更小子任务。
  - `T4016b1` 已完成：
    - parser / AST / HIR / resolver / typecheck 已移除用户态 `-> resume` surface，并改为 removed-syntax diagnostic；
    - AST / HIR 级别的 `ImmediateResume` arm kind 已删除；tail `k.resume(...)` 仅作为 lowering / codegen 内部分类保留；
    - 相关 parse / HIR / typecheck / run-pass fixtures 已迁移到 `, k ->` + `k.resume(...)`，并同步了必要的 golden / 预期；
    - 已验证 `cargo test --all`、受影响 fixture 子集（38 个）以及 `cargo clippy --all-targets -- -D warnings` 通过。
  - 下一步进入 `T4016b2`：把 continuation answer type 接入 binder 静态模型与显式 `Continuation<Resume, Answer, eff E>` surface，再继续推进 `T4016c` / `T4016b3` 的返回值主线。

### P2. annotation markers 与 `inline` 关键字清理

- annotation 保持 compile-time markers only，不进入复杂 nominal / runtime 语义。
- 先收口 non-inline built-in annotations，并补入 `@Experimental(feature = "...")` 这一保留的 built-in feature-gate marker；具体 feature gating wiring 后续再做。
- 再删除 `inline` 关键字与 legacy non-local return 语义残留；若未来仍需内联提示，统一由 `@Inline` 作为纯优化 marker 承担。
- 当前状态：`T4012a -> T4012b -> T4012c -> T4012R -> T4013 -> T4013R` 待开始。

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
- fixtures / tests 需覆盖：
  - `-> resume` removed-syntax diagnostics，以及迁移到 `, k ->` + `k.resume(...)` 后的等价行为；
  - arm 内 `k.resume(...)` 之后继续执行本地代码；
  - nested handle / `finally` / early return；
  - resumed computation 再次 suspend 时捕获 fresh continuation；
  - `Task.poll()/step()` 在新语义下仍保持公开合同不变。
- `Task` 必须不再依赖“调用 `scoop_continuation_resume(...)` 后再偷读 heap frame 前缀”的 runtime hack；若底层仍复用 frame result transport，也必须是统一 continuation ABI 的内部实现细节。

### C2. annotation / `inline` / FFI / const/comptime

- 对应 `ISSUES.md` 条目已关闭，或至少收缩为新的、更窄的剩余 blocker。
- 新增或更新的 fixtures 覆盖 typecheck、HIR / MIR / LLVM lowering、run-pass 或相关 regression。
- `@Experimental(feature = "...")` 若按计划加入，必须先作为 built-in compile-time marker 被编译器识别并校验参数形状；具体语言特性接线可以继续 deferred。
- `inline` 关键字若按计划删除，parser / typecheck / spec / sysroot 叙事必须同步切到 `@Inline`；`@Inline` 不能继续携带任何控制流语义。
- 若规范文字被实现改变或澄清，需同步 `SCOOP_FULL_SPEC.md`，必要时同步 runtime / sysroot 文档。

## 4. 非目标

- 本轮不实现 multi-shot continuation，不定义 continuation cloning / replay 的语言级合同。
- 本轮不引入 undelimited continuation / `call/cc` 风格控制操作。
- 本轮不完成 executor framework，不定义 wake queue、event loop、I/O driver、work-stealing 或 public `spawn` 调度语义。
- 本轮不为了支持 continuation 而把 Scoop 改造成“整体不可变、禁止写回”的另一种语言模型。
- 本轮不把 annotation 扩展成复杂 nominal / runtime feature。
- 本轮不扩展与 `TODO.md` 当前条目无直接关系的 stdlib / runtime surface。

## 5. 最终验收

- `PLAN.md` 与 `TODO.md` 中本轮任务已按顺序推进并留下明确结论。
- `Continuation` / `Task` / `async` / effect 文档叙事一致：spec、runtime 文档、sysroot surface 与实现不再对 continuation answer/result model 各说各话。
- 相关实现通过必要的定向测试；阶段收口时复验 `cargo test --all` 与 `cargo run -p scoop -- test`。
- 若修改了 `SCOOP_FULL_SPEC.md` 中带 fixture 的代码块，还需执行 `cargo run -p scoop_tools -- spec-fixtures check`。
