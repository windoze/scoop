# Scoop：Effect Refactor 落地计划

> 生成时间：2026-05-02  
> 历史参考：`docs/archive/plans/PLAN-9.md`  
> 本轮主题：按 [`EFFECT_REFACTOR.md`](./EFFECT_REFACTOR.md) 的设计基线，把 effect / continuation 从当前的 legacy HIR-driven codegen + runtime/TLS bridge 形态，收口为“surface contract -> direct-style MIR -> complete effect facts -> late-lowered `Step` pipeline -> LLVM”的新主线。

## 0. 工作原则

- [`EFFECT_REFACTOR.md`](./EFFECT_REFACTOR.md) 是本轮唯一设计基线。若实现过程中改变主张，必须先回写该文档，再继续实现。
- 本轮严格按 AST -> HIR -> MIR -> late lowering -> LLVM codegen 的顺序推进，不跳阶段。
- 本轮在对接最终 LLVM codegen 之前，不做全量回归。
  - P0-P5 只做阶段内的定向验证；
  - full regression 只在 P7 主线切换后与 P8 清理后执行。
- 本轮在 P0-P6 不允许“在旧主线上打补丁式推进”。
  - 必须先建立新的并行路径；
  - 旧主线在 P7 之前继续保持默认与稳定；
  - 新行为通过显式 dispatcher / pipeline mode / 并行模块逐步接入。
- 对于新旧路线都需要用到的代码，只允许两种组织方式：
  - 抽成独立模块，并提供**单一 API**同时供两边消费；该 API 中禁止包含“新旧线路标志”，模块自身也必须在**完全不了解自己是被哪条线调用**的前提下正常工作；
  - 若上述条件无法满足，则必须将旧线路上的相关代码完整复制到新路线上来，确保两条线路逻辑上完全独立。
- 绝对禁止将两条线路的业务逻辑混在一起。
  - 不允许在同一个业务模块/同一个实现函数里通过 `if new_pipeline { ... } else { ... }`、`PipelineMode` 开关、或等价标志把两条业务逻辑塞在一起；
  - 允许共享的只有“对两条线路都真正中立”的基础设施模块，而不是带线路分叉的半共享业务逻辑。
- 新旧主线的并存期必须通过 `scoop` / `scoopc` 的显式命令行参数暴露出来。
  - 新主线在 P0-P6 期间不能只靠内部开关或测试专用入口激活；
  - `scoop` 与 `scoopc` 都必须把该选择收口到同一个 session/pipeline config bit；
  - P7 切主线前，新路径的所有端到端验证都应通过该 CLI 参数进入；
  - P7 切主线后，旧路径可短暂保留为显式 legacy flag，直到 P8 删除旧主线。
- 每个阶段都必须向下一阶段输出**语义上闭包**的信息包，严格遵守 [`EFFECT_REFACTOR.md`](./EFFECT_REFACTOR.md) 顶部的“闭包原则”。
  - 下一阶段只能依赖：本阶段显式输入、上阶段显式产出的 facts/schema/table、以及外部输入（target ABI、opt level、feature flags）；
  - 不允许为补齐语义回看 HIR / AST / 旧 pass 私有缓存。
- 若某阶段同时存在“per-op/per-case 的 authoritative contract”和“按 effect family 分组的 packing/vtable/interface 层”，则 authoritative key 必须始终是前者；后者只可作为实现/查询层的 packing helper，不能在下游阶段反向充当 semantic source of truth。
- 对任何 `needs_reentry = true` 的 effectful callable，其 `direct-style body -> state machine` 转化必须统一遵守 [`EFFECT_REFACTOR.md`](./EFFECT_REFACTOR.md) §4.16、§5.5。
  - boundary 选择、整函数 segmentation、frame lifting、control-transfer encoding 都必须由同一算法决定；
  - `NoOutward` 也只能被视为同一 facts + `ImplPlan` 框架下得到的退化结果，而不是 code-shape 特判通道；
  - 不允许因为 code shape 简单而切到“单 `perform`”“线性 body”“tail-`resume`”“仅 `handle` 内局部状态机”之类专用 lowering；
  - 若某些简单 shape 未来可以压缩，也只能作为统一 transformation 之后的优化，而不是另一条 lowering 入口。
- P6 refactor LLVM backend 必须遵守 [`EFFECT_REFACTOR.md`](./EFFECT_REFACTOR.md) §5.6 的 clean backend 边界。
  - refactor backend 拥有整个 function protocol：entry/return ABI、state CFG、boundary、handle、continuation、runtime error、GC/runtime；
  - 允许共享的旧代码只能是 effect-neutral value/expression primitive；
  - 禁止把 legacy statement/function/call/return/control-flow codegen 当成 refactor backend 的 fallback。
