# TODO 索引

> 这是任务索引文件。仅列出任务 `id`、所在文件和任务标题。  
> 具体任务描述、实现要求、约束、验证条件与完成条件，请查阅对应的 `TODO-Px.md` 文件。

| ID | 文件 | 标题 |
| --- | --- | --- |
| `P0-T01` | `TODO-P0.md` | [DONE] 建立新旧主线共享的 CLI / Session pipeline selector |
| `P0-T01R` | `TODO-P0.md` | [DONE] Review CLI / Session selector，确认新主线入口对两端一致且默认行为稳定 |
| `P0-T02` | `TODO-P0.md` | [DONE] 建立并行 pipeline dispatcher 壳层，禁止新路径直接侵入旧业务模块 |
| `P0-T02R` | `TODO-P0.md` | [DONE] Review 并行 dispatcher 壳层，确认没有把新旧业务逻辑混写在一起 |
| `P0-T03` | `TODO-P0.md` | [DONE] 建立“共享模块 vs 复制实现”边界清单，并把它固化为仓库文档 |
| `P0-T03R` | `TODO-P0.md` | [DONE] Review 边界清单，确认后续实现不会再靠临时判断混线 |
| `P0-T04` | `TODO-P0.md` | [DONE] 建立 P0 baseline parity 验证矩阵，锁定“新路径壳层不改变旧语义” |
| `P0-T04R` | `TODO-P0.md` | [DONE] Review baseline parity 与 P0 退出条件 |
| `P1-T01` | `TODO-P1.md` | [DONE] 建立 refactor AST stage 专用入口与阶段输出类型 |
| `P1-T01R` | `TODO-P1.md` | [DONE] Review AST stage 入口与 handoff 类型，确认 parser 仍是中立共享模块 |
| `P1-T02` | `TODO-P1.md` | [DONE] 锁定 continuation/resume 与单一 `Unit` 参数调用的 AST 形状 |
| `P1-T02R` | `TODO-P1.md` | [DONE] Review surface parse contract，确认 continuation / `Unit` sugar 仍是普通调用语法 |
| `P1-T03` | `TODO-P1.md` | [DONE] 建立 AST -> HIR handoff contract，并锁定 refactor AST stage parity |
| `P1-T03R` | `TODO-P1.md` | [DONE] Review P1 阶段退出条件，确认可以进入 HIR / typecheck 新路径 |
| `P2-T01` | `TODO-P2.md` | [DONE] 建立 refactor typed HIR stage 入口，并让 `dump-hir` 新路径不再调用 legacy `lower_for_dump` |
| `P2-T01R` | `TODO-P2.md` | [DONE] Review refactor typed HIR stage，确认新路径已从 legacy `lower_for_dump` 分离 |
| `P2-T02` | `TODO-P2.md` | [DONE] 对齐 `Continuation` surface contract，并把单一 `Unit` 参数 sugar 落到 typed 阶段 |
| `P2-T02R` | `TODO-P2.md` | [DONE] Review `Continuation` surface 与 typed sugar，确认零参 sugar 没有污染 AST 和 parser |
| `P2-T03` | `TODO-P2.md` | [DONE] 落地 `Continuation` typed 语义、runtime error 的普通 effect 传播，以及 compiler-owned interface 约束 |
| `P2-T03R` | `TODO-P2.md` | [DONE] Review continuation typed 语义，确认没有残留隐藏通道或 legacy 魔法 |
| `P2-T04` | `TODO-P2.md` | [DONE] 输出 typed HIR effect/continuation side tables，并锁定 `dump-hir` / typecheck 验证矩阵 |
| `P2-T04R` | `TODO-P2.md` | [DONE] Review P2 阶段退出条件，确认 P3 不再需要回 AST/typecheck 猜语义 |
| `P3-T01` | `TODO-P3.md` | [DONE] 建立 refactor direct-style MIR stage 入口与显式 stage 输出，切断 `dump-mir` 对 legacy `mir::lower_for_dump` 的依赖 |
| `P3-T01R` | `TODO-P3.md` | [DONE] Review refactor MIR stage 入口，确认新路径已与 legacy `mir::lower_for_dump` 分离 |
| `P3-T02` | `TODO-P3.md` | [DONE] 把 P2 typed contract 下沉到 direct-style MIR，停止基于 span / 名字 / HIR fallback 猜测 `Call / Perform / Resume / Handle` 语义 |
| `P3-T02R` | `TODO-P3.md` | [DONE] Review direct-style MIR contract，下沉信息是否已足够并且不再依赖 span / 名字猜测 |
| `P3-T03` | `TODO-P3.md` | [DONE] 显式化 boundary 所在的 CFG / cleanup / evaluation context，并为 `SiteId` 与 refactor MIR 形状建立 verifier |
| `P3-T03R` | `TODO-P3.md` | [DONE] Review CFG / cleanup / `SiteId` invariants，确认 refactor MIR 已经语义闭包 |
| `P3-T04` | `TODO-P3.md` | [DONE] 建立 refactor 专属 `dump-mir` snapshot / golden 矩阵，并冻结 P3 -> P4 的 MIR handoff contract |
| `P3-T04R` | `TODO-P3.md` | [DONE] Review P3 阶段退出条件，确认 P4 可以只消费 MIR 而不回 HIR |
| `P4-T01` | `TODO-P4.md` | [DONE] 建立 refactor effect-facts stage 与独立 side-table 子系统边界 |
| `P4-T01R` | `TODO-P4.md` | [DONE] Review facts stage 边界，确认没有把新 facts 混进 legacy `effect` / `summary` / `ProgramFacts` |
| `P4-T02` | `TODO-P4.md` | [DONE] 落地 schema identity、canonical schema pool 与 callable-level facts 壳层 |
| `P4-T02R` | `TODO-P4.md` | [DONE] Review schema pool 与 callable facts，确认 identity 和 case contract 已经固定 |
| `P4-T02a` | `TODO-P4.md` | [DONE] 修复 canonical materialized MIR pass-view 对普通非泛型 callable body 的发布，确保 P4 能在稳定 `InstanceKey` 键空间上看到 request-root / caller body |
| `P4-T02aR` | `TODO-P4.md` | [DONE] Review canonical pass-view 对 ordinary callable body 的发布结果，确认 P4 不再需要 raw/fallback 键空间 |
| `P4-T03` | `TODO-P4.md` | [DONE] 构建 `BodyEffectFacts` / `SiteEffectFacts` 与 local-case 结构化分析 |
| `P4-T03R` | `TODO-P4.md` | [DONE] Review body/site facts，确认 contract 已经闭包且不再依赖 HIR/span 推断 |
| `P4-T04` | `TODO-P4.md` | [DONE] 实现 `resolved_outward_cases` SCC/dataflow 求解，并完成 `needs_reentry` / `impl_plan` / final block facts 回填 |
| `P4-T04R` | `TODO-P4.md` | [DONE] Review solver / widening / `impl_plan`，确认求解结果完全由 facts 驱动 |
| `P4-T05` | `TODO-P4.md` | [DONE] 新增 `dump-effect-facts` / snapshot 基线，并冻结 P4 -> P5 handoff contract |
| `P4-T05R` | `TODO-P4.md` | [DONE] Review P4 阶段退出条件，确认 P5 可以只消费 MIR + facts 完成 lowering 决策 |
| `P4-T05a` | `TODO-P4.md` | [DONE] 把 compiler-generated continuation 的 one-shot runtime error 纳入 canonical `StepSchema` / facts handoff |
| `P4-T05b` | `TODO-P4.md` | [DONE] 修正 `ContinuationSchema.surface_ty` 与 `out_step_schema` 的 contract 边界，避免把 one-shot runtime-error 上界并入 `Continuation` effect 参数 |
| `P5-T01` | `TODO-P5.md` | [DONE] 建立 refactor late-lowering stage 与独立 late-lowered representation 边界 |
| `P5-T01R` | `TODO-P5.md` | [DONE] Review late-lowering stage 边界，确认新路径没有借壳 legacy `effect/state_machine` 或 LLVM backend |
| `P5-T02` | `TODO-P5.md` | [DONE] 定义 late-lowered representation 的最终目标形状，包括 version key、state graph、frame schema、`Step` / continuation carrier 壳层 |
| `P5-T02R` | `TODO-P5.md` | [DONE] Review late-lowered representation，确认 version key / `Step` / continuation carrier 已按最终形态固定 |
| `P5-T03` | `TODO-P5.md` | [DONE] 依据 `MaterializedEffectFacts` 实现 boundary 选择与 whole-function segmentation，产出 owner-state / resume-state 骨架 |
| `P5-T03R` | `TODO-P5.md` | [DONE] Review segmentation 骨架，确认 boundary 识别与 owner/resume 状态只由 facts 驱动 |
| `P5-T04` | `TODO-P5.md` | [DONE] 实现 frame lifting，以及 `return` / `break` / `continue` / `finally` / cleanup / dropped continuation 的显式状态机合同 |
| `P5-T04a` | `TODO-P5.md` | [DONE] 为 frame lifting 建立稳定的 MIR local 来源分类，避免把源码 `tmp*` local 误判为 compiler temporary |
| `P5-T04R` | `TODO-P5.md` | [DONE] Review frame lifting 与控制流合同，确认没有残留 direct-style 隐式语义或错误的 dropped-continuation 行为 |
| `P5-T04b` | `TODO-P5.md` | [DONE] 对齐 late lowering 对 `ContinuationSchema.surface_ty` / `out_step_schema` 的消费边界，避免在 continuation 物化时重新引入 surface-row 漂移 |
| `P5-T05` | `TODO-P5.md` | [DONE] 物化 `Step_F` enum、canonical dynamic `invoke`、continuation object、internal resume interfaces，并按 `ImplPlan` 完成 boundary lowering |
| `P5-T05R` | `TODO-P5.md` | [DONE] Review `Step` / continuation 物化结果，确认没有第二套 ABI、没有 TLS 依赖、没有删减接口方法 |
| `P5-T06` | `TODO-P5.md` | [DONE] 在 late-lowered representation 上加入窄的 devirtualization / inlining / DCE 后处理 |
| `P5-T06R` | `TODO-P5.md` | [DONE] Review late-lowered 后处理，确认它只做抽象层收缩，不重新回到高层 effect 分析 |
| `P5-T07` | `TODO-P5.md` | [DONE] 新增 `dump-effect-lowered` / snapshot 基线，并冻结 P5 -> P6 handoff contract |
| `P5-T07R` | `TODO-P5.md` | [DONE] Review P5 阶段退出条件，确认 P6 只需把 late-lowered representation 翻译到 LLVM |
| `P6-T01` | `TODO-P6.md` | [DONE] 建立 refactor LLVM codegen stage 入口，并让 `build` / `run` / `--emit-llvm` 新路径不再回落到 `production_lowered_hir` |
| `P6-T01a` | `TODO-P6.md` | [DONE] 为 refactor LLVM stage 建立 fail-fast 守卫，禁止 effectful lowering 静默回落到 legacy handler-stack / `EffectOutcome` backend |
| `P6-T01R` | `TODO-P6.md` | [DONE] Review LLVM stage 入口，确认 refactor 路径已与 legacy `production_lowered_hir` / old effect backend 分离 |
| `P6-T01b` | `TODO-P6.md` | [DONE] 扩展 refactor build/LLVM handoff 的 ABI 可见性，保证 P6-T02 build fixtures 能在不触发 legacy lowering 的前提下观察 effectful `Step` / continuation 形状 |
| `P6-T02` | `TODO-P6.md` | [DONE] 把 P5 的 `Step` / frame / continuation / resume-interface 合同下沉到 LLVM type/layout lowering |
| `P6-T02a` | `TODO-P6.md` | [DONE] 让 refactor LLVM ABI materializer 严格消费 P5 发布的 resume-interface contract，禁止在 P6 现场补造 interface identity |
| `P6-T02b` | `TODO-P6.md` | [DONE] 让 refactor LLVM ABI materializer 对 authoritative resume-interface method completeness fail fast，禁止接受缺失 method 的 published shell |
| `P6-T02R` | `TODO-P6.md` | Review LLVM type/layout 合同，确认 canonical `Step_F`、frame、continuation ABI 已固定且不再依赖 legacy signal/outcome 模型 |
| `P6-T03` | `TODO-P6.md` | 按 P5 state graph / boundary contract 完成 refactor LLVM body lowering，停止在 backend 重做 state-machine transformation |
| `P6-T03R` | `TODO-P6.md` | Review LLVM body lowering，确认 backend 只翻译 state graph，而不再重做 segmentation / frame lifting / shape 推断 |
| `P6-T04` | `TODO-P6.md` | 接通 GC roots / stackmaps / runtime 语义，并锁定 dropped continuation、runtime error 与 Managed ABI 边界 |
| `P6-T04R` | `TODO-P6.md` | Review GC/runtime 集成，确认没有残留 legacy handler-stack 依赖，也没有错误的 dropped-continuation / FFI 语义 |
| `P6-T05` | `TODO-P6.md` | 建立 refactor LLVM 定向 build/run-pass/runtime_gc 验证矩阵，并冻结 P6 -> P7 handoff contract |
| `P6-T05R` | `TODO-P6.md` | Review P6 阶段退出条件，确认 P7 只需切主线并执行 full regression |
| `P7-T01` | `TODO-P7.md` | 翻转顶层 selector 默认值为 refactor，同时保留显式 `legacy` 参数作为短期 compare/rollback 入口 |
| `P7-T01R` | `TODO-P7.md` | Review selector 默认值翻转，确认 omission=refactor 且 explicit legacy 仍是唯一短期回滚入口 |
| `P7-T02` | `TODO-P7.md` | 更新默认主线切换后的 driver/fixture/test/docs 假设，并锁定“无显式 selector 时不得悄悄回 legacy” |
| `P7-T02R` | `TODO-P7.md` | Review 默认主线假设与 hidden-fallback 守护，确认 omission/default 真正代表 refactor 主线 |
| `P7-T03` | `TODO-P7.md` | 在 refactor 成为默认主线后运行标准 full regression 矩阵，并修复所有默认路径回归 |
| `P7-T03R` | `TODO-P7.md` | Review 标准 full regression，确认新默认主线已经覆盖常规回归而不是靠 legacy 兜底 |
| `P7-T04` | `TODO-P7.md` | 运行 GC env 全开验证，并冻结 P7 -> P8 handoff：legacy 仅剩显式 compare/rollback 入口 |
| `P7-T04R` | `TODO-P7.md` | Review P7 阶段退出条件，确认默认主线已切换且 P8 只需删除旧主线并再次 full regression |
| `P8-T01` | `TODO-P8.md` | 删除顶层 legacy selector 与并行 dispatcher 壳层，收口为单一 refactor 主线入口 |
| `P8-T01R` | `TODO-P8.md` | Review selector/dispatcher 删除结果，确认仓库已不存在 legacy 顶层入口或隐藏切换点 |
| `P8-T02` | `TODO-P8.md` | 删除 legacy effect/continuation lowering 主线、legacy LLVM effect backend，以及所有 code-shape-specific 旧入口 |
| `P8-T02R` | `TODO-P8.md` | Review legacy 主线删除结果，确认旧 backend 与 shape-specific 入口已经真正消失 |
| `P8-T03` | `TODO-P8.md` | 清理 tests / fixtures / docs 中的 legacy 主线残留，并把 compare 型资产改写为纯新主线回归 |
| `P8-T03R` | `TODO-P8.md` | Review 测试/文档残留清理，确认仓库公开叙述与主测试路径都只剩新主线 |
| `P8-T04` | `TODO-P8.md` | 在“只有新主线存在”的条件下重跑完整回归矩阵，并锁定最终收口状态 |
| `P8-T04R` | `TODO-P8.md` | Review P8 阶段退出条件，确认仓库已真正收口到单一新主线且本轮工作结束 |
