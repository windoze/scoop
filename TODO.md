# TODO（Scoop：正确 delimited continuation 与剩余 issue 收口）

> 生成时间：2026-04-21  
> 历史归档：`TODO-5.md` / `PLAN-5.md`  
> 顺序约束：严格按当前文件中的条目顺序推进；不得跨条目并行实现。  
> 本轮先修复全量回归暴露的 `@Extern` + moving-GC native-roots 既有问题，再完成正确的单次 delimited continuation / `Task` review，并按 `SCOOP_TASK.md` 继续做 core task surface 收口 / Scoop 化与 `Task` 去内建 lock 的轻量 claim 版收口（`T4016T1 -> T4016T1a -> T4016T1b -> T4016T1c -> T4016T1R -> T4016T1d1 -> T4016T1d2 -> T4016T1d3 -> T4016T1d4 -> T4016T1d5 -> T4016T2 -> T4016T3 -> T4016T4 -> T4016T5 -> T4016T5a -> T4016T6 -> T4016T7 -> T4016T7a -> T4016T8 -> T4016T9 -> T4016T4R`，只覆盖 phase 1-3；phase 4 executor / wake / reactor 延期到 stdlib），随后按 `CONTINUATION.md` 推进显式 `EffectCtx` / `EffectOutcome` 收口（`T4017a -> T4017b -> T4017c -> T4017d -> T4017e -> T4017f -> T4017R`），最后再回到 annotation、删除 `inline` 关键字、FFI / ABI、const / comptime。

## 全局约束

- `TODO-5.md` 中的 `[DONE]` 条目只作历史归档；新的收口工作必须在当前文件中重新立任务，不能回写归档。
- 当前剩余实现顺序为：`T4017e2 -> T4017e3 -> T4017f -> T4017R`，随后回到 annotation markers（下一步 `T4012b3`） -> `inline` -> FFI / ABI -> const / comptime。
- continuation 继续保持 **single-shot only**；multi-shot、continuation cloning、resume-many replay 明确 out-of-scope。
- 语言层面只保留 `Effect.op(args) -> expr` 与 `Effect.op(args), k -> expr` 两种 handler arm；`-> resume` 从用户态语法移除。若编译器内部仍需要 immediate-resume fast path，只能作为 lowering / codegen 优化分类。
- `Task<T>` 是 general API；raw `Continuation` 是 advanced API。`T4016` 完成后，`Task` runtime 不得再依赖“resume 后偷读 heap frame 前缀结果”的私有 hack。
- 基于 `SCOOP_TASK.md`，core task 设计仍在进行中，不保留 `Poll<T>` / `poll()` 等命名或 surface 的向后兼容包袱；若公开 API 要改名，应直接收口到最终形态。
- core `Task` 继续收口为轻量 single-driver object：不支持多个父 task / 多线程共享同一 task 驱动；public `step()` 的并发 / reentrant 误用直接 trap，`Pending` 不再承担竞争失败语义。
- annotation 保持 **compile-time markers only**，不进入复杂 nominal / runtime 语义。
- `inline` 关键字以移除为默认方向；若后续仍需表达内联偏好，统一由 `@Inline` 作为 compile-time marker 承担，不再保留关键字与控制流语义双轨。
- 每个实现任务后必须立即做 review 任务；review 只审查生产代码与规范一致性，不以测试命名代替结论。
- 若某项实现改变公开语义，必须同步 `SCOOP_FULL_SPEC.md`；若涉及运行时合同，还要同步 `SCOOP_RUNTIME.md`、`sysroot/core.scoop` 或相关文档。
- 本轮不设计 executor framework；所有与 executor、wakeup、queueing、work-stealing、public `spawn` scheduling 相关内容一律留待后续 stdlib stage。

## T4016：正确的单次 delimited continuation、移除 `-> resume` 语法与 `Task` 去 hack

### T4016 [DONE] 收口正确的单次（one-shot）delimited continuation，移除用户态 `-> resume` 语法，并让 `Task` 摆脱 runtime hack（拆分执行）
- 说明：
  - 当前 `Continuation<T, eff E>` + `Continuation.resume(...): Unit` 更接近“为 effect / async lowering 服务的 step-driving advanced API”，还不是完整的 delimited continuation surface。
  - 本组任务要把 continuation 收口为**正确的、单次、deep、以最近 `handle` 为 delimiter** 的语义：`k` 捕获剩余计算，`k.resume(v)` 在 resumed computation 正常完成 delimiter 时返回 answer type，本地后续代码可继续执行。
  - 语言层面固定只保留 `Effect.op(args) -> expr` 与 `Effect.op(args), k -> expr` 两种 arm；原 `-> resume` 用户态语法删除，其语义能力统一由 continuation arm + `k.resume(...)` 承担。
  - `Task` 仍可继续隐藏 raw continuation，但内部不再允许靠“调用 `resume` 后手动窥视 frame 结果槽”恢复 step result；内部 step driver 必须建立在同一 continuation answer model 之上。
  - multi-shot 明确不做；Scoop 也不为此转向“immutable everything”运行时模型。
- 验收：
  - `SCOOP_FULL_SPEC.md`、`SCOOP_RUNTIME.md`、`sysroot/core.scoop`、parser / typecheck / HIR / LLVM / runtime 对 continuation answer model 与 handler arm surface 形成统一叙事；`-> resume` 不再作为用户态语法存在。
  - `Task` 不再依赖 task-only 的 runtime frame-peek hack；若底层仍有 frame result transport，也必须是 continuation ABI 的通用内部细节。
- 依赖：无

### T4016a [DONE] 定义正确的 one-shot deep delimited continuation surface、answer type 与最终 handler 语法（拆分执行）
- 说明：
  - 原条目同时要求“规范/运行时叙事定稿”和“sysroot / compiler 可见表示对齐”，一次性改动面仍然偏大，也会与 `T4016b` 的主线接入互相牵连。
  - 因此先拆成“文档/设计收口”与“sysroot / 内部注释过渡合同”两步，再进入 `T4016b` 做 parser / typecheck / HIR / lowering 实装。
- 验收：
  - 子任务全部完成后，spec / runtime doc / sysroot / 注释对 continuation answer model、deep handler、one-shot 合同与 `-> resume` 移除形成统一叙事。
- 依赖：T4016

### T4016a1 [DONE] 在 spec / runtime 设计文档中收口 answer-returning continuation 与最终 handler surface
- 范围：
  - 在 `SCOOP_FULL_SPEC.md` 与 `SCOOP_RUNTIME.md` 中明确 continuation 的 answer type 语义：`k.resume(payload...): Answer / (E + Raise<RuntimeError>)`，并说明 resumed computation 正常完成 delimiter 后，本地代码可在调用点之后继续执行。
  - 固定 deep handler 语义：`k` 在捕获的 handler stack 下恢复，arm body 期间当前 handler inactive，再次 suspend 时捕获 fresh continuation。
  - 在用户态设计文档中移除 `-> resume` 语法，只保留 `Effect.op(args) -> expr` 与 `Effect.op(args), k -> expr`；同时写清从旧语法迁移到 `, k ->` + `k.resume(...)` 的方向。
  - 明确 one-shot、重复 resume 的运行时错误合同，以及 multi-shot / clone / replay 继续 deferred。
- 验收：
  - 设计文档不再同时保留“`k.resume(...)` 返回 `Unit`”与“continuation 代表剩余计算结果”的分裂表述。
  - 文档能明确回答 arm 内 `k.resume(...)` 后续语句、nested handle、fresh continuation、cross-thread resume，以及 `-> resume` 的替代表达。
- 依赖：T4016a

### T4016a2 [DONE] 在 sysroot / 实现注释中对齐 continuation / Task 的过渡合同
- 范围：
  - 在 `sysroot/core.scoop` 与必要的实现注释中，把 continuation / `Task` 的术语对齐到 `T4016a1` 已定稿的 answer model 与 handler surface。
  - 在不提前切断现有实现的前提下，明确标出哪些 sysroot / 内部注释仍属 `T4016b/c/d` 之前的过渡表达，避免继续把“`resume` 返回 `Unit`”写成稳定设计结论。
  - 说明 `Task` 将在后续任务中退化为“基于 continuation answer type 的薄封装”，并把 task-only runtime hack 明确保留为待移除的实现债务。
- 验收：
  - sysroot / 注释不再把旧 continuation surface 写成最终设计；与 `T4016a1` 的术语和迁移方向保持一致。
- 依赖：T4016a1

### T4016b [DONE] 把 answer type、returning-resume 与 arm syntax removal 接入前端 / 中端主线（拆分执行）
- 说明：
  - 现状里有三件事被绑在同一个任务里：删除 `-> resume` 语法、把 continuation binder 升级为带 answer type 的静态模型、以及让 `Continuation.resume(...)` 真正返回 delimiter answer。
  - 代码上这三件事横跨 parser / AST / HIR / typecheck / LLVM state machine，而且 runtime ABI 目前仍是 `void scoop_continuation_resume(void*)`；因此若不拆开，很容易把“删旧语法”和“接通 answer-return channel”强行耦合在一起。
  - 本条先拆成 `T4016b1 -> T4016b2 -> T4016c -> T4016b3`：先删掉用户态 `-> resume` 并把 tail-resume 收口为内部优化分类，再把 answer type 接入 continuation 静态 surface，随后由 `T4016c` 提供 runtime / ABI 返回通道，最后回到 `T4016b3` 完成 answer-returning `resume` 的主线接入。
- 验收：
  - 子任务全部完成后，`-> resume` 不再保留任何用户态或 lowering 侧 special form，continuation answer type 也不再是 task-private 概念。
  - `Continuation.resume(...)` 能作为真正返回 delimiter answer 的表达式 surface 工作，而不是继续被 typecheck / HIR 钉死为 `Unit`。
- 依赖：T4016a2

### T4016b1 [DONE] 删除用户态 `-> resume` 语法，并把 tail-resume 收口为 lowering / codegen 内部分类
- 范围：
  - parser / AST / HIR / resolver / typecheck 不再把 `Effect.op(...) -> resume { ... }` 当作独立用户态 arm surface；语法层改为 removed-syntax diagnostics，并显式指向 `Effect.op(...), k -> { k.resume(...) }` 的迁移路径。
  - 原先依赖 `-> resume` 的 fixtures / 回归统一改写为 `, k ->` + `k.resume(...)`；用户态只保留 `Effect.op(args) -> expr` 与 `Effect.op(args), k -> expr` 两种 arm 形态。
  - lowering / codegen 若仍需要“tail-resume fast path”，只能在 escape-continuation arm 内部按 `k.resume(...)` 的尾位置形状做内部分类，不能继续把 `ImmediateResume` 当成公开语义节点。
  - 补充 parse / typecheck / HIR / lowering / run-pass regression，覆盖 removed-syntax diagnostics、迁移后等价行为，以及 tail `k.resume(...)` 的内部优化路径仍可工作。
- 验收：
  - `-> resume` 在用户态彻底消失；相关回归全部迁移到 continuation arm。
  - 生产代码中不再保留 AST / HIR 级别的 immediate-resume arm kind；若有 tail-resume 优化，只存在于内部 lowering / codegen 分类。
- 依赖：T4016b

### T4016b2 [DONE] 把 continuation answer type 接入 binder 静态模型与显式 `Continuation<Resume, Answer, eff E>` surface
- 范围：
  - continuation binder 类型不再只携带 payload type 与 resumed-step effect row；handle type inference 必须把 delimiter answer type 一并接入 `, k ->` arm 的静态模型。
  - `sysroot/core.scoop`、type lowering、type pretty-print 与相关 diagnostics 统一切到 `Continuation<Resume, Answer, eff E>` surface；显式 continuation 类型注解与推导出的 `k` binder 类型都要能看到 answer type。
  - 补充 typecheck / HIR regression，覆盖显式 `Continuation<Resume, Answer, eff E>` 注解、answer type mismatch、以及 handle delimiter answer 的推导与打印。
- 验收：
  - continuation answer type 成为 compiler 主线的一等静态语义对象，而不再只存在于文档或 task-private 叙事中。
  - `, k ->` binder 的静态类型与 spec/runtime 文档中的 `Continuation<Resume, Answer, eff E>` 口径一致。
- 依赖：T4016b1

### T4016c [DONE] 收口 state machine / runtime / ABI，使 continuation result 成为一等返回通道
- 范围：
  - runtime / LLVM / state-machine contract 要把 continuation answer 作为统一返回通道收口，而不是让调用方在 `resume` 之后再按 task-private 规则窥视 frame 布局取值。
  - 继续保留 one-shot、cross-thread resume、cleanup / `finally`、fresh continuation on re-suspend 等既有运行时能力，但它们都必须与 answer-returning resume 语义对齐。
  - 若底层继续复用 frame `resume_word` / `resume_gc_ref` 承载结果，也必须通过统一 ABI / helper 暴露给 codegen，不能让 `Task` 或其他上层逻辑直接依赖 frame prefix 细节。
  - 同步 runtime 文档与必要注释，说明 continuation answer、resume payload、fresh continuation、cleanup 之间的关系。
- 验收：
  - generic `Continuation.resume(...)` 的 lowering / runtime 路径可统一消费 answer-return channel；`Task` 之外的普通 continuation 调用点也不再需要特殊解释。
  - runtime 合同不再要求“先 resume，再由 task 私有代码手动解码 frame 结果槽”。
- 依赖：T4016b2

### T4016b3 [DONE] 基于统一 answer-return 通道完成 `Continuation.resume(...): Answer` 的 typecheck / lowering 主线接入
- 范围：
  - `Continuation.resume(...)` 改为真正返回表达式值的 builtin surface，不再在 typecheck / HIR / lowering 中被钉死成 `Unit` 返回。
  - 基于 `T4016c` 已提供的 runtime / ABI 返回通道，接通 expression-position `k.resume(...)` 的 lowering / codegen，并补齐对 safe-call、tuple payload surface、required effects 与 hidden `Raise<RuntimeError>` 边界的统一处理。
  - 补充 typecheck / lowering / run-pass regression，覆盖：
    - arm 内 `k.resume(...)` 后继续执行本地代码；
    - `k.resume(...)` 结果参与表达式求值；
    - nested handle / `finally` / early return；
    - resumed computation 再次 suspend 并暴露 fresh continuation。
- 验收：
  - 在语言层面，`k.resume(...)` 后续代码既能 typecheck，也能在 resumed computation 正常完成时稳定执行。
  - expression-position `k.resume(...)` 可真实观察到 delimiter answer，而不是“静态上返回值、运行时却仍是 `Unit`”。
- 依赖：T4016c

### T4016b4 [DONE] 收口 legacy `Continuation<Resume, eff E>` / `Continuation<Resume>` 兼容 shorthand，避免 answer-hole 泄漏到 codegen（拆分执行）
- 说明：
  - 此前前端曾临时接受旧 shorthand，并用内部 continuation answer-hole 补齐缺失的 answer type。
  - 也正因此，当 shorthand continuation 被存进字段/局部并跨 suspend 进入 effect frame / runtime object model 时，answer-hole 会以 `TypeKind::Param` 形式泄漏到 monomorph / LLVM codegen；最早暴露在 `continuation_escape_binder_resume_effect_row_runtime_basic.scoop`，随后又在 `continuation_resume_continuation.scoop`、`continuation_resume_enum.scoop`、`effect_escape_continuation_async_executor_fifo.scoop` 等用例上确认了同一路径的问题。
  - 当前工作树已基本完成 pure shorthand 的 lowering 清理与显式 answer type 迁移，但在复验全量 `run-pass` 时暴露出新的 runtime 前置缺陷：`gc_continuation_multi_thread_concurrent_alloc_resume.scoop` 现可成功 build，却会在 `SCOOP_GC_STRESS=1` 下于 `workerA_resuming` 后异常退出。
  - 因此本条现按 `T4016b4a -> T4016b4a0 -> T4016b4b0 -> T4016b4b` 顺序推进：先移除显式 `eff` shorthand，再补齐模块级 GC 指针全局槽的永久 roots 合同，随后修复阻断全量 `run-pass` 验收的 cross-thread continuation runtime 崩溃，最后回到 pure shorthand 的收尾验收。
- 范围：
  - 子任务全部完成后，legacy shorthand 的过渡规则要清晰：显式 `eff` shorthand 不再允许进入前端主线；仍保留的 pure shorthand 也不能再把 answer-hole 带进 codegen。
- 验收：
  - 子任务全部完成后，`cargo run -p scoop -- test --fixtures tests/fixtures/run-pass` 不再因 legacy shorthand 在任意 continuation replay / resume site 上触发 `effect frame slot type`。
- 依赖：T4016b3

### T4016b4a [DONE] 移除 legacy `Continuation<Resume, eff E>` shorthand，并收口首批 answer-hole codegen blocker
- 范围：
  - type lowering 不再接受 `Continuation<Resume, eff E>`；改为尽早报 removed/compatibility diagnostic，要求显式写成 `Continuation<Resume, Answer, eff E>`。
  - 更新首批直接受影响的 fixture / 单测到 answer-return continuation surface，重点包括：
    - `continuation_escape_binder_resume_effect_row_runtime_basic.scoop`
    - `continuation_resume_from_escape_binder_requires_step_effect.scoop`
    - LLVM `non_pure_continuation_resume_classifies_as_call_suspend_site` 单测
  - 把一批显然 answer=`Unit` 的 payload / resume fixtures 迁到显式 `Continuation<Payload, Unit>`，避免继续混用旧 shorthand 叙事。
