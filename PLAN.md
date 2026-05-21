# Pipeline Refactor 执行计划

> 生成时间：2026-05-20
> 设计基线：[`PIPELINE_REFACTOR.md`](./PIPELINE_REFACTOR.md)
> 审计基线：[`PIPELINE-CLEANUP.md`](./PIPELINE-CLEANUP.md)
> 归档前置基线：
> - [`docs/archive/designs/SYSROOT_RESHAPE_R2.md`](./docs/archive/designs/SYSROOT_RESHAPE_R2.md)
> - [`docs/archive/plans/PLAN-sysroot-reshape-r2.md`](./docs/archive/plans/PLAN-sysroot-reshape-r2.md)
> - [`docs/archive/plans/TODO-sysroot-reshape-r2.md`](./docs/archive/plans/TODO-sysroot-reshape-r2.md)
> 当前状态：P2 HIR barrier / `hir_facts` 收口进入清场验收；下一步进入 `TODO-4.md` / P3 初始化

## 0. 目标

本轮的目标不是继续在现有 `scoopc` 单 crate 上局部修补，而是把编译器收口成清晰的阶段图、facts 图和 crate DAG。

目标主线是：

```text
AST -> HIR -> MIR -> effect facts -> LIR -> codegen
```

其中：

1. `effect_lowered` 被正式视为 LIR。
2. `AST -> HIR` 被正式视为 cone-level semantic frontend barrier。
3. `codegen` 的唯一 authoritative IR 输入是 `LIR`。
4. 所有静态可判定的 Scoop 源码错误，目标上都必须在 `AST -> HIR` 之前收口。

## 1. 硬约束

### 1.1 阶段边界

1. 每个阶段只消费前一阶段的 authoritative 输出，以及更早阶段发布的 facts。
2. 后续阶段不得回头重跑上游阶段。
3. 后续阶段不得把上游 `StageOutput` 整包嵌套进自己的输出中作为长期接口。

### 1.2 crate 依赖

1. stage crate 只允许依赖：
   - 基础 crate
   - 自己的 fact crate
   - 前一个阶段 crate
   - 更早阶段的 fact crate
2. fact crate 只允许依赖基础 crate。
3. fact crate 不得依赖任何 stage crate，也不得依赖任何其它 fact crate。

### 1.3 facts 规则

1. 每个阶段发布的 facts 必须有独立含义。
2. facts 不能简单搬运、重新导出或嵌套别的 facts/stage outputs。
3. 下游不得把两个 fact table 当作可替代输入；若出现替代关系，要么是 facts 不完整，要么是下游设计错误。

### 1.4 编译单元与编译顺序

1. 编译单元 = 一个 cone 的全部 Scoop 源文件。
2. 整个 build 输入 = 多个 cone-level compilation unit 构成的 source-cone DAG。
3. cone 之间按 DAG 拓扑顺序编译。
4. cone 内不定义语义性的文件编译顺序，只定义阶段屏障。
5. 文件顺序只用于稳定性，不用于语义。

### 1.5 前端语义约束

1. `global object / top-level val / top-level var` 不能是 generic。
2. `@CallingConvention` 函数不能是 generic。
3. `top-level var` 必须显式标注 `@Global` 或 `@ThreadLocal`。

### 1.6 comptime 范围裁剪

1. 现有 comptime surface 与实现整体移除。
2. 这里应包括现有 `const` 相关语义点。
3. 不保留“旧 comptime surface 的专门 reject 逻辑”；移除后相关代码应自然以普通 parse/resolve/typecheck 失败暴露。

### 1.7 优化框架

1. HIR 不承载 optimization pass，只承载语义规范化。
2. MIR 承载 backend-neutral 的普通调用图/控制流/实例级优化。
3. LIR 承载 effect/control 相关的窄优化 family。
4. codegen 只承载 backend-specific 的 target/physical IR 优化。

