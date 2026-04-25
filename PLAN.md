# Scoop：下一轮计划（early MIR / ANF 优化基线与 LLVM codegen 分层收口）

> 生成时间：2026-04-25  
> 历史归档：`PLAN-6.md` / `TODO-6.md`  
> 本轮主题：从当前 LLVM codegen 重构入手，建立 backend-agnostic 的 early MIR / ANF 优化基线，并依次接入 monomorphization、summary、通用 devirtualization、summary-driven inlining、continuation / closure escaping analysis，同时把编译器自身性能，尤其是 `-O0` / debug build 成本，作为一等目标。

## 0. 工作原则

- 本轮以 [`OPTIMIZATION.md`](./OPTIMIZATION.md) 为设计基线；实现顺序严格按 `TODO.md` 推进，不跨条目并行实现。
- 先做边界整理，再做新优化。
  - 在现有 `llvm/codegen` 仍混杂中端分析与 backend lowering 的情况下，继续叠加优化规则只会让边界更糟。
- early MIR / ANF 必须后端无关。
  - 不把 LLVM statepoint、`gc.relocate`、address space、stackmap 形状、mangled symbol name 等 backend 细节编码进 MIR 语义。
- 优化只能依赖结构事实，不能依赖函数名白名单。
  - 不做“专门给 `map` / `filter` / `Iterator.next()` 开洞”的方案。
- monomorphization 放在 early MIR 内部。
  - 顺序是 `generic MIR template -> monomorphic MIR instance`，而不是 HIR 直接落最终实例，更不是 codegen 现场按 mangled FQN 猜目标。
- 自动优化由优化级别驱动。
  - `@Inline` 只保留为将来的 override / hint 位置，不成为优化体系的主机制。
- effect / state-machine planning 必须晚于 monomorphization、summary、devirtualization、inlining、escape analysis。
  - 否则这些优化对 state-machine 形状的主要收益会被浪费掉。
- 编译器自身性能是本轮显式目标。
  - 任何新分析 / 新 IR / 新缓存都必须考虑 `-O0` / debug build 路径的固定成本。
- `mem2reg` 暂不作为主线。
  - 近期主方向仍是减少调用边界与 safepoint 压力，而不是先推动 register-root 改造。

## 0.5 当前进度

- 2026-04-25：`T5000a 建立编译器性能与 codegen 边界基线` 已完成。
  - 统一 baseline 已固化在 [`OPTIMIZATION.md`](./OPTIMIZATION.md) 的第 0、10、11 节；
  - 当前已经能直接回答：
    - `llvm/codegen` 的主要巨型文件与职责簇在哪里；
    - `MainCodegen::new` 的重复构造点在哪里；
    - `-O0` / debug build 的固定成本主要来自哪些路径；
    - reachability / eager inclusion / codegen 查询的重复工作主要在哪里。
- 2026-04-25：`T5000aR Review：确认 baseline 已足够支撑后续实现顺序` 已完成。
  - 已抽样核对 `MainCodegen::new` 调用点、`llvm/mod.rs` 中的 reachability / eager inclusion、`-O0` pass pipeline、以及 `HandlePlanContext::from_codegen` 的依赖方向；
  - 已补充确认 `crates/scoopc/src/effect_step_summary.rs` 通过 `include!` 复用 `state_machine_plan.rs`，说明 effect summary 已有 backend 外消费者；
  - review 结论是：这条 `include!` 耦合属于既有 effect middle-end / shared facts 边界问题，不需要在 `T5000b` 前额外插入独立前置任务。
- 2026-04-25：`T5000b 清理 LLVM codegen 边界，拆分 MainCodegen 与巨型模块` 已判定为单轮过大任务，已拆成 `T5000b1`～`T5000b4` 四个实现子任务与对应 review。
  - 拆分顺序：
    - `T5000b1`：先拆 `llvm/mod.rs` 的 emit API / pipeline / reachability / tests；
    - `T5000b2`：再提炼 `MainCodegen` 共享编译单元上下文与 child-codegen 构造路径；
    - `T5000b3`：按主题拆 `llvm/codegen/mod.rs`；
    - `T5000b4`：最后把 `MainCodegen` 进一步分成 module / function / cache / effect emitter 上下文。
- 下一条待执行任务切换为 `T5000b1 拆分 llvm/mod.rs 的 emit API / pipeline / reachability / tests`。
- baseline review 后，仍未发现需要插入到 `T5000b1` 之前的新前置缺陷任务。

## 1. 当前判断

当前代码库已经清楚暴露出两个问题：

1. `llvm/codegen` 的工程边界过差。
   - `codegen/mod.rs`、`effect/state_machine_plan.rs`、`llvm/mod.rs` 等文件过大，不只是文件长度问题，而是职责混放问题。
2. 一部分逻辑已经明显越过 backend 边界。
   - monomorphized callee resolution、concrete type 恢复、devirtualization 判定、higher-order suspendability summary、state-machine plan/segments/transform 等，都更像中端分析而不是 LLVM lowering。

因此本轮第一步不是再加更多优化点，而是：

- 先把属于 LLVM backend 的部分拆清；
- 再把不属于 LLVM 的部分抽出来；
- 最后再让 early MIR / ANF 成为后续优化与 lowering 的主承载层。

## 2. 顺序总览

1. 建立当前编译器性能与 codegen 边界基线。
2. 先拆 `llvm/mod.rs`，再拆 `MainCodegen` 构造边界与 `llvm/codegen/mod.rs` 主题模块，最终完成 `MainCodegen` 上下文分层。
3. 抽离 backend-agnostic 的 `ProgramFacts` / `EffectAnalysisCtx` / shared side tables。
4. 扩展现有 MIR，形成最小 generic early MIR / ANF template。
5. 在 MIR 层实现 monomorphization / instance materialization。
6. 按单态实例建立 summary 基础设施。
7. 在 MIR 层实现通用 devirtualization。
8. 在 MIR 层实现 summary-driven inlining。
9. 加入 continuation / closure escaping analysis，并把 effect/state-machine planning 迁到正确边界。
10. 在前述主线稳定后，再扩覆盖面并继续跟踪 safepoint / `mem2reg` 方向。