- 验收：
  - `Continuation<Resume, eff E>` 不再进入 codegen；相关位置会在 typecheck lowering 阶段直接失败。
  - 原始 blocker `continuation_escape_binder_resume_effect_row_runtime_basic.scoop` 不再以 `effect frame slot type` 崩溃，并保持既有 stdout。
  - 已验证 `cargo run -p scoop -- build tests/fixtures/run-pass/continuation_escape_binder_resume_effect_row_runtime_basic.scoop -o /tmp/cont-shorthand.out && /tmp/cont-shorthand.out`、`cargo run -p scoop -- test --fixtures tests/fixtures/typecheck`、`cargo test --all` 与 `cargo clippy --all-targets -- -D warnings` 通过。
- 依赖：T4016b4

### T4016b4a0 [DONE] 把 object property / top-level immutable backing globals 纳入永久 GC roots，恢复显式 GC 后的模块级引用稳定性
- 范围：
  - 定位并修复模块级 GC 指针全局槽未被 GC 作为永久 roots 扫描/更新的问题，至少覆盖：
    - object property globals（`__scoop_object_prop__*`）
    - top-level immutable backing globals（`top_level_immutable_values` 对应的 module-local backing globals）
  - 明确并实现编译器 / runtime 对这类全局槽的 roots 合同：显式 GC、minor/major collection 与可能的 compaction 之后，这些槽仍必须稳定指向正确的 heap 对象，而不是悬挂地址或被后续分配复用的对象。
  - 补 runtime / run-pass regression，覆盖“对象只通过模块级全局槽保活跨 GC 仍可正确读取”的路径，避免继续把该类问题伪装成 continuation/线程专用崩溃。
- 验收：
  - `Shared.cellA` 这类 object property 在显式 GC 后仍保持对象身份与字段值稳定，不会被后续字符串/闭包等分配复用。
  - 模块级 GC 指针全局值在 GC 后不会因未扫描或未更新而悬挂；相关回归能稳定复现并锁定该合同。
- 依赖：T4016b4a

### T4016b4b0 [DONE] 修复 GC stress 下 cross-thread escaped continuation resume 的 runtime 崩溃，恢复 `T4016b4b` 的有效验收前提
- 范围：
  - 在 `T4016b4a0` 补齐模块级 GC 指针全局槽 roots 合同之后，继续定位并修复 `tests/fixtures/run-pass/gc_continuation_multi_thread_concurrent_alloc_resume.scoop` 在 `SCOOP_GC_STRESS=1` 下由 worker 线程 `resume` 已逃逸 continuation 时，于 `workerA_resuming` 后异常退出的问题。
  - 说明：`T4016b4a0` 已补齐 `__scoop_object_prop__*` / `__scoop_top_level_val__*` / `__scoop_object_instance__*` 的永久 roots/update 合同；本轮已确认 `gc_continuation_multi_thread_concurrent_alloc_resume.scoop` 在 `SCOOP_GC_STRESS=1` 下按预期 stdout 正常结束，不再在 `workerA_resuming` 后异常退出，并已把该用例恢复为真实的 stress-mode `run-pass` 回归（fixture 内显式启用 `ENV: SCOOP_GC_STRESS=1`）。
  - 核查 continuation / thread registration / GC rooting / frame liveness / cross-thread resume 合同，确保该 fixture 已能 build 的前提下，运行路径也与预期一致。
  - 补 runtime / run-pass regression，确认该类“cross-thread continuation + object allocation + GC collect”场景不再作为 `T4016b4b` 全量 `run-pass` 验收的噪声 blocker。
- 验收：
  - 已验证 `cargo run -p scoop -- build tests/fixtures/run-pass/gc_continuation_multi_thread_concurrent_alloc_resume.scoop -o /tmp/gc_continuation_multi_thread_concurrent_alloc_resume.out` 成功，且 `SCOOP_GC_STRESS=1 /tmp/gc_continuation_multi_thread_concurrent_alloc_resume.out` 按预期 stdout 正常结束，不再在 `workerA_resuming` 后异常退出。
  - 已验证隔离的 fixture runner 子集 `cargo run -p scoop -- test --fixtures <temp-dir-containing-run-pass-fixture>` 通过；相关 cross-thread continuation / GC stress run-pass 用例重新具备稳定纳入自动回归的前提，可继续推进 `T4016b4b` 的全量 `run-pass` 验收。
- 依赖：T4016b4a0

### T4016b4b [DONE] 在 `T4016b4b0` 之后完成 pure `Continuation<Resume>` shorthand 的收尾迁移与全量 run-pass 验收
- 范围：
  - 在 `T4016b4b0` 修复 runtime 崩溃后，重新盘点并复验仍可能通过 `Continuation<Resume>` 保存/转运后再 `resume(...)` 的剩余 legacy fixture/source；若仍有遗漏，继续把 answer type 显式化，或在无法保持正确语义时给出更早的 compatibility diagnostic。
  - 用全量 `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass` 收尾验证 pure shorthand 已不再沿 replay / resume path 把 continuation answer-hole 泄漏到 codegen。
  - 如全量回归仍揭示新的 pure shorthand 残留点，补对应 run-pass / codegen regression，直至 `TypeKind::Param(_)` 不再出现在 `__resume_site*` frame slot。
- 验收：
  - `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass` 全量通过。
  - legacy `Continuation<Resume>` shorthand 不再在任何 resume / replay path 上把 continuation answer-hole 泄漏进 codegen。
  - 已重新盘点仓库内剩余 `Continuation<Resume>` 文本匹配；除文档、任务记录与 removed-diagnostic 用例外，不再有会进入生产主线的 legacy pure shorthand。
  - 已复验 `cargo test --all` 与 `cargo clippy --all-targets -- -D warnings` 通过。
- 依赖：T4016b4b0

### T4016d [DONE] 让 `Task` 退化为基于 continuation answer type 的薄封装，并移除 runtime hack
- 范围：
  - task-private step driver 的 continuation answer type 要显式化：当前 `__TaskStepResult` 可继续作为内部 carrier，但它应成为 raw continuation 的显式 answer，而不是在 runtime `resume` 后由 task 私有代码回读得到的隐藏结果。
  - 当时公开 surface 里的 `Task.poll()/step()` 仍以 `Poll<T>` 作为投影层；内部 richer step result 仍保持私有，但要建立在统一 continuation answer model 上，并为后续 `T4016T1~T4016T3` 的 public renaming / surface 收口清障。
  - async/await lowering、task runtime 与 docs 叙事必须能解释为“ordinary object + private step-result continuation”，而不是“对 continuation ABI 另加一个 task-only 黑箱协定”。`T4016c` 已把 runtime resume path 改到共享 helper；本条继续收口剩余 surface / narrative 债务。
  - 不重新引入 executor / scheduler special-case；若需要 helper API，也必须是 continuation-based 的通用 helper，而不是新的 task-only runtime hack。
- 验收：
  - `Task` 可被解释为 continuation-based thin wrapper，而不再需要在设计文档里保留“runtime hack” caveat。
- 依赖：T4016b4b

### T1510c1 [DONE] 修复 `@Extern` + `enter_native` 在 moving GC 下把过期 SSA keepalive 写回 managed 局部槽位
- 范围：
  - 修复 `tests/fixtures/runtime_gc/extern_enter_native_roots_gc.scoop` 暴露的既有问题：当前 LLVM IR 已为 extern 调用点生成 `scoop_enter_native(root_slots = 1)`，但 extern body 内触发 moving GC 后，codegen 仍沿用 native 期间的 SSA `gc.relocate` 值，再在 `leave_native` 之后把它写回原局部槽位。
  - 该行为会覆盖 `native_roots` 已经原地更新过的新地址，把 pre-move/stale 指针 resurrect 回 managed frame；随后 `GC.handleNew/handleGet` 等路径会在错误地址上失败并 `exit(3)`。
  - 修复应收口在 managed-frame 的 extern-call lowering / spill-reload 合同上：native 期间依赖 `enter_native` 注册的 roots slots，返回 managed 侧后必须从真实槽位重新取值，不能继续把 extern statepoint 的旧 SSA keepalive 当成 authoritative root。
  - 同时修复同类的“先求值的 direct GC SSA 值跨后续 extern/native 子表达式继续存活”问题：call args / class ctor params 等 pointer-shaped 临时值现在会先落到受根集管理的槽位，再在 native 返回后 reload。
  - 补定向 regression，锁定 `@Extern` + `SCOOP_GC_MOVE=1` 下“roots 被更新且回 managed 后不复写旧地址”的合同，而不是只验证对象未被回收。
- 验收：
  - `cargo run -p scoop -- test --fixtures tests/fixtures/runtime_gc` 通过。
  - `tests/fixtures/runtime_gc/extern_enter_native_roots_gc.scoop` 在 `SCOOP_GC_MOVE=1` 下输出 `hello 7`，不再 `exit(3)`。
  - `tests/fixtures/runtime_gc/extern_enter_native_gc_arg_spill_reload.scoop` 在 `SCOOP_GC_MOVE=1` 下输出 `hello 7`，证明外层表达式中先求值的 direct GC SSA 也会在 native 返回后改为 reload。
  - `tests/fixtures/build/extern_enter_native_no_statepoint_writeback.scoop` 锁定 LLVM IR：extern/native 三连调用不再被 statepoint 包裹，managed 侧通过 slot reload 继续使用更新后的 roots。
- 依赖：无

### T1510c2 [DONE] 修复 runtime stackmap statepoint smoke 在 extern/native leaf lowering 后失效
- 范围：
  - `tests/fixtures/run-pass/stackmap_registry_statepoint_smoke.scoop` 当前仍假定 `@Extern("scoop_test_stackmap_statepoint_smoke")` 这个调用点会在 entry `main` 中生成真实的 statepoint / stackmap record，并在 native helper 内通过 `__builtin_return_address(0)` 命中 registry。
  - 但 `T1510c1` 已把 extern/native 三连调用改成 leaf lowering，并新增 `tests/fixtures/build/extern_enter_native_no_statepoint_writeback.scoop` 明确锁定“不再生成 statepoint”的合同；因此该 smoke fixture 现在实际输出 `-3`（registry 非空，但 caller return address lookup 失败）。
  - 需要在**不回退 `T1510c1` 合同**的前提下，恢复一个真实的 end-to-end statepoint smoke：改用保证会产出 managed safepoint / statepoint record 的调用点，或补最小 compiler/runtime 支撑，让真实产物里的 smoke 调用点 return address 仍能稳定命中 registry。
  - 同步更新相关 fixture / runtime test / 注释，明确“哪些调用点必须保留 statepoint，哪些 extern/native leaf 调用明确不应保留 statepoint”，避免两套回归继续互相打架。
- 验收：
  - `tests/fixtures/run-pass/stackmap_registry_statepoint_smoke.scoop` 已改用 `__scoop_stackmap_statepoint_smoke()`；其 lowering 走 ordinary managed runtime call，真实产物重新稳定输出 `1`。
  - 新增 `tests/fixtures/build/stackmap_registry_statepoint_smoke_managed_call.scoop`，锁定 smoke 选中的调用点仍会生成 statepoint；同时 `tests/fixtures/build/extern_enter_native_no_statepoint_writeback.scoop` 继续通过，证明没有把 `T1510c1` 回退掉。
  - 已复验 `cargo run -p scoop -- test`、`cargo test --all` 与 `cargo clippy --all-targets -- -D warnings` 通过。
- 依赖：T1510c1

### T4016R [DONE] Review：确认 continuation 已是正确的单次 delimited continuation，且 `Task` 不再依赖 runtime hack
- 重点：
  - 不允许在实现 returning-resume 后偷偷引入 multi-shot / clone / replay。
  - 不允许 `-> resume` 继续以用户态语法、隐藏 special form 或第二套 lowering contract 存活；若存在 immediate-resume fast path，也只能是 continuation primitive 的内部优化。
  - `k.resume(...)` 必须真实返回 delimiter answer type；“resume 后本地代码继续执行”的语义要通过生产代码与回归确认，而不是只停在 spec 文字。
  - `Task` 必须使用同一 continuation contract；不允许继续依赖 frame layout poking、task-private ABI 假设或“resume 返回 `Unit` 但 task 另有旁路结果”的双重叙事。
- 结论：
  - parser / AST / HIR 中 `-> resume` 只剩 removed-syntax diagnostic；生产 surface 仅保留 `Effect.op(args) -> expr` 与 `Effect.op(args), k -> expr`。
  - `Continuation.resume(...): Answer` 的静态与运行时实现统一走 continuation answer-return 通道；`Task` 仅在私有层把同一 answer channel 解释为 `__TaskStepResult`。
  - 已机械复核仓库残留文本：legacy continuation 简写只剩 removed-diagnostic fixtures 与前端报错文案，不再进入生产主线。
- 已复验：
  - `cargo run -p scoop -- test`
  - `cargo test --all`
  - `cargo clippy --all-targets -- -D warnings`
- 依赖：T1510c1, T1510c2, T4016d

## T4016T：最小化 core Task surface、把 Task 主体迁回 Scoop，并收口无锁 single-driver Task 合同（只覆盖 phase 1-3；phase 4 延期到 stdlib）

- 说明：
  - `T4016d` / `T4016R` 已完成的是 continuation answer model 收口与 task runtime hack 移除；这并不等于 core task public naming、runtime/codegen surface 与实现落点已经最终定稿。
  - 按 `SCOOP_TASK.md` 的新设计，本组只覆盖 phase 1-3：公开 surface 收口、task 主体 Scoop 化、删除 task-only runtime/codegen ABI；executor / wake / reactor / public `spawn` / `join` 的 phase 4 明确延期到 stdlib stage。
  - `T4016T3` 虽已删除 task-only runtime/codegen ABI，但 `Task` 仍保留 per-task `Mutex` 与“共享/竞争 `step()` 可由 `Pending` 吸收”的过渡合同；本组继续前插 `T4016T4 -> T4016T5 -> T4016T5a -> T4016T6 -> T4016T7 -> T4016T7a -> T4016T8 -> T4016T9 -> T4016T4R`，完整收口到无锁、轻量 claim、single-driver 的 core task 版本。
  - `T4016T1a` / `T4016T1d1` / `T4016T1d2` 已经收口了“function-type payload + generic task-state object model”主线，但继续推进 `T4016T2` 时又暴露出三个新的前置 blocker：
    1. 限定 payload enum ctor / `when` pattern 仍不完整，`TaskStep.Ready(value)` 报 `unresolved_member`，`when (step) { TaskStep.Ready(v) -> ... }` 仍 parse 失败；
    2. `emit_minimal_main_ir(...)` / single-file LLVM 测试路径没有随 `scoop build` 一起纳入 `sysroot/task.scoop` 这类可编译 sysroot 源；
    3. 普通 Scoop `Task` 若直接持有 `Mutex`，当前 sync runtime 仍只有显式 `destroy()` 合同，没有能覆盖 task 生命周期的无泄漏释放路径。
  - 因此 `T4016T2` 必须继续前插更窄的 blocker 任务，不能带着这些实现边界直接推进。

### T4016T1 [DONE] 将 core task public surface 直接收口为 `TaskStep<T>` + `step()`，并同步语言 / 运行时规格
- 范围：
  - 按 `SCOOP_TASK.md` 收口 `scoop.core` 的最小公开 task surface：保留 `Task<T>`、`TaskStep<T>`、`Task.step()` 与 `Async.await`，移除 `Poll<T>` 与 `Task.poll()`。
  - 同步 `SCOOP_FULL_SPEC.md`、`SCOOP_RUNTIME.md`、`SCOOP_TASK.md`、`sysroot/core.scoop`、相关实现注释、diagnostics 与 fixtures，把 `step()` 明确成唯一 core drive API；不保留 alias / compatibility 叙事。
  - 明确 executor / wake / reactor / public `spawn` / `join` 继续 deferred，且属于后续 stdlib stage，而不是 `scoop.core` 或本组任务的范围。
- 验收：
  - 生产 surface 与生产文档中不再暴露 `Poll<T>` / `poll()`；`TaskStep<T>` + `step()` 成为唯一 core task 命名。
  - 语言 spec、runtime spec、design doc、sysroot 与相关 fixtures 对 task public surface 形成统一叙事。
