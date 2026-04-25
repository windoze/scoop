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
- 2026-04-25：`T5000b3b 拆出 intrinsics/ lowering 模块` 已完成。
  - 实现结果：
    - 新增 `crates/scoopc/src/llvm/codegen/intrinsics/{mod,builtin,sysroot,sync,thread,channels,containers,atomic}.rs`；
    - `intrinsics/` 已按稳定语义边界拆分：
      - `builtin.rs`：标量 builtin、`print`/`println`、`toString`/`toInt`/`hash`、`sizeOf`
      - `sysroot.rs`：io/env/time/fs/process/path
      - `sync.rs`：mutex/condvar/once/destroy
      - `thread.rs`：thread、task transport、thread-specific intrinsic
      - `channels.rs`：channel send/recv/close
      - `containers.rs`：array builder / array get-set 与 array-like helper
      - `atomic.rs`：atomic int intrinsics
    - `crates/scoopc/src/llvm/codegen/mod.rs` 现仅保留 `mod intrinsics;` 声明与其它非 intrinsics 主题代码，不再直接承载 builtin/sysroot intrinsics 主体实现；
    - `crates/scoopc/src/llvm/codegen/call/dispatch.rs` 保持为 FQN dispatch 层，具体 builtin/sysroot lowering 主体改由 `intrinsics/` 主题模块承接；
  - 收尾修复：
    - 清理了 `crates/scoopc/src/llvm/codegen/mod.rs` 中迁移后残留的 `inkwell::AtomicOrdering` 未使用导入；
  - 验证：
    - `cargo fmt --all`
    - `cargo test -p scoopc llvm::`
    - `cargo test --all`
    - `cargo clippy --all-targets -- -D warnings`
    - 全部通过。
- 2026-04-25：`T5000b3bR Review：确认 intrinsics/ 拆分没有把 builtin/sysroot 继续堆回根模块` 已完成。
  - review 先暴露并修复了一个既有边界问题：
    - `crates/scoopc/src/llvm/codegen/mod.rs` 中仍残留 `codegen_string_trim_indent`、`codegen_string_method`、`codegen_to_string_method`、`expr_is_builtin_char`，以及 `Char` / `Int` / `Float` builtin member-call helper；
    - 这些实现已在本轮迁入 `crates/scoopc/src/llvm/codegen/intrinsics/builtin.rs`，从而把上一轮未完全收口的 builtin lowering 主体真正移出根模块。
  - 复核结论：
    - `codegen/mod.rs` 现不再定义 string/char/int/float builtin lowering，也不再承载 io/env/time/fs/process/path、sync/thread/channels、array/task transport、atomic int 等 sysroot/intrinsics 主体实现；
    - `call/dispatch.rs` 仍只承担 FQN/member dispatch，具体 lowering 主体单向落到 `intrinsics/`；
    - `intrinsics/` 与 `runtime_abi` / `gc` 的交互保持为单向消费 runtime/GC helper，没有出现新的双向耦合。
  - 仍留在 `codegen/mod.rs` 的相关共享 helper：
    - `codegen_addressable_place` / `AddressablePlace` 属于通用 lvalue / object 访问边界，当前仅被 atomic intrinsics 复用，不再算 builtin/sysroot 主题残留；
    - `lookup_pure_unit_closure_type` 仍被 `sync.Once.run` / `thread.spawn` 暂借，属于后续 `T5000b3c` 需要继续收口的 closure 主题桥接。
  - 验证结果：
    - `cargo fmt --all`
    - `cargo test -p scoopc llvm::`
    - `cargo test --all`
    - `cargo clippy --all-targets -- -D warnings`
    - 全部通过。
- 2026-04-25：`T5000b3c 拆出 closure/ 与 class_ctor.rs lowering 模块` 已完成。
  - 实现结果：
    - 新增 `crates/scoopc/src/llvm/codegen/closure/mod.rs`，将 `ClosureParamBindings`、`ClosureBodyCodegenSpec`、`codegen_closure_expr`、`closure_param_bindings`、`codegen_closure_fun_body`、`llvm_closure_env_type`、`build_closure_callee_suspend_plan`、`lookup_pure_unit_closure_type` 与 `closure_callee_resume_entry_fn_name` 迁出根模块；
    - 新增 `crates/scoopc/src/llvm/codegen/class_ctor.rs`，将 `codegen_class_ctor_call`、ctor 选择/实参求值、super/delegation、init-step 与 invoke lowering 主题整体迁出根模块；
    - `crates/scoopc/src/llvm/codegen/mod.rs` 已收口为主题模块声明、共享上下文与通用 helper；`crates/scoopc/src/llvm/codegen/call/resume.rs` 则改为从 `closure/` 导入 `closure_callee_resume_entry_fn_name`，避免 closure resume 命名 helper 留在根模块。
  - 验证结果：
    - `cargo fmt --all`
    - `cargo test -p scoopc llvm::`
    - `cargo test --all`
    - `cargo clippy --all-targets -- -D warnings`
    - 全部通过。
- 2026-04-25：`T5000b3cR Review：确认 closure/ 与 class_ctor.rs 主题边界成立` 已完成。
  - 复核结论：
    - `crates/scoopc/src/llvm/codegen/closure/mod.rs` 已集中承接 closure expr/env/body lowering、capture/env layout、callee suspend-plan 与 expected-function-type helper；`expr.rs`、`effect/mod.rs`、`call/abi.rs`、`intrinsics/{sync,thread}.rs` 只经 `codegen_closure_expr` / `lookup_pure_unit_closure_type` 等窄接口进入该主题；
    - `crates/scoopc/src/llvm/codegen/class_ctor.rs` 已集中承接 ctor 选择、arg-eval/default binding、super/this delegation、init-step 与 invoke lowering；`call/dispatch.rs` 仅保留 unresolved ctor call 的分派入口，并通过 `ctor_call_sites` 单向委托到 `codegen_class_ctor_call`；
    - review 过程中顺手修复了一个既有文档错配：`class_ctor.rs` 顶部注释此前仍声称“不支持 named/default args”，现已改为准确描述 `CtorCallInfo` 优先路径与 positional-only fallback；
    - 剩余仍在根模块或相邻主题中的相关桥接，已经收敛为调用点委托、函数值调用桥接和 `gc.rs` 中的 closure object/runtime layout helper；它们不再承载 closure/class ctor lowering 主体实现，下一步可直接继续 `T5000b3d` 的 enum/object lowering 拆分；
  - 验证结果：
    - `cargo fmt --all`
    - `cargo test -p scoopc llvm::`
    - `cargo test --all`
    - `cargo clippy --all-targets -- -D warnings`
    - 全部通过。
