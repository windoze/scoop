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
| `P4-T06` | `TODO-P4.md` | [DONE] 为 `NoOutward` 发布 plain callable ABI 合同，停止强制为 pure body 建 `StepSchema` |
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
| `P5-T07a` | `TODO-P5.md` | [DONE] 修正 pure caller 经 call boundary 消费 compiler-generated runtime-error case 时的 late-lowering case 投影，保证 P5 -> P6 handoff 可用于 P6-T03 验证 |
| `P5-T07b` | `TODO-P5.md` | [DONE] 清理 P5 late-lowered handoff 的 resume contract 主次关系，固定 per-op/per-schema authoritative 表达 |
| `P5-T08` | `TODO-P5.md` | [DONE] 让 `NoOutward` 在 late-lowered handoff 中保持 plain callable，不物化 `Step` / continuation / state-machine 壳 |
| `P6-T01` | `TODO-P6-part1.md` | [DONE] 建立 refactor LLVM codegen stage 入口，并让 `build` / `run` / `--emit-llvm` 新路径不再回落到 `production_lowered_hir` |
| `P6-T01a` | `TODO-P6-part1.md` | [DONE] 为 refactor LLVM stage 建立 fail-fast 守卫，禁止 effectful lowering 静默回落到 legacy handler-stack / `EffectOutcome` backend |
| `P6-T01R` | `TODO-P6-part1.md` | [DONE] Review LLVM stage 入口，确认 refactor 路径已与 legacy `production_lowered_hir` / old effect backend 分离 |
| `P6-T01b` | `TODO-P6-part1.md` | [DONE] 扩展 refactor build/LLVM handoff 的 ABI 可见性，保证 P6-T02 build fixtures 能在不触发 legacy lowering 的前提下观察 effectful `Step` / continuation 形状 |
| `P6-T02` | `TODO-P6-part1.md` | [DONE] 把 P5 的 `Step` / frame / continuation / resume-interface 合同下沉到 LLVM type/layout lowering |
| `P6-T02a` | `TODO-P6-part1.md` | [DONE] 让 refactor LLVM ABI materializer 严格消费 P5 发布的 resume-interface contract，禁止在 P6 现场补造 interface identity |
| `P6-T02b` | `TODO-P6-part1.md` | [DONE] 让 refactor LLVM ABI materializer 对 authoritative resume-interface method completeness fail fast，禁止接受缺失 method 的 published shell |
| `P6-T02R` | `TODO-P6-part1.md` | [DONE] Review LLVM type/layout 合同，确认 canonical `Step_F`、frame、continuation ABI 已固定且不再依赖 legacy signal/outcome 模型 |
| `P6-T02c` | `TODO-P6-part1.md` | [DONE] 发布 continuation surface-resume ABI/query contract，禁止 P6-T03 在 backend 现场猜测 `resume(...)` 入口 |
| `P6-T02d` | `TODO-P6-part1.md` | [DONE] 发布 canonical dynamic-invoke callable-object ABI/query contract，禁止 P6-T03 在 backend 现场猜测 indirect call 入口 |
| `P6-T02e` | `TODO-P6-part1.md` | [DONE] 发布 pure caller call boundary 本地消费 compiler-generated runtime-error case 的 lowering contract，禁止 P6-T03 在 backend 现场发明传播路径 |
| `P6-T02f` | `TODO-P6-part1.md` | [DONE] 发布 straight-line source-slice 非 boundary dynamic call 的 callable-object ABI/query contract，禁止 P6-T03 在 body emitter 现场回落旧 callable wrapper |
| `P6-T02g` | `TODO-P6-part1.md` | [DONE] 发布 callable carrier -> canonical dynamic entry 的 refactor contract，确保 closure/vtable/itable 不再指向 legacy 调用 ABI |
| `P6-T02h` | `TODO-P6-part1.md` | [DONE] 发布 `LocalRuntimeError` synthetic terminal state 的 authoritative lowering contract，禁止 P6-T03 在 backend 现场发明 pure caller runtime-error 的结束路径 |
| `P6-T02i` | `TODO-P6-part1.md` | [DONE] 发布 synthetic invoke-carrier / source-type ABI value lowering contract，禁止 P6-T03 把 refactor handoff 类型回塞 legacy codegen `TypeStore` |
| `P6-T02j` | `TODO-P6-part1.md` | [DONE] 发布 `HandleDispatch` / completion-state lowering contract，禁止 P6-T03 在 backend 现场发明 handle body/arm/finally 的内部返回协议 |
| `P6-T02k` | `TODO-P6-part1.md` | [DONE] 发布 `HandleDispatch` arm payload binder / escape-continuation binder contract，禁止 P6-T03 在 body emitter 现场回 canonical MIR handle arm 恢复绑定形状 |
| `P6-T02kR` | `TODO-P6-part1.md` | [DONE] Review `HandleDispatch` arm binder / continuation binder contract，确认 P6-T03 不再需要回 canonical MIR handle arm 恢复绑定形状 |
| `P6-T02l` | `TODO-P6-part1.md` | [DONE] 发布 `HandleDispatch` state-region / boundary-consumption contract，禁止 P6-T03 在 backend 现场重建 body/arm/finally 子图归属 |
| `P6-T02ma` | `TODO-P6-part1.md` | [DONE] 发布 authoritative surface-resume dispatch-source inventory，覆盖 shared-schema surface case、handle continuation binder 与 resume-site-only schema |
| `P6-T02m` | `TODO-P6-part2.md` | [DONE] 发布 continuation surface-resume -> owner dispatch contract，禁止 P6-T03 在 backend 现场扫描 continuation object 或猜 owner callable |
| `P6-T02n` | `TODO-P6-part2.md` | [DONE] 清理 refactor LLVM ABI/query 的 resume 主键，降级 effect-level resume interface 为 packing 层 |
| `P6-T02o` | `TODO-P6-part2.md` | [DONE] 发布 statement/terminator anchored boundary operand contract，禁止 P6-T03 在 body emitter 现场回 raw MIR statement/terminator 恢复 `Call / Perform / Resume` 输入 |
| `P6-T02p` | `TODO-P6-part2.md` | [DONE] 发布 callable version 选择 contract，禁止 P6-T03 在 backend 现场按 `root_fqn` / 单壳层假定选择 late-lowered body |
| `P6-T02qa` | `TODO-P6-part2.md` | [DONE] 发布 escaped continuation aggregate/member write-read provenance contract，禁止 P6-T02q 在 late-lowered/ABI materialization 现场从 unresolved assign-lhs TODO 或 source shape 猜 `cell.k` 回读 continuation 的底层 surface route |
| `P6-T02q` | `TODO-P6-part2.md` | [DONE] 发布 resume-boundary wrapper -> underlying continuation surface route contract，禁止 P6-T03 在 backend 现场从 continuation local / source type 猜 `k.resume(...)` 实际调用的 schema |
| `P6-T02qb` | `TODO-P6-part2.md` | [DONE] 发布 cleanup/finally pending payload carrier contract，禁止 P6-T03 在 backend 现场发明 `ResumePayloadCarrier` 的 boxing / projection 规则 |
| `P6-T02qc` | `TODO-P6-part2.md` | [DONE] 发布 shared surface-resume wrapper 的 owner-step -> wrapper-step 投影 contract，禁止 P6-T03 在 shared surface body 现场反推 inverse dispatch |
| `P6-T02qd` | `TODO-P6-part2.md` | [DONE] 发布 continuation resume payload -> resumed local/home 注入 contract，禁止 P6-T03 在 backend 现场回 canonical MIR 恢复 `PerformResult` / boundary-result 绑定 |
| `P6-T02qe` | `TODO-P6-part2.md` | [DONE] 发布 refactor source-slice member read/write LLVM lowering contract，禁止 P6-T03 在 body emitter 现场回 HIR 或 legacy member lowering |
| `P6-T02qf` | `TODO-P6-part2.md` | [DONE] 把 `scoop test` run-pass 子进程接到父级 effect-pipeline selector，确保 P6-T03 验证真实覆盖 refactor LLVM path |
| `P6-T02qg` | `TODO-P6-part2.md` | [DONE] 发布 non-`Unit` completion payload source / return-value contract，禁止 P6-T03 在 backend 回 raw MIR/tail shape 恢复完成值 |
| `P6-T02qga` | `TODO-P6-part3.md` | [DONE] 发布 call-boundary 本地消费 outward case 的 continuation composition contract，禁止 escaped continuation 绕过 callee resume body |
| `P6-T02qh` | `TODO-P6-part3.md` | [DONE] 发布 surface-resume wrapper completion payload projection contract，禁止 P6-T03 在 owner-step `Complete` 投影时发明 wrapper answer 值 |
| `P6-T03` | `TODO-P6-part2.md` | [DONE] [ABANDONED] 旧单体 LLVM body lowering 任务，已拆分为 clean backend 小任务链 |
| `P6-T03a` | `TODO-P6-part3.md` | [DONE] 固化 clean refactor LLVM backend 边界，抽出 effect-neutral value/expression primitive |
| `P6-T03b` | `TODO-P6-part3.md` | [DONE] 发布 source-slice statement classification contract，禁止 body emitter 静默 skip 或回 raw shape 猜语义 |
| `P6-T03c` | `TODO-P6-part3.md` | [DONE] 实现 refactor pure statement lowering，停止调用 legacy statement-level lowering |
| `P6-T03d` | `TODO-P6-part3.md` | [DONE] 闭合 refactor function ABI 与 entry shell lowering，包括 main wrapper |
| `P6-T03e` | `TODO-P6-part3.md` | [DONE] 闭合 direct/dynamic/virtual/interface call lowering，不再回 legacy callable wrapper |
| `P6-T03f` | `TODO-P6-part3.md` | [DONE] 闭合 boundary lowering，覆盖 Call / Perform / Resume / runtime-error / nested-handle outward |
| `P6-T03g` | `TODO-P6-part3.md` | [DONE] 闭合 HandleDispatch protocol，覆盖 body / arm / finally / exit / pending completion transport |
| `P6-T03h` | `TODO-P6-part3.md` | [DONE] 闭合 continuation protocol，覆盖 one-shot、double resume、wrapper projection、drop/unwind/abandon |
| `P6-T03i` | `TODO-P6-part3.md` | [DONE] 闭合 runtime error、diagnostics 与 body verifier，冻结 clean body lowering 完成条件 |
| `P6-T03R` | `TODO-P6-part3.md` | [DONE] Review clean LLVM body lowering，确认 backend 拥有 whole function protocol 且不再胶合 legacy codegen |
| `P6-T04` | `TODO-P6-part3.md` | [DONE] 接通 GC roots / stackmaps / runtime 语义，并锁定 dropped continuation、runtime error 与 Managed ABI 边界 |
| `P6-T04R` | `TODO-P6-part3.md` | [DONE] Review GC/runtime 集成，确认 clean refactor path 没有 legacy runtime 语义依赖 |
| `P6-T05` | `TODO-P6-part3.md` | [DONE] 建立 refactor LLVM 定向 build/run-pass/runtime_gc 验证矩阵，并冻结 P6 -> P7 handoff contract |
| `P6-T05a` | `TODO-P6-part3.md` | [DONE] 闭合 `NoOutward` plain callable 对本地 effect/control body 的 handoff，禁止 P6-T06 用 legacy fallback 或 complete-only `Step_F` 绕过 |
| `P6-T06` | `TODO-P6-part3.md` | [DONE] 把 `NoOutward` LLVM lowering 改回 plain ABI，调用点使用普通 dcall/icall/vcall |
| `P6-T06R` | `TODO-P6-part3.md` | [DONE] Review `NoOutward` plain ABI 修复，确认 P7 前不再存在 complete-only `Step_F` 回归 |
| `P6-T05R` | `TODO-P6-part3.md` | [DONE] Review P6 阶段退出条件，确认 P7 只需切主线并执行 full regression |
| `P7-T01` | `TODO-P7.md` | [DONE] 翻转顶层 selector 默认值为 refactor，同时保留显式 `legacy` 参数作为短期 compare/rollback 入口 |
| `P7-T01R` | `TODO-P7.md` | [DONE] Review selector 默认值翻转，确认 omission=refactor 且 explicit legacy 仍是唯一短期回滚入口 |
| `P7-T02` | `TODO-P7.md` | [DONE] 更新默认主线切换后的 driver/fixture/test/docs 假设，并锁定“无显式 selector 时不得悄悄回 legacy” |
| `P7-T02R` | `TODO-P7.md` | [DONE] Review 默认主线假设与 hidden-fallback 守护，确认 omission/default 真正代表 refactor 主线 |
| `P7-T02T` | `TODO-P7.md` | [DONE] 发布并消费 generic class instance layout handoff，解除 `Task<T>` constructor 在 refactor LLVM 默认路径上的阻塞 |
| `P7-T02S` | `TODO-P7.md` | [DONE] 修复默认 build fixture 中暴露的 refactor LLVM/lowering 缺口，解除 P7-T03 full regression 阻塞 |
| `P7-T02U` | `TODO-P7.md` | [DONE] 修复默认 run-pass 暴露的 refactor async/task resume payload ABI 阻塞 |
| `P7-T02V` | `TODO-P7.md` | 修复默认 run-pass 暴露的 refactor callable-value receiver / pattern binder / FunPtr 阻塞 |
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