- 依赖：T4016R
- 已完成：
  - `sysroot/core.scoop` 已移除公开 `Poll<T>` / `Task.poll()`，将 core task surface 收口为 `TaskStep<T>` + `Task.step()`；相关注释同步改写为 step-only 叙事。
  - LLVM codegen 现在只对 `scoop.core.step` 做 task public surface special-case，`TaskStep<T>` 取代 `Poll<T>` 成为返回类型；`structured_concurrency_deferred` 诊断也改为只推荐 `Task.step()`。
  - `SCOOP_FULL_SPEC.md`、`SCOOP_RUNTIME.md`、`SCOOP_TASK.md`、`ISSUES.md` 与 `STDLIB_COMPLETENESS.md` 已同步到 step-only public surface；runtime 注释明确 `scoop_task_poll` 只是内部历史命名，不再代表公开 `poll()` 合同。
  - run-pass 回归已重命名为 `task_step_manual_basic.scoop`，并新增 `task_poll_removed_is_error.scoop` / `task_poll_type_removed_is_error.scoop`，锁定 `poll()` / `Poll<T>` 均已从公开 surface 移除。
- 已复验：
  - `cargo run -p scoop_tools -- spec-fixtures check`
  - `cargo run -p scoop -- test --fixtures tests/fixtures/typecheck`
  - `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass`
  - `cargo run -p scoop -- test`
  - `cargo test --all`
  - `cargo clippy --all-targets -- -D warnings`

### T4016T1a [DONE] 补齐 rich enum variant 对 function-type payload 的布局 / LLVM 主线，允许 `Created(val start: () -> ...)`
- 范围：
  - 修复当前 `TypeRef::Function` 在 enum / struct layout side table 中丢失真实 `TypeId` / 可恢复布局键的问题，确保 HIR `EnumVariantFieldLayout`、boxed payload descriptor 与 LLVM enum layout 都能恢复 function-type 字段，而不是在 `struct field type`、`enum payload (non-scalar)` 或等价旧旁路上提前失败。
  - 打通 custom rich enum 上的 function-type payload 构造、存储、`when` 解构、局部 binder 与后续 callable-value 调用主线；不得要求改用 `Option<() -> ...>`、额外 wrapper class / object，或 task-private runtime helper 作为 workaround。
  - 补 `run-pass` / `typecheck` / 必要 LLVM 单测，直接覆盖 `enum Step { Ready(val f: () -> Int) }` 与 `Task` 目标形状 `Created(val start: () -> __TaskStepResult)` 的最小 probe，锁定自定义 enum variant 直接承载 closure payload 已可执行。
- 验收：
  - LLVM 路径下，自定义 enum variant 可直接承载 function type payload，并能在 ctor + pattern binder + callable invocation 主线上工作。
  - `Task` 后续可直接用 `enum` + closure payload 表示内部 state，而不是先引入额外 wrapper / ABI 特判来绕过该既有问题。
- 依赖：T4016T1
- 已完成：
  - `hir/lower/util.rs` 现可为 `TypeRef::Function` 生成稳定的 layout `TypeId` / effect row，并在 generic struct/enum layout 收集时保留函数字段的真实类型信息，不再把它们降成 `None`。
  - `typecheck/expr/call.rs` 与 `typecheck/expr/member.rs` 现已把 value member 的函数值/`FunPtr` 调用接回统一 callable-value 主线，并把重新解析出的 member resolution 写回 side table，避免 build 阶段丢失 `receiver.f()` 的精确信息。
  - `llvm/codegen/mod.rs` 现可恢复 member access / struct ctor 结果的 concrete type，并让 struct/class 风格字段上的函数值命中统一 callable-callee 分发；不再在 `call callee`、`struct field type` 等旧旁路上失败。
  - 新增 `enum_function_payload_basic.scoop`、`task_state_function_payload_basic.scoop`、`struct_function_field_call_basic.scoop` 与对应 typecheck fixtures，分别锁定 custom enum payload、`Task` 目标形状与字段函数值调用。
- 已复验：
  - `cargo check -p scoopc --features llvm`
  - `cargo run -p scoop -- test --fixtures tests/fixtures/typecheck`
  - `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass`
  - `cargo run -p scoop -- test`
  - `cargo test --all`
  - `cargo clippy --all-targets -- -D warnings`

### T4016T1b [DONE] 禁止带 effect 的函数类型使用 `as/as?`，收口函数类型转换语义
- 范围：
  - 在 typecheck / infer / diagnostics 中，若 `as` / `as?` 的 source 或 target 含函数类型且其 effect row 非 `Pure`，则直接报错；不得再把这类语法解释成 runtime cast，也不得把它包装成“只是编译期 `static_cast` 风格提示”的特判。
  - 保留普通函数子类型 / coercion 主线作为唯一合法路径：参数逆变、返回协变、effect row widening 与后续统一的 closed-row 规则仍通过赋值/期望类型/分支 LUB 生效，而不是通过显式 cast 驱动。
  - 明确跨 runtime nominal 边界的推荐做法是 wrapper / nominal container，而不是直接对 effectful function value 做 cast；`Pure! -> Any` 的既有擦除门禁继续独立保留，不被新的 cast 规则绕开。
  - 同步 `SCOOP_FULL_SPEC.md`、必要实现注释与 fixtures，明确 effect row 不具备 runtime-checkable semantics，non-`Pure` function type 上不定义 `as/as?`。
- 验收：
  - 新增 typecheck 回归锁定 `(() -> T / E) as ...` 与 `as?` 的禁止诊断。
  - 文档与诊断不再暗示 non-`Pure` function type 可被 runtime cast。
  - 现有函数子类型 / coercion 场景保持可用，不需要通过显式 cast 才能上转到更宽 effect row。
- 依赖：T4016T1a
- 已完成：
  - `typecheck/expr/infer.rs` 的 cast 路径已新增 `check_function_type_cast_boundary`：显式 `as/as?` 不再把函数类型当成 runtime cast 目标；唯一保留特例仍是闭合纯函数值显式擦除到 `Any`。
  - 新增 `scoop::typecheck::function_type_cast_not_supported` 与 `scoop::typecheck::effectful_function_type_cast_not_supported` 两个稳定诊断，分别覆盖 “`Any -> pure function` / `function -> function` 未定义” 与 “non-`Pure` function type 不具备 runtime-checkable effect row” 两类错误。
  - `SCOOP_FULL_SPEC.md` 已同步写明：函数值的显式 runtime cast 不成立，合法路径是普通函数子类型 / coercion；若必须跨 runtime nominal 边界，应先包成 nominal wrapper。
  - 新增 `fn_type_cast_closed_pure_asq_is_error.scoop`、`fn_type_cast_effectful_as_is_error.scoop`、`fn_type_cast_effectful_asq_is_error.scoop` 与 `fn_value_as_any_closed_pure_explicit_cast_ok.scoop`，锁定 direct function cast 边界与 `Pure! -> Any` 显式擦除保留合同。
  - 先前会漏到 LLVM 并报 `unsupported_main_body: type check target type` 的 `Any as? (() -> Int / Pure!)` 路径，现已在 typecheck 阶段被稳定拒绝。
- 已复验：
  - `cargo run -p scoop -- test --fixtures tests/fixtures/typecheck`
  - `cargo run -p scoop -- test`
  - `cargo test --all`
  - `cargo run -p scoop_tools -- spec-fixtures check`
  - `cargo clippy --all-targets -- -D warnings`

### T4016T1c [DONE] 对 opaque function values 以静态 function type 的 effect row 上界决定 may-suspend 编译，补齐 wrapper/member 路径
- 范围：
  - 统一 state-machine planner、segment classification、callable-value codegen 与 concrete-type 恢复逻辑：对 opaque function values，call-site suspendability 由其静态 function type 的 effect row 上界决定；non-`Pure` row 必须按 may-suspend 编译。
  - 补齐 struct/class/enum field、member access、wrapper object、分支 LUB / higher-order 返回等会把函数值藏进 opaque 表达式的路径，确保它们与 `val f = ...; f()` 使用同一套 suspendability 规则，而不是只在局部变量路径上正确。
  - 验证同一调用点可同时兼容“静态类型为 non-`Pure`，实际值是 `Pure` closure”和“静态类型为 non-`Pure`，实际值会 `perform` 并触发 outward propagation / callee state machine”的两种情形。
  - 同步 `SCOOP_FULL_SPEC.md`、`SCOOP_RUNTIME.md` 或相关设计注释，把 opaque function values 的 call-site suspendability 规则写成显式合同。
- 完成：
  - `typecheck/expr/call.rs` 已补齐 “callee 是普通表达式且其类型为 function/FunPtr” 的调用类型推导，`choose(mode)()` 这类 higher-order 返回值直调不再被 `UnsupportedExpr { kind: "call" }` 拒绝。
  - state-machine planner / suspend analysis / LLVM callable-value codegen 已统一使用更完整的 concrete-type 恢复：`MemberAccess`、`Call`、`Block`、`If`、`When`、object property、struct/class field 上的函数值都能按静态 function type 的 effect row 决定 may-suspend。
  - `handle { wrapper.f() }` 与 `handle { choose(mode)() }` 这两类此前会漏掉的 opaque callable 调用点，现已在 `call-may-suspend` / runtime 语义上与局部函数值路径对齐。
- 已复验：
  - `cargo test -p scoopc segment_dump_classifies_ -- --nocapture`
  - `cargo test -p scoopc unified_state_machine_transforms_all_segment_kinds_from_feature_matrix -- --nocapture`
  - 新增 fixture 单独 `build + run` 的 stdout 校验
  - `cargo run -p scoop -- test --fixtures tests/fixtures/typecheck`
  - `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass`
  - `cargo run -p scoop -- test`
  - `cargo test --all`
  - `cargo run -p scoop_tools -- spec-fixtures check`
  - `cargo clippy --all-targets -- -D warnings`
- 验收：
  - `handle { wrapper.f() }` 一类 direct member callable 在 `Pure` / effectful 两种实际值下都能正确工作。
  - 新增 run-pass / state-machine dump 回归覆盖 wrapper field + direct member call + handler capture。
  - planner 与 codegen 不再对同一 callee 表达式得出相互矛盾的 suspendability 结论。
- 依赖：T4016T1b

### T4016T1R [DONE] Review：确认 function-type payload、cast 边界与 opaque callable suspendability 已进入统一 callable-value / rich-enum 主线，而不是 task-only workaround
- 范围：
  - 复审 typecheck layout、cast / infer diagnostics、HIR enum field metadata、state-machine suspendability 规划、LLVM enum ctor / `when` 提取 / binder / callable-value call 路径，确认 function type 没有再被降格成无类型 `Ref` / `Any`，也没有把 effect row 误当成 runtime-checkable metadata。
  - 复核新增回归同时覆盖 inline/boxed variant、top-level/local callable value、wrapper/member field direct call，以及 `Created(val start: () -> ...)` 这类 `T4016T2` 直接依赖的形状。
  - 确认实现不再要求“先把 `wrapper.f` 存进局部变量再调用”这类仅为绕过 planner/codegen 脱节而存在的临时写法。
- 已完成：
  - 复审过程中暴露出一个既有真实缺口：boxed multi-field enum variant 经 `val Variant(...) = expr` 解构后，若 payload 含 function type 且后续直接调用函数值，LLVM 路径会在隐藏 `Raise.raise(...)` resume-site 上把 `Any` 误落成 `Ref`，最终触发 `Ref -> Int` coercion 失败。
  - `hir/lower/patterns.rs` / `hir/lower/util.rs` 现已为 variant pattern 的隐藏 binder 恢复真实字段类型；boxed rich enum 上的函数值字段不再在解构路径中退化成 `Any`。
  - `hir/lower/expr.rs` 现已把 `synth_raise_null_assertion_failed()` 生成的隐藏 `Raise.raise(...)` HIR 类型收口为 `Nothing`，并使用零宽 span 避免与外层合成 `when` 完全重叠；ordinary callee suspend plan 不再把该 hidden raise 误建模为 `Ref` 型 resume slot。
  - 新增 `tests/fixtures/run-pass/enum_function_payload_boxed_multi_field_basic.scoop`，直接覆盖 boxed payload ctor、`when` 解构调用与 `val Variant(...) = expr` 解构调用三条路径；并同步更新 `local_val_destructuring_lowering.hir` / `safe_call_not_null_assert.hir` 的 HIR golden。
- 已复验：
  - `cargo run -p scoop -- build tests/fixtures/run-pass/enum_function_payload_boxed_multi_field_basic.scoop -o /tmp/enum_function_payload_boxed_multi_field_basic.out`
  - `/tmp/enum_function_payload_boxed_multi_field_basic.out`
  - `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass`
  - `cargo run -p scoop -- test --fixtures tests/fixtures/typecheck`
  - `cargo test -p scoopc segment_dump_classifies_ -- --nocapture`
  - `cargo run -p scoop -- test`
  - `cargo test --all`
  - `cargo clippy --all-targets -- -D warnings`
- 验收：
  - 未发现“只对 task 私有形状可用”“只有局部变量路径可用”或“换成 `Option` / wrapper / direct cast 才能过”的残余旁路；`T4016T2` 可以基于自定义 enum + closure payload 继续推进。
- 依赖：T4016T1c

### T4016T1d（拆分）ordinary Scoop generic task-state object model 的 LLVM / typecheck 缺口
- 说明：
  - 在尝试实现 `T4016T2` 时确认，这个前置问题实际包含两层不同的 blocker，不能再作为一个单一任务含混推进。
  - `T4016T1d1` 先收口“concrete-instance generic state carrier/object model 可落到 LLVM”的路径；
  - `T4016T1d2` 再收口“generic helper / method body 内的 monomorph/type-param leak”；
  - `T4016T2` 需要在这两个子任务都完成后才可继续，不能把 `driveInt(...)` / `lock.destroy()` 这种窄化验收误记成原始 `T4016T1d` 已全部完成。

### T4016T1d1 [DONE] 补齐 concrete-instance generic task-state carrier/object model 的 LLVM / typecheck 主线
- 范围：
  - 补齐 generic class instantiation 的递归类型替换，使 `Option<TaskState<T>>`、`Continuation<Any, DriverStep<T>>`、nominal type args、tuple、function 与 effect row 中的类型参数都能被具体化，而不是只替换顶层 `TypeKind::Param`。
  - 补齐布局收集 / nominal lowering 对带 type args 的 layout type 恢复，让只出现在字段类型里的 `TaskState<Int>`、`DriverStep<Int>`、`Continuation<Any, DriverStep<Int>>` 也能拿到稳定 `TypeId`。
  - 打通 `Any` receiver 在 smart-cast 分支内的 generic class field late-resolution，并保证 HIR lowering / LLVM codegen 优先使用当前表达式语境中的具体 receiver 类型，而不是退回声明处擦除类型。
  - 用 ordinary Scoop 定义的 concrete instance 锁定最小可执行前提：`TaskCarrier<Int>` + `Option<TaskState<Int>>` + `Continuation<Any, DriverStep<Int>>` + `Mutex` 字段可 build/run，不退回 task-only C struct。
  - 明确 generic helper / method body 内仍会泄漏 `TypeKind::Param(T)` 的路径留给 `T4016T1d2`；本子任务只收口 concrete-instance 主线。
- 验收：
  - `tests/fixtures/run-pass/task_generic_state_object_model_basic.scoop` 在 LLVM 路径上可 build/run，证明 ordinary Scoop 定义的 generic state carrier 至少能以 concrete instance 形式落地。
  - `tests/fixtures/run-pass/smart_cast_any_member_access_generic_class_basic.scoop` 在 LLVM 路径上可 build/run，证明 `Any` receiver 的 smart-cast + generic class field access 不再被 resolver/typecheck/codegen 任何一环提前擦除或拒绝。
  - 已复验 `cargo run -p scoop -- test`、`cargo test --all` 与 `cargo clippy --all-targets -- -D warnings` 通过。
- 依赖：T4016T1R
- 已完成：
  - `crates/scoopc/src/hir/lower/util.rs` 已把 generic class instantiation 的类型替换升级为递归替换，并补齐带 `type_kinds` 的 layout type lowering / nominal interning。
  - `crates/scoopc/src/resolve/scopes.rs` 已允许 `Any` receiver 的成员访问延后到 typecheck；`crates/scoopc/src/llvm/codegen/mod.rs` 现会优先使用 smart-cast 后的 concrete receiver type 做 member access codegen。
  - `crates/scoopc/src/hir/lower/expr.rs` 已让 typecheck side-table 的类型回填重新套用 active type param bindings，并让局部 `VarRef` lowering 优先读取表达式位点的 typechecked type。
  - 已新增 `tests/fixtures/run-pass/task_generic_state_object_model_basic.scoop` 与 `tests/fixtures/run-pass/smart_cast_any_member_access_generic_class_basic.scoop` 两个回归，分别锁定 concrete-instance state carrier/object model 与 `Any` smart-cast generic member access。
- 已复验：
  - `cargo run -p scoop -- build tests/fixtures/run-pass/task_generic_state_object_model_basic.scoop -o /tmp/task_generic_state_object_model_basic.out`
  - `/tmp/task_generic_state_object_model_basic.out`
  - `cargo run -p scoop -- build tests/fixtures/run-pass/smart_cast_any_member_access_generic_class_basic.scoop -o /tmp/smart_cast_any_member_access_generic_class_basic.out`
  - `/tmp/smart_cast_any_member_access_generic_class_basic.out`
  - `cargo run -p scoop -- test`
  - `cargo test --all`
  - `cargo clippy --all-targets -- -D warnings`

