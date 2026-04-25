# TODO（Scoop：early MIR / ANF 优化基线与 LLVM codegen 分层收口）

> 生成时间：2026-04-25  
> 历史归档：`TODO-6.md` / `PLAN-6.md`  
> 顺序约束：严格按当前文件中的条目顺序推进；不得跨条目并行实现。  
> 本轮主线：先建立编译器性能与 codegen 边界基线，再清理 LLVM codegen 的职责分层，随后引入 generic early MIR / ANF template、在 MIR 层完成 monomorphization / instance materialization，并依次接入 per-instance summary、通用 devirtualization、summary-driven inlining、continuation / closure escaping analysis 与 effect/state-machine planning 收口。

## 全局约束

- [`OPTIMIZATION.md`](./OPTIMIZATION.md) 是本轮设计基线；若实现过程改变主张，必须先回写该文档，再继续实现。
- `PLAN-6.md` / `TODO-6.md` 只作历史归档；新的优化主线不得回写旧 round 的计划与任务记录。
- 本轮先做边界整理，再做优化能力扩张；不得在 `llvm/codegen` 现有错位边界上继续堆新的“只在某处生效”的特判。
- early MIR / ANF 必须后端无关。
  - 不允许把 LLVM statepoint、address space、stackmap 形状、`gc.relocate`、mangled symbol name 等 backend 细节编码成 MIR 语义。
- 优化触发必须依赖结构事实，而不是函数名白名单。
  - 不允许把 `map` / `filter` / `Iterator.next()` / `Iterator.hasNext()` 之类名字写成优化成立的前提。
- monomorphization 放在 MIR 内部。
  - 必须形成 `generic MIR template -> monomorphic MIR instance` 的独立阶段；
  - 不允许继续在 LLVM codegen 中通过 mangled FQN 重定向来承担主要实例化职责。
- 自动优化由优化级别控制；`@Inline` 只保留为后续 override / hint 位置。
- effect / state-machine planning 必须晚于 monomorphization、summary、devirtualization、inlining、escape analysis。
- `mem2reg` / register-root 改造不是本轮主线。
  - 近期重点仍是减少调用边界与 safepoint 压力。
- 编译器自身性能是一等目标。
  - 每个主要阶段都必须考虑 `-O0` / debug build 的固定成本；
  - 不允许默认把昂贵 interprocedural 分析塞进 `-O0` 路径。
- 每个实现任务后必须紧跟 review 任务。
  - review 重点是边界是否正确、是否继续把中端逻辑长回 LLVM codegen、以及是否引入不必要的编译器性能回退。
- 若任务改变公开语义或文档口径，必须同步 `SCOOP_FULL_SPEC.md`、`SCOOP_RUNTIME.md`、`OPTIMIZATION.md` 与必要实现注释。

## T5000：建立 early MIR / ANF 优化基线，并把 LLVM codegen 收口到正确边界

### [DONE] T5000a 建立编译器性能与 codegen 边界基线
- 范围：
  - 用当前代码库建立一份最小但可复验的 baseline，至少覆盖：
    - `llvm/codegen` 的主要巨型文件与职责簇；
    - `MainCodegen::new` 的重复构造点；
    - `-O0` / debug build 路径上固定存在的主要成本；
    - 当前 reachability / eager inclusion / codegen 查询临时分析的重复工作。
  - 把这些 baseline 回写到文档或注释中能稳定引用的位置，作为后续各阶段的 guardrail。
- 验收：
  - 能明确回答“当前最主要的 codegen 边界错位在哪里”“当前编译器自身最可能的固定成本热点在哪里”。
  - 后续任务可引用统一 baseline，而不需要每轮重新做一遍同类调查。
- 依赖：无
- 完成记录（2026-04-25）：
  - baseline 已统一固化到 `OPTIMIZATION.md` 第 0、10、11 节；
  - 已记录巨型模块/职责簇、`MainCodegen::new` 重复构造点、`-O0` / debug build 固定成本、reachability / eager inclusion / codegen 查询重复工作及其 guardrail。

### [DONE] T5000aR Review：确认 baseline 已足够支撑后续实现顺序
- 重点：
  - baseline 是否覆盖了 `MainCodegen`、effect middle-end、reachability、O0 cost 这四类关键热点；
  - 是否已经能支持“先 codegen refactoring，再 early MIR”的顺序判断；
  - 是否仍遗漏会直接影响后续阶段划分的结构性热点。