- 阶段完成条件不是“代码大致可跑”，而是“这一阶段的输出已经有完整验证，且其输出本身符合设计预期”。
- P7 切换主线后，必须先完成 full regression，再进入 P8 删除旧主线。
- P8 清理完成后，必须再次完成 full regression，确保仓库中不再存在对旧主线的隐藏依赖。

## 1. 顺序总览

1. P0：并行主线脚手架与现状固化
2. P1：AST / surface contract 冻结
3. P2：HIR / typecheck 新路径落地
4. P3：direct-style MIR 新路径落地
5. P4：effect facts 与 `resolved_outward_cases` 分析落地
6. P5：late-lowered `Step` 路径落地（尚不接 LLVM）
7. P6：LLVM codegen 新路径对接（仍不切主线）
8. P7：切换主线并执行 full regression
9. P8：删除旧主线并再次 full regression

## 2. 分阶段计划

### P0. 并行主线脚手架与现状固化

参考：[`EFFECT_REFACTOR.md`](./EFFECT_REFACTOR.md) §4.10, §4.11, §4.13.1a, §8。

目标：

- 建立新的 effect-refactor 并行路径，不扰动当前默认主线。
- 让后续 AST/HIR/MIR/codegen 阶段都能在“legacy vs refactor”两个路径上显式分流，而不是边做边侵入旧逻辑。
- 固化本轮需要守住的 legacy baseline，避免后续出现“不知道是新路径问题还是旧主线漂移”的情况。

实现：

- 在编译驱动、session 或等效总入口上增加一个显式 pipeline selector。
- 在 `scoop` 与 `scoopc` 上新增成对的命令行参数，把 selector 暴露为用户可选 pipeline。
  - 推荐形态：一个“新 effect-refactor 主线” flag，必要时配一个“legacy 主线”逆向 flag；
  - 具体 flag 名称可在实现时定稿，但两端必须共享同一个 session 入口。
- 所有新代码都从该 selector 派生出的新 dispatcher 进入，不直接侵入旧主线函数体。
- 在 P0 就要同步建立“共享模块 vs 复制实现”的边界清单；若某段旧代码无法满足上面的单一 API / 完全中立要求，则后续阶段默认走复制到新主线的方案，而不是继续把逻辑揉在一起。
- 为 AST/HIR/MIR/late lowering/LLVM 分别预留新的并行入口函数或模块边界。
- 固化一批 effect/continuation 相关的 baseline fixtures / LLVM regression / MIR dump regression，后续用来监控“旧主线保持稳定，新主线逐步闭合”。

阶段输出：

- 一个可显式切换的并行 pipeline 壳层；
- 不改变默认行为的前提下，可从 `scoop` / `scoopc` 的 CLI 参数分流到“新路径（暂时可能只是委托）”；
- 一组用于锁定旧行为的定向 baseline 回归。

验证：

- 旧主线默认入口的定向 effect/continuation 相关测试继续通过；
- 新 CLI 参数能贯通到各层 dispatcher，但在 P0 不要求真正改变语义；
- `scoop` 与 `scoopc` 对同一 pipeline 选择产生一致的 session 配置；
- 不执行 full regression。

完成条件：

- 后续 P1-P6 的代码可以只写在新路径中推进；
- 若删除 selector 会导致阶段间边界重新混回旧主线，说明 P0 未完成。

### P1. AST / surface contract 冻结

参考：[`EFFECT_REFACTOR.md`](./EFFECT_REFACTOR.md) §4.3.2, §5.3.1, §5.3.9。

目标：

- 明确 surface 语法层“不新增 keyword / 不新增 continuation 特殊语法”的结论。
- 冻结 `k.resume(...)`、`resume()`、以及一般性 `f() == f(())`（单一 `Unit` 参数）的语法合同。
- 确保 AST 层保持普通调用语法，不把 type-dependent 的 sugar 过早固化到 AST 节点里。

实现：

- AST 层保持普通 `CallExpr` / member-call 形态，不引入 `ResumeExpr` 等专用节点。
- 记录并验证以下 surface 规则：
  - `Continuation` 交互使用普通方法调用 `k.resume(...)`
  - `ResumeTuple = ()` 时允许 `k.resume()`
  - 一般性单一 `Unit` 参数调用允许 `f()` 作为 `f(())` 的语法糖