### T4016T1d2 [DONE] 补齐 generic helper / method body 内的 monomorph/type-param leak
- 范围：
  - 打通 generic state carrier 在 generic helper / method body 内的主线，不再让 `fun <T> drive(carrier: TaskCarrier<T>, fallback: T): T` 这类路径在 LLVM codegen 中泄漏 `TypeKind::Param(T)`。
  - 修复 generic smart-cast / member access 在 type param 语境下的具体化：例如 `if (x is Box<T>) x.value`、generic wrapper field 读取，以及 helper/method 体内的 generic class field 访问。
  - 修复 generic field receiver 上的方法调用具体化，例如 `carrier.lock.destroy()` 这类“字段本身已是 concrete nominal，但通过 generic receiver 访问时仍退化”的路径。
  - 在上述路径打通后，把当前为 `T4016T1d1` 窄化验收而使用的 `driveInt(...)` / 直接 `lock.destroy()` 之类形状升级回真正的 generic helper/method regression，不保留 task-private 或 fixture-only workaround。
- 验收：
  - generic helper / method body 的最小 probe 可在 LLVM 路径上 build/run，不再报 `cg_ty_of: TypeKind::Param(T) encountered in codegen (monomorph miss)`、`unsupported_main_body: class field type` 或同类 monomorph 缺口。
  - `fun <T> drive(...)`、`if (x is Box<T>) x.value` 与 `carrier.lock.destroy()` 这三类路径均具备稳定 regression。
  - `T4016T2` 不再被 generic helper / method body 的 type-param 泄漏阻塞。
- 依赖：T4016T1d1
- 已完成：
  - `crates/scoopc/src/hir/lower/mod.rs` 的 `LoweringInputs` 现可携带 `typecheck_types`；`lower_fun_with_type_bindings`、`lower_member_fun_with_type_bindings` 与 `lower_value_property_getter_with_type_bindings` 会在单态化重 lowering 时复用原始 typecheck side table，再叠加当前的 type-param 绑定。
  - `crates/scoopc/src/hir/lower/util.rs` 的 generic fun/member instantiation 主线现会在 compilation-unit lowering 路径上传递 `Some(typecheck_types)`，而 `monomorph/lower.rs` 与 `cone/pre_specialize.rs` 这类 dump / 预专门化路径继续显式传 `None`，避免把无 typecheck 的调试入口与正式编译主线混在一起。
  - `crates/scoopc/src/llvm/codegen/mod.rs` 的 `sync.destroy` receiver 类型恢复已切到统一的 `resolve_expr_concrete_type(...)` 主线，`carrier.lock.destroy()` 这类 generic receiver 字段上的 concrete nominal 方法调用不再被旧的 local-var-only 逻辑卡住。
  - 新增 `tests/fixtures/run-pass/task_generic_state_generic_helper_method_basic.scoop`，在一个最小回归里同时锁定 `fun <T> drive(...)`、generic method body 中的 `if (x is Box<T>) x.value`，以及 `carrier.lock.destroy()`。
- 已复验：
  - `cargo fmt --check`
  - `cargo run -p scoop -- build tests/fixtures/run-pass/task_generic_state_generic_helper_method_basic.scoop -o /tmp/task_generic_state_generic_helper_method_basic.out`
  - `/tmp/task_generic_state_generic_helper_method_basic.out`
  - `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass`
  - `cargo run -p scoop -- test`
  - `cargo test --all`
  - `cargo clippy --all-targets -- -D warnings`

### T4016T1d3 [DONE] 补齐限定 enum variant ctor / `when` pattern 主线，避免 ordinary Scoop task 依赖未定义解析旁路
- 范围：
  - 让表达式位置的 `Enum.Variant(...)` 能稳定走 enum variant ctor 主线，而不是只支持 unqualified `Variant(...)` 或 unit variant 的 `Enum.Variant` 值。
  - 让 `when` variant pattern 与现有 `val Result.Ok(v) = r` 解构能力对齐，支持 `Enum.Variant(...)` / `Enum.Variant` 的限定写法，而不是在 parser 阶段就报“期望 `->`，但遇到 `.`”。
  - 补 cross-file / sysroot regression，锁定 `TaskStep.Ready(value)`、`__TaskState.TaskCompleted(value)` 与 `when (step) { TaskStep.Ready(v) -> ... }` 这类 `T4016T2` 直接依赖的形状可用。
- 验收：
  - `cargo run -p scoop -- build tests/fixtures/run-pass/task_step_manual_basic.scoop -o /tmp/task_step_manual_basic.out` 不再因 `scoop.core.TaskStep.Ready` unresolved_member 失败。
  - 限定 enum variant ctor / `when` pattern 有独立 parser / typecheck / run-pass regression，不再只能靠 unqualified 写法绕过。
- 依赖：T4016T1d2
- 已完成：
  - `crates/scoopc/src/ast/mod.rs` 的 `when` variant pattern 现已记录完整 `TypePath`；`crates/scoopc/src/parser/expr.rs` 已支持 `Enum.Variant(...)` 与 `Enum.Variant` 的限定写法，不再在 parser 阶段把 `.` 误判成 arm 分隔前的非法 token。
  - `crates/scoopc/src/resolve/mod.rs` 现会把所有 enum variants 注入 value namespace；`crates/scoopc/src/typecheck/expr/call.rs` / `infer.rs` / `llvm/codegen/mod.rs` 已把 `Enum.Variant(...)` 接回统一 enum variant ctor 主线，而不是只支持 unqualified ctor 或 0-arg `Enum.Variant` 值。
  - `crates/scoopc/src/typecheck/when_pat.rs` 与 `val_pat.rs` 现按“限定名前缀解析到的 enum FQN”做匹配，允许 `TaskStep.Ready(...)` 这类 generic enum 在省略 type args 的前缀写法下稳定通过，不再误报 `type_arity_mismatch`。
  - `tests/fixtures/run-pass/task_step_manual_basic.scoop` 已改为使用 `TaskStep.Pending` / `TaskStep.Ready(value)` 的 sysroot 限定写法；新增多文件回归 `tests/fixtures/typecheck_multi/qualified_enum_variant_ctor_when_pattern_cross_file`，锁定 cross-file qualified ctor / `when` pattern。
- 已复验：
  - `cargo test -p scoopc parse_when_qualified_variant_patterns -- --nocapture`
  - `cargo run -p scoop -- test --fixtures tests/fixtures/typecheck_multi/qualified_enum_variant_ctor_when_pattern_cross_file`
  - `cargo run -p scoop -- build /tmp/qualified_variant_expr_only.scoop -o /tmp/qualified_variant_expr_only.out`
  - `cargo run -p scoop -- build /tmp/qualified_variant_ctor.scoop -o /tmp/qualified_variant_ctor.out`
  - `/tmp/qualified_variant_ctor.out`
  - `cargo run -p scoop -- test`
  - `cargo test --all`
  - `cargo clippy --all-targets -- -D warnings`

### T4016T1d4 [DONE] 让 single-file / minimal LLVM IR 路径纳入可编译 sysroot 源，与 `scoop build` 保持一致
- 范围：
  - 修复 `emit_minimal_main_ir(...)`、`build_minimal_main_module(...)`、相关 test helper 与 source-map/fun-index 组装路径，使其像 `scoop build` 一样把 `stdlib/*.scoop` 与 `sysroot/task.scoop` 这类可编译 sysroot 源纳入完整前端 / lowering / codegen 主线。
  - 确保 async/task 相关 LLVM 单测看到的是 ordinary Scoop helper 函数体，而不是只看得到 sysroot 签名、最终在 codegen 阶段落成 `UnsupportedMainBody { kind: "call callee type" }`。
  - 新增或修正定向测试，锁定 single-file/minimal IR 路径与 build pipeline 不再分叉。
- 验收：
  - `cargo test -p scoopc --features llvm async_task_ir_uses_task_create_and_internal_step_result_helpers -- --nocapture` 通过。
  - `emit_minimal_main_ir(...)` 对 async/task helper 与 `String.substring(...)` 这类 support source 的行为与 `scoop build` 保持一致，不再遗漏 `stdlib` / 可编译 sysroot source。
- 依赖：T4016T1d3
- 已完成：
  - 新增 `crates/scoopc/src/llvm/frontend.rs`，为单文件 LLVM 路径补齐 build 同款的 parse / resolve / typecheck / monomorph key 收集 / `lower_for_compilation_unit_multi_files_with_type_env(...)` 组装。
  - `emit_minimal_main_ir(...)` / `build_minimal_main_module(...)` 不再直接走 `hir::lower_for_dump(...)`；现在会把 `stdlib/*.scoop`、`session.sysroot().compilable_source_paths` 与入口文件一起纳入完整前端与 source map。
  - `scoopc --emit-llvm tests/fixtures/run-pass/async_await_minimal_int_basic.scoop` 与 `scoopc --emit-llvm tests/fixtures/run-pass/stdlib_string_basic.scoop` 已从此前的 `unsupported_main_body` / `unresolved_member` 恢复为稳定产出 LLVM IR。
  - 新增 LLVM 单测 `single_file_minimal_ir_supports_handled_async_await` 与 `single_file_minimal_ir_includes_compilable_sysroot_string_helpers`，并把既有 `@CLayout` / `@Extern` IR 单测源码调整为与 build 路径一致的合法输入，避免继续依赖旧的最小调试旁路。
- 已复验：
  - `cargo fmt`
  - `cargo test -p scoopc --features llvm async_task_ir_uses_task_create_and_internal_step_result_helpers -- --nocapture`
  - `cargo test -p scoopc --features llvm single_file_minimal_ir_supports_handled_async_await -- --nocapture`
  - `cargo test -p scoopc --features llvm single_file_minimal_ir_includes_compilable_sysroot_string_helpers -- --nocapture`
  - `cargo run -p scoopc --features llvm -- --emit-llvm tests/fixtures/run-pass/async_await_minimal_int_basic.scoop -o /tmp/async_await_minimal_int_basic.ll`
  - `cargo run -p scoopc --features llvm -- --emit-llvm tests/fixtures/run-pass/stdlib_string_basic.scoop -o /tmp/stdlib_string_basic.ll`
  - `cargo test -p scoopc --features llvm`
  - `cargo run -p scoop -- test`
  - `cargo test --all`
  - `cargo clippy --all-targets -- -D warnings`

### T4016T1d5 [DONE] 为 ordinary Scoop task 持有的 sync 资源补齐无泄漏释放合同
- 已完成：
  - `runtime/c/scoop_sync.c` 现已把 `Mutex` / `CondVar` / `Once` 统一切到 `scoop_alloc_typed(...)` + `ScoopTypeDescriptor.release_fn`；显式 `destroy()` 与 sweep cleanup 共享同一组内部 helper，不再只靠“用户必须手动 destroy”维持平台资源释放。
  - `Mutex` / `CondVar` 新增 `initialized` 防护，`Once` 新增初始化 flag，避免 typed allocation 失败或 sweep 路径对未完成初始化的底层平台对象做误销毁。
  - 新增 sync destroy 测试计数器导出与 allowlist：`scoop_test_sync_destroy_counts_reset`、`scoop_test_sync_mutex_destroy_count`、`scoop_test_sync_condvar_destroy_count`、`scoop_test_sync_once_destroy_count`。
  - `sysroot/sync.scoop` 注释已同步到新合同：显式 `destroy()` 仍是提前释放路径，但未显式释放的 sync 对象会在不可达后由 runtime sweep 做受限 cleanup。
  - 新增 run-pass 回归 `tests/fixtures/run-pass/sync_gc_release_task_like_object_basic.scoop`，用 ordinary Scoop task-like object 直接锁定：
    - `Mutex` / `CondVar` / `Once` 作为普通字段持有时，外层对象丢弃并 GC 后会释放资源；
    - 显式 `destroy()` 仍可提前释放；
    - sweep 不会 double-destroy。
- 已复验：
  - `cargo fmt`
  - `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass`
  - `cargo run -p scoop -- test`
  - `cargo test --all`
  - `cargo clippy --all-targets -- -D warnings`
- 依赖：T4016T1d4

### T4016T2 [DONE] 将 task 内部 driver / state / sync 主体迁回 Scoop，并把 async lowering 改写到普通 helper target
- 范围：
  - 依照 `SCOOP_TASK.md` 把 task-private driver result、task state 与 `Task.step()` 主体实现迁到 Scoop 代码；`__TaskStepResult` / `__TaskState` / 内部 helper 名称可调整，但必须是普通 Scoop 定义而不是新的 C runtime 语义节点。
  - `async { ... }` / `async fun` / `await` 的 lowering 继续只保留语言特有 sugar 责任，但其落点要改成 ordinary Scoop helper / internal core definitions，不再依赖 `__scoop_task_*` runtime intrinsics 作为语义宿主。
  - 为跨线程 drive/resume 定义最小同步合同：每个 task 至少有独占 drive attempt 的同步机制；优先复用 generic `Mutex` / sync runtime，而不是引入 task-only lock ABI。
  - 同步 `SCOOP_FULL_SPEC.md`、`SCOOP_RUNTIME.md`、`SCOOP_TASK.md`、`sysroot/core.scoop` 与必要实现注释，明确“task 主体在 Scoop 中、continuation / GC / thread / sync 在 runtime 中”的分层。
- 完成说明：
  - `sysroot/task.scoop` 已承载 ordinary Scoop task driver/state/sync 主体：`__task_create`、`__task_step_ready`、`__task_step_pending`、`__task_from_result`、`Task.step()`、`__task_join()`、`__task_drive_created()`、`__task_drive_waiting()` 与 `__task_apply_step()`。
  - async sugar lowering 与 single-file/full build LLVM 路径已统一落到 `scoop.core.__task_*` helper；`crates/scoopc/src/llvm/mod.rs` 与 effect state-machine 回归现已锁定 ordinary helper 路径不再直接依赖 legacy `scoop_task_*` runtime ABI。
  - `SCOOP_RUNTIME.md` 已改写为当前的 task stepping layer contract：runtime 只保留 continuation / GC / thread / sync substrate；legacy `scoop_task_*` ABI 仅作为 `T4016T3` 待删实现债务。
  - 为收口真实路径，本轮同时修复了三个被全量验收暴露的前置缺口：
    - 跨文件成员 mutability 查询，恢复 `sysroot/task.scoop` 对 `Task.__state` 的写入 typecheck；
    - monomorphized `scoop.core.__task_drive_waiting::<T>` 的 `Continuation.resume(...)` resume-slot rewrite，修复 async runtime segfault；
    - bare enum variant ctor 候选过滤，避免跨包源码因 `scoop.core.__TaskState.Created` / `__TaskStepResult.Ready` 等 internal helper variant 污染 `Created(...)` / `Ready(...)` 的裸名字解析。
- 验收：
  - 大部分 task state / step-driving 逻辑已以普通 Scoop 代码存在并可测试，不再主要驻留于 `runtime/c/scoop_task.c`。
  - async lowering 只剩 sugar / private glue，不再把 task-only runtime helper 当作语言主线的一部分。
  - 文档已说明跨线程 `step()` / resume 的最小同步合同与 GC/rooting 约束。
  - 已复验：
    - `cargo fmt --check`
    - `cargo run -q -p scoop_tools -- spec-fixtures check`
    - `cargo run -q -p scoop -- test --fixtures tests/fixtures/mir`（`fixtures: ok (6)`）
    - `cargo run -q -p scoop -- test --fixtures tests/fixtures/typecheck`（`fixtures: ok (379)`）
    - `cargo run -q -p scoop -- test --fixtures tests/fixtures/run-pass`（`fixtures: ok (388)`）
    - `cargo run -q -p scoop -- test`（`fixtures: ok (1160)`）
    - `cargo test --all -q`
    - `cargo clippy --all-targets -- -D warnings`
- 依赖：T4016T1d5

### T4016T3 [DONE] 删除 task-only runtime / codegen ABI，并把最终合同收口为 generic continuation + sync substrate
- 范围：
  - 删除 `scoop_task_create`、`scoop_task_poll`、`scoop_task_step_ready`、`scoop_task_step_pending`、`scoop_task_from_result`、`scoop_task_join` 及其 LLVM codegen special-case，移除 `runtime/c/scoop_task.c`。
  - 若 core 仍需要 internal helper 名称，它们必须是 ordinary Scoop definitions，或者 generic continuation / sync runtime 的普通调用；不得再保留 task-only C ABI / intrinsic 分支。
  - 重写 `SCOOP_RUNTIME.md` 中当前 task polling runtime contract，使其不再把 `scoop_task_*` 当作稳定 / 现行的 core task 合同；`SCOOP_FULL_SPEC.md`、`SCOOP_TASK.md`、`sysroot/core.scoop` 与实现注释同步到最终形态。
  - 明确 executor / wake / reactor / public `spawn` / `join` 的 phase 4 完全延期到 stdlib stage，本组在此收口，不继续扩张 `scoop.core`。
