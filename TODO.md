# TODO（Scoop：统一 effect 主线）

> 生成时间：2026-04-15  
> 历史归档：`TODO-3.md` / `PLAN-3.md`  
> 范围：本文件只记录当前 effect 统一主线。`segmentation` 与 `state machine transformation` 视为当前基线；接下来先继续清理残余的 shape-based logic，再完成只消费 state machine 的 LLVM codegen。

## 约束

- 生产代码中的 effect codegen 禁止根据源码或代码形状分流。禁止把 `single` / `multi` / `direct` / `indirect` / `top-level` / `nested` / callee shape 等分类当作 lowering 决策输入。
- LLVM codegen 的单一输入是 state machine。除类型信息、符号信息与运行时 ABI 必需信息外，不允许再读取源码形状、旧 scanner 结果或旧 shape 分类结果。
- 当前阶段默认生成 heap-allocated full state machine；暂不做 simplification / optimization。
- 每个实现任务之后都必须立即执行一个 review 任务；review 只检查生产代码，不以测试代码命名作为问题。

## 当前已知问题

- `crates/scoopc/src/llvm/codegen/mod.rs` 仍保留旧的 callee-suspend shape-based 路线：
  - `CalleeSuspendResumeMode`
  - `scan_for_callee_suspend`
  - `codegen_top_level_fun_suspendable`
  - `codegen_closure_fun_body_suspendable`
- `crates/scoopc/src/llvm/codegen/effect/mod.rs` 的统一主线入口仍未完全接到 state-machine-driven LLVM lowering。

## T30：统一 effect LLVM codegen

### T3001 [TODO] 删除 `llvm/codegen/mod.rs` 中剩余的 callee-suspend shape-based 主路径
- 描述：先删除当前 review 已确认的旧主路径，避免后续 LLVM lowering 继续从顶层函数 / closure 的源码形状分流。
- 目标：
  - 删除 `CalleeSuspendResumeMode`、`scan_for_callee_suspend`、`codegen_top_level_fun_suspendable`、`codegen_closure_fun_body_suspendable` 及其入口接线。
  - 顶层函数与 closure 不再按 `val = perform` / `return perform(...)` / block 尾 `perform(...)` 等形状进入专用路径。
  - 允许这一步暂时破坏编译；本任务的目标是彻底删除旧 shape-based 生产逻辑，而不是修旧路径。
- 验收：
  - 生产代码中不再出现上述符号。
  - `crates/scoopc/src/llvm/codegen/mod.rs` 不再存在按 callee/source shape 选择 suspendable lowering 的入口。
- 依赖：无

### T3001R [TODO] Review：确认 `mod.rs` 的 callee-suspend shape-based 逻辑已清零
- 描述：在继续后续实现前，专门审查 `crates/scoopc/src/llvm/codegen/mod.rs` 与 effect 入口接线，确认没有等价回流。
- 目标：
  - 确认没有以新名字保留旧的 callee-shape scanner / mode enum / suspendable top-level/closure route。
  - 确认相关调用点已经转为空、删除或等待统一主线接管，而不是换名保留旧分流。
- 验收：
  - 审查结论明确记录“生产代码无 callee-suspend shape-based 主路径残留”。
- 依赖：T3001

### T3002 [TODO] 清扫 effect codegen 生产代码中的剩余 shape-based 分支与旧专用路径
- 描述：在删掉 `mod.rs` 旧路线后，继续审计并删除 effect codegen 其余残留，不保留旧 scanner、旧 resolver、旧 dedicated matrix 或其它基于代码形状的专用 lowering 主路径。
- 目标：
  - 清理 `crates/scoopc/src/llvm/codegen/effect/**` 以及相邻入口中的旧分流残留。
  - 删除已经失去主线路由职责的旧 helper / module / dead branch。
  - 保证后续 LLVM lowering 只沿统一主线推进，不再回到旧 shape-based 实现上补 case。
- 验收：
  - 生产代码中的 effect codegen 不再依赖旧 scanner 结果或 shape 分类结果决定主路径。
  - 旧专用路径要么被删除，要么彻底脱离生产入口，不再是可执行主线。
- 依赖：T3001R

### T3002R [TODO] Review：确认 effect codegen 生产代码中不存在 shape-based 主分流
- 描述：对 `crates/scoopc/src/llvm/codegen/**` 做定向审查，只看生产代码，不看测试。
- 目标：
  - 确认 effect codegen 的主路径不再按源码形状、site 形状、arm 形状或 callee 形状选路。
  - 确认新增代码没有重新引入新的 shape-based helper。
- 验收：
  - 审查结论明确记录“当前生产代码无 shape-based effect codegen 主分流残留”。
- 依赖：T3002

### T3003 [TODO] 冻结 `state machine -> LLVM lowering` 的统一输入合同
- 描述：在真正发射 LLVM 之前，先把 lowering 输入收口到统一合同，确保后续实现无法回头读取源码形状。
- 目标：
  - 明确并落地 LLVM emitter 所需的统一输入：state table、state edge、suspend edge、cleanup、frame layout、dispatch、capture / payload / slot 元数据。
  - 这些输入必须全部来自 state machine 本身，而不是来自 HIR shape、scanner 结果或旧分类器。
  - 统一入口对外暴露“只吃 state machine”的 API / contract，作为后续所有 LLVM effect lowering 的唯一输入面。
- 验收：
  - 统一 LLVM lowering 入口能够只接受 state machine 及必需的类型 / 符号 / ABI 上下文。
  - 旧 shape-based resolver 不再是主线输入来源。
- 依赖：T3002R