- 验收：
  - baseline 可作为后续 `T5000b+` 的统一前提，不再需要反复回到“从哪里开始”这个问题。
- 依赖：T5000a
- 完成记录（2026-04-25）：
  - 已核对 `MainCodegen::new` 调用点、`llvm/mod.rs` 中的 reachability / eager inclusion、`run_pass_pipeline` / `llvm_pass_pipeline_for_opt_level`、以及 `HandlePlanContext::from_codegen` 等关键证据；
  - 已补记 `crates/scoopc/src/effect_step_summary.rs` 对 `llvm/codegen/effect/state_machine_plan.rs` 的 `include!` 复用，确认这是现有 effect middle-end / shared facts 边界泄漏的一部分，而不是新的前置缺陷任务；
  - review 结论：baseline 足以支撑“先 codegen refactoring，再 early MIR”的顺序，下一条可直接进入 `T5000b`。

### [DONE] T5000b1 拆分 `llvm/mod.rs` 的 emit API / pipeline / reachability / tests
- 范围：
  - 把当前 `crates/scoopc/src/llvm/mod.rs` 中的几类职责拆分到独立模块：
    - emit API / module build；
    - LLVM pass pipeline；
    - reachability；
    - tests。
  - `llvm/mod.rs` 根模块只保留：
    - LLVM 后端公共入口 re-export；
    - 错误类型 / 常量；
    - 子模块声明与少量桥接。
  - 不改变公开行为与优化语义；只做边界收口与可维护性整理。
- 验收：
  - `llvm/mod.rs` 不再同时承载 emit API、module build、reachability、pipeline 与大体量测试；
  - 现有 `emit_minimal_main_*` API、`build_main_module_from_lowered_hir`、`run_pass_pipeline` 的调用点保持工作；
  - 测试仍能覆盖拆分后的模块边界。
- 依赖：T5000aR
- 完成记录（2026-04-25）：
  - 新增 `crates/scoopc/src/llvm/emit.rs`、`crates/scoopc/src/llvm/pipeline.rs`、`crates/scoopc/src/llvm/reachability.rs`、`crates/scoopc/src/llvm/tests.rs`，将原 `llvm/mod.rs` 的实现细节和测试迁出；
  - `crates/scoopc/src/llvm/mod.rs` 已收口为错误类型、常量、子模块声明与必要 re-export，不再承载 emit/pipeline/reachability 的主体实现；
  - 已验证 `cargo test -p scoopc llvm::`、`cargo test --all`、`cargo clippy --all-targets -- -D warnings` 全部通过。

### [DONE] T5000b1R Review：确认 `llvm/mod.rs` 已收口为根模块而非实现巨型文件
- 重点：
  - 根模块是否只保留后端入口、错误边界与必要 re-export；
  - emit / pipeline / reachability / tests 是否已形成稳定的独立文件边界；
  - 拆分后是否给后续 `MainCodegen` / codegen 主题拆分留下清晰入口。
- 验收：
  - 可以明确指出 `llvm/mod.rs` 根模块的职责上界，不再把新的实现细节继续堆回去。
- 依赖：T5000b1
- 完成记录（2026-04-25）：
  - 已复核 `crates/scoopc/src/llvm/mod.rs`，确认根模块当前只保留子模块声明、对外 emit/target re-export、测试期窄桥接 re-export、LLVM GC 策略常量、一次性全局 LLVM 选项配置与统一错误诊断边界；
  - 已复核 `crates/scoopc/src/llvm/emit.rs`、`crates/scoopc/src/llvm/pipeline.rs`、`crates/scoopc/src/llvm/reachability.rs`、`crates/scoopc/src/llvm/tests.rs`，确认 emit API / module build、pass pipeline、reachability 扫描与测试主体实现均已迁出根模块；
  - 已确认 `llvm/codegen/effect/state_machine_emitter.rs` 的测试仅通过 `#[cfg(test)]` 下的窄桥接 re-export 访问 `build_main_module_from_lowered_hir` 与 `run_pass_pipeline`，未把新的实现职责倒灌回 `llvm/mod.rs`；
  - 已验证 `cargo test -p scoopc llvm::` 与 `cargo clippy --all-targets -- -D warnings` 通过，未发现必须插入到 `T5000b2` 之前的新前置缺陷任务。