- 验收：
  - 生产代码路径中不再存在 task-only runtime ABI 或 task-only LLVM codegen special case。
  - `Task` 只依赖 generic continuation、GC、thread 与 sync runtime substrate；task 主体逻辑留在 Scoop。
  - 语言 spec、runtime spec、design doc 与 sysroot 对 core task 最终合同完全一致。
- 依赖：T4016T2
- 已完成：
  - 删除 `runtime/c/scoop_task.c`、`runtime/c/scoop_runtime_api.h` 中对应 allowlist 项，以及 `crates/scoop_runtime/build.rs` 里的编译入口；task-only C ABI 已从 runtime 构建产物中移除。
  - 删除 `sysroot/core.scoop` 中 legacy `__scoop_task_*` 声明；`Task.step()`、`__task_create()`、`__task_step_ready()`、`__task_step_pending()`、`__task_from_result()`、`__task_join()` 全部只保留 ordinary Scoop 定义。
  - 删除 LLVM codegen 中 `scoop.core.step` / `__scoop_task_*` 的 task-only runtime special-case，以及 `runtime_symbols.rs` / `runtime_abi.rs` 对 `scoop_task_*` 的声明；task transport 只剩 `__task_transport_pack()` / `__task_transport_unpack()` intrinsic。
  - 删除直接测试 `scoop_task_*` ABI 的 runtime integration test，改由编译器 IR 回归与现有 run-pass fixture 锁定最终合同；新增 `task_step_ir_uses_ordinary_scoop_definition_not_legacy_poll_abi`，补锁 `Task.step()` 不再调用 `scoop_task_poll`。
  - 同步 `SCOOP_RUNTIME.md`、`SCOOP_TASK.md`、`ISSUES.md`、`STDLIB_COMPLETENESS.md`、`sysroot/core.scoop` 与 `sysroot/task.scoop` 到“Task 仅依赖 generic continuation + sync/thread substrate”的最终叙事。
- 已复验：
  - `cargo fmt`
  - `cargo test -p scoopc --features llvm`
  - `cargo run -p scoop -- test`
  - `cargo run -p scoop_tools -- spec-fixtures check`
  - `cargo test --all`
  - `cargo clippy --all-targets -- -D warnings`

### T4016T4 [DONE] 收口 core `Task.step()` 的 single-driver / trap-on-contention 合同，并先同步设计文档
- 范围：
  - 明确公开 `Task` 合同：`Task` 不是可共享并发 drive 的 thread-safe object；结构化并发中的 task 保持树状层级，不支持“多个父 task 共享同一个子 task”这类语义。
  - 把 `Task.step()` 的公开行为收口为：同一时刻只允许一个 driver；public `step()` 观察到 `Running`、或并发 / reentrant `step()` 竞争，均视为 executor / driver 的严重错误并直接 trap，而不是返回 `Pending` 或抛出 `Raise<RuntimeError>`。
  - 明确 `Pending` 只表示 task 尚未完成且当前无法继续推进；它不再承担“另一个线程正在 drive”或“竞争失败稍后重试”的语义。
  - 先同步 `SCOOP_TASK.md` 与必要规格草案 / 实现注释到上述合同，为后续编译器 / sysroot / runtime 实装清障；完整文档扫尾留给 `T4016T9`。
- 验收：
  - 设计文档能明确回答 shared subtask、cross-thread sequential handoff、public `step()` 观察到 `Running`、以及并发 / reentrant `step()` 的合同。
  - 仓库内不再把 `Pending` 解释为 drive contention，也不再把 concurrent `step()` 误用定义成 `Raise<RuntimeError>`。
- 依赖：T4016T3
- 已完成：
  - `SCOOP_TASK.md` 已把 core `Task` 设计主线改写为 single-driver contract：明确 shared subtask / multiple parents 不属于 core、cross-thread 只允许顺序 handoff、`Pending` 只表示真实 not-ready、public `step()` 的并发 / reentrant 误用直接 trap。
  - `SCOOP_TASK.md` 的 step algorithm / synchronization design 已从“per-task mutex + contention returns Pending”改写为“exclusive drive ownership + trap-on-contention”的目标合同，同时保留当前 `Mutex` 只是 `T4016T3` checkpoint 细节的说明，为 `T4016T5~T4016T7` 的 claim-bit 实装清障。
  - `SCOOP_FULL_SPEC.md` 与 `SCOOP_RUNTIME.md` 已补最小规格草案：`Task<T>` 是 single-driver core abstraction，cross-thread 只支持顺序 handoff；`Pending` 不再承载 contention；public `step()` 观察到 `Running` / race / reentrant misuse 必须 trap，而不是 `Pending` 或 `Raise<RuntimeError>`。
  - `sysroot/core.scoop` 与 `sysroot/task.scoop` 注释已同步到同一叙事：当前 per-task `Mutex` 只是过渡实现细节，不是稳定 public contract。
- 已复验：
  - `cargo run -p scoop_tools -- spec-fixtures check`
  - `cargo run -p scoop -- test`
  - `cargo test --all`
  - `cargo clippy --all-targets -- -D warnings`

### T4016T5 [DONE] 补齐 internal atomic intrinsic 对对象字段 lvalue 的编译器主线，作为 claim-bit 实现 blocker
- 范围：
  - 补齐 `__AtomicInt` / `__atomicIntLoad` / `__atomicIntStore` / `__atomicIntCompareExchange` 对 ordinary object/class/struct field lvalue 的前端 / typecheck / lowering / LLVM codegen 支持，不再只停在局部变量或顶层 var 路径。
  - 若在 object-field atomics 路径上暴露出更基础的 addressable-lvalue / monomorph / codegen 缺口，必须在本任务内先补齐；不允许为 `Task` 单独增加 special-case 绕过主线。
  - 新增 compiler / LLVM / fixture regression，锁定 object field atomic load/store/CAS 的 ordinary Scoop 主线可用。
- 验收：
  - `Task` 可把 atomic claim bit 作为普通对象字段承载，而不需要 task-only compiler/runtime special-case。
  - internal atomic intrinsic 在对象字段 lvalue 上的行为与现有局部变量 / 顶层槽位路径一致，并进入持续回归。
- 依赖：T4016T4
- 已完成：
  - `crates/scoopc/src/llvm/codegen/mod.rs` 已把 `__atomicInt*` 的目标求址从“仅支持局部变量 / 顶层 var”升级为可递归恢复真实槽位地址的 `AddressablePlace` 主线；ordinary class 字段、nested class 字段，以及由 addressable class field 派生出的 nested struct 字段都不再先退化成 rvalue load。
  - 在沿 object-field atomic 路径继续 probing 时暴露出的更基础既有缺口也已一并修复：`crates/scoopc/src/hir/lower/util.rs` 现在会把 `scoop.unsafe.__AtomicInt` / `scoop.core.UIntPtr` 这类 layout alias 恢复到稳定的 builtin `TypeId`；`crates/scoopc/src/llvm/codegen/ty.rs` 也补上了 `__AtomicInt` 的 fallback lowering 与 GC-free 判定，避免 nested struct field path 再因缺失 `TypeId` 落回 `struct field type`。
  - 新增 run-pass 回归 `tests/fixtures/run-pass/unsafe_atomic_int_field_lvalue_basic.scoop`，同一用例同时锁定 direct class field、nested class field 与 nested struct field 上的 atomic load/store/CAS 行为。
  - 新增 build LLVM 回归 `tests/fixtures/build/unsafe_atomic_int_field_lvalue_llvm.scoop`，锁定 LLVM 必须直接在 `class_field_gep` / `atomic_int_field_gep` 上发出 `load atomic`、`store atomic` 与 `cmpxchg`，而不是先把成员访问降成普通值读取。
- 已复验：
  - `cargo fmt`
  - `cargo run -p scoop -- test --fixtures tests/fixtures/build`（`fixtures: ok (16)`）
  - `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass`（`fixtures: ok (389)`）
  - `cargo run -p scoop -- test`（`fixtures: ok (1162)`）
  - `cargo test --all`
  - `cargo clippy --all-targets -- -D warnings`

### T4016T5a [DONE] 修复 cross-file class ctor 参数存储沿错绑 source 解析整数文本并在 UTF-8 源文件上 panic
- 范围：
  - 修复 LLVM codegen 在 cross-file class ctor 调用里对“已求值/已类型对齐的 ctor args”继续做 source-backed integer literal 反查时，错误沿用 callee/class 的 `current_source_id` 与 declaration span 的问题。
  - 使 `SourceMap::slice` / `SourceFile::slice` 在遇到非字符边界 span 时返回正常错误而不是 panic，避免类似路径在含中文注释等 UTF-8 多字节源码上直接崩溃。
  - 补最小 regression，直接覆盖“caller 传 `0/1/...` 这类整数字面量给跨文件 class ctor，callee 源文件含非 ASCII 注释/文本”的 LLVM build/codegen 路径。
- 验收：
  - cross-file class ctor 参数本地化/属性写入路径不再依赖错绑 source 的文本切片；含 UTF-8 注释的 callee 源文件不会再因整数文本回读而 panic。
  - 已补回归锁定该场景，且 `cargo run -p scoop -- test`、`cargo test --all` 与 `cargo clippy --all-targets -- -D warnings` 通过。
- 依赖：T4016T5
- 已完成：
  - `crates/scoopc/src/llvm/codegen/gc.rs` 新增 `store_local_value_exact(...)`，供“已完成类型对齐”的值直接落槽；避免 cross-file class ctor 在 callee source 上对 caller 的整数字面量 span 再次做 source-backed 反查。
  - `crates/scoopc/src/llvm/codegen/mod.rs` 的 class ctor 参数本地化与 ctor-parameter-property 写回路径已切到上述 exact-store helper，不再沿错绑 source 的 `store_local_value(...)` 回读文本。
  - `crates/scoopc/src/source.rs` 现会在 `offset_to_line_col` / `SourceMap::slice` 路径上显式拒绝非 UTF-8 字符边界的 offset/span，避免同类 bug 直接 panic。
  - 新增 `source.rs` 单测覆盖非字符边界 offset/span；新增 LLVM 单测 `cross_file_class_ctor_literal_codegen_uses_correct_source_with_utf8_comments`，直接锁定“跨文件 class ctor + 整数字面量参数 + 中文注释”回归。
- 已复验：
  - `cargo fmt`
  - `cargo test -p scoopc --features llvm`
  - `cargo run -p scoop -- test`
  - `cargo test --all`
  - `cargo clippy --all-targets -- -D warnings`

### T4016T6 [DONE] 把 core `Task` object model 从 per-task `Mutex` 改成轻量 atomic claim field
- 范围：
  - 改写 `sysroot/core.scoop` / `sysroot/task.scoop` 中 `Task` 的内部布局：移除 `__lock: scoop.sync.Mutex`，引入最小 atomic claim / driving 字段与对应 internal helper。
  - 去掉 `Task` 创建路径上的 `mutexCreate()` / `destroy()` 依赖，保证 single-thread executor / 手动 drive 路径不再为每个 task 分配同步对象。
  - 保持 `Created/Running/Waiting/Completed` 状态机与现有 async lowering 私有 glue 兼容，但后续 drive ownership 改由轻量 claim 位表达，而不是 mutex critical section。
- 验收：
  - 新建 `Task` 不再携带 per-task `Mutex` 或等价 heavyweight sync object。
  - `Task` 的 ordinary Scoop 定义、async lowering glue 与 runtime substrate 能共同表达 atomic claim field 的最小形状。
- 依赖：T4016T5a
- 已完成：
  - `sysroot/core.scoop` 已把 `Task<T>` 的内部布局从 `__lock: scoop.sync.Mutex` 改成 `__claim: scoop.unsafe.__AtomicInt`，并同步把 task 注释改写到“atomic claim 字段 + 私有 `__TaskState<T>`”的过渡叙事。
  - `sysroot/task.scoop` 已删除 `mutexCreate()` / `lock()` / `unlock()` 依赖；新增 `__task_claim_acquire()` / `__task_claim_release()`，通过 `__atomicIntCompareExchange` / `__atomicIntStore` 承担原先的短临界区串行化。
  - `__task_create()` / `__task_from_result()` 不再为每个 task 分配同步对象；`Task.step()`、`__task_apply_step()` 与 `__task_restore_waiting()` 已切到 atomic claim helper，同时保持当前 `Running -> Pending` 的过渡可观察语义，留待 `T4016T7` 继续收口为 trap。
  - 新增 build LLVM 回归 `tests/fixtures/build/task_atomic_claim_no_mutex_llvm.scoop`，锁定 task manual-drive 主线会发出 atomic `cmpxchg` / `store atomic`，且不再引用 `scoop_sync_mutex_{create,lock,unlock,destroy}`。
  - `Task` 的 sysroot 类型表变化使 MIR 中的部分 `TypeId` 稳定编号前移；已同步更新 `tests/fixtures/mir/closure_capture_val.mir` 与 `tests/fixtures/mir/closure_capture_var.mir` 两份 golden，使全量 fixture 基线重新对齐当前实现。
- 已复验：
  - `cargo run -p scoop -- test --fixtures tests/fixtures/build`（`fixtures: ok (17)`）
  - `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass`（`fixtures: ok (389)`）
  - `cargo run -p scoop -- test`（`fixtures: ok (1163)`）
  - `cargo test --all`
  - `cargo clippy --all-targets -- -D warnings`

### T4016T7 [DONE] 用轻量 claim bit 重写 `Task.step()`，并把并发 / reentrant 误用收口为 trap
- 范围：
  - 重写 `Task.step()` 的入口 / 退出协议：以 atomic claim/release 获取 drive ownership，保留 `Created/Running/Waiting/Completed` 的 step-driving 主状态机。
  - 当 claim 失败、public `step()` 成功 claim 后又观察到 `Running`、或同一 `Task` 出现 reentrant drive 时，一律直接 trap；不得回落为 `Pending`、隐藏重试或 `Raise<RuntimeError>`。
  - 明确 release / cleanup 路径，确保正常 `Ready/Pending` 返回与 trap 前 invariant check 各自拥有一致的 claim 生命周期，不引入 claim 泄漏或 double-release。
  - 新增定向回归，覆盖 single-thread 手动 drive、cross-thread sequential handoff，以及错误的 concurrent/reentrant `step()` trap 语义。
- 验收：
  - `Pending` 只剩“尚未完成且当前无法继续推进”的语义；public `step()` 再也不会把 `Running` 作为正常可观察状态泄漏给调用方。
  - 并发 / reentrant `step()` 误用在生产路径上稳定 trap，且不污染调用链 effect row。
- 依赖：T4016T6
- 已完成：
  - `sysroot/task.scoop` 中 `__task_claim_acquire()` 已从“claim 失败后 `yield()` 重试”改成“一次 `cmpxchg` try-claim，失败直接 `exit(3)` trap”，不再把 drive ownership 竞争隐藏成阻塞式重试。
  - `Task.step()` 的 `Running -> Pending()` 过渡行为已删除；成功 claim 后若观察到 `Running`，现在直接按 single-driver misuse trap 处理。
  - `sysroot/core.scoop` / `sysroot/task.scoop` 注释已对齐到当前实现：claim 字段负责最小独占 drive ownership，claim 冲突与 reentrant drive 都不再通过 `Pending` 暴露给调用方。
  - `tests/fixtures/build/task_atomic_claim_no_mutex_llvm.scoop` 已补锁 LLVM 合同：manual-drive 路径保留 `cmpxchg` / `store atomic`，包含 `scoop_process_exit` trap 路径，且不再出现 claim 竞争自旋用的 `scoop_thread_yield`。
  - 新增 `tests/fixtures/run-pass/task_step_cross_thread_sequential_handoff_basic.scoop`、`tests/fixtures/run-pass/task_step_reentrant_trap.scoop` 与 `tests/fixtures/run-pass/task_step_concurrent_running_trap.scoop`，分别覆盖顺序跨线程 handoff、同线程重入 trap 与并发线程竞争 trap。
- 已复验：
  - `cargo run -p scoop -- build tests/fixtures/run-pass/task_step_cross_thread_sequential_handoff_basic.scoop -o /tmp/task_step_cross_thread_sequential_handoff_basic.out`
  - `/tmp/task_step_cross_thread_sequential_handoff_basic.out`
  - `cargo run -p scoop -- build tests/fixtures/run-pass/task_step_reentrant_trap.scoop -o /tmp/task_step_reentrant_trap.out`
  - `/tmp/task_step_reentrant_trap.out`
  - `cargo run -p scoop -- build tests/fixtures/run-pass/task_step_concurrent_running_trap.scoop -o /tmp/task_step_concurrent_running_trap.out`
  - `/tmp/task_step_concurrent_running_trap.out`
  - `cargo run -p scoop -- test --fixtures tests/fixtures/build`（`fixtures: ok (17)`）
  - `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass`（`fixtures: ok (392)`）
  - `cargo run -p scoop -- test`（`fixtures: ok (1166)`）
  - `cargo test --all`
  - `cargo clippy --all-targets -- -D warnings`