- 不在 AST 阶段执行 type-dependent desugar；该工作留到 HIR/typecheck 阶段。

阶段输出：

- surface 语法 contract 的固定测试；
- 不引入新 AST 节点种类的明确边界；
- 为 P2 准备的“typed desugar points”清单。

验证：

- parser / parse fixtures 覆盖 `k.resume(...)`、`k.resume()`、单一 `Unit` 参数调用；
- 若仓库缺少合适的 AST dump 基础设施，则以 parse fixtures + parser unit tests 代替；
- 若验证需要走 end-to-end 入口，统一通过 P0 引入的 CLI 参数进入新路径；
- 不执行 HIR 之后的阶段验证，不执行 full regression。

完成条件：

- 语法层对 continuation / `Unit` sugar 的主张已被 fixture 锁定；
- 后续阶段不再需要为这些语法重新开设计讨论。

### P2. HIR / typecheck 新路径落地

参考：[`EFFECT_REFACTOR.md`](./EFFECT_REFACTOR.md) §3.1, §4.1, §4.3.1, §5.3.1, §5.3.9。

目标：

- 在新路径上把 surface contract 变成 typed HIR contract。
- 让 typechecker 真正理解 `Continuation<ResumeTuple, Answer, eff Out>`、`resume(value): Answer / (Out + Raise<RuntimeError>)`、以及运行时错误作为普通 `Raise<RuntimeError>` 传播的语义。
- 让 HIR 到 MIR 的下一阶段不再需要猜测这些 surface contract。

实现：

- 引入源码层 `Continuation<ResumeTuple, Answer, eff Out>` 的 compiler-owned interface 语义。
- 明确：用户可以持有/传递/调用 continuation，但不能自己实现/伪造该接口。
- 明确：`Continuation` 的 effect 参数只表示 residual `Out`；`Raise<RuntimeError>` 是 `resume(...)` 方法额外暴露的 ordinary effect，不得被反写回 `Out` 参数。
- 在 typecheck 阶段完成 type-dependent desugar：
  - 单一 `Unit` 参数调用的 `f()` -> `f(())`
  - `ResumeTuple = ()` 时的 `k.resume()` -> `k.resume(())`
- 在 HIR / typed side table 中记录：
  - `allowed_row`
  - `Continuation` 的 `ResumeTuple/Answer/Out` contract
  - `perform` / `resume` / `handle` 的 typed 关系
- 统一把 `ContinuationAlreadyResumed` 一类语言内部 runtime error 建模为普通 `Raise<RuntimeError>` 传播，不引入第二条特殊错误通道。

阶段输出：

- typed HIR contract 已经能完整表达 continuation surface 语义；
- 后续 MIR 阶段不再需要回 parser/AST 或临时猜测 `resume` 的类型关系。

验证：

- `typecheck` fixtures 覆盖：
  - continuation surface 类型
  - `resume(...)` 参数/返回类型
  - `resume()` 的 `Unit` sugar
  - 运行时错误 effect surface 语义
- 新增 HIR/typecheck 单元测试或 debug dump（若已有 HIR dump 基础设施则用 snapshot；若无则用 unit tests + diagnostics fixtures）。
- 若通过 `scoopc` 驱动 typed pipeline 验证，则统一使用新 CLI 参数激活新路径。
- 不执行 MIR/LLVM/full regression。

完成条件：

- 新路径的 HIR/typecheck 已能独立描述 continuation/effect surface contract；
- P3 不再需要回看 AST 解释 `resume` / `Unit` sugar。

### P3. direct-style MIR 新路径落地

参考：[`EFFECT_REFACTOR.md`](./EFFECT_REFACTOR.md) §4.10, §4.11, §4.12, §4.13, §5.3, §5.5.3-§5.5.6。

目标：

- 在新路径上建立 production-grade 的 direct-style MIR：仍保留 `Call / Perform / Handle / Resume` 级语义节点。
- 禁止新路径内部继续以“缺节点就退回 HIR codegen”的方式偷渡语义。
- 让 effect 相关结构在 MIR 层真正成为 source of truth，而不是 HIR-compatible 边界上的临时桥。

实现：

- 新路径的 HIR->MIR lowering 必须覆盖 effect/continuation 相关形状：
  - `Call`
  - `Perform`
  - `Resume`
  - `Handle`
  - `finally` / cleanup 相关 block/edge