## 2. 当前问题摘要

详细问题见 `PIPELINE-CLEANUP.md`。这里只提炼执行计划直接要处理的几类根问题：

1. P2 已让 `HirStageOutput = { hir, hir_facts }` 成立，并移除 HIR-carried MIR snapshot、HIR typed contract bridge 与 `ProgramFacts` 重叠 owner。
2. `MirStageOutput` 仍混有 direct-style `LoweredMir`、MIR-owned root inventories 和 optional `MaterializedMir`，尚未收口为 `{ mir, mir_facts }`。
3. `EffectFactsStageOutput` 和 `EffectLoweredStageOutput` 都还在嵌套上游输出，而不是只发布本阶段产物。
4. MIR/codegen 各层仍残留优化性逻辑，尤其是去虚化与内联分散在多处。
5. codegen 仍直接依赖 HIR compatibility scaffold、raw MIR/pass view、effect facts 和 LIR 的混合输入。

## 3. 阶段总览

| 阶段 | 名称 | 目标 |
| --- | --- | --- |
| P0 | Remove current comptime | 先移除现有 comptime/const surface 与实现，清空边界条件 |
| P1 | Base crates + cone unit model | 固定基础 crate、cone-level compilation unit 和 source-cone DAG 定义 |
| P2 | HIR barrier + hir_facts | 把所有静态源码语义收口到 `AST -> HIR`，建立独立 `hir_facts` |
| P3 | MIR boundary + MIR pass pipeline | 收口 `MirStageOutput`，建立 `mir_facts` 与正式 MIR analysis/opt pipeline |
| P4 | effect facts purity | 让 effect facts 变成真正只读分析输出，不修改 MIR 输出本体 |
| P5 | LIR output + LIR opt family | 把 `effect_lowered` 收实为正式 LIR，并发布独立 `lir_facts` |
| P6 | Global init model | 落实 object once、top-level eager init、per-cone init routine 和 storage policy |
| P7 | LLVM backend cleanup | 让 LLVM backend 只依赖 `LIR + LIR facts + base context` |
| P8 | Final verification | 清理残余、冻结边界、为未来 C backend 预留干净接口 |

阶段间原则上顺序推进；只有在不破坏上述依赖关系时，才允许在相邻阶段间拆小步 PR。

## 4. 各阶段计划

### P0：Remove current comptime

目标：在正式讨论任何 stage/fact crate 之前，先把现有 comptime 整体移出主线。

必须完成：

1. 删除 package-level `comptime if` 裁剪路径。
2. 删除 runtime comptime plan。
3. 删除 `const` 相关语言 surface 与其 lowering/codegen 支持。
4. 删除只为 comptime 保留的跨阶段特判、占位节点和绕路接口。

完成标准：

1. 正式 pipeline 中不再保留现有 comptime/const surface。
2. 代码库中不再存在“为了兼容旧 comptime”而保留的专门逻辑。
3. `PIPELINE_REFACTOR.md` 中的主线阶段图不再依赖任何 comptime 特例。

### P1：Base crates + cone compilation unit model

目标：先把基础层和编译单元模型固定下来，给后续 stage/fact crate 留出落点。

必须完成：

1. 设计并落地基础 crate 壳层：
   - `span`
   - `source`
   - `types`
   - `ids`
   - `project_model`
2. 明确 `ProjectInput` / `ProjectContext` / `SourceConeGraph` 在未来 architecture 中的角色。
3. 明确 “cone = compilation unit” 的正式含义，并让 facade 层按这个概念组织后续 API。

完成标准：

1. 后续 stage/fact crate 都有明确的基础依赖层。
2. 文档和代码中不再把“单文件”当正式 compilation unit 定义。

### P2：HIR barrier + hir_facts

目标：把 `AST -> HIR` 变成真正的 semantic frontend barrier，并建立独立 `hir_facts`。

必须完成：