- 2026-04-25：`T5000b3d 拆出 enum_lowering.rs 与 object_init.rs lowering 模块` 已完成。
  - 实现结果：
    - 新增 `crates/scoopc/src/llvm/codegen/enum_lowering.rs`，将 `codegen_unresolved_ident`、`codegen_enum_variant_ctor_call`、`build_enum_variant_value_from_field_values`、`coerce_enum_payload`、`build_enum_value` 与 `try_codegen_qualified_enum_unit_variant_value` 迁出根模块；
    - 新增 `crates/scoopc/src/llvm/codegen/object_init.rs`，将 object property access、singleton value access、object init function 生成与 body lowering，以及 object once-guard / singleton global / property global helper 迁出根模块；
    - `crates/scoopc/src/llvm/codegen/mod.rs` 现仅新增 `mod enum_lowering;` / `mod object_init;` 声明，并删除上述 enum/object lowering 主体实现。
  - 收尾修复：
    - 迁移后发现 `object_init.rs` 需要显式从 `crate::llvm` 导入 `LLVM_GC_STRATEGY_STATEPOINT_EXAMPLE`；该可见性问题已在本轮立即修复，没有形成新的前置阻塞任务；
    - 迁移过程中发现 `build_enum_value` 初版拷贝遗漏了 `CgEnumRepr::ValueOnly` 分支且未对齐当前 `NicheStorage::U8` lowering，已在提交前回补并与原实现对齐，避免语义回退。
  - 验证结果：
    - `cargo fmt --all`
    - `cargo test -p scoopc llvm::`
    - `cargo test --all`
    - `cargo clippy --all-targets -- -D warnings`
    - 全部通过。
- 2026-04-25：`T5000b3dR Review：确认 codegen/mod.rs 的主题拆分已收口到共享上下文与通用 helper` 已完成。
  - review 先暴露并修复了一个既有边界问题：
    - `crates/scoopc/src/llvm/codegen/mod.rs` 中仍残留 `codegen_sysroot_funptr_invoke`、`codegen_sysroot_funptr_to_uintptr`、`codegen_sysroot_uintptr_to_funptr` 三个 `scoop.unsafe.*` intrinsic lowering；
    - 这些实现已在本轮迁入 `crates/scoopc/src/llvm/codegen/intrinsics/sysroot.rs`，从而把 `call/dispatch.rs` 对 `scoop.unsafe.*` 的分派重新收口为“根模块外的 intrinsics 主题实现”。
  - 复核结论：
    - `enum_lowering.rs` 与 `object_init.rs` 已稳定承接 enum ctor/payload/enum 常量，以及 object singleton/property/init-body lowering；`expr.rs`、`call/dispatch.rs`、`effect/mod.rs` 和 `codegen/mod.rs` 中的顶层值/成员访问路径只通过窄 helper 接口进入这两个主题；
    - `codegen/mod.rs` 当前剩余职责已收敛为共享编译单元/函数上下文、顶层 const/immutable/var 初始化与访问、GC-sensitive spill/root/sret/return helper、通用 lvalue bridge、单态化/具体类型恢复 helper，以及字面量/聚合值/成员访问/运算符/类型转换/通用 coercion lowering；
    - 未再发现需要在 `T5000b3R` 之前追加的 enum/object/sysroot 主题前置缺陷任务；剩余更深层的上下文/cache/effect emitter 分层仍属于后续 `T5000b4`。
  - 验证结果：
    - `cargo fmt --all`
    - `cargo test -p scoopc llvm::`
    - `cargo test --all`
    - `cargo clippy --all-targets -- -D warnings`
    - 全部通过。
- 2026-04-25：`T5000b3R Review：确认 llvm/codegen/mod.rs 的主题拆分是真正的边界整理` 已完成。
  - 汇总复核结果：
    - `call/` 继续稳定承接调用分派、调用点 ABI / 实参绑定、ordinary callee resume 与 effect-call wrapper；
    - `intrinsics/` 继续稳定承接 builtin 与 sysroot intrinsics；
    - `closure/` / `class_ctor.rs` 继续稳定承接 closure lowering 与 class ctor lowering；
    - `enum_lowering.rs` / `object_init.rs` 继续稳定承接 enum constructor / payload / 常量，以及 object singleton / property / init-body lowering；
    - `codegen/mod.rs` 中对应的少量同名入口均已退回薄委托或表达式层统一分派桥接，不再承载这些主题的主体实现。
  - review 进一步确认了根模块剩余职责的性质：
    - `CompilationUnitCodegenCx` / `MainCodegen` 及相关缓存、顶层 const/immutable/var 初始化与访问、GC-sensitive spill/root/sret/return helper、`codegen_addressable_place` 这类通用 lvalue bridge、具体类型恢复 / 通用 coercion / 字面量与聚合值 lowering，当前都更接近“共享上下文 + generic lowering”，而不是应立即继续拆走的已成型主题；
    - `codegen_addressable_place` 虽目前仅被 `intrinsics/atomic.rs` 直接复用，但语义上仍是通用可寻址 place 抽象，因此本轮未把它误判为 atomic lowering 主体残留。
  - review 过程中发现并修复了一个既有文档错配：
    - `crates/scoopc/src/llvm/codegen/mod.rs` 顶部模块注释仍沿用早期“最小子集 / 不支持 if/loop”的旧描述；
    - 现已改为准确描述根模块的共享上下文 / generic lowering 边界，以及 `call/`、`intrinsics/`、`closure/`、`class_ctor.rs`、`enum_lowering.rs`、`object_init.rs` 等主题模块的职责分布。
  - review 结论：
    - `llvm/codegen/mod.rs` 的主题拆分已经是实质性的边界整理，而不是机械切碎；
    - 下一步 `T5000b4` 的切入点已经明确收敛为继续拆分 `CompilationUnitCodegenCx` / `MainCodegen` 的 module / function / cache / effect emitter 职责，而不是再次回头追打本轮已拆完的主题模块。
  - 验证结果：
    - `cargo fmt --all`
    - `cargo test -p scoopc llvm::`
    - `cargo test --all`
    - `cargo clippy --all-targets -- -D warnings`
    - 全部通过。
- 2026-04-25：`T5000b4 继续拆分 MainCodegen 为 module / function / cache / effect emitter 上下文` 已判定为单轮过大任务，现拆成 `T5000b4a`～`T5000b4c` 三个实现子任务与对应 review。
  - 拆分依据：
    - `crates/scoopc/src/llvm/codegen/mod.rs` 仍有 7185 行；
    - `crates/scoopc/src/llvm/codegen/effect/state_machine_emitter.rs` 仍有 5923 行，且存在多处手动保存/恢复 `MainCodegen` 字段；
    - `layout.rs` / `ty.rs` 仍独占多类 layout cache，适合作为首先收口的共享 cache 上下文。
  - 拆分顺序：
    - `T5000b4a`：先抽出编译单元级共享 layout / suspend-analysis cache；
    - `T5000b4b`：再拆 `MainCodegen` 的 function/body 级上下文；
    - `T5000b4c`：最后抽出 effect/state-machine emitter 专用上下文。