- 继续沿用并扩展 `SiteId`，确保所有 effect-sensitive site 都有稳定身份。
- 明确新路径在 late lowering 之前保持 direct-style，不提前构造 `Step` IR。
- 为后续 facts 建立必要的 MIR hook points，但在本阶段不引入 `StepSchema` / `ContinuationSchema` 的完整求解。
- 若源码中的 boundary 位于更大表达式内部，P3 必须已经把相关求值顺序、临时值、CFG 分支与局部结果显式化到 MIR；P5 不负责回 HIR 重建 evaluation context。
- `return` / `break` / `continue` / `finally` / cleanup 相关控制流必须在 MIR 中已经是显式 block/edge，而不是留到 P5 再凭源码形状补建。

阶段输出：

- 一套可用于 production 的 direct-style MIR 新路径；
- effect/continuation 相关节点在新路径 MIR 中都是一等节点，而不是 HIR fallback。

验证：

- `dump-mir` fixtures 覆盖：
  - direct call / indirect call / callable value
  - `perform`
  - `resume`
  - `handle` / arm / finally
  - `SiteId` 稳定性
- MIR verifier / unit tests 断言 CFG、site identity、cleanup edge、resume site 的完整性。
- 这些验证应优先通过 `scoop dump-mir` / 等价入口加新 CLI 参数触发，而不是通过替换默认主线实现。
- 不连接 LLVM，不执行 full regression。

完成条件：

- 对本轮 effect refactor 相关语义，P3 产出的 MIR 已足以独立喂给下游 facts 构建；
- 新路径中不再需要为了 effect 主线回退到 HIR。

### P4. effect facts 与 `resolved_outward_cases` 分析落地

参考：[`EFFECT_REFACTOR.md`](./EFFECT_REFACTOR.md) §4.13.1a, §5.4, §5.5.1-§5.5.6, §6, §7.3。

目标：

- 在新路径上建立完整的 `MaterializedEffectFacts`。
- 让后续阶段只消费 facts/schema，不再回看 HIR。
- 把 `resolved_outward_cases`、`needs_reentry`、`impl_plan` 这些核心分析结果在 MIR 之后显式化。
- 让 P5 仅凭 MIR + facts 就能决定整函数 state-machine transformation 的 boundary 集、per-boundary contract、以及 nested handle 是否向外传播 suspension。

实现：

- 落地 `MaterializedEffectFacts` 顶层容器：
  - `step_schemas`
  - `continuation_schemas`
  - `callable_facts`
  - `bodies`
- 落地 `BodyEffectFacts`：
  - `blocks`
  - `sites`
- 落地最小必要 facts：
  - `StepSchema`
  - `ContinuationSchema`
  - `CallableEffectFacts`
  - `BlockEffectFacts`
  - `CallSiteEffectFacts`
  - `PerformSiteEffectFacts`
  - `ResumeSiteEffectFacts`
  - `HandleSiteEffectFacts`
- `BlockEffectFacts` 至少要覆盖 [`EFFECT_REFACTOR.md`](./EFFECT_REFACTOR.md) §5.4.5 中为 P5 所需的 block 摘要：`ambient_cases`、`outward_cases`、`has_suspend_boundary`、`has_handle_boundary`。
- `SiteEffectFacts` 必须足以让 P5 在不回 HIR 的情况下回答：
  - 该 site 是否属于真正的 state-machine boundary；
  - 它对应哪个 `StepSchema` / `ContinuationSchema` / `CaseTag`；
  - payload tuple、resume tuple、answer/outward schema 是什么；
  - nested `handle` 是 self-contained 还是 `may_suspend_outward`。
- `ContinuationSchema` 必须同时保留 source-visible continuation contract 与 internal step upper bound 的边界：
  - `surface_ty` 的 effect 参数只表示源码层 residual `Out`；
  - `out_step_schema` 可为 one-shot 语义保守包含 compiler-generated ordinary `Raise<RuntimeError>` case；
  - 后续阶段不得把 `out_step_schema` 的 runtime-error 上界反写回 `surface_ty`。
- 实现 `resolved_outward_cases` 的统一 SCC/dataflow 求解：
  - direct known callee -> 并入 callee `resolved_outward_cases`
  - candidate set -> 并入候选并集
  - dynamic fallback -> 直接取 `cases(StepSchema(F))`
  - 超预算 -> 整 SCC / 受影响实例 widen 到 schema 全集
- 产出 `impl_plan = NoOutward | SingleCase | CanonicalFull`。

阶段输出：

- 一份对下游阶段“语义闭包”的 effect facts 包；
- 一套不需要回 HIR 的 `resolved_outward_cases` / schema / site contract。
- 一套足以驱动整函数 boundary segmentation 和 frame-lifting 判定的 callable/block/site facts。

