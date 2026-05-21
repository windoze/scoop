# Pipeline Refactor TODO 索引

> 生成时间：2026-05-21
> 计划基线：[`PLAN.md`](./PLAN.md)
> 设计基线：[`PIPELINE_REFACTOR.md`](./PIPELINE_REFACTOR.md)
> 审计基线：[`PIPELINE-CLEANUP.md`](./PIPELINE-CLEANUP.md)
> 当前状态：任务包已划分；`TODO-1.md`、`TODO-2.md`、`TODO-3.md` 与 `TODO-4.md` 已细化，其它任务包暂为范围说明。

## 总原则

- `PLAN.md` 是当前执行计划基线；如果实现时发现阶段边界、crate DAG、facts 归属或全局初始化语义需要改变，必须先回写 `PIPELINE_REFACTOR.md`，再调整 TODO。
- 所有任务按 `TODO-1.md` 到 `TODO-6.md` 顺序推进；除非对应文件明确允许，不跨包并行实现。
- 每个实现小阶段后必须紧跟一个独立 review 任务，复审该小阶段的完整变更、阶段目标和约束遵守情况。
- review 任务不是形式检查；如果发现前一任务没有真正完成目标，review 任务必须直接修正或阻塞下一任务。
- 任务完成后必须同时更新 `TODO.md` 和对应 `TODO-[1-6].md` 中的任务状态与完成记录；不得只更新其中一边。
- 后续生成新任务时，正式任务编号统一使用 `P[1-6]-NN` 格式；如果执行过程中发现某个任务过于复杂、需要进一步拆解，则使用 `P[1-6]-NN-[a-z]` 格式。
- 任何 fact crate 不得依赖 stage crate 或其它 fact crate；任何 stage output 不得长期嵌套上一阶段完整输出。
- HIR 不承载 optimization pass；MIR 承载普通调用图/实例级优化；LIR 承载 effect/control 相关窄优化；codegen 只承载 backend-specific 优化。

## 任务包划分

| 包 | 文件 | 覆盖 PLAN 阶段 | 目标 | 当前细化状态 |
| --- | --- | --- | --- | --- |
| 1 | [`TODO-1.md`](./TODO-1.md) | P0 | 删除现有 comptime/const surface 与实现，清空后续边界条件 | 已细化 |
| 2 | [`TODO-2.md`](./TODO-2.md) | P1 | 固定基础 crate 壳层、cone-level compilation unit 和 source-cone DAG 语义 | 已细化 |
| 3 | [`TODO-3.md`](./TODO-3.md) | P2 | 建立 `AST -> HIR` semantic frontend barrier 和独立 `hir_facts` | 已细化 |
| 4 | [`TODO-4.md`](./TODO-4.md) | P3 | 收口 MIR stage 输出，建立 `mir_facts` 与 MIR pass pipeline | 已细化 |
| 5 | [`TODO-5.md`](./TODO-5.md) | P4-P5 | 纯化 effect facts，正式收实 LIR 输出和 LIR optimization family | 暂为范围说明 |
| 6 | [`TODO-6.md`](./TODO-6.md) | P6-P8 | 闭合 global init model，清理 LLVM backend 输入边界，并做最终验证 | 暂为范围说明 |

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
| TODO-5-INIT | [TODO] | [`TODO-5.md`](./TODO-5.md#todo-todo-5-init初始化并细化本任务包) | 分析 P4-P5 需求，生成 `TODO-5.md` 详细任务列表并更新本索引 |
| TODO-6-INIT | [TODO] | [`TODO-6.md`](./TODO-6.md#todo-todo-6-init初始化并细化本任务包) | 分析 P6-P8 需求，生成 `TODO-6.md` 详细任务列表并更新本索引 |

## 包间验收门禁

- 进入 `TODO-2.md` 前，仓库中不得再存在现有 Scoop comptime/const surface 的主线实现或专门兼容逻辑。
- 进入 `TODO-3.md` 前，基础 crate 和 cone-level compilation unit 的概念必须已经能被后续 stage/fact crate 直接引用。
- 进入 `TODO-4.md` 前，`HIR` 与 `hir_facts` 必须已经能表达后续阶段需要的所有静态源码语义事实。
- 进入 `TODO-5.md` 前，`MirStageOutput = { mir, mir_facts }` 或等价窄输出语义必须成立。
- 进入 `TODO-6.md` 前，effect facts 不得修改 MIR 输出本体，且 LIR 必须是 codegen 的唯一 authoritative IR 输入候选。
- 完成 `TODO-6.md` 后，LLVM backend 和未来 C backend 应共享同一套 `LIR + LIR facts + base context` 输入边界。