### T4016T7a [DONE] 修复 ordinary/statepoint 调用对含 GC refs 的 by-value aggregate 值的 safepoint 合同
- 范围：
  - 修复 ordinary/statepoint call 上“按值传递含 GC refs 的 aggregate 实参”会把未 relocate 的旧指针直接送入 callee 的编译器缺口；不得再依赖 SSA aggregate 在 safepoint 前后自动保持有效。
  - 统一 `DeferredCgValue` / effect transport box / enum boxed payload / ordinary call arg lowering 的 GC 合同，确保 tuple / struct / tagged-union enum 这类聚合值在 `SCOOP_GC_STRESS=1` 与 moving GC 下都不会丢 root、悬挂或把 stale payload 写回 heap。
  - 至少补两类回归：一个最小 ordinary-call 聚合实参 GC stress 回归；一个覆盖 `__TaskStepResult` / `TaskStep` 经由 ordinary helper 与 effect transport 传递的 task regression。
- 验收：
  - 含 GC refs 的 by-value aggregate 不再裸穿过 ordinary/statepoint call；callee 观察到的 payload 必须与 safepoint relocate 后的对象一致。
  - `task_step_manual_basic` 在 `SCOOP_GC_STRESS=1` / `SCOOP_GC_MOVE=1 SCOOP_GC_STRESS=1` 下不再因 aggregate transport 崩溃。
  - `Task` 顺序 handoff 的后续修复不再被 generic aggregate transport bug 干扰。
- 依赖：T4016T7
- 已完成：
  - `crates/scoopc/src/llvm/codegen/mod.rs` 已将 ordinary/internal 调用里“含 GC refs 的 aggregate 实参”统一降为 hidden by-ref 参数，并把 ordinary direct/vtable/itable aggregate 返回统一接入 hidden sret；`operator overload` 路径也已复用同一 ordinary-call lowering，不再保留分叉 ABI。
  - `DeferredCgValue` / spill roots / effect transport 的 GC 合同已对齐：`crates/scoopc/src/llvm/codegen/gc.rs` 只对 stack-backed spill slot 做保守 roots writeback；effect runtime 调用改走保 roots 的 call helper；object/global init helper 补上 `gc "statepoint-example"`，并修复 `gc-leaf-function` 误判。
  - runtime 侧已补齐三个既有缺口：`InNative` 线程在 native 期间会保留 enter-native 时捕获的 stack-walking ctx 以枚举更高层 managed caller frames；`yield()` 现在先做 safepoint poll；collect 入口若发现其他线程已发起 STW，会先参与 safepoint/park 而不是以 `Running` 状态傻等。
  - 新增 runtime GC 回归 `tests/fixtures/runtime_gc/gc_move_ordinary_call_struct_arg_basic.scoop` 与 `tests/fixtures/runtime_gc/task_step_manual_gc_aggregate_transport_basic.scoop`，分别覆盖 ordinary aggregate transport 与 `TaskStep`/effect transport 聚合搬运主线。
  - 在最终全量验证中还收口了一个既有测试隔离问题：`crates/scoop_runtime/tests/gc_immix_compaction.rs` 现已加入进程内 `Mutex` 串行化保护，避免同一 test binary 内并发跑两个依赖全局 GC/runtime 状态的 Immix compaction 测试时触发 STW 死锁。
- 已复验：
  - `cargo test -p scoopc async_task_resume_replay_ir_terminates_step_fn_on_active_effect --lib -- --nocapture`
  - `cargo test -p scoop_runtime --test gc_immix_compaction -- --nocapture`
  - `cargo test --all`
  - `cargo clippy --all-targets -- -D warnings`
  - `cargo run -p scoop -- test`（`fixtures: ok (1168)`）

### T4016T8 [DONE] 收口编译器 / runtime / substrate 对无锁 Task 的 handoff 与 trap 合同
- 范围：
  - 清理 async lowering、LLVM/runtime 注释与任何剩余实现分支中“contention 可能返回 `Pending`”或“Task 依赖 mutex serialization”的假设，统一到 single-driver + claim-bit contract。
  - 明确并实现 cross-thread sequential handoff 所需的 atomic / memory-order 最小合同，确认不同线程顺序 drive 同一 `Task` 时 continuation / GC / thread registration 路径仍然正确。
  - 若去锁化过程中暴露 trap helper、atomic ordering、thread/GC substrate 或 compiler plumbing 的基础缺口，必须在本任务内先行补齐；不允许以恢复 mutex 或保留旧旁路作为折中。
  - 补 compiler / runtime / run-pass / stress regression，覆盖无锁 task 在 single-thread 与 cross-thread handoff 下的主线行为。
- 验收：
  - 生产代码中不再残留“竞争失败返回 `Pending`”或“Task 依赖 per-task mutex”的旁路。
  - 无锁 `Task` 在顺序跨线程 handoff 场景下可稳定工作，runtime / compiler / sysroot 合同一致。
- 已完成：
  - `crates/scoopc/src/llvm/codegen/mod.rs` 中 `threadSpawn` / `Thread.join` / `sleepMillis` / `yield` 现统一走 `build_call_preserving_gc_local_roots(...)`，不再把 blocking/safepoint 线程调用当作普通 leaf call。
  - 修复了 `worker.join()` 期间 moving GC 不会更新 caller frame `inner` / `outer` / `worker` roots 的 compiler 缺口；`tests/fixtures/runtime_gc/task_step_cross_thread_sequential_handoff_gc_stress.scoop` 已能稳定覆盖 spawn pin、cross-thread handoff、join 后继续 collect/step 的整条路径。
  - 新增 LLVM 单测 `thread_join_statepoint_preserves_live_gc_locals`，直接锁定 `@scoop_thread_join` 的 statepoint `gc-live` 里包含 `inner / outer / worker` keepalive，且调用后把 relocated roots 写回真实局部槽位。
  - 在全量验收中还顺手收口了一个既有 fixtures runner 缺口：`crates/scoop/src/fixtures/mod.rs` 现在正确支持 `--fixtures tests/fixtures/run_pass_cone` 与 `--fixtures tests/fixtures/run_pass_cone/<case>`，不再把 cone case 名误识别成 phase 名。
- 已复验：
  - `cargo test -p scoopc --features llvm thread_join_statepoint_preserves_live_gc_locals -- --nocapture`
  - `cargo test -p scoopc --features llvm task_step_ir_uses_seqcst_atomic_claim_and_trap_without_mutex -- --nocapture`
  - `env SCOOP_GC_MOVE=1 /tmp/task_step_cross_thread_sequential_handoff_gc_stress.out`
  - `env SCOOP_GC_MOVE=1 SCOOP_GC_STRESS=1 /tmp/task_step_cross_thread_sequential_handoff_gc_stress.out`
  - `env SCOOP_GC_MOVE=1 SCOOP_GC_STRESS=1 SCOOP_GC_VERIFY_ROOTS=1 /tmp/task_step_cross_thread_sequential_handoff_gc_stress.out`
  - `cargo run -p scoop -- test --fixtures tests/fixtures/runtime_gc`（`fixtures: ok (24)`）
  - `cargo run -p scoop -- test --fixtures tests/fixtures/run_pass_cone`（`fixtures: ok (19)`）
  - `cargo run -p scoop -- test`（`fixtures: ok (1169)`）
  - `cargo test --all`
  - `cargo clippy --all-targets -- -D warnings`
- 依赖：T4016T7a

### T4016T9 [DONE] 全量同步 core Task 无锁 single-driver 合同的文档、规格与源码注释
- 范围：
  - 更新 `SCOOP_TASK.md`、`SCOOP_FULL_SPEC.md`、`SCOOP_RUNTIME.md`、`ISSUES.md`、`STDLIB_COMPLETENESS.md`、`sysroot/core.scoop`、`sysroot/task.scoop`、`sysroot/unsafe.scoop` 及相关编译器 / runtime 注释，统一去掉 per-task mutex / shared drive / contention-is-pending 旧叙事。
  - 把 core `Task` 的最终公开合同写清：轻量 claim bit、single-driver、cross-thread 只允许顺序 handoff、public `step()` 上的 `Running` 与并发 / reentrant 误用直接 trap。
  - 同步所有受影响示例、fixture 注释与实现说明，避免文档继续把旧的 mutex 设计写成现行语义。
- 验收：
  - 仓库内所有相关文档 / 注释对 core `Task` 合同形成统一叙事，不再自相矛盾。
  - 设计文档、语言规范、运行时规范与 sysroot 注释可以直接回答“为什么 `Task` 无锁且不是共享 thread-safe object”。
- 依赖：T4016T8
- 已完成：
  - 已同步 `SCOOP_TASK.md`、`SCOOP_FULL_SPEC.md`、`SCOOP_RUNTIME.md`、`ISSUES.md`、`STDLIB_COMPLETENESS.md`、`sysroot/core.scoop`、`sysroot/task.scoop`、`sysroot/unsafe.scoop`，统一到 “ordinary Scoop state + atomic claim-bit + single-driver + sequential cross-thread handoff + misuse trap” 叙事。
  - 已补齐相关实现说明：`crates/scoopc/src/llvm/codegen/mod.rs` 注释明确 task-aware lowering 仅剩 erased payload transport；`runtime/c/scoop_thread.c` 注释明确 cross-thread task handoff 只依赖通用 thread substrate，不存在 task-specific scheduler / handoff ABI。
  - 已修掉 `SCOOP_RUNTIME.md` 中一处过期的 “正在向 T4016 收口” 时态说明，避免 continuation/runtime 文档继续保留已完成任务的进行时叙事。
- 已复验：
  - `cargo run -p scoop_tools -- spec-fixtures check`
  - `cargo test --all`
  - `cargo run -p scoop -- test`（`fixtures: ok (1169)`）
  - `cargo clippy --all-targets -- -D warnings`

### T4016T4R [DONE] Review：确认 core `Task` 已收口为无锁、轻量 claim、single-driver 合同
- 重点：
  - 不允许 `Task` 继续内建 per-task `Mutex` 或其他等价 heavyweight sync object。
  - 不允许 public `step()` 再把 `Running` / contention 暴露为 `Pending`；并发 / reentrant 误用必须稳定 trap，且不引入 `Raise<RuntimeError>` 污染。
  - object-field atomic intrinsic 支持必须走 ordinary compiler/runtime 主线，而不是为 `Task` 单独开 special-case。
  - 跨线程语义只允许顺序 handoff；不允许回到“共享子 task / 多父 task / 多 driver 并发 step”旧模型。
  - `SCOOP_TASK.md`、`SCOOP_FULL_SPEC.md`、`SCOOP_RUNTIME.md`、`sysroot/*` 注释与回归必须和最终实现一致。
- 结论：
  - `sysroot/task.scoop` / `sysroot/core.scoop` 已确认 `Task<T>` 只保留私有 `__claim: scoop.unsafe.__AtomicInt` 与 `__state`；claim 失败或成功 claim 后观察到 `Running` 都直接 `exit(3)`，`Pending` 只表示真实未就绪。
  - `crates/scoopc/src/llvm/codegen/mod.rs` 已确认 task-aware lowering 只剩 erased payload transport；对象字段原子操作统一经 `scoop.unsafe.__atomicInt*` + ordinary lvalue/addressable-place 主线 lowering，不存在 task-only atomic special-case。
  - `SCOOP_TASK.md`、`SCOOP_FULL_SPEC.md`、`SCOOP_RUNTIME.md`、`ISSUES.md`、`STDLIB_COMPLETENESS.md` 与 `sysroot/*` 注释已和实现对齐：core `Task` 是 single-driver object，只允许顺序跨线程 handoff；shared subtask / multi-parent / contention-as-`Pending` 旧叙事已清理。
- 已复验：
  - `cargo test -p scoopc --features llvm task_step_ir_uses_seqcst_atomic_claim_and_trap_without_mutex`
  - `cargo run -p scoop -- test --fixtures tests/fixtures/build`（`fixtures: ok (17)`）
  - `cargo run -p scoop -- test --fixtures tests/fixtures/runtime_gc`（`fixtures: ok (24)`）
  - `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass`（`fixtures: ok (392)`）
  - `cargo run -p scoop -- test`（`fixtures: ok (1169)`）
  - `cargo run -p scoop_tools -- spec-fixtures check`
  - `cargo test --all`
  - `cargo clippy --all-targets -- -D warnings`
- 依赖：T4016T9

## T4017：将 effect / continuation 运行时从 TLS side channel 收口为显式 `EffectCtx` / `EffectOutcome`

### T4017 [TODO] 按 `CONTINUATION.md` 将 effect / continuation 内部语义、ABI 与 runtime 从 TLS side channel 迁到显式上下文 / 显式 outcome（拆分执行）
- 说明：
  - 当前实现虽已完成 one-shot deep continuation 与 `Task` 去 hack，但 effect / continuation 主语义仍拆在 state machine frame 与 TLS side channel 之间：`handler stack`、`active + perform_slot`、`callee_suspend_state`、`pending_continuation`、`continuation_resume_active` 仍分别承担传播、恢复与 replay 桥接职责。
  - `CONTINUATION.md` 已给出新的内部模型：`EffectCtx*` 表示运行时动态 effect 环境，`EffectOutcome<R>` 表示一次 eager 执行是 `Complete` 还是 `Propagate(signal)`；continuation 捕获的是 `frame + captured ctx`，而不是 resuming thread 当前 TLS 上碰巧残留的 effect 状态。
  - 本组任务不改公开语言 surface；目标是先把文档/spec、分析分层与 compiler/runtime contract 收口到这套模型，再分阶段迁出 TLS 的主语义职责。
  - 当前项目没有为了兼容而保留 effect TLS 的需求；TLS 若在最终实现中仍有残留，只能承担调试职责。
  - 固定顺序为 `T4017a -> T4017b -> T4017c -> T4017d -> T4017e1 -> T4017e2 -> T4017e3 -> T4017f -> T4017R`：先文档更新，再做 fast-path 分层、显式抽象接入、ordinary call ABI 迁移、resume-driver bookkeeping 收口、replay token 迁移、ordinary callee resume 状态迁移、剩余边界与 TLS 清理，最后 review。
- 验收：
  - effect propagation 的 source of truth 不再是 `TLS active + perform slot`；普通 effectful call、`perform`、`handle` 与 `Continuation.resume(...)` 在内部合同上都可统一解释为 `ctx + outcome`。
  - ordinary call sites 若静态证明不会 outward-effect，不再无差别支付 effect TLS 分流成本。
  - continuation capture / resume 的 authoritative state 可解释为 `frame + captured ctx (+ signal/resume token)`，而不是 resuming thread 当前 TLS。
- 依赖：T4016T4R

### T4017a [DONE] 文档更新：将 `CONTINUATION.md`、spec 与 runtime 设计文档收口到显式 `EffectCtx` / `EffectOutcome`
- 范围：
  - 更新 `CONTINUATION.md`，把当前草案补成实现导向的设计文档：明确 `EffectCtx`、`EffectSignal`、`EffectOutcome`、frame、continuation 的职责边界，以及 staged rollout 的迁移顺序。
  - 同步 `SCOOP_FULL_SPEC.md`、`SCOOP_RUNTIME.md`、`docs/effect_unified_state_machine.md` 与必要的编译器 / runtime / sysroot 注释，去掉仍把 effect propagation 写成 TLS side channel source-of-truth 的叙事，改成与新设计一致的规范与实现说明。
  - 若 `SCOOP_FULL_SPEC.md` 中受影响代码块 tagged as fixtures，需要在本任务内同步 `cargo run -p scoop_tools -- spec-fixtures sync` / `check` 的维护路径。
- 验收：
  - 仓库文档可以一致回答：为什么 `EffectCtx` 不是 frame，为什么 `EffectOutcome` 是 eager step result，为什么 continuation 捕获 `frame + ctx`，以及为什么本项目最终删除 effect TLS 的语义路径。
  - `CONTINUATION.md`、`SCOOP_FULL_SPEC.md`、`SCOOP_RUNTIME.md` 与 `docs/effect_unified_state_machine.md` 不再互相冲突。
- 完成情况：
  - 已将 `CONTINUATION.md` 收口为 `T4017` 实施基线，并把 staged rollout 细化到 `T4017a -> T4017f`。
  - 已同步更新 `SCOOP_FULL_SPEC.md`、`SCOOP_RUNTIME.md`、`docs/effect_unified_state_machine.md` 与 `runtime/c/scoop_runtime.c` 注释，明确 `EffectCtx + EffectOutcome` 才是权威语义模型，TLS 仅是过渡 transport / scratch。
  - 已验证 `cargo run -p scoop_tools -- spec-fixtures check`、`cargo test --all` 与 `cargo clippy --all-targets -- -D warnings` 通过。
- 依赖：T4017

### T4017b [DONE] 接入 `declared_effectful` / `body_may_outward_effect` / `needs_resumable_frame` 分层，只在真正可能 outward-effect 的调用点保留 TLS 检查
- 范围：
  - 将 `CONTINUATION.md` 中区分出的三层事实接入编译器主线：`declared_effectful` 只保留静态签名语义，`body_may_outward_effect` 决定 ordinary call 是否需要 effect propagation 分流，`needs_resumable_frame` 决定是否真正物化 continuation/state-machine frame。
  - 让 ordinary codegen、state-machine planner、callable-value path 与现有 `may_suspend`/`known_fun_effects` 分析统一消费 `body_may_outward_effect`，只在真正可能 outward-effect 的 call-like site 上保留 TLS propagation check。
  - 补 regression，覆盖 latent effect、未调用的 higher-order 参数、存在 `handle` 但不会触发对应 effect、以及普通 pure fast path 不再做多余 TLS 分流。
