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
- 2026-04-25：`T5000b1 拆分 llvm/mod.rs 的 emit API / pipeline / reachability / tests` 已完成。
  - 新增 `emit.rs`、`pipeline.rs`、`reachability.rs`、`tests.rs` 四个子模块；
  - `llvm/mod.rs` 现已只保留错误边界、常量、子模块声明与必要 re-export；
  - 验证结果：
    - `cargo test -p scoopc llvm::`
    - `cargo test --all`
    - `cargo clippy --all-targets -- -D warnings`
    - 全部通过。
- 2026-04-25：`T5000b1R Review：确认 llvm/mod.rs 已收口为根模块而非实现巨型文件` 已完成。
  - 已复核 `crates/scoopc/src/llvm/mod.rs` 的职责上界：
    - 根模块只保留子模块声明；
    - 对外 emit/target 入口 re-export；
    - 测试期窄桥接 re-export；
    - LLVM GC 策略常量、一次性全局 LLVM 选项配置；
    - 统一 `LlvmEmitError` 诊断边界及其轻量辅助函数；
  - 已复核 `crates/scoopc/src/llvm/emit.rs`、`crates/scoopc/src/llvm/pipeline.rs`、`crates/scoopc/src/llvm/reachability.rs`、`crates/scoopc/src/llvm/tests.rs`：
    - emit API / module build 主体留在 `emit.rs`；
    - pass pipeline 完整落在 `pipeline.rs`；
    - HIR reachability 扫描完整落在 `reachability.rs`；
    - 根模块测试已迁到 `tests.rs`，effect emitter 侧测试只通过 `#[cfg(test)]` 下的窄桥接 re-export 使用内部 helper；
  - 验证结果：
    - `cargo test -p scoopc llvm::`
    - `cargo clippy --all-targets -- -D warnings`
    - 全部通过；
  - review 结论：`llvm/mod.rs` 已收口为根模块，没有发现需要在 `T5000b2` 前新增的阻塞缺陷任务。
- 2026-04-25：`T5000b2 提炼 MainCodegen 共享编译单元上下文与 child-codegen 构造路径` 已完成。
  - 已在 `crates/scoopc/src/llvm/codegen/mod.rs` 中新增 `CompilationUnitCodegenCx` / `CompilationUnitCodegenInputs`，集中承接稳定编译单元输入、共享 `effect_op_tags`、共享 `known_fun_call_suspend_cache` 与预计算的 `known_effect_instances_by_effect_fqn`；
  - `MainCodegen` 当前已改为持有 `shared: &CompilationUnitCodegenCx`，并通过 `fresh_child_codegen()` 统一 effect-call wrapper、top-level immutable init、closure body lowering、object init lowering 四条 child/nested codegen 构造路径；
  - `crates/scoopc/src/llvm/emit.rs` 中的顶层声明、reachable top-level function body 发射与入口 `main` exit-code lowering 三条路径，现统一经由单次编译单元上下文构造 + `fresh_main_codegen()` 进入；
  - 这一步先把“共享编译单元输入”和“函数级局部状态”从构造入口上分离，尚未继续推进到 `type_layout_cache` / `enum_cg_layout_cache` / effect emitter 专用上下文等更深层 cache/状态拆分；这些属于后续 `T5000b2R` / `T5000b4` 继续确认与推进的范围；
  - 验证结果：
    - `cargo fmt --all`
    - `cargo test -p scoopc llvm::`
    - `cargo test --all`
    - `cargo clippy --all-targets -- -D warnings`
    - 全部通过。
- 2026-04-25：`T5000b2R Review：确认 MainCodegen 构造边界已开始从“巨型输入包”收口` 已完成。
  - 已确认 `crates/scoopc/src/llvm/emit.rs` 中 `CompilationUnitCodegenCx::new(...)` 在实现代码里只剩 1 个编译单元构造入口；
  - 已确认 `fresh_main_codegen()` 统一承接顶层声明、reachable top-level function body 发射与入口 `main` exit-code lowering，`fresh_child_codegen()` 覆盖 effect-call wrapper、top-level immutable init、closure body lowering、object init lowering 四条 child/nested 路径；
  - 已确认实现代码中不再残留 `MainCodegenInputs { ... }` 手写构造，说明构造样板已经显著收敛；
  - 已复核共享编译单元输入/共享事实与函数级局部状态已开始分离，但 `type_layout_cache`、`enum_cg_layout_cache`、effect emitter 专用上下文等更深层分层仍属于后续 `T5000b3` / `T5000b4` 范围；
  - 验证结果：
    - `cargo test -p scoopc llvm::`
    - `cargo clippy --all-targets -- -D warnings`
    - 全部通过；
  - review 结论：当前改动已经显著收敛构造样板，可以直接继续 `T5000b3`，无需在其前插入新的缺陷修复任务。