### [DONE] T5000b2 提炼 `MainCodegen` 共享编译单元上下文与 child-codegen 构造路径
- 范围：
  - 先从 `MainCodegen` 中抽出稳定的共享只读输入与 child-codegen 工厂路径；
  - 消除 `llvm/mod.rs` 与 `llvm/codegen/mod.rs` 内部多处重复拼装 `MainCodegenInputs` 的模式；
  - 为后续 module / function / cache / effect emitter 分层准备稳定入口。
- 验收：
  - `MainCodegen::new` 的主要重复构造点显著收敛；
  - child/nested codegen 不再每次手写整套编译单元输入拼装；
  - 后续拆 cache / effect emitter 上下文时不需要再先做一轮大范围构造点清理。
- 依赖：T5000b1R
- 完成记录（2026-04-25）：
  - 在 `crates/scoopc/src/llvm/codegen/mod.rs` 中新增 `CompilationUnitCodegenCx` / `CompilationUnitCodegenInputs`，把稳定的编译单元输入、共享 `effect_op_tags`、共享 `known_fun_call_suspend_cache` 与预计算的 `known_effect_instances_by_effect_fqn` 收口到统一入口；
  - `MainCodegen` 现改为持有 `shared: &CompilationUnitCodegenCx`，并提供 `fresh_child_codegen()`，从而消除 effect-call wrapper、top-level immutable init、closure body lowering、object init lowering 4 处 child/nested codegen 的整包 `MainCodegenInputs { ... }` 手写拼装；
  - `crates/scoopc/src/llvm/emit.rs` 现仅在一个位置构造编译单元上下文，并通过 `fresh_main_codegen()` 复用到顶层声明、reachable top-level function body 发射与入口 `main` exit-code lowering 3 条路径；
  - `known_effect_instances_by_effect_fqn` 不再随着每次 child-codegen 构造重新扫描 `TypeStore`，为后续继续拆分 `MainCodegen` 的 module/function/cache/effect emitter 上下文留出了稳定入口；
  - 已验证 `cargo fmt --all`、`cargo test -p scoopc llvm::`、`cargo test --all`、`cargo clippy --all-targets -- -D warnings` 全部通过。

### [DONE] T5000b2R Review：确认 `MainCodegen` 构造边界已开始从“巨型输入包”收口
- 重点：
  - 共享编译单元输入是否已和函数级运行时状态分开；
  - 是否仍有大量重复 `MainCodegenInputs { ... }` 手写构造残留；
  - 这一步是否真的降低了后续分层改动的耦合面。
- 验收：
  - 下一步可以在不反复搬运构造样板的前提下继续拆 `MainCodegen` 与 `codegen/mod.rs`。
- 依赖：T5000b2
- 完成记录（2026-04-25）：
  - 已核对 `crates/scoopc/src/llvm/emit.rs` 中 `CompilationUnitCodegenCx::new(codegen::CompilationUnitCodegenInputs { ... })` 仅保留 1 个编译单元构造入口，顶层声明、reachable top-level function body 发射与入口 `main` exit-code lowering 均统一经 `fresh_main_codegen()` 进入；
  - 已核对 `crates/scoopc/src/llvm/codegen/mod.rs` 中 effect-call wrapper、top-level immutable init、closure body lowering、object init lowering 4 条 child/nested 路径均改经 `fresh_child_codegen()`，实现代码内已无残留 `MainCodegenInputs { ... }` 手写构造；
  - 已复核 `MainCodegen` 当前仍保留函数级 builder/env/cache/return/suspend 等局部状态，而共享编译单元输入、`effect_op_tags` 与 `known_effect_instances_by_effect_fqn` 已收口到 `CompilationUnitCodegenCx`；`type_layout_cache` / `enum_cg_layout_cache` / effect emitter 专用上下文等更深层分层仍留待 `T5000b3` / `T5000b4`；
  - 已验证 `cargo test -p scoopc llvm::` 与 `cargo clippy --all-targets -- -D warnings` 通过，未发现必须插入到 `T5000b3` 之前的新前置缺陷任务。

### T5000b3 按主题拆分 `llvm/codegen/mod.rs` 的独立 lowering 模块
- 说明：
  - 经核对，`crates/scoopc/src/llvm/codegen/mod.rs` 当前仍有 17671 行，并且至少同时承载四组稳定函数簇：
    - call dispatch / callable ABI / extern-native / vtable-itable / callee-resume；
    - sysroot / builtin intrinsics；
    - closure / class ctor；
    - enum lowering / object init。
  - 原任务单轮过大，现按稳定主题拆成以下实现子任务；所有子任务均保持语义不变，只做 lowering 边界整理。