- 2026-04-25：`T5000b4a 抽出编译单元级共享 layout / suspend-analysis cache` 已完成。
  - 实现结果：
    - 在 `crates/scoopc/src/llvm/codegen/mod.rs` 中新增 `SharedCodegenCaches`，把 `known_fun_call_suspend_cache`、`type_layout_cache`、`option_niche_cache`、`enum_cg_layout_cache`、`class_init_layout_cache` 与 `pack_field_indices` 统一收口到 `CompilationUnitCodegenCx` 的编译单元级共享 cache；
    - `MainCodegen` 当前已删除上述 layout / analysis cache 字段，`fresh_main_codegen()` / `fresh_child_codegen()` 构造的新实例统一复用 `shared_caches`，不再为每个函数级 codegen 重新携带一份 cache 容器；
    - `crates/scoopc/src/llvm/codegen/layout.rs`、`ty.rs`、`effect/state_machine_plan.rs` 与 `codegen/mod.rs` 的相关访问路径已统一改为经由共享 cache 读写；
    - `cg_enum_layout(...)` 现改为返回从共享 cache 克隆出的 `CgEnumLayout`，从而避免在 `RefCell` 化之后把 cache 借用继续带入后续 lowering 路径。
  - 文档/注释收尾：
    - 已同步更新 `enum_lowering.rs`、`control_flow.rs`、`ty.rs` 中关于 enum layout 获取方式的注释，确保口径与“共享 cache + clone layout”的新行为一致。
  - 验证结果：
    - `cargo fmt --all`
    - `cargo test -p scoopc llvm::`
    - `cargo test --all`
    - `cargo clippy --all-targets -- -D warnings`
    - 全部通过。
- 2026-04-25：`T5000b4aR Review：确认共享 cache 已脱离 MainCodegen 的函数级状态` 已完成。
  - 复核结果：
    - 已复核 `crates/scoopc/src/llvm/codegen/mod.rs`，确认 `SharedCodegenCaches` 现由 `CompilationUnitCodegenCx` 持有，`MainCodegen` 已不再直接持有六类 layout / suspend-analysis cache；
    - 已复核 `crates/scoopc/src/llvm/emit.rs`，确认实现代码中仍只有一个 `CompilationUnitCodegenCx::new(...)` 构造入口；顶层声明、reachable top-level function body 发射与入口 `main` lowering 统一经 `fresh_main_codegen()` 进入，而 effect-call wrapper、closure body、object init 等 nested lowering 统一经 `fresh_child_codegen()` 进入，均会复用同一编译单元级共享 cache；
    - 已复核 `crates/scoopc/src/llvm/codegen/layout.rs`、`ty.rs` 与 `effect/state_machine_plan.rs`，确认 `known_fun_call_suspend_cache`、`type_layout_cache`、`option_niche_cache`、`enum_cg_layout_cache`、`class_init_layout_cache`、`pack_field_indices` 的读写均已统一经由 `self.shared_caches` 进行，没有残留“每个 MainCodegen 自带一份 cache 容器”的路径；
    - 已确认 `cg_enum_layout(...)` 继续返回从共享 cache 克隆出的 layout，`packed-field` 索引回填也稳定写回共享 cache，因此后续 `T5000b4b` / `T5000b4c` 不再需要同时处理 cache 借用或缓存所有权迁移问题。
  - review 结论：
    - 共享 cache 已成功脱离 `MainCodegen` 的函数级状态；
    - 后续 function/body 与 effect emitter 上下文拆分可以聚焦真正的生命周期状态；
    - 未发现需要插入到 `T5000b4b` 之前的新前置缺陷任务。
  - 验证结果：
    - `cargo test -p scoopc llvm::`
    - `cargo test --all`
    - `cargo clippy --all-targets -- -D warnings`
    - 全部通过。
- 2026-04-25：`T5000b4b 拆出 MainCodegen 的 function/body 级上下文` 已完成。
  - 实现结果：
    - 在 `crates/scoopc/src/llvm/codegen/mod.rs` 中新增 `FunctionBodyCodegenCx`，把 `env`、`extra_gc_root_slots` / `next_extra_gc_root_slot_id`、`loop_context_stack`、`return_context`、`current_fun_return_ty`、`current_sret_return_ptr` 与 `top_level_const_eval_stack` 收口为独立的函数 / body 生命周期上下文；
    - `MainCodegen` 当前改为持有 `function_cx: FunctionBodyCodegenCx<'ctx>`，并新增 `take_function_body_cx()` / `restore_function_body_cx()` 作为整组函数级状态的保存/恢复入口；
    - `stmt.rs`、`control_flow.rs`、`gc.rs`、`class_ctor.rs`、`closure/mod.rs`、`object_init.rs`、`call/abi.rs`、`call/dispatch.rs`、`effect/mod.rs`、`effect/state_machine_plan.rs`、`effect/state_machine_emitter.rs`、`intrinsics/containers.rs`、`intrinsics/sysroot.rs` 与 `codegen/mod.rs` 的相关 lowering helper，现已统一经 `self.function_cx` 访问函数级状态，不再直接把这些字段平铺在 `MainCodegen` 上；
    - `crates/scoopc/src/llvm/codegen/call/resume.rs` 的 callee resume entry 发射，以及 `crates/scoopc/src/llvm/codegen/effect/state_machine_emitter.rs` 的 step/dispatch runtime function 发射入口，现已改为直接交换整组 `function_cx`，从而把 child / nested lowering 对函数级状态的保存/重建边界显式化，而不是继续手动搬运 `env + loop + return` 多个普通字段。
  - 阶段结论：
    - `MainCodegen` 与编译单元级共享输入之间的函数 / body 生命周期边界已经从字段层面显式分离；
    - effect emitter 入口处的普通函数级保存/恢复面已经显著收窄，后续 `T5000b4c` 可以更集中地处理 effect 专属运行态；
    - 本轮未发现需要插入到 `T5000b4bR` 之前的新前置缺陷任务。
  - 验证结果：
    - `cargo fmt --all`
    - `cargo test -p scoopc llvm::`
    - `cargo test --all`
    - `cargo clippy --all-targets -- -D warnings`
    - 全部通过。
- 2026-04-25：`T5000b4bR Review：确认 function/body 级上下文边界成立` 已完成。
  - 复核结果：
    - 已确认 `MainCodegen` 当前只剩 `current_source_id`、`function_cx` 与 effect 专属状态；原任务列出的七类函数 / body 生命周期字段都已收口到 `FunctionBodyCodegenCx`，实现代码中也不再残留 `self.env`、`self.return_context`、`self.current_fun_return_ty` 等旧访问；
    - 已确认 `call/resume.rs` 的 callee resume entry 发射，以及 `effect/state_machine_emitter.rs` 的 step / dispatch runtime function 发射入口，都已改为整组 `take_function_body_cx()` / `restore_function_body_cx()` 交换函数级状态；effect emitter 内剩余的 `return_context` / `current_fun_return_ty` 保存恢复仅用于同一 runtime function 内的局部语义覆写，不再构成“普通函数级状态成片手工保存/恢复”；
    - 已确认 `effect_function_return_context`、`current_callee_suspend_plan`、`current_callee_resume_entry_fn`、`current_continuation_resume_replay`、`current_continuation_resume_replay_context`、`active_suspend_site_effect_outcome_capture` 与 `suspend_site_explicit_effect_outcomes` 现已清晰收敛为下一步 `T5000b4c` 的 effect emitter 专属上下文范围；`current_source_id` 继续保留为 generic lowering / 诊断上下文，没有暴露出必须先插入的新前置缺陷任务。
  - review 结论：
    - function/body 生命周期边界已经成立；
    - `T5000b4c` 可以直接聚焦 effect/state-machine emitter 专属运行态，而不必再夹带普通函数级状态收口。
  - 验证结果：
    - `cargo test -p scoopc llvm::`
    - `cargo test --all`
    - `cargo clippy --all-targets -- -D warnings`
    - 全部通过。