- 验收：
  - 生产代码不再在所有 ordinary 调用点后一律读取 effect TLS 决定分流。
  - 现有 effect / continuation / async 回归语义不变，同时为后续 ABI 迁移清出明确 fast-path 边界。
- 完成情况：
  - `state_machine_plan` 已把“签名 effectful”与“函数体会向外传播 effect”拆开：`known_fun_effects` fixpoint 改为追踪 `body_may_outward_effect`，closure/local function value/`handle` 路径也改为按 outward-effect 语义判断，而不是见到 non-`Pure` row 就直接视为需要传播。
  - ordinary direct call、closure/function-value call 与 `FunPtr` call 已只在 `body_may_outward_effect == true` 时发射 TLS propagation check；`vtable` / `itable` 调用暂时保持按 `declared_effectful` 保守决策，避免在 override / 动态分派目标未知时做不 sound 的去分流。
  - 已新增 regression，覆盖“签名 effectful 但 body 不 outward-effect”“未调用的 higher-order effectful 参数”“局部 `handle` 吃掉 helper effect”“真实 outward-effect 仍保留 TLS 检查”等边界。
  - 为满足 `clippy -D warnings`，callable-value / funptr 调用元数据已统一收口到 `CallableValueCallSpec`，消除新增 `call_may_suspend` 参数带来的 `too_many_arguments` lint，同时不改变调用语义。
  - 已验证 `cargo run -p scoop -- test`、`cargo test --all` 与 `cargo clippy --all-targets -- -D warnings` 通过。
- 依赖：T4017a

### T4017c [DONE] 在 compiler / runtime contract 中引入显式 `EffectCtx` / `EffectOutcome` / `EffectSignal` 抽象，并停止新增任何依赖 effect TLS 语义的路径
- 范围：
  - 在内部 IR、state-machine contract、LLVM emitter 与 runtime helper 中引入显式 `EffectCtx`、`EffectOutcome`、`EffectSignal`、`ValueTransport` 抽象，明确“完成”与“向外传播”是结果协议而不是 TLS side effect。
  - 从这一层开始，新的 compiler/runtime 主线不再新增任何依赖 effect TLS 作为 source of truth 的路径；迁移可以分批落地，但不以新旧双轨桥接为目标。
  - 统一 `perform`、`handle`、ordinary state-machine step 与 continuation resume driver 的内部叙事，避免后续再为不同路径各写一套传播/恢复合同。
- 验收：
  - 新代码路径不再把 `TLS active + perform slot` 当成唯一传播表示；显式 `ctx + outcome` 已成为 compiler/runtime 可消费的一等内部抽象。
  - 后续 ordinary call ABI 与 continuation runtime 迁移都能建立在同一套内部模型上，而不再各自发明私有旁路。
- 完成情况：
  - 已新增 `crates/scoopc/src/llvm/codegen/effect/contract.rs`，把 `ValueTransport` / `EffectSignal` / `EffectOutcome` 收口为 effect codegen 的显式 contract helper；ordinary propagation check、`perform` lowering、`Continuation.resume(...)` active fallback 与 handler dispatch 不再各自散落手写 TLS 读写。
  - `crates/scoopc/src/llvm/codegen/runtime_abi.rs` 已新增 `ScoopEffectCtx` / `ScoopValueTransport` / `ScoopEffectSignal` / `ScoopEffectOutcome` 的 LLVM 结构类型，并让 continuation/runtime comment 明确“captured handler stack top”在语义上代表 `captured EffectCtx.handler_top`。
  - `runtime/c/scoop_runtime.c` 已新增同名内部结构与 helper；runtime-originated propagate/clear path 现在通过 `ScoopEffectOutcome` helper 收口，continuation alloc/resume 也改为围绕显式 `ScoopEffectCtx` 组织 captured/restored handler context 叙事，而不是继续把 TLS 字段名当作抽象边界。
  - 已新增 LLVM 回归 `effect_contract_struct_types_are_registered_for_effect_codegen`，并同步更新受命名变化影响的 state-machine / ordinary-call LLVM 单测断言，锁定新的 contract 命名已经进入主线。
  - 已验证 `cargo fmt --check`、`cargo test -p scoopc --features llvm effect_contract_struct_types_are_registered_for_effect_codegen`、`cargo test -p scoop_runtime --test effect_tls`、`cargo test -p scoop_runtime --test continuation_one_shot continuation_double_resume_uses_shared_runtime_error_transport_contract`、`cargo run -p scoop -- test`、`cargo test --all` 与 `cargo clippy --all-targets -- -D warnings` 通过。
- 依赖：T4017b

### T4017d [DONE] 将 ordinary direct / closure / funptr effectful call 切到显式 `ctx + outcome` internal ABI
- 范围：
  - 先把 direct call、closure call、function pointer call 切到显式 `EffectCtx*` hidden arg + `EffectOutcome` 结果协议；caller 通过 outcome tag / out slots（或等价 lowering）判断 `Complete` / `Propagate`，不再依赖“call 后查 TLS active”。
  - 同步处理 payload / answer transport、resume value、ordinary callee outward propagate 与 local handle dispatch 的 lowering，保证 direct 与 higher-order 普通调用共享同一内部 ABI 语义。
  - 补 LLVM / run-pass / state-machine regression，覆盖 pure fast path、latent effect fast path、真正 outward propagate、以及 closure/funptr 上的 non-`Pure` row 调用。
- 验收：
  - 已迁移的 ordinary call 路径不再依赖 post-call TLS probing 决定 effect propagation。
  - direct / closure / funptr 调用的 effectful internal ABI 与 `CONTINUATION.md` 约定一致。
- 完成记录：
  - `crates/scoopc/src/llvm/codegen/mod.rs` 已把 ordinary direct / closure / funptr outward-effect call 统一接到显式 `EffectCtx + EffectOutcome` boundary；direct top-level call 通过新 wrapper `__scoop_effect_call_wrapper__*` 安装 caller ctx、调用 legacy body、consume 当前 outcome，再由 caller 按 outcome tag 决定继续/传播。
  - `runtime/c/scoop_runtime.c` / `runtime/c/scoop_runtime_api.h` / `crates/scoopc/src/llvm/codegen/runtime_{abi,symbols}.rs` 已补 `scoop_effect_handler_stack_top`、`scoop_effect_handler_stack_swap_top`、`scoop_effect_outcome_consume_current`、`scoop_effect_outcome_publish` 等桥接 helper，使 ordinary boundary 可在显式 outcome 与 legacy TLS transport 之间往返。
  - `crates/scoopc/src/llvm/codegen/effect/state_machine_emitter.rs` 已同步修复 `SuspendCall` fresh path：对 direct / closure / funptr ordinary call，不再重新 probing TLS active，而是读取该 site 捕获到的显式 outcome tag，并在 active 分支先 publish 回 TLS 再进入既有 suspend / dispatch 主线。
  - 已新增 runtime / LLVM / state-machine regression：`effect_outcome_roundtrip_consumes_and_republishes_current_tls_signal`、`direct_call_with_real_outward_effect_uses_wrapper_and_explicit_outcome`、`closure_call_with_real_outward_effect_uses_explicit_outcome_boundary`、`effectful_funptr_call_uses_explicit_outcome_boundary`、`direct_suspend_call_fresh_path_uses_explicit_outcome_instead_of_tls_probe`，并同步更新受旧 TLS-probing 断言影响的 state-machine IR 测试。
  - 已验证 `cargo fmt --all`、`cargo run -p scoop -- test`（`fixtures: ok (1169)`）、`cargo test --all` 与 `cargo clippy --all-targets -- -D warnings` 通过。
- 依赖：T4017c

### T4017e [TODO] 将 continuation replay、`callee_suspend_state` 与 `pending_continuation` 迁出 TLS，收口到 `frame + ctx + signal/resume token`（拆分执行）
- 说明：
  - 当前条目同时覆盖 runtime resume-driver bookkeeping、`Continuation.resume(...)` replay token、ordinary indirect callee resume 入口、以及 cross-thread / cleanup 回归，实际改动横跨 runtime、ordinary ABI、unified state machine 与测试基线，一次性实现面过大。
  - 因此拆成 `T4017e1 -> T4017e2 -> T4017e3`：先收口 resume-driver 内部 bookkeeping，再把 replay token 接入显式 outcome / state-machine，最后迁掉 ordinary callee `callee_suspend_state` 的 TLS 入口。
- 验收：
  - 子任务完成后，continuation replay、ordinary indirect callee resume 与 cross-thread resume 的 authoritative state 不再由 TLS 承担。
- 依赖：T4017d

### T4017e1 [DONE] runtime：把 `pending_continuation` 与 `continuation_resume_active` 收口到显式 resume-driver scope
- 范围：
  - `scoop_continuation_resume_common()` 引入显式 resume-driver scope / bookkeeping，对应当前一次 `Continuation.resume(...)` 驱动的动态范围。
  - `pending_continuation` 的 authoritative state 迁到该 scope，而不是 `__scoop_continuation_resume_pending_continuation` 这类裸 TLS 槽位；嵌套 resume 时要按 scope 链正确隔离。
  - `continuation_resume_active` 不再保留为独立语义 TLS 计数器；若 runtime 仍需“当前 active resume scope”指针，只能作为局部化的 driver bookkeeping。
  - 现阶段允许 `callee_suspend_state` 继续作为 bridge helper 留在 TLS，`T4017e2/e3` 再迁掉；但 `publish_pending_continuation` 不能继续依赖裸 TLS continuation 临时槽位作为 source of truth。
- 验收：
  - runtime 中不再有 `__scoop_continuation_resume_pending_continuation` / `__scoop_continuation_resume_active` 这两个原始 TLS source-of-truth 槽位。
  - 相关 runtime tests 覆盖“仅 active resume scope 可发布 pending continuation”与“resume 结束后不会把上一层 pending bookkeeping resurrect 回外层 scope”。
- 完成记录：
  - `runtime/c/scoop_runtime.c` 已引入 `ScoopContinuationResumeScope`，并删除 `__scoop_continuation_resume_pending_continuation` / `__scoop_continuation_resume_active` 两个原始 TLS 槽位；当前线程只保留一个 active resume-scope 指针作为局部 bookkeeping。
  - `scoop_continuation_resume_publish_pending_continuation()` 现在只向当前 active scope 写入 pending continuation；`scoop_continuation_resume_common()` 通过 scope 链隔离 nested resume。
  - `crates/scoop_runtime/tests/continuation_one_shot.rs` 已新增 `continuation_publish_pending_continuation_is_scoped_to_active_resume_driver`，锁定“scope 外 publish 为 no-op，scope 内 publish 会被包装成 replay-state，而不是泄漏 raw continuation 指针”。
  - 已验证 `cargo test --all`、`cargo run -p scoop -- test`（`fixtures: ok (1169)`）与 `cargo clippy --all-targets -- -D warnings` 通过。
- 依赖：T4017e

### T4017e2 [TODO] 将 `Continuation.resume(...)` replay token 接入显式 outcome / state-machine，不再通过 TLS replay-state 取回 inner continuation
- 范围：
  - `Continuation.resume(...)` 的 fresh / replay path 改为显式消费 `EffectOutcome` / `EffectSignal.resume_token`；outer replay 不再把 inner continuation 包成 TLS replay-state 再让 codegen 取回。
  - unified state machine suspend/replay、answer-return path 与 `Continuation.resume` builtin 统一走显式 replay token。
  - 补 LLVM / run-pass 回归，锁定 replay path 不再通过 `scoop_callee_suspend_state_get()` 读取 continuation-resume replay-state。
- 验收：
  - `Continuation.resume(...)` replay path 的 authoritative pending continuation / replay token 不再来自 TLS callee-state 槽位。
- 依赖：T4017e1

### T4017e3 [TODO] 将 ordinary indirect callee `callee_suspend_state` 迁入显式 frame / resume-token metadata，并去掉 TLS resume 入口
- 范围：
  - ordinary top-level fun / closure / relevant call-like boundary 的 callee resume 入口不再通过 `scoop_callee_suspend_state_get()` 判断 resume / fresh。
  - `callee_suspend_state` 改为显式跟随 frame / continuation / resume token 传播，cross-thread resume 只依赖 continuation 自身捕获状态。
  - 清理剩余 runtime bridge helper / 测试叙事，使 `callee_suspend_state` TLS 不再承担语义职责。
- 验收：
  - ordinary indirect callee resume 的 authoritative state 不再由 TLS 承担。
- 依赖：T4017e2

### T4017f [TODO] 补齐 vtable / itable / object init / top-level init / extern thunk 等剩余边界，并删除 effect TLS 的主语义职责
- 范围：
  - 将 vtable / itable dispatch、object init、top-level init、必要的 extern/native thunk 与其余 call-like boundary 统一迁到显式 `ctx + outcome` internal ABI；需要边界转换时，也应显式通过 thunk 完成，而不是继续保留隐式 TLS 分流。
  - 删除 effect TLS 的主语义职责：`handler stack` 迁入 `EffectCtx`，`active + perform_slot` 不再是传播 source of truth；TLS 若仍保留，只能承担调试职责。
  - 收尾清理与更新受影响的 runtime 注释、ABI 说明、helper allowlist 与测试基线，避免仓库同时保留两套互相竞争的 propagation narrative。
- 验收：
  - 仓库内剩余 effect TLS 不再承担“当前计算是否 outward-propagate”的主语义职责。
  - polymorphic / initialization / boundary paths 与 direct / closure / funptr 路径使用同一套内部 propagation contract，不再各走一套旁路。
- 依赖：T4017e3

### T4017R [TODO] Review：确认 effect / continuation 运行时已从 TLS side channel 收口为显式 `EffectCtx` / `EffectOutcome`
- 重点：
  - 不允许新实现表面上引入了 `EffectCtx` / `EffectOutcome`，但生产路径仍继续以 `TLS active + perform slot` 作为真正的 source of truth。
  - 不允许 ordinary pure / latent-effect fast path 继续无差别支付 effect TLS 检查成本。
  - 不允许 continuation capture / resume 继续依赖 resuming thread 当前 TLS 来恢复核心语义状态；若保留 TLS，只能是调试辅助。
  - `CONTINUATION.md`、`SCOOP_FULL_SPEC.md`、`SCOOP_RUNTIME.md`、`docs/effect_unified_state_machine.md`、sysroot 注释与生产实现必须对 `EffectCtx` / `EffectOutcome` / continuation capture model 保持同一叙事。
- 依赖：T4017f

## T4012：annotation markers、built-in annotations 与 `@Experimental` feature-gate marker

### T4012 [TODO] 收口 compile-time marker annotations 与 non-inline built-in annotations（拆分执行）
- 说明：
  - annotation 的方向修正为“compile-time markers only”，不再把它们推进成复杂 nominal / runtime feature。
  - 因此本组任务的目标不是“增强 annotation class 语义”，而是把 annotation surface 收口为最小、清晰、可诊断的编译期标记模型，并补齐 non-inline built-ins。
  - 同时加入内建的 `@Experimental(val feature = "...")` annotation，作为未来 experimental language features 的标准 feature-gate marker；本轮只要求把它作为 built-in annotation 加入，不要求任何具体语言特性接入。
  - `@Inline` 与 `ISSUES.md` 第 10 条的 legacy inline 清理强耦合，因此本组任务先只覆盖 marker annotations 与 non-inline built-ins，`@Inline` 明确顺延到 `T4013`。
- 验收：
  - 子任务全部完成后，`ISSUES.md` 第 9 条至少收窄到与 `@Inline` 交叉的剩余项，或被完全关闭。
- 依赖：T4017R

### T4012a [DONE] 将 annotation 收口为 compile-time markers only，并拒绝复杂 nominal 语义
- 范围：
  - 明确 annotation 不是一般 nominal type / class 能力的延伸；它们只承载编译期标记信息，不引入复杂继承、接口实现、运行时对象模型或额外控制流语义。
  - parser / resolver / typecheck / docs 要对允许的 annotation declaration 形状、参数承载方式与非法组合给出统一 contract。
  - 补充 parse / typecheck regression，覆盖合法 marker annotation、非法复杂语义组合，以及相关 diagnostics。
- 验收：
  - annotation model 的方向与文档统一，不再保留“未来要把 annotation 做成复杂 nominal feature”的错误叙事。