1. 收口所有静态可判定的源码语义错误到 HIR 屏障。
2. 从 `LoweredHir` 拆出正式 `hir_facts`。
3. 把当前分散在：
   - `LoweredHir` side tables
   - `TypedHirEffectContracts`
   - `ProgramFacts`
   中的职责重新分配。
4. 明确 HIR 阶段的 declaration legality 约束：
   - `@CallingConvention` non-generic
   - global roots non-generic
   - `top-level var` storage policy gate
5. 保证 HIR 内部若需要 CFG 式分析，也只使用 HIR-owned semantic CFG，不依赖 MIR crate。

完成标准：

1. `HirStageOutput = { hir, hir_facts }` 语义成立。
2. `LoweredHir` 不再扮演多阶段 bundle。
3. 通过 HIR 屏障后，后续阶段不再报新的普通源码语义错误。

### P3：MIR boundary + MIR pass pipeline

目标：把 MIR 从“materialization 附带物”变成正式阶段，并建立清晰的 MIR analysis/optimization 流水线。

必须完成：

1. `MirStageOutput` 只发布 MIR-owned 产物。
2. 在 P2 已移除 HIR typed contract 泄漏的基础上，收口 optional materialized snapshot、root inventories 与 pass artifacts。
3. 建立独立 `mir_facts` / pass artifacts 查询面。
4. 把现有优化逻辑重排成显式 MIR pipeline：
   - escape analysis（analysis/facts）
   - devirtualization（optimization）
   - summary-driven inlining（optimization）
   - closure simplification（optimization）
   - 必要的 cleanup / summary refresh
5. 删除 HIR 层 dispatch 去虚化。

完成标准：

1. `MirStageOutput = { mir, mir_facts }` 语义成立。
2. MIR 去虚化/内联只有一个 authoritative owner。
3. MIR pass 不再只是 materializer 尾部的隐式开关。

### P4：effect facts purity

目标：把 effect facts 变成真正的只读分析输出，而不是会修改 MIR 输出本体的半分析半改写层。

必须完成：

1. effect facts stage 对 MIR 输入只读。
2. 若它需要额外 type context 或 analysis-owned context，这些内容作为自己的输出发布，而不是写回 MIR。
3. `EffectFactsStageOutput` 不再嵌套整份 `MirStageOutput` 作为长期接口。
4. `effect_facts` crate 只承载 effect/control 合同本身。

完成标准：

1. `EffectFactsStageOutput = { effect_facts }` 成立，或等价的窄输出形式成立。
2. effect facts stage 不再修改 `MirStageOutput` 本体。

### P5：LIR output + LIR opt family

目标：把 `effect_lowered` 收实为正式 LIR，并补齐 codegen 需要的 backend-neutral 合同。

必须完成：

1. `EffectLoweredStageOutput` 退化成正式 `LirStageOutput`。
2. 建立独立 `lir_facts` / query layer。
3. LIR 输出补齐：
   - plain callable surface
   - effect-step callable contract
   - dynamic invoke contract
   - dispatch owner/slot selection结果
   - continuation/resume publication
4. 建立正式的 `LIR optimization family`：
   - local state-machine elimination
   - 简单 higher-order wrapper 的定向 inline/devirt
   - wrapper state folding
   - dead state / dead slot cleanup

完成标准：

1. codegen 不再为了 plain callable / dynamic invoke / dispatch 去回看 raw MIR/HIR。
2. LIR optimization 被明确为一组窄 pass，而不是散落在 lowering/codegen 里的特判。

### P6：Global init model

目标：把全局初始化模型正式落到 HIR/MIR/LIR contract 与 codegen entry 之上。

必须完成：

1. 明确并实现全局初始化根分类：
   - `object`
   - top-level `val`
   - annotated top-level `var`