- 2026-04-25：`T5000b4c 抽出 effect/state-machine emitter 专用上下文` 已完成。
  - 实现结果：
    - 在 `crates/scoopc/src/llvm/codegen/mod.rs` 中新增 `EffectLoweringCodegenCx`，并细分 `CalleeSuspendLoweringCodegenCx`、`ContinuationResumeReplayCodegenCx` 与 `SuspendSiteEffectOutcomeCodegenCx` 三个子上下文，把 `effect_function_return_context`、ordinary callee suspend/replay 状态、continuation replay 状态与 suspend-site explicit outcome 捕获状态从 `MainCodegen` 根字段整体收口到 effect 专用上下文；
    - `MainCodegen` 当前新增 `take_effect_lowering_cx()` / `restore_effect_lowering_cx()`、`with_effect_function_return_context(...)`、`with_callee_suspend_lowering(...)`、`with_active_suspend_site_effect_outcome_capture(...)` 与 `with_continuation_resume_replay(...)` 等 helper，用于整组切换或局部覆写 effect 运行态；
    - `crates/scoopc/src/llvm/codegen/effect/state_machine_emitter.rs` 的 step / dispatch runtime function 发射入口现已改为整组交换 effect 上下文，step/dispatch return bridge 统一经 helper 安装；`SuspendCall`、object-init boundary、runtime-raise boundary、ordinary callee fresh path 以及 continuation replay 的 effect 状态覆写，也都已改为新 helper，而不是继续在 emitter 内手动保存/恢复多串字段；
    - `crates/scoopc/src/llvm/codegen/mod.rs` 的顶层函数 lowering、`closure/mod.rs` 的 closure body lowering、`call/resume.rs` 的 callee resume dispatch 与 `effect/mod.rs` 的相关读取点，现也都统一改为走新的 effect 上下文入口，不再直接依赖 `MainCodegen` 平铺字段。
  - 阶段结论：
    - `MainCodegen` 已不再直接承载 effect emitter 的主要运行态；
    - step / dispatch runtime function 与 ordinary callee/continuation replay 的 effect 专属状态切换边界已经显式化；
    - 本轮验证中未发现需要插入到 `T5000b4cR` 之前的新前置缺陷任务。
  - 验证结果：
    - `cargo fmt --all`
    - `cargo test -p scoopc llvm::`
    - `cargo test --all`
    - `cargo clippy --all-targets -- -D warnings`
    - 全部通过。
- 2026-04-25：`T5000b4cR Review：确认 effect/state-machine emitter 上下文边界成立` 已完成。
  - 复核结果：
    - 已复核 `crates/scoopc/src/llvm/codegen/mod.rs`，确认 `EffectLoweringCodegenCx` 及其 `callee_suspend` / `continuation_resume_replay` / `suspend_site_effect_outcomes` 子上下文已完整承接 `effect_function_return_context`、ordinary callee suspend/resume lowering、continuation replay 与 suspend-site explicit outcome 捕获状态；
    - 已复核 `crates/scoopc/src/llvm/codegen/effect/state_machine_emitter.rs`、`effect/mod.rs`、`call/resume.rs` 与 `closure/mod.rs`，确认 effect 专属状态的安装、切换与查询统一经 `take_effect_lowering_cx()` / `restore_effect_lowering_cx()`、getter 与 `with_*` helper 进入；`state_machine_emitter` 不再直接读写 `effect_cx` 内部字段；
    - review 过程中暴露并修复了一个既有边界问题：`state_machine_emitter.rs` 仍有 4 处手工保存/恢复 `function_cx.return_context` 与 `current_fun_return_ty = Never` 的局部覆写，而且中间夹着会 `?` 提前返回的调用；现已在 `crates/scoopc/src/llvm/codegen/mod.rs` 中新增 `with_local_never_return_semantics(...)`，并让 ordinary callee replay、`Continuation.resume(...)` replay 与 handler arm body 等路径统一改走该 helper，从而在成功/失败两条路径上都能恢复函数级返回状态；
    - 已明确区分边界：
      - effect emitter 专属上下文：`function_return_context`、callee suspend/resume lowering、continuation replay、suspend-site outcome capture；
      - backend generic lowering / function-body 上下文：`current_source_id`、`FunctionBodyCodegenCx` 中的 env/loop/return/return-ty/sret 等状态；
    - 未发现需要插入到 `T5000b4R` 之前的新前置缺陷任务。
  - 验证结果：
    - `cargo fmt --all`
    - `cargo test -p scoopc llvm::`
    - `cargo test --all`
    - `cargo clippy --all-targets -- -D warnings`
    - 全部通过。
- 2026-04-25：`T5000b4R Review：确认 MainCodegen 的上下文分层已经成立` 已完成。
  - 复核结果：
    - 已复核 `crates/scoopc/src/llvm/codegen/mod.rs` 中的 `CompilationUnitCodegenCx` / `SharedCodegenCaches` / `FunctionBodyCodegenCx` / `EffectLoweringCodegenCx` / `MainCodegen` 构造入口，确认编译单元级只读输入与共享 cache、函数体生命周期状态、effect emitter 专属运行态之间已形成稳定分层；共享 cache 不再随着 `fresh_main_codegen()` / `fresh_child_codegen()` 重建，effect 专属状态也已退出 `MainCodegen` 的平铺字段；
    - 已复核 `crates/scoopc/src/llvm/codegen/effect/state_machine_emitter.rs`、`call/resume.rs`、`closure/mod.rs` 与 `object_init.rs` 的 runtime-function / child-codegen 路径，确认 step/dispatch runtime function 通过 `take_*_cx()` / `restore_*_cx()` 成组切换上下文，ordinary callee resume entry 则只重置函数级上下文，没有新的 effect/runtime-function 状态越界回灌到 generic lowering；
    - 已明确下一步 `ProgramFacts` 抽离的切入点：
      - `crates/scoopc/src/llvm/codegen/effect/state_machine_plan.rs` 中的 `HandlePlanContext::from_codegen(...)` 仍直接从 `MainCodegen` 采集 ctor/object/property/type/local metadata；
      - 同文件中的 `ensure_known_fun_body_may_outward_effect_cache(...)` / `known_fun_body_may_outward_effect_map(...)` 仍在 LLVM backend 内构造 higher-order suspendability facts；
      - `crates/scoopc/src/llvm/codegen/mod.rs` 与 `state_machine_plan.rs` 仍各自维护 concrete-type / field-type 恢复 helper，说明这部分 shared facts 尚未统一迁到 backend 外；
      - `crates/scoopc/src/effect_step_summary.rs` 已直接复用 `state_machine_plan.rs` 的纯分析实现，进一步证明这些事实层已经有 backend 外消费者；
    - review 结论：`MainCodegen` 的上下文分层已经成立，下一步要迁出的主要是 shared facts / analysis side tables，而不是继续在 backend 主上下文上拆更多 runtime 状态；未发现需要插入到 `T5000bR` 之前的新前置缺陷任务。
  - 验证结果：
    - `cargo test -p scoopc llvm::`
    - `cargo test --all`
    - `cargo clippy --all-targets -- -D warnings`
    - 全部通过。