### T3003R [TODO] Review：确认 LLVM lowering 输入面只剩 state machine
- 描述：审查统一 lowering 入口与其依赖链，确认没有旁路输入。
- 目标：
  - 确认 LLVM lowering 主线不再读取源码路径、旧 scanner 输出、旧 mode 选择结果。
  - 确认所有 effect lowering 所需结构信息都来自 state machine 合同。
- 验收：
  - 审查结论明确记录“LLVM lowering 的主输入只有 state machine”。
- 依赖：T3003

### T3004 [TODO] 实现 heap-allocated full state machine 的 LLVM lowering 主体
- 描述：基于统一合同实现真正的 LLVM lowering，当前阶段只做 full machine，不做任何 simplification。
- 目标：
  - 从 state machine 发射 frame、`pc` dispatch、state block、suspend/resume、cleanup/unwind、handler stack 交互与 payload transport。
  - 统一支持当前 state machine 能表达的合法边，不再按源码形状拆专用 emitter。
  - 默认使用 heap-allocated full state machine，保持语义完整优先。
- 验收：
  - LLVM emitter 能从 state machine 构建完整控制流与运行时交互骨架。
  - 当前支持范围内的 state machine 不再需要 shape-based fallback emitter。
- 依赖：T3003R

### T3004R [TODO] Review：确认 full-state-machine LLVM emitter 不含 shape-based 选路
- 描述：在接主入口之前，先检查 emitter 主体本身是否纯粹消费 state machine。
- 目标：
  - 确认 emitter 不根据源码结构、旧 site 分类或 arm 形状切换不同发射流程。
  - 确认 emitter 内部所有分支都来自 state machine 语义边，而非代码形状推断。
- 验收：
  - 审查结论明确记录“full-state-machine LLVM emitter 只按 state machine 语义发射”。
- 依赖：T3004

### T3005 [TODO] 将统一 state-machine LLVM lowering 接回 effect codegen 主入口
- 描述：把统一 emitter 接到当前生产入口，替换剩余旧主线路由与占位入口。
- 目标：
  - `handle` / `perform` / 相关 continuation 与 callee-suspend 入口统一走新的 state-machine lowering。
  - 删除或替换旧的主入口占位与 fallback route。
  - 生产路径中不再存在“先判源码形状，再决定是否走 unified lowering”的结构。
- 验收：
  - effect codegen 主入口只走统一 state-machine LLVM lowering。
  - 已知旧主线路由不再参与生产路径。
- 依赖：T3004R

### T3005R [TODO] Review：确认 effect codegen 主入口只接统一 state-machine lowering
- 描述：主入口接通后，再做一轮生产代码审查，防止旧入口残留成隐式 fallback。
- 目标：
  - 确认统一 lowering 已成为唯一主路径。
  - 确认不存在“失败时退回旧 shape-based emitter”的隐藏逻辑。
- 验收：
  - 审查结论明确记录“effect codegen 主入口没有旧 fallback / 双轨”。
- 依赖：T3005

### T3006 [TODO] 用定向测试补齐统一 LLVM lowering 覆盖，并修复暴露出的合同缺口
- 描述：基于当前统一主线补齐测试。若暴露出 plan builder / state machine / lowering 合同缺口，先修实现，再继续扩大覆盖。
- 目标：
  - 为 state-machine-driven LLVM lowering 建立定向单测与 representative fixture。
  - 覆盖完整状态机语义：dispatch、resume、cleanup、nested handle、indirect call-site suspension、payload transport 等。
  - 发现不支持的合法 state machine 时，优先补合同或 emitter，而不是新增 shape-based 快捷分支。
- 验收：
  - 定向 LLVM codegen 测试与代表性 fixture 能稳定覆盖当前统一主线。
  - 暴露出的缺口已经在统一合同 / emitter 内修复，而不是回退到 shape-based 方案。
- 依赖：T3005R

### T3006R [TODO] Review：确认测试补齐后生产代码仍然零 shape-based logic
- 描述：在阶段性收口前做一次完整审查，确认“为过测试临时加的例外”没有污染主线。
- 目标：
  - 确认新增测试修复没有把 shape-based 逻辑重新带回生产代码。
  - 确认 effect LLVM codegen 仍然满足“只从 state machine 出发”的总约束。
- 验收：
  - 审查结论明确记录“当前 effect LLVM codegen 生产代码中不存在 shape-based logic”。
- 依赖：T3006

### T3007 [TODO] 删除统一主线接管后剩余的 legacy effect codegen 死代码
- 描述：在统一主线可工作后，继续删除剩余 legacy effect codegen 文件、helper 与不再可达的过渡分支，避免仓库再次回到旧路线。
- 目标：
  - 删除不再被生产路径引用的 legacy effect codegen 文件与 helper。
  - 清理旧文档 / 注释 / 命名中仍把旧 shape-based 方案当作现行主线的残留。
  - 保持 effect codegen 仓库形态与当前统一架构一致。
- 验收：
  - 生产代码目录中只保留当前统一主线所需的 effect codegen 实现。
  - 已删除的 legacy 路径不再可能被重新接回主入口。
- 依赖：T3006R

### T3007R [TODO] Review：确认仓库中的 effect codegen 生产实现只剩统一主线
- 描述：最终审查，确认 cleanup 真正完成，而不是“旧代码还在，只是暂时不用”。
- 目标：
  - 确认 effect codegen 生产实现只剩统一主线。
  - 确认后续 LLVM 阶段可以直接在统一主线上继续推进，不再受旧 shape-based 代码干扰。
- 验收：
  - 审查结论明确记录“effect codegen 生产实现只剩统一主线，无 shape-based legacy 残留”。
- 依赖：T3007
