# Scoop：当前计划（统一 effect codegen / LLVM）

> 生成时间：2026-04-15  
> 历史归档：`PLAN-3.md` / `TODO-3.md`  
> 范围：本计划只覆盖当前 effect 统一主线。`segmentation` 与 `state machine transformation` 视为已建立的基线；从这里开始，先把剩余 shape-based logic 彻底清掉，再把 LLVM codegen 收口到“只消费 state machine”的统一实现。

## 0. 工作原则

- 删除优先于修补。看到 shape-based 生产逻辑就直接删除，不以“先补一个 case”维持旧路径。
- LLVM effect codegen 的单一输入是 state machine。除类型、符号与 ABI 必需信息外，不能再读取源码形状、旧 scanner 结果或旧分类器输出。
- 当前阶段只实现 heap-allocated full state machine lowering，不做 simplification，不做模式化优化。
- 每个实现任务后立即插入一个 review 任务；review 必须显式确认生产代码中不存在 shape-based logic。
- review 范围只看生产代码，重点是 `crates/scoopc/src/llvm/codegen/**`；测试命名不作为问题。

## 1. 当前已知缺口

- `crates/scoopc/src/llvm/codegen/mod.rs` 仍保留旧 callee-suspend shape-based 路线：
  - `CalleeSuspendResumeMode`
  - `scan_for_callee_suspend`
  - `codegen_top_level_fun_suspendable`
  - `codegen_closure_fun_body_suspendable`
- `crates/scoopc/src/llvm/codegen/effect/mod.rs` 统一入口仍未完全接到真正的 state-machine-driven LLVM lowering。
- 这意味着当前 effect codegen 虽然已经有统一的 segmentation 与 state machine transformation 基线，但 LLVM 生产主线还没有完全摆脱旧形状分流。

## 2. 阶段顺序

### 阶段 A：先把残余 shape-based 主路径删干净

#### T3001：删除 `llvm/codegen/mod.rs` 中剩余的 callee-suspend shape-based 主路径
- 先删 `mod.rs` 里的旧 callee-suspend 路线，不允许顶层函数或 closure 再按源码形状走专用 lowering。
- 这一步的目标是清除旧主线路由，不要求立即恢复编译。

#### T3001R：Review
- 定向检查 `mod.rs` 与调用点，确认旧 callee-shape scanner / mode enum / suspendable top-level/closure 路线已经完全消失，没有换名保留。

#### T3002：清扫 effect codegen 生产代码中的其余 shape-based 分流
- 在 `mod.rs` 清掉之后，继续删除 effect codegen 里的旧 scanner、旧 resolver、旧 dedicated matrix、旧 fallback route。
- 目标不是把它们改成“更像统一主线”，而是让它们不再构成生产主路径。

#### T3002R：Review
- 审查 `crates/scoopc/src/llvm/codegen/**`，确认生产代码里已经不存在按源码 / site / arm / callee 形状做主选路的 effect codegen。

### 阶段 B：冻结 LLVM lowering 的唯一输入面

#### T3003：冻结 `state machine -> LLVM lowering` 的统一合同
- 在真正发射 LLVM 之前，先把 emitter 输入收口到统一合同。
- 这个合同必须完整表达 state、edge、suspend/resume、cleanup、frame layout、dispatch、capture / payload / slot 等 lowering 所需信息。
- 合同的全部结构信息都必须来自 state machine 本身，而不是回头看 HIR shape 或旧 scanner 输出。

#### T3003R：Review
- 审查 lowering 入口及其依赖链，确认 LLVM lowering 主线的结构输入只剩 state machine。

### 阶段 C：实现 full state machine LLVM emitter

#### T3004：实现 heap-allocated full state machine 的 LLVM lowering 主体
- 从统一 state machine 发射 frame、`pc` dispatch、state block、resume edge、cleanup/unwind、handler stack 交互与 payload transport。
- 当前阶段不做 simplification；即使后续会优化，也必须先保证 full state machine 语义完整可发射。

#### T3004R：Review
- 审查 emitter 主体，确认所有发射分支都来自 state machine 语义边，而不是代码形状推断。

### 阶段 D：把统一 emitter 接回生产入口

#### T3005：将统一 state-machine LLVM lowering 接回 effect codegen 主入口
- `handle` / `perform` / continuation 相关入口都要接到统一 emitter。
- 旧主线路由、旧 fallback、旧占位入口要同步删除或彻底失效，不能保留“双轨”。

#### T3005R：Review
- 审查 effect codegen 主入口，确认统一 lowering 已经成为唯一主路径，不存在“失败时退回旧路线”的隐藏逻辑。

### 阶段 E：用测试补齐覆盖，但修复必须仍在统一主线内完成

#### T3006：补齐统一 LLVM lowering 的定向测试与代表性 fixture
- 这一步的目标是确认当前 state machine 合同与 LLVM lowering 真正能覆盖合法输入。
- 如果测试暴露出缺口，先补 plan builder / state machine / emitter 合同，再继续测试；不能为了过例子新增 shape-based 快捷分支。
- 当前阶段以定向测试为主，不把 full suite 当作前置门槛。

#### T3006R：Review
- 审查测试修复后的生产代码，确认没有因为补 case 把 shape-based logic 带回主线。

### 阶段 F：收尾 legacy 清理

#### T3007：删除统一主线接管后剩余的 legacy effect codegen 死代码
- 在统一 LLVM 主线稳定后，继续删除仓库里剩余的 legacy effect codegen 文件、helper、注释与过渡分支。
- 目标是让仓库结构本身也与“统一主线”一致，而不是代码虽然不走、但旧实现还大面积留存。

#### T3007R：Review
- 最终审查 effect codegen 生产实现，确认仓库中只剩统一主线，没有可重新接回的 shape-based legacy。

## 3. 验收策略

- 清理阶段允许临时破坏编译；目标是先删旧主线。
- 接统一 LLVM emitter 后，再用定向测试恢复并扩大覆盖。
- review 任务是强制门，不是可选项；任何实现任务完成后，如果 review 发现 shape-based 逻辑回流，必须先回退到清理状态，再进入下一任务。

## 4. 当前执行顺序

1. `T3001`
2. `T3001R`
3. `T3002`
4. `T3002R`
5. `T3003`
6. `T3003R`
7. `T3004`
8. `T3004R`
9. `T3005`
10. `T3005R`
11. `T3006`
12. `T3006R`
13. `T3007`
14. `T3007R`