- 2026-04-25：`T5000bR Review：确认 LLVM codegen 已收口到“只做 backend lowering”的方向` 已完成。
  - 复核结果：
    - 已复核 `crates/scoopc/src/llvm/codegen/` 当前模块面：`call/`、`intrinsics/`、`closure/`、`class_ctor.rs`、`enum_lowering.rs`、`object_init.rs`、`effect/`、`gc.rs`、`runtime_abi.rs` 等主题模块均继续承接稳定 backend lowering；`crates/scoopc/src/llvm/codegen/mod.rs` 当前剩余职责则集中在共享上下文、顶层初始化/访问、generic expr/value lowering、GC-sensitive helper 与通用 lvalue bridge，说明本轮确实在拉直 backend 边界，而不只是把原来的巨型文件打散；
    - 已再次确认“仍不应继续留在 LLVM backend 内”的 shared facts / analysis side tables 入口已经清晰暴露：
      - `crates/scoopc/src/llvm/codegen/effect/state_machine_plan.rs` 的 `HandlePlanContext::from_codegen(...)` 仍直接从 `MainCodegen` 采集 ctor/object/property/type/local metadata；
      - 同文件的 `ensure_known_fun_body_may_outward_effect_cache(...)` / `known_fun_body_may_outward_effect_map(...)` 仍在 codegen 内拼装 `SuspendCallProgramFacts` 并缓存 higher-order suspendability facts；
      - `crates/scoopc/src/llvm/codegen/mod.rs` 与 `state_machine_plan.rs` 仍各自维护 concrete-type / field-type 恢复 helper，说明 receiver exactness、field specialization 与 concrete-type resolution 还没有形成 backend-agnostic 的共享事实层；
      - `crates/scoopc/src/effect_step_summary.rs` 继续直接复用 `state_machine_plan.rs` 的纯分析实现，证明这些事实层已经同时服务 backend 外消费者；
    - review 过程中顺手修复了一个既有注释错配：`crates/scoopc/src/llvm/codegen/mod.rs` 顶部模块注释此前仍写“下一步 T5000b4”，现已改为准确指向下一条 `T5000c` 的 shared-facts 抽离工作；
    - review 结论：LLVM codegen 的主题拆分与上下文分层已经把边界拉直到“backend lowering 与 shared facts 的分界线”可清晰审计；下一步应进入 `T5000c`，抽离 `ProgramFacts` / `EffectAnalysisCtx` / shared side tables，而不是继续在 backend 内扩张分析逻辑。当前未发现需要插到 `T5000c` 之前的新前置缺陷任务。
  - 下一条待执行任务切换为 `T5000c 抽离 backend-agnostic 的 ProgramFacts / EffectAnalysisCtx / shared side tables`。
- 2026-04-25：`T5000c 抽离 backend-agnostic 的 ProgramFacts / EffectAnalysisCtx / shared side tables` 已判定为单轮过大任务，现拆成 `T5000c1`～`T5000c3` 三个实现子任务与对应 review。
  - 拆分依据：
    - `crates/scoopc/src/llvm/codegen/effect/state_machine_plan.rs` 中同时存在 `HandlePlanContext::from_codegen(...)`、`ensure_known_fun_body_may_outward_effect_cache(...)` 与 `SuspendCallProgramFacts` 现场拼装，shared facts 来源仍混在 backend codegen / effect planning / suspendability cache 里；
    - `crates/scoopc/src/llvm/codegen/effect/state_machine_segments.rs` 与 `state_machine_transform.rs` 的测试 helper 也各自重复构造同类 facts，说明 `ProgramFacts` 尚未形成统一边界；
    - `crates/scoopc/src/effect_step_summary.rs` 仍通过 `include!(\"llvm/codegen/effect/state_machine_plan.rs\")` 复用纯分析实现，说明迁移 shared facts 后还需要单独收口 effect analysis context 与共享消费者。
  - 拆分顺序：
    - `T5000c1`：先抽出 backend-agnostic `ProgramFacts` 数据结构与统一 builder，消除 `SuspendCallProgramFacts` 的重复拼装；
    - `T5000c2`：再抽出 `EffectAnalysisCtx` 与 shared local metadata / synthetic symbol / source-path 上下文；
    - `T5000c3`：最后迁移 planning / direct-step summary 等共享分析消费者，并清理 `include!` backend 源文件耦合。
  - 下一条待执行任务切换为 `T5000c1 抽出 backend-agnostic 的 ProgramFacts 数据结构与统一 builder`。
- 2026-04-25：`T5000c1 抽出 backend-agnostic 的 ProgramFacts 数据结构与统一 builder` 已完成。
  - 实现结果：
    - 新增 `crates/scoopc/src/program_facts.rs` 与 `lib.rs` 模块入口，定义 backend-agnostic `ProgramFacts`，统一承接 ctor / continuation resume call-site、top-level value / function return / object property / struct/class field type、class super-key、object/property/top-level immutable value 集合等共享 facts；
    - `ProgramFacts::from_lowered(&hir::LoweredHir)` 现作为单一构造入口，从 lowering side tables 一次性生产 shared facts；
    - `crates/scoopc/src/llvm/emit.rs` 现会在进入 LLVM backend 前构造共享 `Rc<ProgramFacts>`，`crates/scoopc/src/llvm/codegen/mod.rs` 的 `CompilationUnitCodegenCx` 现持有该 shared facts，从而不再继续保存一组仅供 effect analysis 重建使用的 backend 专有 side tables；
    - `crates/scoopc/src/llvm/codegen/effect/state_machine_plan.rs` 中的 `HandlePlanContext`、`SuspendCallAnalysis`、`ensure_known_fun_body_may_outward_effect_cache(...)` 与 higher-order function-value suspendability 查询，现已统一复用同一份 `ProgramFacts`，原 `SuspendCallProgramFacts` 临时拼装结构已删除；
    - `crates/scoopc/src/llvm/codegen/effect/state_machine_segments.rs` 与 `state_machine_transform.rs` 的测试 helper 也已改为从 `LoweredHir` 统一构造 `ProgramFacts`，不再各自复制一份 facts 拼装逻辑。
  - 收尾修复：
    - 为满足“无告警构建”约束，本轮顺手修复了 `crates/scoopc/src/effect_step_summary.rs` 在 `--no-default-features` 路径下因直接 `include!` 整个 `state_machine_plan.rs` 而暴露的大量 intentional dead-code / unused-import warnings；当前已把告警边界收口在 `effect_step_summary.rs` 自身，保持共享语义不变并恢复无告警构建。
  - 验证结果：
    - `cargo fmt --all`
    - `cargo test -p scoopc llvm::`
    - `cargo test -p scoopc --no-default-features`
    - `cargo test --all`
    - `cargo clippy --all-targets -- -D warnings`
    - 全部通过。
  - 下一条待执行任务切换为 `T5000c1R Review：确认 ProgramFacts 已成为 backend-agnostic 的共享 side table`。