验证：

- 新增 dedicated facts dump / snapshot 测试（必要时新增 `dump-effect-facts` 或等价调试入口）；
- 单元测试覆盖：
  - `StepSchema`
  - `ContinuationSchema`
  - call/perform/resume/handle site facts
  - nested handle `self-contained` vs `may_suspend_outward` 分类
  - SCC/widening 分析
  - `impl_plan` 选择
- 如需通过 CLI 做集成验证，统一经 P0 的新路径激活参数进入。
- 不连接 LLVM，不执行 full regression。

完成条件：

- P5 可以只消费 MIR + `MaterializedEffectFacts` 完成全部 lowering 决策；
- P5 可以仅凭这些 facts 确定“哪里切 boundary、boundary 后如何恢复、哪些 nested handle 需要向外层扩散、每个 site 的 payload/resume contract 是什么”；
- 若 P5 仍需要回 HIR 才能知道 `Step` / continuation / site contract，则 P4 未完成。

### P5. late-lowered `Step` 路径落地（尚不接 LLVM）

参考：[`EFFECT_REFACTOR.md`](./EFFECT_REFACTOR.md) §4.9, §4.10, §4.16, §5.2, §5.3.2-§5.3.5, §5.4.7, §5.5, §7.3, §8。

目标：

- 在新路径上把 direct-style MIR + facts 统一转换成 late-lowered representation。
- 对所有仍需独立存在的 effectful callable，用**同一套整函数 transformation**把 `direct-style body` 改写成 state machine；不因 code shape 分叉出第二套 lowering。
- 这一阶段开始物化：
  - `Step_F` enum
  - canonical dynamic `invoke(args_tuple) -> Step_F`
  - continuation object
  - per-op resume contracts，以及必要时按 effect family 分组的 internal resume-interface packing
  - `ImplPlan` 对应的版本形态
- 但此阶段仍不接 LLVM；先把表示和 contract 自己闭合。

实现：

- 落地 late lowering pass，其输入必须固定为“当前 materialized MIR snapshot + `MaterializedEffectFacts`”；禁止回 HIR/AST/typecheck 内部缓存补语义。
- 在 pass 起点先依据 `CallableEffectFacts` / `SiteEffectFacts` 确定整函数 boundary 集。至少包括：
  - `perform` site；
  - outward cases 非空的 call/invoke site；
  - continuation `resume` site；
  - ordinary runtime error outward boundary；
  - `may_suspend_outward` 的 nested `handle` boundary。
- 用统一的 whole-function segmentation 算法从这些 boundary 出发重写整个函数 CFG：
  - boundary 所在 region 被切开；
  - 若 boundary 位于条件、循环、局部 block、或更大表达式求值上下文内部，则切分递归向外扩展，直到整个函数都成为“可编号状态 + 显式边”的形式；
  - 每个 boundary 都必须拥有唯一的 owner state；
  - 每个 boundary 之后的继续执行位置都必须拥有唯一的 resume state；
  - 不能只支持“boundary 恰好落在独立 statement 上”的简单 shape。
- 对所有 boundary 统一使用同一类 lowering 骨架：
  - `state_before -> boundary(site) -> resume_state -> post_resume_suffix`；
  - boundary 自身按 `StepSchema` / site facts 产出 outward case、payload 与 continuation contract；
  - resume/re-entry 之后从 `resume_state` 继续执行，而不是回放源码级控制结构。
- 在同一 pass 中完成 frame lifting。凡是跨 boundary live 且在之后仍会被读取的值，都必须进入 frame/object fields。至少包括：
  - 源码 local；
  - 编译器引入的临时值与中间表达式结果；
  - CFG 合流后继续使用的 join/phi-like 值；
  - `handle` arm binder、resume payload、replayed answer/result slot；
  - state tag、resume payload carrier、cleanup flag、one-shot flag、completion tag 等系统字段。
- 对不跨任何 boundary live 的值，不因函数整体进入状态机就强制 lift；但任何跨 cut 存活的中间结果都必须按同一规则 lift，不能只照顾源码具名 local。
- 把以下控制转移一并编码进 late-lowered representation：
  - `return`；
  - `break` / `continue`；
  - `finally` / cleanup；
  - handler arm 结束后的续点；
  - dropped continuation 导致的“剩余计算被放弃”。