- 依赖：T4012
- 已完成：
  - `typecheck::annotations` 已把 annotation declaration contract 收口为 compile-time markers only：`annotation` 关键字只允许服务于 `annotation class`；annotation class 继续保持 data-only，并新增拒绝 nominal modifier、type params、effect params、`where` 子句等复杂 nominal 形态。
  - `check_type_decl_annotations` 的报错顺序已调整为先检查 annotation class 自身形状，再处理 `@Target/@Retention` 等注解 use-site，避免对非法 `annotation interface/...` 给出误导性的次级报错。
  - 已同步 `SCOOP_FULL_SPEC.md`、`sysroot/core.scoop`、`ISSUES.md` 与 parser / AST / built-in annotation 注释，把 annotation 统一叙述为 compile-time marker surface，而不是一般 nominal/runtime feature。
  - 新增定向 typecheck fixtures，覆盖 `annotation interface`、非法 nominal modifier、effect/type params、`where`、supertypes、type body，以及把 `annotation` 用在 `fun` 上的诊断。
- 已复验：
  - `cargo run -p scoop -- test --fixtures tests/fixtures/typecheck`
  - `cargo run -p scoop -- test`
  - `cargo test --all`
  - `cargo run -p scoop_tools -- spec-fixtures check`
  - `cargo clippy --all-targets -- -D warnings`

### T4012b [TODO] 补齐 non-inline built-in annotations 的编译器语义（拆分执行）
- 说明：
  - 当前条目同时混合了三类复杂度明显不同的工作：`@AllowIntrinsic` 的文件级 gate、`@Deprecated` 的 warning-on-use 合同，以及 `@Suppress` 所需的 warning-code / suppression / expression-annotation surface。
  - 现有前端虽然已具备注解 declaration / use-site 基础，但还没有“声明注解元数据沿调用路径传播 + 结构化 warning / suppression”这一整条主线；若不拆开，`@AllowIntrinsic` 这类局部 gate 很容易被 `@Deprecated/@Suppress` 的 warning 基础设施拖住。
  - 因此先拆成 `T4012b1 -> T4012b2 -> T4012b3`：先收口 `@AllowIntrinsic` 与用户态 `@Intrinsic` 门禁，再补 `@Deprecated` 的最小可测 warning 合同，最后接通 `@Suppress` 所需的 warning-code 与表达式 / 声明 / 文件级 suppression surface。
- 验收：
  - 子任务全部完成后，`ISSUES.md` 第 9 条中除 `@Inline` 外的 built-in annotation behavior 缺口收窄或关闭。
- 依赖：T4012a

### T4012b1 [DONE] 将 `@AllowIntrinsic` 收口为 file/module built-in gate，并禁止未授权的用户态 `@Intrinsic` 声明
- 范围：
  - 把 `@AllowIntrinsic` 纳入编译器硬编码识别的 built-in annotations，但其 target 只允许 file/module；错误 target 与错误参数形状要给出稳定 diagnostics。
  - `@Intrinsic` 不再默认允许出现在普通用户源码中；用户态 `@Intrinsic` 函数 / 类型声明必须先通过文件级 `@file:AllowIntrinsic` 显式开门，sysroot 继续作为默认允许来源。
  - 同步 `SCOOP_FULL_SPEC.md`、`sysroot/core.scoop`、相关注释与 fixtures，明确 `@AllowIntrinsic` 是“允许当前文件声明 intrinsic surface”的 gate，而不是普通 declaration marker。
- 验收：
  - 未标注 `@file:AllowIntrinsic` 的用户源码里，`@Intrinsic` 声明会在 typecheck 阶段被拒绝，并提示迁移到文件级 gate。
  - 标注 `@file:AllowIntrinsic` 后，用户态 `@Intrinsic` 函数 / 类型声明可通过既有的最小 intrinsic declaration checks。
  - `@AllowIntrinsic` 在非 file/module 目标或带参数时会给出稳定 diagnostics。
- 依赖：T4012b
- 已完成：
  - `BuiltinAnnotationKind` 已新增 `AllowIntrinsic`，file-level annotations 现在会显式校验它的 target 与“无参数”合同。
  - `check_file_annotations` 已把 `@file:AllowIntrinsic` 收口为当前文件的 intrinsic gate；用户源码中的 `@Intrinsic` 函数 / 类型声明若未开门，会报稳定的 `intrinsic_user_decl_requires_allow_intrinsic` 诊断。
  - `stdlib/mutable_array.scoop` 与相关 typecheck fixtures 已同步迁移到新的 gate 合同；`ISSUES.md` 第 9 条也已缩小为剩余的 `@Deprecated/@Inline/@Suppress` 语义缺口。
- 已复验：
  - `cargo run -p scoop -- test --fixtures tests/fixtures/typecheck`
  - `cargo run -p scoop -- test`
  - `cargo test --all`
  - `cargo run -p scoop_tools -- spec-fixtures check`
  - `cargo clippy --all-targets -- -D warnings`

### T4012b2 [DONE] 为 `@Deprecated` 建立最小可测的 declaration/use-site warning 合同
- 范围：
  - 将 `@Deprecated(message, replaceWith?)` 纳入 built-in annotation surface，校验参数形状、默认值与 target contract。
  - 建立最小可测的 warning plumbing：使用点（至少覆盖当前已支持的函数 / 类型 / 属性引用路径）能看到 stable deprecation warning，而不是仅在 declaration 头部静态接受注解。
  - 为跨文件 / sysroot 声明保留必要的注解元数据通路，避免 warning 只在当前文件 AST 上偶然生效。
- 验收：
  - `@Deprecated` 不再只是普通 annotation class 名字；其参数与使用点 warning 行为可被回归测试覆盖。
- 依赖：T4012b1
- 已完成：
  - `sysroot/core.scoop` 已补齐 `annotation class Deprecated(val message: String = "", val replaceWith: String = "")`，并将 built-in `@Deprecated` 的 target/参数 surface 收口到编译器检查主线。
  - `TypeEnv` 现会为类型、顶层属性/值与函数收集 deprecation 元数据；type lowering、顶层值读取、函数调用与顶层函数值引用路径已能在 use-site 发出结构化 warning。
  - `scoop build/run` 已接入 warning capture，并以 `path:line:col: warn[deprecated]: ...` 的稳定格式输出到 stderr。
  - 已新增定向 fixtures，覆盖非法 file target、第二个位置参数非法、`message` 类型不匹配，以及函数/类型/顶层属性 use-site warning 输出。
- 已复验：
  - `cargo run -p scoop -- test --fixtures tests/fixtures/typecheck`
  - `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass`
  - `cargo test --all`
  - `cargo clippy --all-targets -- -D warnings`

### T4012b3 [TODO] 为 `@Suppress` 建立 warning-code 与 expression/declaration/file suppression surface
- 范围：
  - 将 `@Suppress(warnings...)` 纳入 built-in annotation surface，校验参数形状并与 warning code 命名形成统一 contract。
  - 接通 declaration / file-level suppression；若 expression annotation 仍缺语法面或 AST 支撑，需要在本任务中一并补齐，而不是继续把 spec 示例停留在文档层。
  - 补 parse / typecheck / fixture runner regression，覆盖合法 suppression、未知 warning 名称、错误参数类型，以及 suppression 对 deprecation / lint warning 的实际效果。
- 验收：
  - `@Suppress` 不再只是被 parser 接受的普通 annotation use；其 warning suppression 行为与 spec 中的 expression/declaration/file surface 形成统一叙事。
- 依赖：T4012b2

### T4012c [TODO] 加入 built-in `@Experimental(val feature = "...")` annotation，作为保留的 feature-gate marker
- 范围：
  - 将 `@Experimental` 加入 built-in annotation surface，形状固定为带 `feature` 命名参数的 compile-time marker；默认方向是 `@Experimental(feature = "some_feature")`，并允许文档中保留 `val feature: String` 的声明叙事。
  - parser / resolver / typecheck / docs 需要统一其最小合同：它是 built-in annotation，会被编译器识别；参数形状需可校验；错误用法应有明确 diagnostics。
  - 本任务**不**要求任何具体 experimental language feature 接入该 gate；也不要求在本轮引入完整的 feature-flag framework。现阶段只建立统一语法面与 built-in 身份，供后续按 feature 名称 allow/disallow。
  - 补充 parse / typecheck regression，覆盖合法 `feature = "..."` 用法、缺少 `feature`、错误参数类型、未知位置或非法使用场景的诊断。
- 验收：
  - `@Experimental(feature = "...")` 已成为编译器识别的 built-in annotation，可作为未来 feature gate 的标准 marker。
  - 文档明确说明：当前只引入 annotation surface 与参数校验，不代表任何实验特性已经开始由它控制。
- 依赖：T4012b3

### T4012R [TODO] Review：确认 annotation system 已收口为 compile-time markers，而不是新的复杂 nominal feature
- 重点：
  - 不允许借 built-in annotation 之名重新引入复杂 nominal / runtime 语义。
  - `@Experimental(feature = "...")` 当前只能是 compile-time marker / reserved gate surface，不能偷偷演变为运行时 feature object、隐式 capability 或半成品 feature-flag framework。
  - `@Inline` 的剩余交叉项必须明确移交给 `T4013`，不能在本条 review 里含混带过。
- 依赖：T4012c

## T4013：删除 `inline` 关键字与 legacy non-local return 语义残留

### T4013 [TODO] 删除 `inline` 关键字，并把 `@Inline` 收口为唯一的内联提示 surface
- 范围：
  - 从 parser / resolver / typecheck / docs / spec / sysroot surface 移除 `inline` 关键字；原 `inline fun` 或相关语法若仍存在，统一迁移为普通声明 + `@Inline`。
  - 同时移除 inline 函数 lambda 实参中的 non-local return 语义门禁、错误文案与对应 fixture 口径，不再让任何 inline-related surface 参与控制流语义。
  - `@Inline` 若保留，则成为唯一的内联提示 surface，只作为优化提示 / compile-time marker 存在，不引入任何额外的 non-local return / break / continue 语义。
  - 若规范文字、spec fixtures 或 sysroot 注释需要同步，在本任务内显式列出相应更新、removed-syntax diagnostics 与 `spec-fixtures check` 验收。
- 验收：
  - 用户态不再依赖 `inline` 关键字；若保留兼容诊断，也必须明确指向 `@Inline` 的迁移路径。
  - `ISSUES.md` 第 10 条收窄或关闭。
  - `ISSUES.md` 第 9 条中与 `@Inline` 相关的剩余交叉项一并关闭。
- 依赖：T4012R

### T4013R [TODO] Review：确认 `inline` 已移除，且 `@Inline` 不再参与控制流语义
- 重点：
  - 不允许把旧的 non-local return 规则换个入口继续保留。
  - 不允许保留“关键字负责语法，annotation 负责语义”的双轨 inline surface；若仍存在兼容诊断，也只能作为明确的 removed-syntax 指引。
  - 若未来还要重新引入相关能力，也只能作为显式 deferred design item 留下。
- 依赖：T4013

## T4014：FFI / ABI 边界与 stable handle / pin 职责分离

### T4014 [TODO] 收口普通 `@Extern` 的 effect-impermeable 边界与 stable handle / pin 合同（拆分执行）
- 说明：
  - 最新 `ISSUES.md` 第 11 条当前可收口为两条：普通 FFI 边界仍缺少“effect / continuation / non-local control 不可穿透”的明确契约；long-lived GC object identity 也仍需要以 stable opaque handle 而不是 pin 来跨越 ABI / reactor 边界。
  - `Pinned` 在本阶段的定位应收窄为“短时裸地址借出”；它不再承担 wake token、long-lived identity 或长期注册语义。
  - 这两条既共享 FFI 边界主题，又会分别影响 typecheck contract 与 sysroot / ABI surface，因此拆分为 `T4014a -> T4014b -> T4014R`。
- 验收：
  - 子任务全部完成后，`ISSUES.md` 第 11 条收窄或关闭。
- 依赖：T4013R

### T4014a [TODO] 明确普通 `@Extern` 不能穿透 effect / continuation / non-local control
- 范围：
  - 普通 `@Extern` ABI 的 typecheck / lowering / runtime contract 明确禁止 effect、continuation 与 non-local control 穿越边界。
  - `@NoGC`、`Ptr<T>` / `UIntPtr` / stable handle 桥接与普通 FFI 约束之间的关系要形成统一叙事，而不是继续依赖隐含约定。
  - 补充 typecheck / docs / regression，覆盖违规签名、违规调用与允许的显式桥接路径。
- 验收：
  - 普通 FFI 接口不再暴露隐藏的 GC / effect 语义；边界契约在诊断与文档上都可见。
- 依赖：T4014

### T4014b [TODO] 完善 stable handle 的 FFI / reactor 合同，并把 `Pinned` 收口为短时裸地址借出
- 范围：
  - stable handle（`GcHandle` / `raw: UIntPtr`）要形成统一的 FFI / reactor / callback round-trip 合同，作为 long-lived object identity / wake token 的标准 opaque surface。
  - handle 的创建、保活、drop、stale token / cancelled registration / lookup failure 边界要在语言文档、runtime 文档与 ABI 叙事中一致；不能再让 pin 与 handle 的职责混杂。
  - `Pinned` 继续保留为“短时把 GC 对象借成稳定裸地址”的 unsafe bridge；sysroot / runtime 文档中不得再把它描述为长期 token。
  - 补充 extern signature / round-trip regression，覆盖 handle 传递、回传、drop，以及 pin/unpin 的短时地址借出边界。
- 验收：
  - `ISSUES.md` 第 11 条中关于 stable handle / pin 职责分离的剩余描述收窄或关闭。
- 依赖：T4014a

### T4014R [TODO] Review：确认普通 FFI 边界不再隐含 GC / effect 语义
- 重点：
  - 不允许普通 `@Extern` ABI 继续默许 effect / continuation 穿越。
  - stable handle 必须成为 long-lived identity / wake token 的统一合同；`Pinned` 只能停留在短时裸地址借出语义。
  - handle / pin 的边界不能只是文档口头概念；必须与 ABI surface 和类型系统叙事对齐。
- 依赖：T4014b

## T4015：const / comptime 扩展

### T4015 [TODO] 将 const/comptime 从最小纯算术子集扩到可用的纯计算模型（拆分执行）
- 说明：
  - 最新 `ISSUES.md` 第 12 条把当前限制拆成三层：`const fun` 解析仍只支持同文件 + 名字/参数个数的最小选择；常量 evaluator / interpreter 仍只覆盖很窄的纯表达式子集；header phase 仍对 effect row / `eff` 参数采取一刀切早退。
  - 这三层依赖顺序不同，因此拆分为 `T4015a -> T4015b -> T4015c -> T4015R`。
- 验收：
  - 子任务全部完成后，`ISSUES.md` 第 12 条收窄或关闭。
- 依赖：T4014R

### T4015a [TODO] 收口 `const fun` 的解析 / 选择 / 跨文件调用主线
- 范围：
  - `const fun` 解释器不再只按“同文件 + 函数名 + 参数个数”做最小选择；需要接入统一的声明处上下文、重载与跨文件解析主线。
  - `const fun` 的 call-site 选择、generic 实例化与 declaration context 要与普通函数解析保持可解释的一致性，而不是继续依赖 comptime 私有旁路。
  - 补充对应单测 / regression，覆盖跨文件 const 调用、重载选择与错误路径。
- 验收：
  - `ISSUES.md` 第 12 条中“const fun 解释器当前只支持同文件、按函数名 + 参数个数的最小选择”的部分收窄或关闭。
- 依赖：T4015

### T4015b [TODO] 扩展纯 comptime evaluator / interpreter 到控制流、局部声明与循环等常见结构
- 范围：
  - 常量 evaluator / interpreter 从“字面量 + 一元/二元运算”扩展到更完整的纯计算子集，包括控制流、局部声明与循环等常见结构。
  - 继续保持纯计算前提，不把 effectful execution 偷偷放进 comptime；必要时通过明确 diagnostics 区分“纯但未支持”和“语义上不允许”。
  - 补充 regression，覆盖条件分支、局部绑定、循环与跨函数纯计算。
- 验收：
  - `ISSUES.md` 第 12 条中“常量 evaluator 仍只覆盖很窄纯计算子集”的部分收窄或关闭。
- 依赖：T4015a

### T4015c [TODO] 重新收口 `const fun` 的 effect-row / `eff` 参数 contract
- 范围：
  - `const fun` 对 non-`Pure` effect row 与 `eff` 参数的限制不能继续停留在“一刀切早退但没有明确 contract”；需要决定并实现可支持的纯兼容子集，或把不支持部分写成显式 deferred contract 与精确诊断。
  - typecheck、文档与 comptime interpreter 的边界表述必须一致，不再出现 header phase、解释器与 spec 三处口径分裂。
  - 若本轮仍选择保守限制，必须把 `SCOOP_FULL_SPEC.md` / 相关文档的同步任务纳入验收。
- 验收：
  - `ISSUES.md` 第 12 条中关于 non-`Pure` effect row / `eff` 参数的剩余描述收窄或关闭。
- 依赖：T4015b

### T4015R [TODO] Review：确认 const/comptime 不再只靠“同文件 + 名字/参数个数 + 字面量求值”的最小旁路
- 重点：
  - 不允许在 parser/typecheck 接受更多语法后，解释器仍偷偷回退到最小选择模型。
  - comptime 的“纯计算”边界必须由统一 contract 说明，而不是散落在多个早期 gate 里。
- 依赖：T4015c