- 2026-04-25：`T5000c1R Review：确认 ProgramFacts 已成为 backend-agnostic 的共享 side table` 已完成。
  - 复核结果：
    - 已复核 `crates/scoopc/src/program_facts.rs` 与 `crates/scoopc/src/lib.rs`，确认 `ProgramFacts` 结构与 `ProgramFacts::from_lowered(...)` 只依赖 HIR lowering side tables 与 `TypeId`，不依赖 LLVM builder / module / GC ABI，因此已形成 backend-agnostic 的共享 facts 层；
    - 已复核 `crates/scoopc/src/llvm/emit.rs`、`crates/scoopc/src/llvm/codegen/mod.rs`、`crates/scoopc/src/llvm/codegen/effect/state_machine_plan.rs`、`state_machine_segments.rs` 与 `state_machine_transform.rs`，确认生产路径与 effect 测试 helper 都统一经 `ProgramFacts::from_lowered(&hir::LoweredHir)` 构造 facts，没有残留第二套 `SuspendCallProgramFacts` 或手写 side-table 现场拼装；
    - review 过程中暴露并修复了一个既有共享性回退：`SuspendCallAnalysis::handle_may_suspend_outward(...)` 原先会把借来的 `ProgramFacts` 整体 `clone` 后重新包成新的 `Rc` 交给 nested `HandlePlanContext`；现已把 `SuspendCallAnalysis` 与相关测试 helper 统一改为持有并传递共享 `Rc<ProgramFacts>`，使 nested-handle suspendability 分析重新复用同一份 side table，而不是复制整表。
  - review 结论：
    - `ProgramFacts` 的来源已经稳定收口为 `LoweredHir -> ProgramFacts::from_lowered(...) -> Rc<ProgramFacts>`；
    - 后续 `T5000c2` 可以直接聚焦 `HandlePlanContext::from_codegen(...)`、known local metadata、synthetic symbol/source-path 等 `EffectAnalysisCtx` 边界，而不必再先清理 facts 来源；
    - 未发现需要插入到 `T5000c2` 之前的新前置缺陷任务。
  - 验证结果：
    - `cargo fmt --all`
    - `cargo test -p scoopc llvm::`
    - `cargo test -p scoopc --no-default-features`
    - `cargo test --all`
    - `cargo clippy --all-targets -- -D warnings`
    - 全部通过。
  - 下一条待执行任务切换为 `T5000c2 抽出 backend-agnostic 的 EffectAnalysisCtx 与 shared local metadata`。
- 2026-04-25：`T5000c2 抽出 backend-agnostic 的 EffectAnalysisCtx 与 shared local metadata` 已完成。
  - 实现结果：
    - 新增 `crates/scoopc/src/effect_analysis.rs` 与 `lib.rs` 模块入口，抽出 backend-agnostic `EffectAnalysisCtx`、`KnownLocalMetadata`，统一承接 known fun/local effect facts、known local metadata、synthetic symbol allocator 状态，以及 source-path / call-site 关联上下文；
    - `crates/scoopc/src/llvm/codegen/effect/state_machine_plan.rs` 现将 `HandlePlanContext` 收口为 `EffectAnalysisCtx` 别名，`SuspendCallAnalysis` 改为直接依赖共享 analysis context，而不再重复平铺 `known_fun_effects`、`known_local_metadata`、`current_source_path` 与 `ProgramFacts`；
    - `HandlePlanContext::from_codegen(...)` 已移除；LLVM backend 当前仅通过 `MainCodegen::effect_analysis_ctx()` 把函数级 env 投影成共享 `EffectAnalysisCtx`，然后供 ordinary callee suspend planning、`build_unified_lowering_contract(...)` 与 local higher-order suspendability 查询复用；
    - `state_machine_segments.rs` 与 `state_machine_transform.rs` 的测试 helper 已统一改为复用 `collect_effect_analysis_context_for_fun(...)`，不再继续手工拼装平行的 plan-analysis context。
  - 收尾修复：
    - 在验证过程中暴露并修复了两个告警回退：`state_machine_segments.rs` 与 `state_machine_transform.rs` 测试模块里残留的 `HashMap` unused import 已删除，从而恢复无告警测试构建。
  - 验证结果：
    - `cargo fmt --all`
    - `cargo test -p scoopc llvm::`
    - `cargo test -p scoopc --no-default-features`
    - `cargo test --all`
    - `cargo clippy --all-targets -- -D warnings`
    - 全部通过。
  - 下一条待执行任务切换为 `T5000c2R Review：确认 EffectAnalysisCtx 已脱离 LLVM backend 现场取数`。
- 2026-04-26：`T5000c2R Review：确认 EffectAnalysisCtx 已脱离 LLVM backend 现场取数` 已完成。
  - 复核结果：
    - `crates/scoopc/src/effect_analysis.rs` 中的 `EffectAnalysisCtx` / `KnownLocalMetadata` 只依赖 `hir`、`TypeId`、`ProgramFacts`、`PathBuf` 与标准库容器/内部可变性，不含 LLVM builder/module/GC ABI/runtime helper 类型，也没有 `feature = "llvm"` 约束，因此 analysis context 本体已经是 backend-agnostic 的共享输入；
    - `crates/scoopc/src/llvm/codegen/effect/state_machine_plan.rs` 中 `HandlePlanContext` 现在仅是 `EffectAnalysisCtx` 别名，`SuspendCallAnalysis` / `HandlePlanBuilder` / direct-step summary helper 都经共享 context 获取 known fun/local effects、known local metadata、synthetic symbol allocator、source-path / call-site 与 `ProgramFacts`，不再平铺字段或从 backend 主上下文直接取数；
    - `MainCodegen::effect_analysis_ctx()` 当前只承担 LLVM backend -> shared context 的单向投影；另一方面，`collect_effect_analysis_context_for_fun(...)` 已为 backend 外构造提供稳定入口，`state_machine_segments.rs` 与 `state_machine_transform.rs` 的测试 helper 现统一经该入口复用 analysis context，不再继续手工拼装平行上下文，因此没有残留“必须通过 MainCodegen 才能做分析”的强耦合路径；
    - `effect_step_summary.rs` 仍通过 `include!` 复用 `state_machine_plan.rs`，但这是 `T5000c3` 已显式跟踪的下一步消费者迁移工作；本次 review 未发现需要插入到 `T5000c3` 之前的新前置缺陷任务。
  - 验证结果：
    - `cargo fmt --all --check`
    - `cargo test -p scoopc llvm::`
    - `cargo test -p scoopc --no-default-features`
    - `cargo test --all`
    - `cargo clippy --all-targets -- -D warnings`
    - 全部通过。