- 物化 late-lowered 形态时，统一生成：
  - `Step_F` variant 构造；
  - canonical dynamic `invoke(args_tuple) -> Step_F` surface；
  - continuation object；
  - per-op resume method/surface contract，以及必要时保留的 internal resume-interface packing / icall boundary；
  - state graph、frame schema、boundary/resume 映射。
- continuation object / per-op resume contract / optional resume-interface packing / boundary lowering 必须消费 `ContinuationSchema.resume_tuple_ty`、`answer_ty` 与 `out_step_schema` 作为 internal `Step` 协议来源；`surface_ty` 只保留源码层 `Continuation<..., eff Out>` 合同，不能从 `out_step_schema` 的 one-shot runtime-error upper bound 反推或扩大其 effect 参数。
- 在 P5 阶段末需要清理 reverse-resume contract 的主次关系，明确 `ConcreteOpKey` / `CaseTag` / `ContinuationSchema` 是 authoritative identity。
- 若仓库继续保留按 effect family 分组的 `LateLoweredResumeInterface`，它只能作为 late-lowered representation 内的 packing/query helper，不能要求 P6 先经 `ResumeInterfaceId` 才能恢复 per-op 语义。
- `dump-effect-lowered` 与 P5 -> P6 stage API 必须能直接展示/查询这一区分，避免分组层反客为主。
- 若现有“单 `handle` 内部状态机”模块能在**不引入 code-shape 分叉**的前提下消费上述整函数 contract，可将其下沉为中立基础设施；否则在新路径中替换/重建，不继续把“仅 `handle` 局部状态机”当成目标架构。
- 在 late-lowered representation 上立即加入一轮窄的：
  - devirtualization
  - inlining
  - DCE
- 这轮优化只作用于统一 transformation 之后的 late-lowered representation，不重新回到高层 effect 语义分析，也不重新选择 `ImplPlan`；也不允许它变成 code-shape-specific 的替代 lowering。

阶段输出：

- 一套完整的 late-lowered internal representation；
- 它的输入是 direct-style MIR + facts，输出是 LLVM 前的 `Step` / continuation / dynamic invoke 形态；
- 其中必须显式包含整函数 state graph、frame schema、boundary/resume mapping，以及与 `StepSchema` / `ContinuationSchema` / `ConcreteOpKey` / `CaseTag` 对齐的 authoritative contract。

验证：

- 新增 late-lowered dump / snapshot 测试（必要时新增 `dump-effect-lowered` 或等价调试入口）；
- 单元测试验证：
  - `Step_F` enum 形状与 `StepSchema` 一一对应
  - continuation object / per-op resume contract 形状正确；若保留 internal resume-interface packing，则其 method 集与 authoritative case 集一致
  - dynamic `invoke(args_tuple) -> Step_F` surface 正确
  - 简单 `single perform`/线性函数与复杂函数共用同一 late-lowering 入口，而不是走另一条专用 transformation
  - boundary 位于 `if` / loop / nested expr / argument evaluation 时，都会被正确切分成 owner-state + resume-state
  - self-contained nested handle 不向外层扩散切分；`may_suspend_outward` nested handle 会成为真正 boundary
  - frame lifting 覆盖 locals / temporaries / join values / binders / resume slots / system fields
  - `return` / `break` / `continue` / `finally` / cleanup 会进入显式 state edge 或 completion/cleanup path
  - late-lowered devirt/inline/DCE 能在局部案例上消除编译器自生的 interface/icall 抽象层
- 如需端到端验证 late-lowered 形态，统一通过 `scoop` / `scoopc` 的新路径参数进入。
- 不接 LLVM，不执行 full regression。

完成条件：

- P6 只需把 late-lowered representation 翻译到 LLVM，而不是重新做 boundary 识别、整函数 segmentation 或 frame-lifting；
- 新路径中不再保留按 code shape 另开入口的 effectful state-machine transformation；
- P5 -> P6 handoff 已明确区分 per-op/per-schema authoritative contract 与可选的 effect-level packing，P6 不再需要把 `ResumeInterfaceId` / effect family 当成 resume 语义主键；
- 不再允许 P6 再临时重做高层 effect lowering 设计。

### P6. LLVM codegen 新路径对接（仍不切主线）

参考：[`EFFECT_REFACTOR.md`](./EFFECT_REFACTOR.md) §4.9, §4.16, §5.2, §5.3.7, §5.3.8, §5.3.9, §5.5, §5.6, §8。

目标：