### [DONE] T5000b3a 拆出 `call/` lowering 模块
- 范围：
  - 将 `llvm/codegen/mod.rs` 中的 call dispatch、ordinary/callable arg ABI、extern/native call boundary、vtable/itable dispatch、funptr / function-value / closure-object call lowering、callee-resume call boundary 等实现迁入 `llvm/codegen/call/`；
  - 保留 `class_ctor`、`closure`、`intrinsics` 等主题暂时留在各自后续子任务中，通过清晰接口互调，而不是继续把 call 主体留在根模块。
- 验收：
  - `codegen/mod.rs` 不再直接承载 `codegen_call`、top-level fun call、vtable/itable call、funptr/function-value call、callable arg binding 与 ordinary param ABI 的主体实现；
  - `call/` 内部边界至少能区分 dispatch / arg binding / indirect call lowering，不是新的单文件巨型模块。
- 依赖：T5000b2R
- 完成记录（2026-04-25）：
  - 新增 `crates/scoopc/src/llvm/codegen/call/mod.rs`、`call/abi.rs`、`call/dispatch.rs`、`call/resume.rs`，将 `codegen/mod.rs` 中原本混放的 call dispatch、ordinary/callable 参数 ABI、extern/native call、vtable/itable dispatch、funptr/function-value call、ordinary callee resume / top-level effect-call wrapper 等主体实现按主题迁出；
  - `crates/scoopc/src/llvm/codegen/mod.rs` 中保留原有入口名，但主体已改为薄委托到 `*_impl`，从而维持现有调用面不变并收口 call 主题边界；
  - `call/` 内部已按稳定职责拆成 3 个实现面：`dispatch.rs` 负责 direct/virtual/interface/funptr/function-value call dispatch，`abi.rs` 负责参数 ABI、named arg 绑定与 deferred materialization，`resume.rs` 负责 ordinary callee resume 与 top-level effect-call wrapper；
  - 已修复迁移过程中暴露的两个边界问题：`call/resume.rs` 缺少 `LLVM_GC_STRATEGY_STATEPOINT_EXAMPLE` 导入，以及 `*_impl` 可见性过宽触发 `private_interfaces` warning；
  - 已验证 `cargo fmt --all`、`cargo test -p scoopc llvm::`、`cargo test --all`、`cargo clippy --all-targets -- -D warnings` 全部通过。

### [DONE] T5000b3aR Review：确认 `call/` 拆分形成稳定 lowering 边界
- 重点：
  - call dispatch、ABI 绑定、indirect call lowering 是否已离开 `codegen/mod.rs` 主体；
  - `class_ctor` / `closure` / `intrinsics` 与 `call/` 的交叉接口是否清晰，没有新的双向耦合；
  - ordinary callee resume / effect-call wrapper 的调用边界是否仍集中在 call 主题内。
- 验收：
  - 可以明确指出 `call/` 的职责上界，以及剩余待拆主题为何仍应留待后续子任务处理。
- 依赖：T5000b3a
- 完成记录（2026-04-25）：
  - 已核对 `crates/scoopc/src/llvm/codegen/call/dispatch.rs`、`call/abi.rs`、`call/resume.rs` 与 `crates/scoopc/src/llvm/codegen/mod.rs`，确认 `codegen_call_impl`、top-level/direct/virtual/interface/funptr/function-value call、call arg ABI、ordinary callee resume、top-level effect-call wrapper 等主体实现均已位于 `call/` 子模块；`codegen/mod.rs` 中对应入口现仅保留薄委托与少量共享 call 数据结构 / 命名 helper；
  - 已确认 `call/dispatch.rs` 对 `codegen_class_ctor_call`、`codegen_closure_expr` 以及各类 `codegen_sysroot_*` / `try_codegen_tostring_iface_builtin` helper 的依赖仍是单向委托，没有出现 class ctor / closure / intrinsics 主题反向承载 call dispatch / ABI 主体实现的新双向耦合；
  - 已确认 closure / effect 主题对 call 的交叉依赖集中在 `declare_*callee_resume_entry`、`codegen_callee_resume_entry_function` 与 `call_callee_resume_entry_from_state` 等 resume 入口，ordinary callee resume 与 top-level effect-call wrapper 的主体实现仍集中在 `call/resume.rs`；
  - review 结论：`call/` 的职责上界已可明确界定为调用分派、调用点 ABI / 实参绑定与 ordinary resume / wrapper lowering；剩余 builtin/sysroot、closure/class ctor、enum/object 主题仍主要留在 `codegen/mod.rs`，继续按 `T5000b3b`～`T5000b3d` 顺序拆分即可，无需先插入新的前置缺陷任务；
  - 已验证 `cargo test -p scoopc llvm::`、`cargo test --all`、`cargo clippy --all-targets -- -D warnings` 全部通过。