## 3. 分阶段目标

### P0. 基线与 guardrail

- 明确当前编译器自身的热点：
  - `MainCodegen::new` 的重复构造；
  - codegen 查询点的临时分析上下文重建；
  - HIR reachability + eager inclusion 的重复扫描；
  - `debug_assertions` 下 effect middle-end 的额外校验成本；
  - `-O0` 仍固定跑 `SROA + rewrite-statepoints-for-gc + verify_each` 的现实。
- 为后续各阶段建立最小可复验的性能基线，避免“结构更好了，但编译器更慢了”。

### P1. LLVM codegen 分层收口

- P1-1：先拆 `llvm/mod.rs`，把 emit API / module build、pipeline、reachability、tests 分开。
  - 目标：先让 `llvm/mod.rs` 根模块退回“入口 + 错误边界 + re-export”角色，避免后续所有改动继续堆回单一根文件。
- P1-2：提炼 `MainCodegen` 的共享编译单元输入与 child-codegen 构造路径。
  - 目标：先消除多处重复 `MainCodegenInputs { ... }` 拼装，为真正的上下文分层扫清构造层噪音。
- P1-3：从 `codegen/mod.rs` 拆出独立主题：
  - `call/`
  - `intrinsics/`
  - `closure/`
  - `class_ctor.rs`
  - `object_init.rs`
  - `enum_lowering.rs`
  - `gc/*`
  - `runtime_abi/*`
  - `control_flow/*`
- P1-4：继续拆分 `MainCodegen`：
  - `ModuleCodegenCx`
  - `FnCodegenCx`
  - `SharedAnalysisCache`
  - `EffectCodegenCx`

### P2. Program facts 与中端分析解耦

- 抽出一套 backend-agnostic 的共享事实层，至少覆盖：
  - callee target resolution 所需事实；
  - receiver exactness / target-set shrinking 所需事实；
  - monomorphic instance identity；
  - higher-order provenance 与 suspendability summary 输入；
  - effect/state-machine planning 所需事实。
- 消除“中端分析必须从 `MainCodegen` 反取信息”的依赖方向。

### P3. generic early MIR / ANF template

- 在现有 MIR 基础上引入：
  - 显式 call kind；
  - 显式 `Perform` / `Resume`；
  - 更稳定的 value provenance / concrete type / dispatch metadata；
  - 后续 pattern / `when` lowering 的正规化入口。
- 这一阶段的产物仍允许包含 type params，因此它是 template，而不是最终优化输入。

### P4. monomorphic MIR instance

- 引入 `InstanceKey` 作为 backend-agnostic 的实例身份。
- 在 MIR 层实现：
  - reachable-driven instance collection；
  - on-demand monomorphization；
  - per-`InstanceKey` 缓存。
- 让后续 summary / devirt / inline / effect planning 都消费 monomorphic instances，而不是 generic template，也不是 codegen 现场猜出来的目标。

### P5. per-instance summary

- 对每个单态实例建立最小 summary：
  - `body_known`
  - `size_cost`
  - `recursive_scc`
  - `may_outward_effect`
  - `may_allocate_closure`
  - `param_use_summaries`
  - `result_provenance`
- summary 应成为 MIR 或 side table 的稳定产物，而不是 codegen 查询时临时现算。

### P6. 通用 devirtualization

- 对所有 `VirtualCall` / `InterfaceCall` 统一做 target-set shrinking。
- 只要 receiver exact type 已知且 target set 为 singleton，就改写为 `DirectCall`。
- LLVM backend 不再负责“能否去虚化”的判定，只负责 lowering 已分类完成的调用。

### P7. summary-driven inlining

- 先做保守但通用的版本：
  - body-known
  - 非递归
  - 小体量
  - `DirectCallOnly` 参数
  - provenance 可知
- 对高阶函数的收益来自结构，而不是函数名：
  - 同样的规则应自动覆盖 stdlib 包装函数与用户自定义小包装函数。

### P8. continuation / closure escaping + effect planning 收口

- 在 MIR 层加入：
  - non-escaping closure simplification；
  - continuation escaping analysis。
- 把 `state_machine_plan / segments / transform` 从 LLVM codegen 语义边界里迁出，使 effect/state-machine planning 依赖 MIR 与 `ProgramFacts`，而不是依赖 `MainCodegen`。

### P9. 覆盖面扩张与 safepoint 跟踪

- 在主线稳定后再继续：
  - 扩展结构识别覆盖面；
  - 改善 summary 精度；
  - 改善 provenance / target-set shrinking；
  - 跟踪减少调用边界后 safepoint 数量、roots 压力与 `mem2reg` 研究窗口。

## 4. 本轮完成标准

本轮计划的“方向性完成”标准不是单独某个优化开关生效，而是以下几件事同时成立：

- LLVM codegen 不再继续承担 monomorphization、devirtualization 判定、higher-order summary 与 effect middle-end 的主职责。
- early MIR / ANF 成为明确存在、可承载后续优化的中端层。
- monomorphization 有独立的 MIR 层实例化阶段与 `InstanceKey` 表示。
- summary、devirt、inline 与 escape analysis 都有明确的层次归属，不再依附于 `MainCodegen` 的现场推断。
- 编译器自身性能有显式 baseline 与 guardrail，而不是等功能做完后再回头补救。