- 把 P5 产出的 late-lowered representation 接到新的 LLVM codegen 路径。
- 在不切默认主线的前提下，让新路径可以端到端生成正确 IR 和可运行程序。
- 以 clean refactor backend 方式完成 P6：新 backend 拥有整个函数执行协议，只复用 effect-neutral value/expression primitive，不把旧 statement/function codegen 胶合进新 body emitter。
- 仍然不做 full regression；只做覆盖新路径的定向 LLVM/run-pass/runtime 验证。

实现：

- 为新路径实现 LLVM lowering：
  - `Step_F` enum lowering
  - dynamic `invoke` / ordinary direct invoke
  - continuation object lowering
  - per-op resume contract lowering，以及必要时保留的 internal resume-interface / icall packing lowering
  - runtime error 作为普通 effect 分支的 lowering
  - dropped continuation / cleanup hook 语义的 lowering 对齐
- LLVM backend 只消费 P5 产出的 late-lowered state graph / frame schema / boundary contract；不得在 backend 再重新识别源码 shape、再切一次 CFG、或临时发明第二套 state-machine transformation。
- refactor body lowering 不能再以 legacy `codegen_mir_statement`、旧函数 ABI、旧 call dispatcher、旧 return lowering、legacy handler-stack/outcome 作为通用 fallback；若需要共享旧逻辑，必须先抽成不知道新旧线路的纯 value/expression primitive。
- P6 后半段按小任务推进：wrapper completion payload projection、clean value/expression primitive、source-slice classification、pure statement lowering、function ABI、dynamic/virtual/interface call、boundary lowering、handle protocol、continuation protocol、runtime error/drop/unwind、GC/runtime、验证矩阵。
- 在进入 body lowering 前，必须先清理 resume ABI/query 的主次关系：`ContinuationSchemaId` / `CaseTag` / `ConcreteOpKey` 是 authoritative lookup；若保留 `ResumeInterfaceId`，它只能服务 continuation object field/vtable packing 与 object-side method lookup，不能作为 backend 恢复 resume 语义的起点。
- 保持 Managed ABI / extern 边界不承载 effect/continuation 语义。
- 旧 LLVM 主线继续保留，直到 P7 切换。
- 所有新路径 LLVM 验证继续通过 P0 引入的 CLI 参数进入，不改变默认主线行为。

阶段输出：

- 一个可通过显式 selector 进入的新 LLVM codegen 路径；
- 不改变默认主线的前提下，可端到端完成 effect/continuation 程序的 IR 生成与运行。

验证：

- 新增并运行定向 LLVM regression，至少覆盖：
  - `NoOutward`
  - `SingleCase`
  - `CanonicalFull`
  - direct `perform` / `handle` / `resume`
  - dynamic callable fallback
  - continuation one-shot / `RuntimeError`
  - `Unit` 零载荷 case
  - `Step_F` enum 物理形状
- 新增并运行定向 run-pass / runtime_gc / effect 相关 fixture 集，覆盖：
  - direct/indirect effect call
  - continuation capture / resume
  - dynamic callable `invoke`
  - dropped continuation 语义
  - GC env 下的 effect/continuation correctness
- 在本阶段结束前仍**不执行**：
  - `cargo test --all`
  - `cargo run -p scoop -- test`

完成条件：

- 新路径在 LLVM 层面对本文档覆盖的 effect/continuation 语义已经端到端闭合；
- backend 已不再以 effect-level resume interface 作为 resume 语义 source of truth；
- backend 已不再依赖 legacy statement/function/control-flow fallback；旧 codegen 中被复用的部分已经收口为 effect-neutral primitive；
- 只差把默认主线切过去和做 full regression。

### P7. 切换主线并执行 full regression

参考：[`EFFECT_REFACTOR.md`](./EFFECT_REFACTOR.md) §7.3, §8。

目标：

- 把新的 effect-refactor 路径切成默认主线。
- 在切主线后完成一次完整回归，证明新路径已经足以替代旧主线。

实现：

- 翻转顶层 selector 的默认值，让新路径成为默认 effect/continuation 主线。
- 在 P7 过渡期可保留一个显式 legacy CLI 参数，作为短期回滚/比对入口；该 legacy 参数在 P8 必须删除。
- 保留旧路径代码，但不再作为默认入口，仅用于必要的短期比对/紧急兜底。

验证：