### [TODO] T5000b3b 拆出 `intrinsics/` lowering 模块
- 范围：
  - 将 `llvm/codegen/mod.rs` 中的 string / char / int / float builtin lowering，以及 sysroot I/O、env、time、fs、process、path、sync、thread、channels、array、task transport、atomic int 等 lowering 迁入 `llvm/codegen/intrinsics/`；
  - 保持现有 builtin/sysroot 语义与错误边界不变，只按主题收口文件边界。
- 验收：
  - `codegen/mod.rs` 不再直接承载 builtin/sysroot intrinsics 主体实现；
  - `intrinsics/` 内部边界能区分标量内建、sysroot API、并发/容器 intrinsics，而不是新的混杂入口。
- 依赖：T5000b3aR

### [TODO] T5000b3bR Review：确认 `intrinsics/` 拆分没有把 builtin/sysroot 继续堆回根模块
- 重点：
  - builtin 与 sysroot lowering 是否已稳定离开 `codegen/mod.rs`；
  - intrinsics 主题内部是否已有按语义分组的边界，而不是机械移动；
  - 与 `call/`、`runtime_abi`、`gc` 的交互是否仍保持单向清晰。
- 验收：
  - 可以明确说出 `intrinsics/` 的职责上界，以及尚未迁出的共享 helper 属于什么后续主题。
- 依赖：T5000b3b

### [TODO] T5000b3c 拆出 `closure/` 与 `class_ctor.rs` lowering 模块
- 范围：
  - 将 closure expr / env / body lowering、closure suspend-plan 相关 helper 迁入 `llvm/codegen/closure/`；
  - 将 class ctor 选择、实参求值、super 调用、init-step 执行与 invoke lowering 迁入 `llvm/codegen/class_ctor.rs`；
  - 保持现有行为不变，只整理主题边界。
- 验收：
  - `codegen/mod.rs` 不再直接承载 closure 与 class ctor lowering 主体实现；
  - closure 与 class ctor 各自形成稳定主题边界，而不是继续依赖根模块中的大段邻接 helper。
- 依赖：T5000b3bR

### [TODO] T5000b3cR Review：确认 `closure/` 与 `class_ctor.rs` 主题边界成立
- 重点：
  - closure env / body lowering 是否已经从 call 与 object/enum 主题中分离；
  - class ctor 相关路径是否已集中，不再散落在根模块不同位置；
  - 两者与 `call/`、`intrinsics/` 的接口是否足够窄。
- 验收：
  - 可以明确指出 closure 与 class ctor lowering 的职责上界，以及仍待抽离的剩余主题。
- 依赖：T5000b3c

### [TODO] T5000b3d 拆出 `enum_lowering.rs` 与 `object_init.rs` lowering 模块
- 范围：
  - 将 enum variant ctor、payload coercion、enum 常量构造与 qualified unit variant lowering 迁入 `llvm/codegen/enum_lowering.rs`；
  - 将 object property / singleton access、object init body 与相关 global helper 迁入 `llvm/codegen/object_init.rs`；
  - 完成后收口 `codegen/mod.rs` 的主题边界，只保留共享上下文、通用字面量/运算/类型转换等尚未进一步抽象的实现。
- 验收：
  - `codegen/mod.rs` 不再继续承载 object/enum lowering 主体实现；
  - 根模块剩余内容能明显收敛为共享上下文与尚未独立成主题的通用 helper，而不是继续混放 call / intrinsics / closure / object / enum 主体。
- 依赖：T5000b3cR