- 2026-04-25：`T5000b3 按主题拆分 llvm/codegen/mod.rs 的独立 lowering 模块` 已判定为单轮过大任务，已拆成 `T5000b3a`～`T5000b3d` 四个实现子任务与对应 review。
  - 拆分依据：
    - `crates/scoopc/src/llvm/codegen/mod.rs` 当前仍有 17671 行；
    - 其中至少存在四组稳定函数簇：
      - call dispatch / callable ABI / extern-native / vtable-itable / callee-resume；
      - sysroot / builtin intrinsics；
      - closure / class ctor；
      - enum lowering / object init。
  - 拆分顺序：
    - `T5000b3a`：先拆 `call/` lowering 模块；
    - `T5000b3b`：再拆 `intrinsics/` lowering 模块；
    - `T5000b3c`：拆 `closure/` 与 `class_ctor.rs`；
    - `T5000b3d`：拆 `enum_lowering.rs` 与 `object_init.rs` 并收口根模块剩余职责。
- 2026-04-25：`T5000b3a 拆出 call/ lowering 模块` 已完成。
  - 实现结果：
    - 新增 `crates/scoopc/src/llvm/codegen/call/{mod,abi,dispatch,resume}.rs`；
    - `codegen/mod.rs` 中 `codegen_call`、top-level fun call、extern/native call、vtable/itable dispatch、funptr/function-value call、callable arg ABI、ordinary callee resume 与 top-level effect-call wrapper 等入口已改为薄委托；
    - `call/` 已按 `dispatch` / `abi` / `resume` 三组稳定职责分层，而不是机械迁移到新的单文件。
  - 收尾修复：
    - 补上 `call/resume.rs` 对 `LLVM_GC_STRATEGY_STATEPOINT_EXAMPLE` 的导入；
    - 将新 `*_impl` 方法的可见性收紧为 `pub(in crate::llvm::codegen)`，消除 `private_interfaces` warning。
  - 验证：
    - `cargo fmt --all`
    - `cargo test -p scoopc llvm::`
    - `cargo test --all`
    - `cargo clippy --all-targets -- -D warnings`
    - 全部通过。
- 2026-04-25：`T5000b3aR Review：确认 call/ 拆分形成稳定 lowering 边界` 已完成。
  - 已核对 `crates/scoopc/src/llvm/codegen/call/{dispatch,abi,resume}.rs` 与 `crates/scoopc/src/llvm/codegen/mod.rs`：
    - `codegen_call_impl`、top-level/direct/virtual/interface/funptr/function-value call、call arg ABI、ordinary callee resume 与 top-level effect-call wrapper 等主体实现均已离开根模块，集中位于 `call/`；
    - `codegen/mod.rs` 当前对这些入口只保留薄委托，以及少量共享 call 数据结构 / 命名 helper。
  - 已确认交叉接口方向：
    - `call/dispatch.rs` 对 `codegen_class_ctor_call`、`codegen_closure_expr`、各类 `codegen_sysroot_*` / `try_codegen_tostring_iface_builtin` helper 的依赖仍是单向委托；
    - closure/effect 主题仅通过 `declare_*callee_resume_entry`、`codegen_callee_resume_entry_function`、`call_callee_resume_entry_from_state` 等 resume 入口消费 call 主题能力；
    - 未发现 class ctor / closure / intrinsics 主题反向承载 call dispatch / ABI 主体实现的新双向耦合。
  - review 结论：
    - `call/` 的职责上界已明确为调用分派、调用点 ABI / 实参绑定、ordinary resume / wrapper lowering；
    - builtin/sysroot、closure/class ctor、enum/object 主题仍主要位于 `codegen/mod.rs`，它们就是后续 `T5000b3b`～`T5000b3d` 的剩余拆分对象，不需要在此之前插入新的前置缺陷修复任务。
  - 验证结果：
    - `cargo test -p scoopc llvm::`
    - `cargo test --all`
    - `cargo clippy --all-targets -- -D warnings`
    - 全部通过。
- 下一条待执行任务切换为 `T5000b3b 拆出 intrinsics/ lowering 模块`。

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
- P1-3：从 `codegen/mod.rs` 按稳定主题逐步拆出独立 lowering 模块：
  - P1-3a：`call/`
  - P1-3b：`intrinsics/`
  - P1-3c：`closure/` + `class_ctor.rs`
  - P1-3d：`enum_lowering.rs` + `object_init.rs`
  - 已存在的 `gc.rs` / `runtime_abi.rs` / `control_flow.rs` 继续保持独立主题边界；若后续需要继续细分，再在 `T5000b4` 之前单独评估，不与本轮从 `mod.rs` 迁出的主题混做一次性搬家。
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
