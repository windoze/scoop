# Pipeline Refactor TODO 索引

> 生成时间：2026-05-21
> 计划基线：[`PLAN.md`](./PLAN.md)
> 设计基线：[`PIPELINE_REFACTOR.md`](./PIPELINE_REFACTOR.md)
> 审计基线：[`PIPELINE-CLEANUP.md`](./PIPELINE-CLEANUP.md)
> 当前状态：P0-P8、`TODO-7-INIT`、P9 全部任务、`P10-T01`、`P10-T01R`、`P10-T02`、`P10-T02R`、`P10-T03-a`、`P10-T03` 与 `P10-T03R` 已完成；下一项为 `P10-T04-a`。

## 总原则

- `PLAN.md` 是当前执行计划基线；如果实现时发现阶段边界、crate DAG、facts 归属或全局初始化语义需要改变，必须先回写 `PIPELINE_REFACTOR.md`，再调整 TODO。
- 所有任务按 `TODO-1.md` 到 `TODO-7.md` 顺序推进；除非对应文件明确允许，不跨包并行实现。
- 每个实现小阶段后必须紧跟一个独立 review 任务，复审该小阶段的完整变更、阶段目标和约束遵守情况。
- review 任务不是形式检查；如果发现前一任务没有真正完成目标，review 任务必须直接修正或阻塞下一任务。
- 任务完成后必须同时更新 `TODO.md` 和对应 `TODO-[1-7].md` 中的任务状态与完成记录；不得只更新其中一边。
- 后续生成新任务时，正式任务编号统一使用 `P[0-9]+-NN` 格式（P9/P10 任务在 `TODO-7.md`）；如果执行过程中发现某个任务过于复杂、需要进一步拆解，则使用 `P[0-9]+-NN-[a-z]` 格式。
- 任何 fact crate 不得依赖 stage crate 或其它 fact crate；任何 stage output 不得长期嵌套上一阶段完整输出。
- HIR 不承载 optimization pass；MIR 承载普通调用图/实例级优化；LIR 承载 effect/control 相关窄优化；codegen 只承载 backend-specific 优化。

## 任务包划分

| 包 | 文件 | 覆盖 PLAN 阶段 | 目标 | 当前细化状态 |
| --- | --- | --- | --- | --- |
| 1 | [`TODO-1.md`](./TODO-1.md) | P0 | 删除现有 comptime/const surface 与实现，清空后续边界条件 | 已细化 |
| 2 | [`TODO-2.md`](./TODO-2.md) | P1 | 固定基础 crate 壳层、cone-level compilation unit 和 source-cone DAG 语义 | 已细化 |
| 3 | [`TODO-3.md`](./TODO-3.md) | P2 | 建立 `AST -> HIR` semantic frontend barrier 和独立 `hir_facts` | 已细化 |
| 4 | [`TODO-4.md`](./TODO-4.md) | P3 | 收口 MIR stage 输出，建立 `mir_facts` 与 MIR pass pipeline | 已细化 |
| 5 | [`TODO-5.md`](./TODO-5.md) | P4-P5 | 纯化 effect facts，正式收实 LIR 输出和 LIR optimization family | 已细化 |
| 6 | [`TODO-6.md`](./TODO-6.md) | P6-P8 | 闭合 global init model，清理 LLVM backend 输入边界，并做最终验证 | 已细化 |
| 7 | [`TODO-7.md`](./TODO-7.md) | P9-P10 | 把 stage/codegen 拆为独立 crate；落地 per-cone build artifact 并解决 cross-process TypeStore wire format | 已细化 |

## 具体任务索引