### [TODO] T5000b3dR Review：确认 `codegen/mod.rs` 的主题拆分已收口到共享上下文与通用 helper
- 重点：
  - enum / object lowering 是否已脱离根模块主体；
  - 根模块剩余内容是否确实以共享上下文、通用 helper、跨主题桥接为主；
  - 是否还残留明显应继续优先迁出的稳定主题簇。
- 验收：
  - 可以明确说出 `codegen/mod.rs` 当前剩余职责上界，并证明主题拆分不是机械切碎。
- 依赖：T5000b3d

### [TODO] T5000b3R Review：确认 `llvm/codegen/mod.rs` 的主题拆分是真正的边界整理
- 重点：
  - 是否只是把大文件切碎，还是确实按 lowering 主题形成了清晰边界；
  - 是否仍有明显的跨主题 helper 继续倒灌回 `codegen/mod.rs`；
  - 后续 `MainCodegen` 分层是否已有更清晰的落点。
- 验收：
  - 可以明确说出每个主题模块的职责上界，以及哪些共享逻辑仍待进一步抽象。
- 依赖：T5000b3dR

### [TODO] T5000b4 继续拆分 `MainCodegen` 为 module / function / cache / effect emitter 上下文
- 范围：
  - 在前两步构造路径与主题模块拆分基础上，进一步把 `MainCodegen` 至少区分为：
    - module 级上下文；
    - function/body 级上下文；
    - shared analysis/layout cache；
    - effect/state-machine emitter 专用上下文。
  - 保证 backend-agnostic 分析不再继续直接挂回 `llvm/codegen/` 主上下文。
- 验收：
  - `MainCodegen` 不再同时承担 module / function / cache / effect emitter 四类职责；
  - effect/state-machine emitter 拥有清晰专用上下文，而不是继续依附整个 `MainCodegen`。
- 依赖：T5000b3R

### [TODO] T5000b4R Review：确认 `MainCodegen` 的上下文分层已经成立
- 重点：
  - module / function / cache / effect emitter 四类职责是否已有稳定边界；
  - 是否仍有明显的中端分析继续依附在 backend 主上下文上；
  - 这一步是否已经为 `ProgramFacts` 抽离提供清楚入口。
- 验收：
  - 可以明确指出“哪些部分仍属于 backend”“哪些共享事实下一步应迁到 backend 之外”。
- 依赖：T5000b4

### [TODO] T5000bR Review：确认 LLVM codegen 已收口到“只做 backend lowering”的方向
- 重点：
  - 本轮拆分是否只是“把大文件拆成更多大文件”，还是确实拉直了 backend 边界；
  - 是否仍有明显的中端分析继续依附在 `MainCodegen` 上；
  - 拆分后是否为后续 `ProgramFacts` 抽离和 MIR 迁移留出了清晰入口。
- 验收：
  - 可以明确指出“哪些部分仍属于 backend”“哪些部分下一步要迁出”，且边界比改动前更清楚。
- 依赖：T5000b4R

### [TODO] T5000c 抽离 backend-agnostic 的 `ProgramFacts` / `EffectAnalysisCtx` / shared side tables
- 范围：
  - 把当前已经像中端分析的共享事实从 LLVM codegen 依赖中解耦出来，至少覆盖：
    - callee target resolution 所需事实；
    - receiver exactness / target-set shrinking 所需事实；
    - type / field / nominal specialization 相关事实；
    - higher-order provenance 与 suspendability summary 输入；
    - effect/state-machine planning 所需事实。
  - 替换当前“分析从 `MainCodegen` 反取信息”的模式，使 LLVM codegen 和 future MIR pass 依赖同一份 backend-agnostic facts / side tables。
- 验收：
  - `HandlePlanContext::from_codegen` 这类由 backend 上下文直接喂给中端分析的路径开始消失；
  - monomorphized callee resolution、concrete type 恢复、receiver exactness 等所需数据不再必须从 LLVM codegen 现场拼装。
- 依赖：T5000bR

### [TODO] T5000cR Review：确认共享事实层已经脱离 LLVM backend 依赖方向
- 重点：
  - `ProgramFacts` / `EffectAnalysisCtx` 是否真的 backend-agnostic；
  - 是否还残留“必须通过 `MainCodegen` 才能做分析”的强耦合路径；
  - 该层是否已经足够支撑 MIR、summary 与 effect planning 的共同消费。
- 验收：
  - 后续 MIR 任务可以在不依赖 LLVM builder / module / GC ABI 细节的前提下推进。
- 依赖：T5000c