- `cargo test --all`
- `cargo run -p scoop -- test`
- `cargo run -p scoop_tools -- spec-fixtures check`
- `cargo clippy --all-targets -- -D warnings`
- 与 effect/continuation 直接相关的 GC env 全开验证：
  - `SCOOP_GC_MOVE=1 SCOOP_GC_STRESS=1 SCOOP_GC_VERIFY_ROOTS=1 cargo run -p scoop -- test --fixtures tests/fixtures/run-pass`
  - `SCOOP_GC_MOVE=1 SCOOP_GC_STRESS=1 SCOOP_GC_VERIFY_ROOTS=1 cargo run -p scoop -- test --fixtures tests/fixtures/runtime_gc`

完成条件：

- 新主线已经是默认路径；
- 全量与 GC env 相关回归通过；
- 可以进入 P8 删除旧主线。

### P8. 删除旧主线并再次 full regression

参考：[`EFFECT_REFACTOR.md`](./EFFECT_REFACTOR.md) 全文，重点参照 §4.10-§4.16、§5.4、§5.5、§8。

目标：

- 删掉旧的 legacy effect/continuation 主线，实现真正收口。
- 保证仓库中不再存在“默认靠新主线，但旧主线还悄悄救场”的隐藏依赖。

实现：

- 删除旧 selector 分支与旧 effect/continuation lowering 主线。
- 删除只服务旧主线的桥接 helper、旧 dump/fixture、以及只为 legacy 形状存在的临时适配层。
- 删除任何残留的 code-shape-specific effect lowering 入口，包括“单 `perform` 快路径”“线性 body 专用路径”“仅 `handle` 局部状态机主线”等不再符合 §4.16 / §5.5 的实现分支。
- 清理实现注释、文档和测试中对旧主线的引用。

验证：

- 重跑 P7 的完整回归矩阵：
  - `cargo test --all`
  - `cargo run -p scoop -- test`
  - `cargo run -p scoop_tools -- spec-fixtures check`
  - `cargo clippy --all-targets -- -D warnings`
  - `SCOOP_GC_MOVE=1 SCOOP_GC_STRESS=1 SCOOP_GC_VERIFY_ROOTS=1 cargo run -p scoop -- test --fixtures tests/fixtures/run-pass`
  - `SCOOP_GC_MOVE=1 SCOOP_GC_STRESS=1 SCOOP_GC_VERIFY_ROOTS=1 cargo run -p scoop -- test --fixtures tests/fixtures/runtime_gc`

完成条件：

- 仓库中不再保留旧主线；
- 全量验证在“只有新主线存在”的条件下仍完整通过。

## 3. 阶段切换门槛

- P0 之后，所有新实现必须走并行路径，不再直接往旧主线里补 effect 语义。
- P1 未完成前，不进入 HIR surface typing。
- P2 未完成前，不进入 direct-style MIR 新路径的 production 化。
- P3 未完成前，不进入 `MaterializedEffectFacts` 的闭包化求解。
- P4 未完成前，不进入 late-lowered `Step` 物化。
- P5 未完成前，不接 LLVM。
- P6 未完成前，不切主线，不做 full regression。
- P7 未完成前，不删除旧主线。
- P8 未完成前，本轮不算结束。

## 4. 完成标准

本轮完成时，必须能够明确陈述以下结论全部成立：

1. surface 语法层不引入新的 continuation 专用语法或 keyword，`k.resume(...)` 与一般 `Unit` 参数 sugar 已在新主线内稳定工作。
2. HIR 已能完整表达 `Continuation<ResumeTuple, Answer, eff Out>`、`resume(value): Answer / (Out + Raise<RuntimeError>)` 与 runtime error 的普通 effect 语义。
3. new-path MIR 在 late lowering 之前保持 direct-style，并且不再依赖 HIR fallback 才能表达 effect/continuation 语义。
4. `MaterializedEffectFacts` 已成为 downstream 唯一 effect contract 来源；后续阶段不再为补语义回看 HIR。
5. `resolved_outward_cases`、`needs_reentry`、`impl_plan` 已在 facts 层闭合，并按统一 SCC/dataflow + budget 规则求解。
6. 所有 `needs_reentry = true` 的 effectful callable 都按统一的整函数 boundary segmentation + frame lifting + explicit resume-state 算法完成 `direct-style -> state machine` 转化，不再按 code shape 分流。
7. late-lowered `Step_F` / dynamic `invoke` / continuation object / per-op resume contracts（必要时带 internal resume-interface packing）已形成新的中后端主线。
8. 新 LLVM codegen 已能在并行路径上端到端生成并运行 effect/continuation 程序。
9. 新路径切为默认主线后，full regression 与 GC env 相关全集验证全部通过。
10. 旧主线已被删除，仓库中不再保留对旧 effect/continuation 主线或 code-shape-specific lowering 的隐藏依赖。