| 任务 | 状态 | 文件 | 目标 |
| --- | --- | --- | --- |
| P0-T01 | [DONE] | [`TODO-1.md`](./TODO-1.md#done-p0-t01删除-package-level-comptime-if-item-与裁剪路径) | 删除 package-level `comptime if` item 与裁剪路径 |
| P0-T01R | [DONE] | [`TODO-1.md`](./TODO-1.md#done-p0-t01rreview-package-level-comptime-删除结果) | Review package-level comptime 删除结果 |
| P0-T02 | [DONE] | [`TODO-1.md`](./TODO-1.md#done-p0-t02删除-statement-level-comptime-iffor-与-runtime-comptime-plan) | 删除 statement-level `comptime if/for` 与 runtime comptime plan |
| P0-T02R | [DONE] | [`TODO-1.md`](./TODO-1.md#done-p0-t02rreview-statement-level-comptime-删除结果) | Review statement-level comptime 删除结果 |
| P0-T03 | [DONE] | [`TODO-1.md`](./TODO-1.md#done-p0-t03删除-scoop-const-surfaceconst-evaluator-与跨阶段-const-hooks) | 删除 Scoop `const` surface、const evaluator 与跨阶段 const hooks |
| P0-T03R | [DONE] | [`TODO-1.md`](./TODO-1.md#done-p0-t03rreview-const-surface-与-evaluator-删除结果) | Review `const` surface 与 evaluator 删除结果 |
| P0-T04 | [DONE] | [`TODO-1.md`](./TODO-1.md#done-p0-t04p0-全仓清场与文档同步) | P0 全仓清场与文档同步 |
| P0-T04R | [DONE] | [`TODO-1.md`](./TODO-1.md#done-p0-t04rreview-p0-全包完成度) | Review P0 全包完成度 |
| TODO-2-INIT | [DONE] | [`TODO-2.md`](./TODO-2.md#done-todo-2-init初始化并细化本任务包) | 分析 P1 需求，生成 `TODO-2.md` 详细任务列表并更新本索引 |
| P1-T01 | [DONE] | [`TODO-2.md`](./TODO-2.md#done-p1-t01建立基础-crate-壳层与依赖门禁) | 建立基础 crate 壳层与依赖门禁 |
| P1-T01R | [DONE] | [`TODO-2.md`](./TODO-2.md#done-p1-t01rreview-基础-crate-壳层与依赖门禁) | Review 基础 crate 壳层与依赖门禁 |
| P1-T02 | [DONE] | [`TODO-2.md`](./TODO-2.md#done-p1-t02迁移-span-与-source-基础设施) | 迁移 `span` 与 `source` 基础设施 |
| P1-T02R | [DONE] | [`TODO-2.md`](./TODO-2.md#done-p1-t02rreview-span--source-迁移结果) | Review `span` / `source` 迁移结果 |
| P1-T03 | [DONE] | [`TODO-2.md`](./TODO-2.md#done-p1-t03迁移-types-并建立-ids-基础身份层) | 迁移 `types` 并建立 `ids` 基础身份层 |
| P1-T03R | [DONE] | [`TODO-2.md`](./TODO-2.md#done-p1-t03rreview-types--ids-迁移结果) | Review `types` / `ids` 迁移结果 |
| P1-T04 | [DONE] | [`TODO-2.md`](./TODO-2.md#done-p1-t04迁移-project-modelcone-graph-与-resolver-cone-identity) | 迁移 project model、cone graph 与 resolver cone identity |
| P1-T04R | [DONE] | [`TODO-2.md`](./TODO-2.md#done-p1-t04rreview-project-model-与-cone-graph-迁移结果) | Review project model 与 cone graph 迁移结果 |
| P1-T05 | [DONE] | [`TODO-2.md`](./TODO-2.md#done-p1-t05固定-cone-level-compilation-unit-facade-api) | 固定 cone-level compilation unit facade API |
| P1-T05R | [DONE] | [`TODO-2.md`](./TODO-2.md#done-p1-t05rreview-cone-level-compilation-unit-api) | Review cone-level compilation unit API |
| P1-T06 | [DONE] | [`TODO-2.md`](./TODO-2.md#done-p1-t06p1-全包清场文档同步与依赖审计) | P1 全包清场、文档同步与依赖审计 |
| P1-T06R | [DONE] | [`TODO-2.md`](./TODO-2.md#done-p1-t06rreview-p1-全包完成度) | Review P1 全包完成度 |
| TODO-3-INIT | [DONE] | [`TODO-3.md`](./TODO-3.md#done-todo-3-init初始化并细化本任务包) | 分析 P2 需求，生成 `TODO-3.md` 详细任务列表并更新本索引 |
| P2-T01 | [DONE] | [`TODO-3.md`](./TODO-3.md#done-p2-t01建立-hir_facts-crate-与事实数据模型) | 建立 `hir_facts` crate 与事实数据模型 |
| P2-T01R | [DONE] | [`TODO-3.md`](./TODO-3.md#done-p2-t01rreview-hir_facts-crate-与事实模型) | Review `hir_facts` crate 与事实模型 |
| P2-T02 | [DONE] | [`TODO-3.md`](./TODO-3.md#done-p2-t02固定-hirstageoutput--hir-hir_facts--输出形状) | 固定 `HirStageOutput = { hir, hir_facts }` 输出形状 |
| P2-T02R | [DONE] | [`TODO-3.md`](./TODO-3.md#done-p2-t02rreview-hir-stage-output-形状) | Review HIR stage output 形状 |
| P2-T03 | [DONE] | [`TODO-3.md`](./TODO-3.md#done-p2-t03移除-hir-反向携带-mir-materialization) | 移除 HIR 反向携带 MIR materialization |
| P2-T03R | [DONE] | [`TODO-3.md`](./TODO-3.md#done-p2-t03rreview-hirmir-单向边界) | Review HIR/MIR 单向边界 |
| P2-T04 | [DONE] | [`TODO-3.md`](./TODO-3.md#done-p2-t04迁移-declarationentity-facts-并收口-programfacts) | 迁移 declaration/entity facts 并收口 `ProgramFacts` |
| P2-T04R | [DONE] | [`TODO-3.md`](./TODO-3.md#done-p2-t04rreview-declarationentity-facts-迁移结果) | Review declaration/entity facts 迁移结果 |
| P2-T05 | [DONE] | [`TODO-3.md`](./TODO-3.md#done-p2-t05迁移-source-site-typed-contracts-并删除-fallback-双轨) | 迁移 source-site typed contracts 并删除 fallback 双轨 |
| P2-T05R | [DONE] | [`TODO-3.md`](./TODO-3.md#done-p2-t05rreview-source-site-contract-迁移结果) | Review source-site contract 迁移结果 |
| P2-T06 | [DONE] | [`TODO-3.md`](./TODO-3.md#done-p2-t06收口-hir-semantic-barrier-legality-gate-与错误边界) | 收口 HIR semantic barrier legality gate 与错误边界 |
| P2-T06R | [DONE] | [`TODO-3.md`](./TODO-3.md#done-p2-t06rreview-hir-semantic-barrier-与错误边界) | Review HIR semantic barrier 与错误边界 |
| P2-T07 | [DONE] | [`TODO-3.md`](./TODO-3.md#done-p2-t07p2-全包清场文档同步与依赖审计) | P2 全包清场、文档同步与依赖审计 |
| P2-T07R | [DONE] | [`TODO-3.md`](./TODO-3.md#done-p2-t07rreview-p2-全包完成度) | Review P2 全包完成度 |
| TODO-4-INIT | [DONE] | [`TODO-4.md`](./TODO-4.md#done-todo-4-init初始化并细化本任务包) | 分析 P3 需求，生成 `TODO-4.md` 详细任务列表并更新本索引 |
| P3-T01 | [DONE] | [`TODO-4.md`](./TODO-4.md#done-p3-t01建立-mir_facts-crate-与-mir-facts-数据模型) | 建立 `mir_facts` crate 与 MIR facts 数据模型 |
| P3-T01R | [DONE] | [`TODO-4.md`](./TODO-4.md#done-p3-t01rreview-mir_facts-crate-与事实模型) | Review `mir_facts` crate 与事实模型 |
| P3-T02 | [DONE] | [`TODO-4.md`](./TODO-4.md#done-p3-t02迁移-mir-owned-root-inventories-到-mir_facts) | 迁移 MIR-owned root inventories 到 `mir_facts` |
| P3-T02R | [DONE] | [`TODO-4.md`](./TODO-4.md#done-p3-t02rreview-mir-root-inventory-迁移结果) | Review MIR root inventory 迁移结果 |
| P3-T03 | [DONE] | [`TODO-4.md`](./TODO-4.md#done-p3-t03固定-canonical-materialized-snapshot-binding-与-pass-artifacts-查询面) | 固定 canonical materialized snapshot binding 与 pass artifacts 查询面 |
| P3-T03R | [DONE] | [`TODO-4.md`](./TODO-4.md#done-p3-t03rreview-mir-snapshot-binding-与-pass-artifacts-查询面) | Review MIR snapshot binding 与 pass artifacts 查询面 |
| P3-T04 | [DONE] | [`TODO-4.md`](./TODO-4.md#done-p3-t04切换下游-mir-查询到-mir_facts--pass-artifacts-surface) | 切换下游 MIR 查询到 `mir_facts` / pass artifacts surface |
| P3-T04R | [DONE] | [`TODO-4.md`](./TODO-4.md#done-p3-t04rreview-downstream-mir-query-切换结果) | Review downstream MIR query 切换结果 |
| P3-T05 | [DONE] | [`TODO-4.md`](./TODO-4.md#done-p3-t05建立显式-mir-pass-pipeline-与-refresh-顺序) | 建立显式 MIR pass pipeline 与 refresh 顺序 |
| P3-T05R | [DONE] | [`TODO-4.md`](./TODO-4.md#done-p3-t05rreview-显式-mir-pass-pipeline) | Review 显式 MIR pass pipeline |
| P3-T06 | [DONE] | [`TODO-4.md`](./TODO-4.md#done-p3-t06迁移-dispatch-去虚化到-mir-pass-并删除-hir-owner) | 迁移 dispatch 去虚化到 MIR pass 并删除 HIR owner |
| P3-T06R | [DONE] | [`TODO-4.md`](./TODO-4.md#done-p3-t06rreview-dispatch-去虚化-owner-迁移结果) | Review dispatch 去虚化 owner 迁移结果 |
| P3-T07 | [DONE] | [`TODO-4.md`](./TODO-4.md#done-p3-t07p3-全包清场文档同步与依赖审计) | P3 全包清场、文档同步与依赖审计 |
| P3-T07R | [DONE] | [`TODO-4.md`](./TODO-4.md#done-p3-t07rreview-p3-全包完成度) | Review P3 全包完成度 |
| TODO-5-INIT | [DONE] | [`TODO-5.md`](./TODO-5.md#done-todo-5-init初始化并细化本任务包) | 分析 P4-P5 需求，生成 `TODO-5.md` 详细任务列表并更新本索引 |
| P4-T01 | [DONE] | [`TODO-5.md`](./TODO-5.md#done-p4-t01建立独立-scoopc_effect_facts-crate-与事实数据模型) | 建立独立 `scoopc_effect_facts` crate 与事实数据模型 |
| P4-T01R | [DONE] | [`TODO-5.md`](./TODO-5.md#done-p4-t01rreview-scoopc_effect_facts-crate-与事实模型) | Review `scoopc_effect_facts` crate 与事实模型 |
| P4-T02 | [DONE] | [`TODO-5.md`](./TODO-5.md#done-p4-t02只读化-effect-facts-builder-与-effect-owned-type-context) | 只读化 effect facts builder 与 effect-owned type context |
| P4-T02R | [DONE] | [`TODO-5.md`](./TODO-5.md#done-p4-t02rreview-effect-facts-只读化结果) | Review effect facts 只读化结果 |
| P4-T03 | [DONE] | [`TODO-5.md`](./TODO-5.md#done-p4-t03收口-effectfactsstageoutput-与-p5-输入边界) | 收口 `EffectFactsStageOutput` 与 P5 输入边界 |
| P4-T03R | [DONE] | [`TODO-5.md`](./TODO-5.md#done-p4-t03rreview-effectfactsstageoutput-收口结果) | Review `EffectFactsStageOutput` 收口结果 |
| P4-T04 | [DONE] | [`TODO-5.md`](./TODO-5.md#done-p4-t04p4-全包清场文档同步与依赖审计) | P4 全包清场、文档同步与依赖审计 |
| P4-T04R | [DONE] | [`TODO-5.md`](./TODO-5.md#done-p4-t04rreview-p4-全包完成度) | Review P4 全包完成度 |
| P5-T01 | [DONE] | [`TODO-5.md`](./TODO-5.md#done-p5-t01建立-scoopc_lir_facts-crate-与正式-lirstageoutput-壳层) | 建立 `scoopc_lir_facts` crate 与正式 `LirStageOutput` 壳层 |
| P5-T01R | [DONE] | [`TODO-5.md`](./TODO-5.md#done-p5-t01rreview-lir_facts-crate-与-lir-output-壳层) | Review `lir_facts` crate 与 LIR output 壳层 |
| P5-T02 | [DONE] | [`TODO-5.md`](./TODO-5.md#done-p5-t02发布-lir-callabledynamic-invokedispatch-与-resume-contracts) | 发布 LIR callable、dynamic invoke、dispatch 与 resume contracts |
| P5-T02R | [DONE] | [`TODO-5.md`](./TODO-5.md#done-p5-t02rreview-lir-contract-与-facts-完整度) | Review LIR contract 与 facts 完整度 |
| P5-T03 | [DONE] | [`TODO-5.md`](./TODO-5.md#done-p5-t03切换-codegen-neutral-abiquery-surface-到-lir--lir_facts) | 切换 codegen-neutral ABI/query surface 到 `LIR + lir_facts` |
| P5-T03R | [DONE] | [`TODO-5.md`](./TODO-5.md#done-p5-t03rreview-codegen-neutral-query-切换结果) | Review codegen-neutral query 切换结果 |
| P5-T04 | [DONE] | [`TODO-5.md`](./TODO-5.md#done-p5-t04建立正式-lir-optimization-family-与-pass-pipeline) | 建立正式 LIR optimization family 与 pass pipeline |
| P5-T04R | [DONE] | [`TODO-5.md`](./TODO-5.md#done-p5-t04rreview-lir-optimization-family) | Review LIR optimization family |
| P5-T05 | [DONE] | [`TODO-5.md`](./TODO-5.md#done-p5-t05p5-全包清场文档同步与依赖审计) | P5 全包清场、文档同步与依赖审计 |
| P5-T05R | [DONE] | [`TODO-5.md`](./TODO-5.md#done-p5-t05rreview-p5-全包完成度) | Review P5 全包完成度 |
| TODO-6-INIT | [DONE] | [`TODO-6.md`](./TODO-6.md#done-todo-6-init初始化并细化本任务包) | 分析 P6-P8 需求，生成 `TODO-6.md` 详细任务列表并更新本索引 |
| P6-T01 | [DONE] | [`TODO-6.md`](./TODO-6.md#done-p6-t01发布-global-init-与-storage-lir-facts-contract) | 发布 global init 与 storage LIR facts contract |
| P6-T01R | [DONE] | [`TODO-6.md`](./TODO-6.md#done-p6-t01rreview-global-initstorage-lir-facts-contract) | Review global init/storage LIR facts contract |
| P6-T02 | [DONE] | [`TODO-6.md`](./TODO-6.md#done-p6-t02实现-per-cone-eager-top-level-init-与-final-entry-order) | 实现 per-cone eager top-level init 与 final entry order |
| P6-T02R | [DONE] | [`TODO-6.md`](./TODO-6.md#done-p6-t02rreview-per-cone-eager-top-level-init-与-final-entry-order) | Review per-cone eager top-level init 与 final entry order |
| P6-T03 | [DONE] | [`TODO-6.md`](./TODO-6.md#done-p6-t03分离-object-once-与-global--threadlocal-storage-policy) | 分离 object once 与 `@Global` / `@ThreadLocal` storage policy |
| P6-T03R | [DONE] | [`TODO-6.md`](./TODO-6.md#done-p6-t03rreview-object-once-与-storage-policy-分离结果) | Review object once 与 storage policy 分离结果 |
| P6-T04 | [DONE] | [`TODO-6.md`](./TODO-6.md#done-p6-t04p6-全包清场文档同步与依赖审计) | P6 全包清场、文档同步与依赖审计 |
| P6-T04R | [DONE] | [`TODO-6.md`](./TODO-6.md#done-p6-t04rreview-p6-全包完成度) | Review P6 全包完成度 |
| P7-T01 | [DONE] | [`TODO-6.md`](./TODO-6.md#done-p7-t01迁移-llvm-entryglobal-查询到-lir-facts) | 迁移 LLVM entry/global 查询到 LIR facts |
| P7-T01R | [DONE] | [`TODO-6.md`](./TODO-6.md#done-p7-t01rreview-llvm-entryglobal-lir-facts-迁移结果) | Review LLVM entry/global LIR facts 迁移结果 |
| P7-T02 | [DONE] | [`TODO-6.md`](./TODO-6.md#done-p7-t02删除-backend-reachability-hirmir-回看与-codegen-去虚化-residual) | 删除 backend reachability HIR/MIR 回看与 codegen 去虚化 residual |
| P7-T02-a | [DONE] | [`TODO-6.md`](./TODO-6.md#done-p7-t02-a修复-run-pass-fixture-baseline-失败) | 修复 run-pass fixture baseline 失败 |
| P7-T02R | [DONE] | [`TODO-6.md`](./TODO-6.md#done-p7-t02rreview-backend-reachability-cleanup) | Review backend reachability cleanup |
| P7-T03 | [DONE] | [`TODO-6.md`](./TODO-6.md#done-p7-t03迁移-llvm-body-emission-离开-raw-mir--hir-fallback) | 迁移 LLVM body emission 离开 raw MIR / HIR fallback |
| P7-T03R | [DONE] | [`TODO-6.md`](./TODO-6.md#done-p7-t03rreview-llvm-body-emission-迁移结果) | Review LLVM body emission 迁移结果 |
| P7-T04-a | [DONE] | [`TODO-6.md`](./TODO-6.md#done-p7-t04-a发布-llvm-backend-收口所需的-lirbase-context-合同) | 发布 LLVM backend 收口所需的 LIR/base context 合同 |
| P7-T04-b-1 | [DONE] | [`TODO-6.md`](./TODO-6.md#done-p7-t04-b-1引入-monotypeid-与-monotypekind--codegen-输入类型纪律基线) | 引入 `MonoTypeId` 与 `MonoTypeKind` —— codegen 输入类型纪律基线 |
| P7-T04-b-1R | [DONE] | [`TODO-6.md`](./TODO-6.md#done-p7-t04-b-1rreview-monotypeid-类型纪律基线) | Review `MonoTypeId` 类型纪律基线 |
| P7-T04-b-2 | [DONE] | [`TODO-6.md`](./TODO-6.md#done-p7-t04-b-2拆分-hirclassinit-为-genericclassdecl-与-monoclassinit) | 拆分 `hir::ClassInit` 为 `GenericClassDecl` 与 `MonoClassInit` |
| P7-T04-b-2R | [DONE] | [`TODO-6.md`](./TODO-6.md#done-p7-t04-b-2rreview-classinit-拆分) | Review `ClassInit` 拆分 |
| P7-T04-b-3 | [DONE] | [`TODO-6.md`](./TODO-6.md#done-p7-t04-b-3引入-classinstancekey-收回-layout-key-字符串形态) | 引入 `ClassInstanceKey` 收回 layout key 字符串形态 |
| P7-T04-b-3R | [DONE] | [`TODO-6.md`](./TODO-6.md#done-p7-t04-b-3rreview-classinstancekey-字符串形态收回) | Review `ClassInstanceKey` 字符串形态收回 |
| P7-T04-b-4 | [DONE] | [`TODO-6.md`](./TODO-6.md#done-p7-t04-b-4codegen-全面切换到-monotypeid--删除-cg_ty_of-的-option-与-expect_cg_ty_of) | codegen 全面切换到 `MonoTypeId` —— 删除 `cg_ty_of` 的 `Option` 与 `expect_cg_ty_of` |
| P7-T04-b-4R | [DONE] | [`TODO-6.md`](./TODO-6.md#done-p7-t04-b-4rreview-codegen-monotypeid-全面切换) | Review codegen `MonoTypeId` 全面切换 |
| P7-T04-b-5 | [DONE] | [`TODO-6.md`](./TODO-6.md#done-p7-t04-b-5修复-p7-t04-b-期间观察到的预存-llvm-库测试失败) | 修复 P7-T04-b 期间观察到的预存 LLVM 库测试失败 |
| P7-T04-b-5R | [DONE] | [`TODO-6.md`](./TODO-6.md#done-p7-t04-b-5rreview-预存-llvm-库测试失败修复结果) | Review 预存 LLVM 库测试失败修复结果 |
| P7-T04-b | [DONE] | [`TODO-6.md`](./TODO-6.md#done-p7-t04-b收窄-llvm-stage-handoff-形状) | 收窄 LLVM stage handoff 形状 |
| P7-T04-bR | [DONE] | [`TODO-6.md`](./TODO-6.md#done-p7-t04-brreview-llvm-stage-handoff-形状收窄) | Review LLVM stage handoff 形状收窄 |
| P7-T04-c | [DONE] | [`TODO-6.md`](./TODO-6.md#done-p7-t04-c迁移-physical-abilayout-查询面到-lir-facts) | 迁移 physical ABI/layout 查询面到 LIR facts |
| P7-T04-cR | [DONE] | [`TODO-6.md`](./TODO-6.md#done-p7-t04-crreview-physical-abilayout-迁移结果) | Review physical ABI/layout 迁移结果 |
| P7-T04 | [DONE] | [`TODO-6.md`](./TODO-6.md#done-p7-t04收尾llvm-stage-handoff-与-physical-abi-cleanup-合并验证) | 收尾——LLVM stage handoff 与 physical ABI cleanup 合并验证 |
| P7-T04R | [DONE] | [`TODO-6.md`](./TODO-6.md#done-p7-t04rreview-llvm-stage-handoff-与-physical-abi-cleanup) | Review LLVM stage handoff 与 physical ABI cleanup |
| P7-T05 | [DONE] | [`TODO-6.md`](./TODO-6.md#done-p7-t05p7-全包清场文档同步与依赖审计) | P7 全包清场、文档同步与依赖审计 |
| P7-T05-a | [DONE] | [`TODO-6.md`](./TODO-6.md#done-p7-t05-a清除-p7-t05r-发现的-llvm-codegen-production-residual) | 清除 P7-T05R 发现的 LLVM codegen production residual |
| P7-T05-b-0 | [DONE] | [`TODO-6.md`](./TODO-6.md#done-p7-t05-b-0发布-lir-owned-class-ctor-init-body-contract) | 发布 LIR-owned class ctor init body contract |
| P7-T05-b | [DONE] | [`TODO-6.md`](./TODO-6.md#done-p7-t05-b清除-p7-t05r-发现的-hir-derived-callable-与-class-ctor-residual) | 清除 P7-T05R 发现的 HIR-derived callable 与 class ctor residual |
| P7-T05-c | [DONE] | [`TODO-6.md`](./TODO-6.md#done-p7-t05-c清除-p7-t05r-发现的最终-llvm-hirbase-context-residual) | 清除 P7-T05R 发现的最终 LLVM HIR/base-context residual |
| P7-T05R | [DONE] | [`TODO-6.md`](./TODO-6.md#done-p7-t05rreview-p7-全包完成度) | Review P7 全包完成度 |
| P8-T01 | [DONE] | [`TODO-6.md`](./TODO-6.md#done-p8-t01最终-residual-搜索文档冻结与未来-c-backend-输入边界) | 最终 residual 搜索、文档冻结与未来 C backend 输入边界 |
| P8-T01R | [DONE] | [`TODO-6.md`](./TODO-6.md#done-p8-t01rreview-final-residual-搜索与文档冻结) | Review final residual 搜索与文档冻结 |
| P8-T02 | [DONE] | [`TODO-6.md`](./TODO-6.md#done-p8-t02最终全仓验证与-release-readiness-清场) | 最终全仓验证与 release readiness 清场 |
| P8-T02R | [DONE] | [`TODO-6.md`](./TODO-6.md#done-p8-t02rreview-final-verification-与-release-readiness) | Review final verification 与 release readiness |
| TODO-7-INIT | [DONE] | [`TODO-7.md`](./TODO-7.md#done-todo-7-init初始化并细化本任务包) | 分析 P9-P10 需求，生成 `TODO-7.md` 详细任务列表并更新本索引 |
| P9-T01-a | [DONE] | [`TODO-7.md`](./TODO-7.md#done-p9-t01-a修复-p9-t01-前置的-llvmhirmir-residual-baseline) | 修复 P9-T01 前置的 LLVM/HIR-MIR residual baseline |
| P9-T01 | [DONE] | [`TODO-7.md`](./TODO-7.md#done-p9-t01消除阻塞-stage-crate-split-的后向边) | 消除阻塞 stage crate split 的后向边 |
| P9-T01R | [DONE] | [`TODO-7.md`](./TODO-7.md#done-p9-t01rreview-后向边消除结果) | Review 后向边消除结果 |
| P9-T02 | [DONE] | [`TODO-7.md`](./TODO-7.md#done-p9-t02抽出-scoopc_ast-crate) | 抽出 `scoopc_ast` crate |
| P9-T02R | [DONE] | [`TODO-7.md`](./TODO-7.md#done-p9-t02rreview-scoopc_ast-抽取) | Review `scoopc_ast` 抽取 |
| P9-T03 | [DONE] | [`TODO-7.md`](./TODO-7.md#done-p9-t03抽出-scoopc_codegen_llvm-crate) | 抽出 `scoopc_codegen_llvm` crate |
| P9-T03R | [DONE] | [`TODO-7.md`](./TODO-7.md#done-p9-t03rreview-scoopc_codegen_llvm-抽取) | Review `scoopc_codegen_llvm` 抽取 |
| P9-T04 | [DONE] | [`TODO-7.md`](./TODO-7.md#done-p9-t04抽出-scoopc_hir-crate) | 抽出 `scoopc_hir` crate |
| P9-T04R | [DONE] | [`TODO-7.md`](./TODO-7.md#done-p9-t04rreview-scoopc_hir-抽取) | Review `scoopc_hir` 抽取 |
| P9-T05 | [DONE] | [`TODO-7.md`](./TODO-7.md#done-p9-t05抽出-scoopc_mir-crate) | 抽出 `scoopc_mir` crate |
| P9-T05R | [DONE] | [`TODO-7.md`](./TODO-7.md#done-p9-t05rreview-scoopc_mir-抽取) | Review `scoopc_mir` 抽取 |
| P9-T06-a | [DONE] | [`TODO-7.md`](./TODO-7.md#done-p9-t06-a收窄-lir-的-hirast-source-payload-边界) | 收窄 LIR 的 HIR/AST source payload 边界 |
| P9-T06-b | [DONE] | [`TODO-7.md`](./TODO-7.md#done-p9-t06-b发布-lir-owned-ordinary-callee-suspend-合同) | 发布 LIR-owned ordinary-callee suspend 合同 |
| P9-T06-c | [DONE] | [`TODO-7.md`](./TODO-7.md#done-p9-t06-c发布-codegen-owned-llvm-stage-handoff-合同) | 发布 codegen-owned LLVM stage handoff 合同 |
| P9-T06 | [DONE] | [`TODO-7.md`](./TODO-7.md#done-p9-t06抽出-scoopc_effect_facts_stage-与-scoopc_lir-crate) | 抽出 `scoopc_effect_facts_stage` 与 `scoopc_lir` crate |
| P9-T06R | [DONE] | [`TODO-7.md`](./TODO-7.md#done-p9-t06rreview-scoopc_effect_facts_stage-与-scoopc_lir-抽取) | Review `scoopc_effect_facts_stage` 与 `scoopc_lir` 抽取 |
| P9-T07 | [DONE] | [`TODO-7.md`](./TODO-7.md#done-p9-t07cone-两层拆分scoopc_project_model-扩展--新-scoopc_cone) | cone 两层拆分（`scoopc_project_model` 扩展 + 新 `scoopc_cone`） |
| P9-T07R | [DONE] | [`TODO-7.md`](./TODO-7.md#done-p9-t07rreview-cone-两层拆分) | Review cone 两层拆分 |
| P9-T08 | [DONE] | [`TODO-7.md`](./TODO-7.md#done-p9-t08scoopc-umbrella-crate-收尾--dependency_gate-全面强化) | `scoopc` umbrella crate 收尾 + dependency_gate 全面强化 |
| P9-T08R | [DONE] | [`TODO-7.md`](./TODO-7.md#done-p9-t08rreview-umbrella-收尾) | Review umbrella 收尾 |
| P9-T09 | [DONE] | [`TODO-7.md`](./TODO-7.md#done-p9-t09p9-全包清场文档同步与依赖审计) | P9 全包清场、文档同步与依赖审计 |
| P9-T09R | [DONE] | [`TODO-7.md`](./TODO-7.md#done-p9-t09rreview-p9-全包完成度) | Review P9 全包完成度 |
| P10-T01 | [DONE] | [`TODO-7.md`](./TODO-7.md#done-p10-t01解决-typeid-cross-process-stable-wire-format) | 解决 `TypeId` cross-process stable wire format |
| P10-T01R | [DONE] | [`TODO-7.md`](./TODO-7.md#done-p10-t01rreview-typestore-wire-format) | Review TypeStore wire format |
| P10-T02 | [DONE] | [`TODO-7.md`](./TODO-7.md#done-p10-t02定义-per-cone-build-artifact-磁盘布局与-scoopc_cone-读写-api) | 定义 per-cone build artifact 磁盘布局与 `scoopc_cone` 读写 API |
| P10-T02R | [DONE] | [`TODO-7.md`](./TODO-7.md#done-p10-t02rreview-per-cone-artifact-schema) | Review per-cone artifact schema |
| P10-T03-a | [DONE] | [`TODO-7.md`](./TODO-7.md#done-p10-t03-a补齐-coneartifact-frontend-import-payload) | 补齐 `ConeArtifact` frontend import payload |
| P10-T03 | [DONE] | [`TODO-7.md`](./TODO-7.md#done-p10-t03run_frontend-改造为按-cone-dag-拓扑顺序运行) | `run_frontend` 改造为按 cone DAG 拓扑顺序运行 |
| P10-T03R | [DONE] | [`TODO-7.md`](./TODO-7.md#done-p10-t03rreview-per-cone-frontend-orchestration) | Review per-cone frontend orchestration |
| P10-T04-a | [TODO] | [`TODO-7.md`](./TODO-7.md#todo-p10-t04-a补齐-per-cone-artifact-cache-handoff-边界) | 补齐 per-cone artifact cache handoff 边界 |
| P10-T04 | [TODO] | [`TODO-7.md`](./TODO-7.md#todo-p10-t04per-cone-fingerprint-cache--增量-build) | per-cone fingerprint cache + 增量 build |
| P10-T04R | [TODO] | [`TODO-7.md`](./TODO-7.md#todo-p10-t04rreview-per-cone-fingerprint-cache) | Review per-cone fingerprint cache |
| P10-T05 | [TODO] | [`TODO-7.md`](./TODO-7.md#todo-p10-t05p10-全包清场文档同步与依赖审计) | P10 全包清场、文档同步与依赖审计 |
| P10-T05R | [TODO] | [`TODO-7.md`](./TODO-7.md#todo-p10-t05rreview-p10-全包完成度) | Review P10 全包完成度 |

## 包间验收门禁

- 进入 `TODO-2.md` 前，仓库中不得再存在现有 Scoop comptime/const surface 的主线实现或专门兼容逻辑。
- 进入 `TODO-3.md` 前，基础 crate 和 cone-level compilation unit 的概念必须已经能被后续 stage/fact crate 直接引用。
- 进入 `TODO-4.md` 前，`HIR` 与 `hir_facts` 必须已经能表达后续阶段需要的所有静态源码语义事实。
- 进入 `TODO-5.md` 前，`MirStageOutput = { mir, mir_facts }` 或等价窄输出语义必须成立。
- 进入 `TODO-6.md` 前，effect facts 不得修改 MIR 输出本体，且 LIR 必须是 codegen 的唯一 authoritative IR 输入候选。
- 完成 `TODO-6.md` 后，LLVM backend 和未来 C backend 应共享同一套 `LIR + LIR facts + base context` 输入边界。
- 进入 `TODO-7.md` 前，P0-P8 必须全部完成（含 P8-T02R）；LLVM backend 不得仍依赖 HIR/raw MIR/effect facts wrapper；`dependency_gate` 已对所有 base + fact crate 强制门禁。
- 完成 `TODO-7.md` 后，每个 stage 都是独立 crate 且依赖方向由 `cargo build` 强制；下游 cone 可消费上游 cone 的 `build/<profile>/cones/<cone>/` artifact 而不再扫上游源；per-cone fingerprint chain 替代旧的整项目 fingerprint。