### [TODO] T5000d 扩展现有 MIR，形成最小 generic early MIR / ANF template
- 范围：
  - 在现有 MIR 上引入最小但稳定的中端承载能力：
    - 显式 call kinds：`DirectCall` / `VirtualCall` / `InterfaceCall` / `ClosureCall` / `FunValueCall`
    - 显式 `Perform` / `Resume`
    - 更稳定的 control-flow / local-binding / provenance 形状
    - 必要的 concrete type / dispatch / receiver metadata
  - 为后续 `when` / pattern lowering、operator-overload target materialization 等提供正规化入口。
  - 本阶段产物仍允许存在 type params，因此它是 generic template，而不是最终优化输入。
- 验收：
  - MIR 能稳定表达后续优化需要观察的调用形态与控制转移；
  - 后续 pass 不必再通过 HIR 语法形状或 LLVM codegen 现场推断来恢复这些信息。
- 依赖：T5000cR

### [TODO] T5000dR Review：确认 generic early MIR / ANF template 的语义边界正确
- 重点：
  - MIR 是否仍保持 backend-agnostic；
  - 是否只表达“语言/运行时抽象事实”，没有提前混入 LLVM 落地细节；
  - generic template 与后续 monomorphic instance 的边界是否清楚。
- 验收：
  - 可以明确回答“这层 MIR 负责什么，不负责什么”，且它还没有越权承担 backend 细节。
- 依赖：T5000d

### [TODO] T5000e 在 MIR 层实现 monomorphization / instance materialization
- 范围：
  - 引入 `InstanceKey` 作为 backend-agnostic 的实例身份，而不是让 mangled symbol name 承担语义身份。
  - 实现：
    - reachable-driven instance collection；
    - on-demand monomorphization；
    - per-`InstanceKey` cache；
    - generic template 到 monomorphic MIR instance 的稳定映射。
  - 消除当前 LLVM codegen 中对 monomorphized target 的主要解析职责。
- 验收：
  - devirt / inline / effect planning 消费的是 monomorphic MIR instances，而不是 generic template；
  - codegen 不再以“现场根据 mangled FQN 重定向目标”为主路径来承担 monomorphization。
- 依赖：T5000dR

### [TODO] T5000eR Review：确认 monomorphization 已成为 MIR 内部独立阶段
- 重点：
  - `InstanceKey` 是否真正独立于 backend 符号名；
  - 是否仍有大量单态化职责遗留在 LLVM codegen；
  - 实例收集 / 缓存策略是否已经考虑 `-O0` / debug build 成本。
- 验收：
  - monomorphization 的主语义与主数据结构已经明确属于 MIR，而不是 HIR 或 LLVM codegen。
- 依赖：T5000e

### [TODO] T5000f 建立 per-instance summary 基础设施
- 范围：
  - 对每个单态实例建立最小 summary，至少覆盖：
    - `body_known`
    - `size_cost`
    - `recursive_scc`
    - `may_outward_effect`
    - `may_allocate_closure`
    - `param_use_summaries`
    - `result_provenance`
  - summary 必须成为 MIR 或 stable side tables 的一等产物，而不是 codegen 查询时临时重建。
- 验收：
  - 后续 devirt / inline / escape analysis 都能共享同一套 per-instance summary；
  - higher-order function value 的 `DirectCallOnly` / `Escapes` 等判断不再由 codegen 现场重复现算。
- 依赖：T5000eR

### [TODO] T5000fR Review：确认 summary 已按单态实例而不是按函数名工作
- 重点：
  - summary 是否真正挂在 monomorphic instance 上；
  - 是否还残留“按函数名一份 summary，再在 codegen 现场补类型”的做法；
  - summary 计算与缓存是否已经具备后续多轮迭代的可扩展性。
- 验收：
  - summary 的层次归属与 identity 已足够稳定，可以直接喂给 devirt / inline。
- 依赖：T5000f

### [TODO] T5000g 在 MIR 层实现通用 devirtualization
- 范围：
  - 对所有 `VirtualCall` / `InterfaceCall` 统一做 receiver exactness / target-set shrinking；
  - 只要静态 target set 为 singleton，就改写为 `DirectCall`；
  - backend 只消费已分类完成的调用节点，不再负责主要去虚化判定。