-  - 下一条待执行任务切换为 `T5000c3 迁移 effect/state-machine planning 与 direct-step summary 到 shared facts / analysis 层`。
- 2026-04-26：`T5000c3 迁移 effect/state-machine planning 与 direct-step summary 到 shared facts / analysis 层` 已判定为单轮过大任务，现拆成 `T5000c3a`～`T5000c3b` 两个实现子任务与对应 review。
  - 拆分依据：
    - `crates/scoopc/src/llvm/codegen/effect/state_machine_plan.rs` 当前约 9800 行，其中除 `#[cfg(feature = "llvm")] impl MainCodegen` 薄 backend 入口外，大部分已是 pure analysis；shared planning / direct-step summary 源文件归属仍挂在 backend 路径上；
    - `crates/scoopc/src/effect_step_summary.rs` 仍通过 `include!("llvm/codegen/effect/state_machine_plan.rs")` 文本级复用 backend 源文件，说明 shared analysis consumer 仍然依赖 backend 源文件所有权，而不是稳定的 shared 归属层；
    - `crates/scoopc/src/llvm/codegen/mod.rs` 与 planning / summary 内部的 concrete-type / field-type / receiver exactness helper 仍存在平行实现，说明 helper 的消费方向尚未真正拉直到 shared facts / analysis 层。
  - 拆分顺序：
    - `T5000c3a`：先抽出共享 `effect_state_machine_analysis.rs` 源文件，并清理 `effect_step_summary.rs` 对 backend 文件的 `include!`；
    - `T5000c3b`：再收口 concrete-type / field-type / receiver exactness helper 的消费方向；
  - 下一条待执行任务切换为 `T5000c3a 抽出共享 effect_state_machine_analysis.rs 源文件并清理 backend include!`。
- 2026-04-26：`T5000c3a 抽出共享 effect_state_machine_analysis.rs 源文件并清理 backend include!` 已完成。
  - 实现结果：
    - 新增 `crates/scoopc/src/effect_state_machine_analysis.rs`，承接原 `llvm/codegen/effect/state_machine_plan.rs` 的 pure analysis 主体，包括 handle planning、higher-order suspendability summary、direct-step summary 与相关测试；
    - `crates/scoopc/src/llvm/codegen/effect/state_machine_plan.rs` 现已收口为薄包装，只负责把 shared analysis 源文件重新 `include!` 到 backend `unified_state_machine_skeleton` 模块的本地可见性作用域，而不再充当 shared analysis 的归属路径；
    - `crates/scoopc/src/effect_step_summary.rs` 已改为直接复用 `effect_state_machine_analysis.rs`，从而消除对 backend 路径 `llvm/codegen/effect/state_machine_plan.rs` 的文本级依赖；
  - 阶段结论：
    - shared planning / direct-step summary 源文件所有权已脱离 `llvm/codegen/effect/`；
    - backend 与非 LLVM 消费者当前继续复用同一份源码，但 shared consumer 已不再依赖 backend 源文件路径；
    - 本轮尚未处理 concrete-type / field-type / receiver exactness helper 的消费方向，这部分继续留给下一条 `T5000c3aR` / `T5000c3b`。
  - 验证结果：
    - `cargo fmt --all --check`
    - `cargo test -p scoopc llvm::`
    - `cargo test -p scoopc --no-default-features`
    - `cargo test --all`
    - `cargo clippy --all-targets -- -D warnings`
    - 全部通过。
  - 下一条待执行任务切换为 `T5000c3aR Review：确认 shared planning / summary 源文件已脱离 backend 路径`。
- 2026-04-26：`T5000c3aR Review：确认 shared planning / summary 源文件已脱离 backend 路径` 已完成。
  - 复核结果：
    - 已复核 `crates/scoopc/src/effect_state_machine_analysis.rs`、`crates/scoopc/src/llvm/codegen/effect/state_machine_plan.rs`、`crates/scoopc/src/effect_step_summary.rs` 与 `crates/scoopc/src/llvm/codegen/effect/mod.rs`，确认 handle planning、higher-order suspendability summary、direct-step summary 与相关测试主体当前统一归属到 crate 根 shared 源文件 `effect_state_machine_analysis.rs`；
    - 已确认 backend 文件 `crates/scoopc/src/llvm/codegen/effect/state_machine_plan.rs` 当前只剩薄包装：文件内容仅负责把 `../../../effect_state_machine_analysis.rs` `include!` 回 `unified_state_machine_skeleton` 的局部作用域，不再直接承载 pure analysis 主体；
    - 已搜索 `crates/scoopc/src` 中所有对 `state_machine_plan.rs` / `effect_state_machine_analysis.rs` 的文本级复用路径，确认 `effect_step_summary.rs` 当前直接复用 shared 源文件，仓库中已不存在 non-LLVM consumer 继续经由 backend 路径复用 shared analysis 源文件的残留。
  - review 结论：
    - shared planning / direct-step summary 源文件归属已稳定脱离 backend 路径；
    - 后续 `T5000c3b` 可以只聚焦 concrete-type / field-type / receiver exactness helper 的消费方向，而不必再回头处理 shared source ownership；
    - 未发现需要插入到 `T5000c3b` 之前的新前置缺陷任务。
  - 验证结果：
    - `cargo fmt --all --check`
    - `cargo test -p scoopc llvm::`
    - `cargo test -p scoopc --no-default-features`
    - `cargo test --all`
    - `cargo clippy --all-targets -- -D warnings`
    - 全部通过。
  - 下一条待执行任务切换为 `T5000c3b 收口 concrete-type / field-type / receiver exactness 共享 helper 的消费方向`。