2. object once 语义与 top-level eager init 语义彻底分离。
3. per-cone init routine 与 final entry init order contract 在 LIR/codegen 之间闭合。
4. `@Global` / `@ThreadLocal` storage policy 全链路闭合。
5. global roots non-generic 约束在 HIR 屏障内稳定拒绝。

完成标准：

1. top-level values 不再 lazy first-access。
2. 所有 linked cones 的 top-level init 在 `main` 前完成。
3. object once 只服务 object，不再和 top-level init 共享旧语义路径。

### P7：LLVM backend cleanup

目标：让 LLVM backend 只依赖 `LIR + LIR facts + base context`。

必须完成：

1. 删除 codegen 层 dispatch 去虚化 fast path。
2. 删除 reachability 层的临时去虚化推断。
3. 删除 HIR scaffold 在 LLVM 入口里的长期地位。
4. LLVM backend 不再直接依赖：
   - HIR body
   - raw MIR body
   - 上游整包 stage outputs
5. 把保留的 LLVM 专属逻辑收口到：
   - backend-specific data initializer helper
   - LLVM target pass pipeline

完成标准：

1. `codegen_llvm` 的输入边界清晰。
2. 任何需要 HIR/MIR/effect facts 的现象，都被视为 LIR 不完整，而不是 backend 正常设计。

### P8：Final verification

目标：冻结新边界，并为未来 C backend 留出干净接口。

必须完成：

1. 全仓搜索确认：
   - 无旧 comptime surface
   - 无 HIR 层去虚化
   - 无 codegen 层去虚化
   - 无 stage output 嵌套上游整包
2. 文档与实现同步。
3. 若暂不实现 C backend，也至少固定它未来应依赖的输入边界。

完成标准：

1. `LLVM backend` 与未来 `C backend` 共用同一套 `LIR + LIR facts` 输入边界。
2. `PIPELINE_REFACTOR.md` 中的结构性约束都能在代码里找到对应实现落点。

## 5. 优化 pass 专项约束

为了避免后续“顺手在某层塞一个优化”的回退，本计划单独固定下面这组规则：

1. HIR 不承载 optimization pass。
2. MIR 承载普通调用图/实例级优化。
3. LIR 承载 state-machine / higher-order wrapper 相关的窄优化 family。
4. codegen 只承载 backend-specific 优化。
5. 去虚化只能有一个普通语义 owner：MIR。
6. LIR 可以做 post-state-machine 的 targeted inline/devirt，但只能服务于局部 state-machine elimination，不能升级成全程序通用 inliner。

## 6. 验收标准

当下面这些条件都满足时，才能认为本计划阶段性完成：

1. 每个 stage crate 的依赖只指向前一阶段 crate、基础 crate和更早阶段的 fact crate。
2. 没有任何 fact crate 依赖其它 fact crate 或 stage crate。
3. 没有任何 `StageOutput` 嵌套上一阶段的完整输出。
4. 没有任何下游逻辑把两个 fact table 当成可替代输入。
5. codegen backend 只依赖 `LIR + LIR facts + base context`。
6. 全部静态可判定的源码错误都在 `AST -> HIR` 屏障前收口。
7. 正式 pipeline 中不再保留现有 comptime/const surface 或任何专门兼容逻辑。
8. global object/var/val 与 `@CallingConvention` 的 non-generic 约束在前端稳定生效。
9. top-level eager init 与 object once 语义闭合，覆盖所有 linked cones。

## 7. 说明

本计划是执行计划，不是 TODO 列表。

它的用途是：

1. 固定阶段顺序和依赖关系。
2. 固定哪些问题必须在哪一层被解决。
3. 为后续拆任务、拆 PR、补 TODO 提供统一基线。

如果实现过程中发现需要改变：

1. 编译单元定义
2. stage/fact crate DAG
3. HIR 错误收口边界
4. MIR/LIR/codegen 的优化归属
5. 全局初始化语义

都必须先回写 `PIPELINE_REFACTOR.md`，再继续修改代码。