- 验收：
  - 去虚化规则对“所有 receiver 类型已知且 target singleton 的 class/interface 调用”统一成立；
  - 不依赖 `Iterator.next()` / `Iterator.hasNext()` 等任何特定函数名。
- 依赖：T5000fR

### [TODO] T5000gR Review：确认 devirtualization 已经是结构驱动而不是热点特判
- 重点：
  - 规则是否对所有符合条件的调用统一生效；
  - 是否仍保留 backend 侧的目标猜测或名字特判；
  - 与后续 inline 的接口是否足够自然。
- 验收：
  - `InterfaceCall -> DirectCall`、`VirtualCall -> DirectCall` 已是 MIR 层统一改写，而不是 codegen 侧例外路径。
- 依赖：T5000g

### [TODO] T5000h 在 MIR 层实现 summary-driven inlining
- 范围：
  - 先做保守但通用的版本：
    - body-known
    - 非递归
    - 小体量
    - `DirectCallOnly` 参数
    - provenance 可知
  - 覆盖两类收益：
    - 普通小 direct call 的边界消除；
    - 高阶 wrapper 函数内对函数值参数的调用摊平。
  - 不允许按函数名白名单触发。
- 验收：
  - 对 `map` / `filter` / `forEach` 类形状的收益来自结构，而不是名字；
  - codegen 不再继续承担“内联后才能去掉的额外高层调用边界”。
- 依赖：T5000gR

### [TODO] T5000hR Review：确认 inlining 已走 summary / structure 路线
- 重点：
  - 是否仍有特定函数名 hard-code；
  - `DirectCallOnly` 与 provenance 是否真的在驱动高阶内联；
  - `@Inline` 是否仍只是 hint，而不是主机制。
- 验收：
  - 内联主路径已经是结构驱动；没有退回“为几个库函数特判”的方案。
- 依赖：T5000h

### [TODO] T5000i 加入 continuation / closure escaping analysis，并把 effect/state-machine planning 迁到正确边界
- 范围：
  - 在 MIR 层加入：
    - non-escaping closure simplification；
    - continuation escaping analysis。
  - 把 effect/state-machine 的 planning / segments / transform 迁出 LLVM codegen 语义边界，使其依赖 MIR 与 `ProgramFacts`。
  - backend 只保留 emitter 与必要的 backend lowering 合同。
- 验收：
  - effect/state-machine planning 不再以 `MainCodegen` 为主要输入上下文；
  - closure / continuation 是否逃逸成为 MIR 层稳定分析结果，而不是 codegen 现场推断。
- 依赖：T5000hR

### [TODO] T5000iR Review：确认 effect middle-end 已从 LLVM backend 语义边界迁出
- 重点：
  - `state_machine_plan / segments / transform` 是否已经脱离 LLVM codegen 主职责；
  - effect planning 是否真正依赖 MIR 与 shared facts，而不是依赖 backend context；
  - closure / continuation escape 分析是否与 summary / call kind / provenance 形成统一体系。
- 验收：
  - LLVM backend 只剩 emitter 与 backend lowering，而不再承担 effect middle-end 的主分析责任。
- 依赖：T5000i

### T5000j 扩展覆盖面，并继续跟踪 safepoint / `mem2reg` 方向
- 范围：
  - 在主线稳定后，继续扩展：
    - `when` / pattern lowering
    - operator-overload target materialization
    - 更多 higher-order / closure / object-init / top-level-init 场景
  - 持续跟踪：
    - 调用边界减少后 safepoint 数量与 roots 压力的变化；
    - 是否出现更适合继续研究 `mem2reg` / register-root 的窗口。
  - 本阶段仍不把 `mem2reg` 作为主交付目标。
- 验收：
  - 优化覆盖面继续沿结构事实扩展，而不是沿函数名特判扩展；
  - safepoint / root-pressure 的变化有可复验结论，可为后续 GC / `mem2reg` 研究提供真实输入。
- 依赖：T5000iR

### [TODO] T5000jR Review：确认优化主线已形成可持续扩展的中端体系
- 重点：
  - 后续扩展是否仍沿 MIR / summary / structure 方向推进；
  - 是否重新出现“把新分析长回 LLVM codegen”的回退；
  - 是否已经为未来 C / JVM / CLR backend 预留了稳定消费边界。
- 验收：
  - 本轮结束后，优化主线已明确从“LLVM codegen 现场推断”转向“backend-agnostic 中端 + backend lowering 分层”。
- 依赖：T5000j
