# TODO（Scoop：正确 delimited continuation 与剩余 issue 收口）

> 生成时间：2026-04-20  
> 历史归档：`TODO-5.md` / `PLAN-5.md`  
> 顺序约束：严格按当前文件中的条目顺序推进；不得跨条目并行实现。  
> 本轮先完成正确的单次 delimited continuation / `Task` 去 hack，再回到 annotation、删除 `inline` 关键字、FFI / ABI、const / comptime。

## 全局约束

- `TODO-5.md` 中的 `[DONE]` 条目只作历史归档；新的收口工作必须在当前文件中重新立任务，不能回写归档。
- 当前剩余实现顺序为：正确的单次 delimited continuation / `Task` 去 hack -> annotation markers / `inline` -> FFI / ABI -> const / comptime。
- continuation 继续保持 **single-shot only**；multi-shot、continuation cloning、resume-many replay 明确 out-of-scope。
- 语言层面只保留 `Effect.op(args) -> expr` 与 `Effect.op(args), k -> expr` 两种 handler arm；`-> resume` 从用户态语法移除。若编译器内部仍需要 immediate-resume fast path，只能作为 lowering / codegen 优化分类。
- `Task<T>` 是 general API；raw `Continuation` 是 advanced API。`T4016` 完成后，`Task` runtime 不得再依赖“resume 后偷读 heap frame 前缀结果”的私有 hack。
- annotation 保持 **compile-time markers only**，不进入复杂 nominal / runtime 语义。
- `inline` 关键字以移除为默认方向；若后续仍需表达内联偏好，统一由 `@Inline` 作为 compile-time marker 承担，不再保留关键字与控制流语义双轨。
- 每个实现任务后必须立即做 review 任务；review 只审查生产代码与规范一致性，不以测试命名代替结论。
- 若某项实现改变公开语义，必须同步 `SCOOP_FULL_SPEC.md`；若涉及运行时合同，还要同步 `SCOOP_RUNTIME.md`、`sysroot/core.scoop` 或相关文档。
- 本轮不设计 executor framework；所有与 executor、wakeup、queueing、work-stealing、public `spawn` scheduling 相关内容一律留待后续。

## T4016：正确的单次 delimited continuation、移除 `-> resume` 语法与 `Task` 去 hack

### T4016 [TODO] 收口正确的单次（one-shot）delimited continuation，移除用户态 `-> resume` 语法，并让 `Task` 摆脱 runtime hack（拆分执行）
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

### T4016b [TODO] 把 answer type、returning-resume 与 arm syntax removal 接入前端 / 中端主线（拆分执行）
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

### T4016b4 [TODO] 收口 legacy `Continuation<Resume, eff E>` / `Continuation<Resume>` 兼容 shorthand，避免 answer-hole 泄漏到 codegen
- 说明：
  - 当前前端仍临时接受旧 shorthand，并用内部 continuation answer-hole 补齐缺失的 answer type。
  - 但当 shorthand continuation 被存进字段/局部并跨 suspend 进入 effect frame / runtime object model 时，answer-hole 可能以 `TypeKind::Param` 形式泄漏到 monomorph / LLVM codegen，当前会在 `continuation_escape_binder_resume_effect_row_runtime_basic.scoop` 上触发 `cg_ty_of: TypeKind::Param(_)` 与 `effect frame slot type`。
  - 该 run-pass fixture 同时仍沿用 returning-resume 之前的旧叙事；本条要一并把 legacy shorthand 的过渡合同与现行 answer-return 语义收口清楚。
- 范围：
  - 明确 legacy shorthand 的最终过渡规则：要么在仍支持的所有位置把 answer type 具体化，要么在无法具体化的位置尽早给出 removed/compatibility diagnostic，不允许继续拖到 codegen 崩溃。
  - 更新受影响 fixtures/source 到与当前 continuation answer model 一致的 surface，重点包括 `continuation_escape_binder_resume_effect_row_runtime_basic.scoop`。
  - 补 typecheck / codegen / run-pass regression，覆盖 legacy shorthand 的允许路径或移除诊断。
- 验收：
  - `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass` 不再在 `continuation_escape_binder_resume_effect_row_runtime_basic.scoop` 上以 `effect frame slot type` 失败。
  - legacy shorthand 不再能把 continuation answer-hole 作为 `TypeKind::Param` 泄漏进 codegen。
- 依赖：T4016b3

### T4016d [TODO] 让 `Task` 退化为基于 continuation answer type 的薄封装，并移除 runtime hack
- 范围：
  - task-private step driver 的 continuation answer type 要显式化：当前 `__TaskStepResult` 可继续作为内部 carrier，但它应成为 raw continuation 的显式 answer，而不是在 runtime `resume` 后由 task 私有代码回读得到的隐藏结果。
  - `Task.poll()/step()` 继续只暴露 `Poll<T>`；内部 richer step result 仍保持私有，但要建立在统一 continuation answer model 上。
  - async/await lowering、task runtime 与 docs 叙事必须能解释为“ordinary object + private step-result continuation”，而不是“对 continuation ABI 另加一个 task-only 黑箱协定”。`T4016c` 已把 runtime resume path 改到共享 helper；本条继续收口剩余 surface / narrative 债务。
  - 不重新引入 executor / scheduler special-case；若需要 helper API，也必须是 continuation-based 的通用 helper，而不是新的 task-only runtime hack。