- 2026-04-26：`T5000c3b 收口 concrete-type / field-type / receiver exactness 共享 helper 的消费方向` 已完成。
  - 实现结果：
    - 新增 `crates/scoopc/src/expr_facts.rs` 与 `lib.rs` 中对应模块声明，把 concrete-type / field-type / call-result 解析收口为 shared `ExprFactResolver`；该 resolver 只依赖 `TypeStore`、`ProgramFacts` 与注入式 local type lookup，不依赖 LLVM builder/module/runtime ABI；
    - `crates/scoopc/src/program_facts.rs` 已补充 `top_level_value_ty`、`object_property_ty`、`fun_return_ty` 与 `resolve_nominal_field_ty` 等查询 helper，使 top-level value、object property、struct/class field 与 class-super 递归查询都由 shared facts 层统一提供；
    - `crates/scoopc/src/effect_state_machine_analysis.rs` 中 planning 的 `resolve_plan_*` concrete-type helper 与 `SuspendCallAnalysis` 内 duplicated 的 concrete-type / field-type / call-result helper 已删除，统一改经 `ExprFactResolver`；
    - `crates/scoopc/src/llvm/codegen/mod.rs` 中原本平行维护的 `resolve_member_access_concrete_type`、`resolve_*_field_concrete_type` 与 `resolve_call_result_type` 已整体删除，`MainCodegen::resolve_expr_concrete_type` 现仅作为对 shared resolver 的薄包装，local exact type 仍由 codegen env 注入。
  - 阶段结论：
    - planning / direct-step summary 与 backend generic lowering 已不再各自维护一套同类 helper；
    - concrete type、receiver exactness 与 field specialization 所需的共同输入已收口到 shared `ProgramFacts` + `expr_facts` 层；
    - 未发现需要插入到 `T5000c3bR` 之前的新前置缺陷任务。
  - 验证结果：
    - `cargo fmt --all --check`
    - `cargo test -p scoopc llvm::tests::lowered_call_results_keep_concrete_types_for_local_bindings`
    - `cargo test -p scoopc direct_step_effect_rows_include_direct_effectful_call_after_escape_site`
    - `cargo test -p scoopc --no-default-features direct_step_effect_rows_include_direct_effectful_call_after_escape_site`
    - `cargo test -p scoopc --no-default-features`
    - `cargo test --all`
    - `cargo clippy --all-targets -- -D warnings`
    - 全部通过。
  - 下一条待执行任务切换为 `T5000c3bR Review：确认 concrete-type / receiver exactness helper 的依赖方向已拉直`。
- 2026-04-26：`T5000c3bR Review：确认 concrete-type / receiver exactness helper 的依赖方向已拉直` 已完成。
  - 复核结论：
    - `crates/scoopc/src/expr_facts.rs` 现已是 concrete-type / field-type / call-result 解析的唯一主体实现；`crates/scoopc/src/llvm/codegen/mod.rs` 与 `crates/scoopc/src/effect_state_machine_analysis.rs` 中保留的 `resolve_expr_concrete_type` 仅负责注入 local type lookup，不再持有独立解析逻辑；
    - `crates/scoopc/src/program_facts.rs` 的 `top_level_value_ty`、`object_property_ty`、`fun_return_ty` 与 `resolve_nominal_field_ty` 已提供 shared resolver 所需的共同输入，effect planning 与 backend generic lowering 现统一经 `ProgramFacts + ExprFactResolver` 获取 top-level/object/field/return 事实，而不再从 backend 现场回捞；
    - 已全文检索旧 helper 名称，确认 `resolve_member_access_concrete_type`、`resolve_*_field_concrete_type`、`resolve_call_result_type` 等主体实现只剩 `expr_facts.rs` 一处；effect planning 侧仅剩 `resolve_plan_expr_concrete_type` 与 `SuspendCallAnalysis::resolve_expr_concrete_type` 两个向 shared resolver 注入 `known_local_metadata` 的轻量桥接；
    - review 同时确认 `MainCodegen::top_level_value_ty` 等 backend 查询仍仅服务 lowering 期 top-level value/function-value 路径，不构成 shared concrete-type / receiver exactness helper 仍留在 backend 的证据；未发现需要插入到 `T5000c3R` 之前的新前置缺陷任务。
  - 验证结果：
    - `cargo fmt --all --check`
    - `cargo test -p scoopc llvm::tests::lowered_call_results_keep_concrete_types_for_local_bindings`
    - `cargo test -p scoopc direct_step_effect_rows_include_direct_effectful_call_after_escape_site`
    - `cargo test -p scoopc --no-default-features direct_step_effect_rows_include_direct_effectful_call_after_escape_site`
    - `cargo test --all`
    - `cargo clippy --all-targets -- -D warnings`
    - 全部通过。
  - 下一条待执行任务切换为 `T5000c3R Review：确认共享分析消费者已脱离 LLVM backend 源文件依赖`。
- 2026-04-26：`T5000c3R Review：确认共享分析消费者已脱离 LLVM backend 源文件依赖` 已完成。
  - 复核结论：
    - `crates/scoopc/src/lib.rs` 现仅在 `#[cfg(not(feature = "llvm"))]` 下暴露 `effect_step_summary`；`crates/scoopc/src/effect_step_summary.rs` 当前直接 `include!("effect_state_machine_analysis.rs")`，而 `crates/scoopc/src/llvm/codegen/effect/state_machine_plan.rs` 只保留 backend 局部可见性的薄包装；全文检索仓库内 shared analysis 源文件的文本级复用路径，确认现仅剩这两条显式入口，不再存在 non-LLVM consumer 经由 backend 路径复用 shared source 的残留；
    - `crates/scoopc/src/program_facts.rs`、`crates/scoopc/src/effect_analysis.rs` 与 `crates/scoopc/src/expr_facts.rs` 已共同提供 planning 与 direct-step summary 的 shared 输入：`ProgramFacts` 负责 top-level/object/field/return 事实，`EffectAnalysisCtx` 负责 known local metadata / local function suspendability / synthetic symbol / source-path 上下文，`ExprFactResolver` 负责 concrete-type / field-type / call-result 解析；共享分析消费者已不再依赖 backend helper 现场回捞这些输入；
    - `crates/scoopc/src/effect_state_machine_analysis.rs` 中剩余 `MainCodegen` 相关入口全部位于 `#[cfg(feature = "llvm")] impl MainCodegen` 内，只作为 backend 调用 shared analysis 的接缝；shared direct-step summary API 与非 LLVM 测试辅助路径继续只消费 shared context / shared functions，没有重新引入对 LLVM backend 类型或 backend 源文件路径的强耦合；
    - review 结论：共享分析消费者已脱离 LLVM backend 源文件依赖，`T5000cR` 已可以基于清晰的 backend-agnostic facts / analysis 边界做总复核；未发现需要插入到 `T5000cR` 之前的新前置缺陷任务。
  - 验证结果：
    - `cargo check -p scoopc --lib`
    - `cargo fmt --all --check`
    - `cargo test -p scoopc llvm::tests::lowered_call_results_keep_concrete_types_for_local_bindings`
    - `cargo test -p scoopc --no-default-features direct_step_effect_rows_include_direct_effectful_call_after_escape_site`
    - `cargo test -p scoopc --no-default-features`
    - `cargo test --all`
    - `cargo clippy --all-targets -- -D warnings`
    - 全部通过。
  - 下一条待执行任务切换为 `T5000cR Review：确认共享事实层已经脱离 LLVM backend 依赖方向`。

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
  - P1-4a：先收口编译单元级共享 layout / suspend-analysis cache；
  - P1-4b：再拆 function/body 级上下文；
  - P1-4c：最后抽出 effect/state-machine emitter 专用上下文；
  - 目标边界最终仍是：
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