- 验收：
  - `Task` 可被解释为 continuation-based thin wrapper，而不再需要在设计文档里保留“runtime hack” caveat。
- 依赖：T4016b4

### T4016R [TODO] Review：确认 continuation 已是正确的单次 delimited continuation，且 `Task` 不再依赖 runtime hack
- 重点：
  - 不允许在实现 returning-resume 后偷偷引入 multi-shot / clone / replay。
  - 不允许 `-> resume` 继续以用户态语法、隐藏 special form 或第二套 lowering contract 存活；若存在 immediate-resume fast path，也只能是 continuation primitive 的内部优化。
  - `k.resume(...)` 必须真实返回 delimiter answer type；“resume 后本地代码继续执行”的语义要通过生产代码与回归确认，而不是只停在 spec 文字。
  - `Task` 必须使用同一 continuation contract；不允许继续依赖 frame layout poking、task-private ABI 假设或“resume 返回 `Unit` 但 task 另有旁路结果”的双重叙事。
- 依赖：T4016d

## T4012：annotation markers、built-in annotations 与 `@Experimental` feature-gate marker

### T4012 [TODO] 收口 compile-time marker annotations 与 non-inline built-in annotations（拆分执行）
- 说明：
  - annotation 的方向修正为“compile-time markers only”，不再把它们推进成复杂 nominal / runtime feature。
  - 因此本组任务的目标不是“增强 annotation class 语义”，而是把 annotation surface 收口为最小、清晰、可诊断的编译期标记模型，并补齐 non-inline built-ins。
  - 同时加入内建的 `@Experimental(val feature = "...")` annotation，作为未来 experimental language features 的标准 feature-gate marker；本轮只要求把它作为 built-in annotation 加入，不要求任何具体语言特性接入。
  - `@Inline` 与 `ISSUES.md` 第 10 条的 legacy inline 清理强耦合，因此本组任务先只覆盖 marker annotations 与 non-inline built-ins，`@Inline` 明确顺延到 `T4013`。
- 验收：
  - 子任务全部完成后，`ISSUES.md` 第 9 条至少收窄到与 `@Inline` 交叉的剩余项，或被完全关闭。
- 依赖：T4016R

### T4012a [TODO] 将 annotation 收口为 compile-time markers only，并拒绝复杂 nominal 语义
- 范围：
  - 明确 annotation 不是一般 nominal type / class 能力的延伸；它们只承载编译期标记信息，不引入复杂继承、接口实现、运行时对象模型或额外控制流语义。
  - parser / resolver / typecheck / docs 要对允许的 annotation declaration 形状、参数承载方式与非法组合给出统一 contract。
  - 补充 parse / typecheck regression，覆盖合法 marker annotation、非法复杂语义组合，以及相关 diagnostics。
- 验收：
  - annotation model 的方向与文档统一，不再保留“未来要把 annotation 做成复杂 nominal feature”的错误叙事。
- 依赖：T4012

### T4012b [TODO] 补齐 non-inline built-in annotations 的编译器语义
- 范围：
  - built-in annotation 的编译器识别不再只停在 `Unsafe / Safe / NoGC / Extern / Intrinsic / CallingConvention`。
  - 先收口 `@Deprecated`、`@AllowIntrinsic`、`@Suppress` 等 non-inline built-ins 的解析、诊断与行为；`@Inline` 明确留到 `T4013`，避免再次把 inline 做成控制流语义。
  - 如公开语义或诊断文本变化涉及规范 / 文档，在本任务中显式列出同步项，而不是直接跳过。
- 验收：
  - `ISSUES.md` 第 9 条中除 `@Inline` 外的 built-in annotation behavior 缺口收窄或关闭。
- 依赖：T4012a

### T4012c [TODO] 加入 built-in `@Experimental(val feature = "...")` annotation，作为保留的 feature-gate marker
- 范围：
  - 将 `@Experimental` 加入 built-in annotation surface，形状固定为带 `feature` 命名参数的 compile-time marker；默认方向是 `@Experimental(feature = "some_feature")`，并允许文档中保留 `val feature: String` 的声明叙事。
  - parser / resolver / typecheck / docs 需要统一其最小合同：它是 built-in annotation，会被编译器识别；参数形状需可校验；错误用法应有明确 diagnostics。
  - 本任务**不**要求任何具体 experimental language feature 接入该 gate；也不要求在本轮引入完整的 feature-flag framework。现阶段只建立统一语法面与 built-in 身份，供后续按 feature 名称 allow/disallow。
  - 补充 parse / typecheck regression，覆盖合法 `feature = "..."` 用法、缺少 `feature`、错误参数类型、未知位置或非法使用场景的诊断。
- 验收：
  - `@Experimental(feature = "...")` 已成为编译器识别的 built-in annotation，可作为未来 feature gate 的标准 marker。
  - 文档明确说明：当前只引入 annotation surface 与参数校验，不代表任何实验特性已经开始由它控制。
- 依赖：T4012b

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
