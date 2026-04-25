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

### [DONE] T5000b3b 拆出 `intrinsics/` lowering 模块
- 范围：
  - 将 `llvm/codegen/mod.rs` 中的 string / char / int / float builtin lowering，以及 sysroot I/O、env、time、fs、process、path、sync、thread、channels、array、task transport、atomic int 等 lowering 迁入 `llvm/codegen/intrinsics/`；
  - 保持现有 builtin/sysroot 语义与错误边界不变，只按主题收口文件边界。
- 验收：
  - `codegen/mod.rs` 不再直接承载 builtin/sysroot intrinsics 主体实现；
  - `intrinsics/` 内部边界能区分标量内建、sysroot API、并发/容器 intrinsics，而不是新的混杂入口。
- 依赖：T5000b3aR
- 完成记录（2026-04-25）：
  - 新增 `crates/scoopc/src/llvm/codegen/intrinsics/`，并按稳定主题拆成 `builtin.rs`、`sysroot.rs`、`sync.rs`、`thread.rs`、`channels.rs`、`containers.rs`、`atomic.rs` 7 个 lowering 子模块，由 `intrinsics/mod.rs` 统一声明；
  - `crates/scoopc/src/llvm/codegen/mod.rs` 已新增 `mod intrinsics;`，并删除原有 builtin/sysroot lowering 主体实现块；根模块中不再直接承载 `print`/`toString`/`sizeOf`、io/env/time/fs/process/path、sync/thread/channels、array/task transport/atomic int 等 intrinsics 主体实现；
  - `crates/scoopc/src/llvm/codegen/call/dispatch.rs` 继续只负责按 FQN 做调用分派，具体 lowering 主体现由 `intrinsics/` 子模块承接，从而维持 call 主题与 intrinsics 主题的单向依赖；
  - 迁移过程中暴露的唯一现存问题是 `crates/scoopc/src/llvm/codegen/mod.rs` 残留的 `inkwell::AtomicOrdering` 未使用导入，已在本轮一并修复；
  - 已验证 `cargo fmt --all`、`cargo test -p scoopc llvm::`、`cargo test --all`、`cargo clippy --all-targets -- -D warnings` 全部通过。

### [DONE] T5000b3bR Review：确认 `intrinsics/` 拆分没有把 builtin/sysroot 继续堆回根模块
- 重点：
  - builtin 与 sysroot lowering 是否已稳定离开 `codegen/mod.rs`；
  - intrinsics 主题内部是否已有按语义分组的边界，而不是机械移动；
  - 与 `call/`、`runtime_abi`、`gc` 的交互是否仍保持单向清晰。
- 验收：
  - 可以明确说出 `intrinsics/` 的职责上界，以及尚未迁出的共享 helper 属于什么后续主题。
- 依赖：T5000b3b
- 完成记录（2026-04-25）：
  - review 首先发现并修复了 `T5000b3b` 的既有边界泄漏：`crates/scoopc/src/llvm/codegen/mod.rs` 中仍残留 `codegen_string_trim_indent`、`codegen_string_method`、`codegen_to_string_method`、`expr_is_builtin_char`，以及 `Char` / `Int` / `Float` builtin member-call helper；这些实现现已整体迁入 `crates/scoopc/src/llvm/codegen/intrinsics/builtin.rs`，并按实际调用面收紧为 `pub(in crate::llvm::codegen)` 或模块私有 helper；
  - 复核后确认 `crates/scoopc/src/llvm/codegen/mod.rs` 已不再定义 string / char / int / float builtin lowering，也不再承载 io/env/time/fs/process/path、sync/thread/channels、array/task transport、atomic int 等 sysroot/intrinsics 主体实现；相应职责现稳定位于 `intrinsics/builtin.rs`、`sysroot.rs`、`sync.rs`、`thread.rs`、`channels.rs`、`containers.rs`、`atomic.rs`；
  - 已确认 `crates/scoopc/src/llvm/codegen/call/dispatch.rs` 仍只负责 FQN/member dispatch，并单向调用 `intrinsics/` 主题 helper；`intrinsics/` 继续单向消费 `runtime_abi` / `gc` / 通用 codegen helper，没有把 builtin/sysroot 主体逻辑倒灌回 `call/` 或 `codegen/mod.rs`；
  - review 结论：`intrinsics/` 的职责上界现可明确界定为 builtin 标量/字符串方法与顶层内建、sysroot API、并发/容器/原子 intrinsics lowering；`codegen/mod.rs` 中尚未迁出的相关共享 helper 仅剩非 intrinsics 主题内容，例如原子 lvalue 地址解析 `codegen_addressable_place`（更接近通用 lvalue / object 访问边界）以及 `lookup_pure_unit_closure_type`（供 `sync.Once.run` / `thread.spawn` 暂借的 closure 主题桥接），它们分别属于后续 object/lvalue 与 `T5000b3c` 的继续收口范围；
  - 已验证 `cargo fmt --all`、`cargo test -p scoopc llvm::`、`cargo test --all`、`cargo clippy --all-targets -- -D warnings` 全部通过。

### [DONE] T5000b3c 拆出 `closure/` 与 `class_ctor.rs` lowering 模块
- 范围：
  - 将 closure expr / env / body lowering、closure suspend-plan 相关 helper 迁入 `llvm/codegen/closure/`；
  - 将 class ctor 选择、实参求值、super 调用、init-step 执行与 invoke lowering 迁入 `llvm/codegen/class_ctor.rs`；
  - 保持现有行为不变，只整理主题边界。
- 验收：
  - `codegen/mod.rs` 不再直接承载 closure 与 class ctor lowering 主体实现；
  - closure 与 class ctor 各自形成稳定主题边界，而不是继续依赖根模块中的大段邻接 helper。
- 依赖：T5000b3bR
- 完成记录（2026-04-25）：
  - 新增 `crates/scoopc/src/llvm/codegen/closure/mod.rs` 与 `crates/scoopc/src/llvm/codegen/class_ctor.rs`，分别收口 closure expr / env / body lowering、closure suspend-plan / expected-function-type helper，以及 class ctor 选择、实参求值、super/delegation、init-step 与 invoke lowering；
  - `crates/scoopc/src/llvm/codegen/mod.rs` 现仅保留 `mod closure;` / `mod class_ctor;` 声明与共享上下文/通用 helper，不再直接定义 closure 或 class ctor lowering 主体实现；
  - `crates/scoopc/src/llvm/codegen/call/resume.rs` 已改为从 `closure/` 复用 `closure_callee_resume_entry_fn_name`，而 `expr.rs`、`effect/mod.rs`、`intrinsics/sync.rs`、`intrinsics/thread.rs`、`call/abi.rs`、`call/dispatch.rs` 等现有调用面继续只经 `MainCodegen` 的窄接口消费这两类主题；
  - 已验证 `cargo fmt --all`、`cargo test -p scoopc llvm::`、`cargo test --all`、`cargo clippy --all-targets -- -D warnings` 全部通过。

### [DONE] T5000b3cR Review：确认 `closure/` 与 `class_ctor.rs` 主题边界成立
- 重点：
  - closure env / body lowering 是否已经从 call 与 object/enum 主题中分离；
  - class ctor 相关路径是否已集中，不再散落在根模块不同位置；
  - 两者与 `call/`、`intrinsics/` 的接口是否足够窄。
- 验收：
  - 可以明确指出 closure 与 class ctor lowering 的职责上界，以及仍待抽离的剩余主题。
- 依赖：T5000b3c
- 完成记录（2026-04-25）：
  - 已复核 `crates/scoopc/src/llvm/codegen/closure/mod.rs`，确认 closure expr / env / body lowering、callee suspend-plan 与 expected-function-type helper 已集中到 `closure/`；`crates/scoopc/src/llvm/codegen/expr.rs`、`effect/mod.rs`、`call/abi.rs`、`intrinsics/{sync,thread}.rs` 仅通过 `codegen_closure_expr`、`lookup_pure_unit_closure_type` 等窄接口复用该主题，没有继续在调用侧承载 closure lowering 主体实现；
  - 已复核 `crates/scoopc/src/llvm/codegen/class_ctor.rs`，确认 ctor 选择、实参求值与默认值绑定、super/this delegation、init-step 执行与 invoke lowering 已集中在该模块；`crates/scoopc/src/llvm/codegen/call/dispatch.rs` 仅保留 unresolved ctor call 的分派入口，并通过 `ctor_call_sites` 单向委托到 `codegen_class_ctor_call`；
  - review 过程中发现并修复了一个既有文档问题：`crates/scoopc/src/llvm/codegen/class_ctor.rs` 顶部注释仍写着“不支持 named/default args”，现已改为准确描述 `CtorCallInfo` 驱动的 named/default arg 支持，以及无 side-table 的内部复用路径才退回 positional-only 的约束；
  - review 结论：closure lowering 的职责上界现可明确界定为 closure expr/env/body lowering、capture/env layout 与 callee suspend-plan helper；class ctor lowering 的职责上界现可明确界定为 ctor 选择、arg-eval/default binding、delegation、super/init/invoke。根模块及相邻主题中与两者相关的剩余桥接仅剩调用点委托、`call/dispatch.rs` 内的函数值调用桥接，以及 `gc.rs` 中 closure object/runtime layout helper；这些都不再承载 closure/class ctor lowering 主体实现。剩余待抽离的稳定主题已收敛为 `T5000b3d` 的 enum/object lowering 与后续 `T5000b4` 的 `MainCodegen` 上下文分层；
  - 已验证 `cargo fmt --all`、`cargo test -p scoopc llvm::`、`cargo test --all`、`cargo clippy --all-targets -- -D warnings` 全部通过。

### [DONE] T5000b3d 拆出 `enum_lowering.rs` 与 `object_init.rs` lowering 模块
- 范围：
  - 将 enum variant ctor、payload coercion、enum 常量构造与 qualified unit variant lowering 迁入 `llvm/codegen/enum_lowering.rs`；
  - 将 object property / singleton access、object init body 与相关 global helper 迁入 `llvm/codegen/object_init.rs`；
  - 完成后收口 `codegen/mod.rs` 的主题边界，只保留共享上下文、通用字面量/运算/类型转换等尚未进一步抽象的实现。
- 验收：
  - `codegen/mod.rs` 不再继续承载 object/enum lowering 主体实现；
  - 根模块剩余内容能明显收敛为共享上下文与尚未独立成主题的通用 helper，而不是继续混放 call / intrinsics / closure / object / enum 主体。
- 依赖：T5000b3cR
- 完成记录（2026-04-25）：
  - 新增 `crates/scoopc/src/llvm/codegen/enum_lowering.rs`，将 `codegen_unresolved_ident`、`codegen_enum_variant_ctor_call`、`build_enum_variant_value_from_field_values`、`coerce_enum_payload`、`build_enum_value` 与 `try_codegen_qualified_enum_unit_variant_value` 迁出根模块，统一收口 enum ctor / payload / enum 常量 lowering；
  - 新增 `crates/scoopc/src/llvm/codegen/object_init.rs`，将 `lookup_object_property_by_fqn`、`codegen_object_property_access`、`ensure_object_init_function_defined`、`codegen_object_init_fun_body`、`codegen_object_value_access` 以及 object once-guard / singleton global / property global helper 迁出根模块；
  - `crates/scoopc/src/llvm/codegen/mod.rs` 现仅新增 `mod enum_lowering;` 与 `mod object_init;` 声明，并删除 object/enum lowering 主体实现块；根模块不再直接定义上述函数；
  - 迁移过程中发现 `object_init.rs` 需要显式从 `crate::llvm` 导入 `LLVM_GC_STRATEGY_STATEPOINT_EXAMPLE`，已在本轮一并修复，未留下新的前置缺陷任务；
  - 已验证 `cargo fmt --all`、`cargo test -p scoopc llvm::`、`cargo test --all`、`cargo clippy --all-targets -- -D warnings` 全部通过。

### [DONE] T5000b3dR Review：确认 `codegen/mod.rs` 的主题拆分已收口到共享上下文与通用 helper
- 重点：
  - enum / object lowering 是否已脱离根模块主体；
  - 根模块剩余内容是否确实以共享上下文、通用 helper、跨主题桥接为主；
  - 是否还残留明显应继续优先迁出的稳定主题簇。
- 验收：
  - 可以明确说出 `codegen/mod.rs` 当前剩余职责上界，并证明主题拆分不是机械切碎。
- 依赖：T5000b3d
- 完成记录（2026-04-25）：
  - review 首先发现并修复了一个既有边界泄漏：`crates/scoopc/src/llvm/codegen/mod.rs` 中仍残留 `codegen_sysroot_funptr_invoke`、`codegen_sysroot_funptr_to_uintptr`、`codegen_sysroot_uintptr_to_funptr` 三个 `scoop.unsafe.*` intrinsic lowering；这些实现现已整体迁入 `crates/scoopc/src/llvm/codegen/intrinsics/sysroot.rs`，`call/dispatch.rs` 继续只做 FQN 分派，具体 lowering 主体不再留在根模块；
  - 复核后确认 enum / object lowering 主体已经稳定落在 `crates/scoopc/src/llvm/codegen/enum_lowering.rs` 与 `crates/scoopc/src/llvm/codegen/object_init.rs`；`expr.rs`、`call/dispatch.rs`、`effect/mod.rs` 以及 `codegen/mod.rs` 中的顶层值 / 成员访问路径只通过 `codegen_unresolved_ident`、`codegen_enum_variant_ctor_call`、`try_codegen_qualified_enum_unit_variant_value`、`codegen_object_value_access`、`codegen_object_property_access` 等窄接口消费这些主题；
  - 已确认 `crates/scoopc/src/llvm/codegen/mod.rs` 当前剩余主体职责已收敛为共享上下文与通用 helper：`CompilationUnitCodegenCx` / `MainCodegen` 状态、顶层 const/immutable/var 访问与初始化、GC-sensitive spill/root/sret/return 上下文、`codegen_addressable_place` 这类通用 lvalue bridge、单态化/具体类型恢复 helper，以及字面量/聚合值/成员访问/运算符/类型转换/通用 coercion lowering；未再发现应立即优先抽离的 enum/object/sysroot/call/closure/class-ctor 主题主体实现；
  - review 结论：`codegen/mod.rs` 的主题拆分已收口到共享上下文、通用 helper 与跨主题桥接，下一条可直接进入 `T5000b3R` 汇总 review；
  - 已验证 `cargo fmt --all`、`cargo test -p scoopc llvm::`、`cargo test --all`、`cargo clippy --all-targets -- -D warnings` 全部通过。

### [DONE] T5000b3R Review：确认 `llvm/codegen/mod.rs` 的主题拆分是真正的边界整理
- 重点：
  - 是否只是把大文件切碎，还是确实按 lowering 主题形成了清晰边界；
  - 是否仍有明显的跨主题 helper 继续倒灌回 `codegen/mod.rs`；
  - 后续 `MainCodegen` 分层是否已有更清晰的落点。
- 验收：
  - 可以明确说出每个主题模块的职责上界，以及哪些共享逻辑仍待进一步抽象。
- 依赖：T5000b3dR
- 完成记录（2026-04-25）：
  - 已复核 `crates/scoopc/src/llvm/codegen/call/{dispatch,abi,resume}.rs`、`intrinsics/{builtin,sysroot,sync,thread,channels,containers,atomic}.rs`、`closure/mod.rs`、`class_ctor.rs`、`enum_lowering.rs`、`object_init.rs`，确认调用分派/ABI/resume、builtin/sysroot intrinsics、closure/class ctor、enum/object lowering 的主体入口均位于各自主题模块，而不是继续留在 `codegen/mod.rs`；
  - 已复核 `crates/scoopc/src/llvm/codegen/mod.rs` 中残留的少量同名入口，确认 `codegen_call`、`codegen_callee_resume_dispatch`、`codegen_callee_resume_entry_function`、`codegen_bound_call_args`、`codegen_callable_value_args`、`codegen_function_value_call*` 等仅为薄委托；`codegen_member_access` 则承担表达式层统一分派与通用 struct/tuple/class-field 访问桥接，不再承载 object/enum/class-ctor/intrinsics 主题主体实现；
  - 已确认仍留在根模块中的跨主题共享逻辑主要是 `CompilationUnitCodegenCx` / `MainCodegen` 状态、顶层 const/immutable/var 初始化与访问、GC-sensitive spill/root/sret/return helper、`codegen_addressable_place` 这类通用 lvalue bridge，以及具体类型恢复/通用 coercion 等 generic lowering；其中 `codegen_addressable_place` 当前仅被 `intrinsics/atomic.rs` 复用，但边界上更接近通用可寻址 place 抽象，而不是 atomic lowering 主体；
  - review 过程中发现并修复了一个既有文档错配：`crates/scoopc/src/llvm/codegen/mod.rs` 顶部注释仍描述早期“最小子集 / 不支持 if/loop”等旧口径，现已改为准确描述根模块的共享上下文 / generic lowering 边界，以及各主题子模块的职责分布；
  - review 结论：本轮拆分不是机械切碎，`llvm/codegen/mod.rs` 已真实收口到共享上下文、generic lowering 与跨主题桥接；后续 `T5000b4` 的明确落点已经收敛为继续拆分 `CompilationUnitCodegenCx` / `MainCodegen` 的 module / function / cache / effect emitter 职责边界；
  - 已验证 `cargo fmt --all`、`cargo test -p scoopc llvm::`、`cargo test --all`、`cargo clippy --all-targets -- -D warnings` 全部通过。

### T5000b4 继续拆分 `MainCodegen` 为 module / function / cache / effect emitter 上下文
- 说明：
  - 经核对，`T5000b4` 当前同时包含 shared cache 收口、function/body 级状态拆分、effect/state-machine emitter 专用上下文抽离三类改动，单轮过大；
  - 现按稳定职责拆成以下子任务，保持语义不变，先整理上下文边界，再进入 `ProgramFacts` 抽离。

### [DONE] T5000b4a 抽出编译单元级共享 layout / suspend-analysis cache
- 范围：
  - 将当前仍挂在 `MainCodegen` 或 `CompilationUnitCodegenCx` 零散字段上的共享 cache 收口为明确的编译单元级 cache 上下文，至少覆盖：
    - `known_fun_call_suspend_cache`
    - `type_layout_cache`
    - `option_niche_cache`
    - `enum_cg_layout_cache`
    - `class_init_layout_cache`
    - `pack_field_indices`
  - 更新 `layout.rs`、`ty.rs`、`effect/state_machine_plan.rs` 与相关调用面，确保顶层函数 / child-codegen / nested lowering 复用同一套共享 cache，而不是继续把 cache 作为 `MainCodegen` 的函数级字段。
- 验收：
  - `MainCodegen` 不再直接持有上述 layout / analysis cache；
  - `CompilationUnitCodegenCx` 拥有清晰的共享 cache 边界，`fresh_main_codegen()` / `fresh_child_codegen()` 进入的新实例会稳定复用同一套缓存状态；
  - 不改变当前 lowering 语义与诊断边界。
- 依赖：T5000b3R
- 完成记录（2026-04-25）：
  - 在 `crates/scoopc/src/llvm/codegen/mod.rs` 中新增 `SharedCodegenCaches`，将 `known_fun_call_suspend_cache`、`type_layout_cache`、`option_niche_cache`、`enum_cg_layout_cache`、`class_init_layout_cache` 与 `pack_field_indices` 收口为 `CompilationUnitCodegenCx` 持有的编译单元级共享 cache；
  - `MainCodegen` 已删除上述 layout / analysis cache 字段，`fresh_main_codegen()` / `fresh_child_codegen()` 进入的新实例不再重建这些缓存，而是统一复用编译单元级 `shared_caches`；
  - `crates/scoopc/src/llvm/codegen/layout.rs`、`ty.rs`、`effect/state_machine_plan.rs` 与 `codegen/mod.rs` 的相关调用面均已改为经由共享 cache 访问；其中 `cg_enum_layout(...)` 现返回从共享 cache 克隆出的 `CgEnumLayout`，避免 `RefCell` 借用穿透到后续 lowering；
  - 已同步更新 `enum_lowering.rs`、`control_flow.rs`、`ty.rs` 中围绕 enum layout 的注释口径，确保文档与新的共享 cache 行为一致；
  - 已验证 `cargo fmt --all`、`cargo test -p scoopc llvm::`、`cargo test --all`、`cargo clippy --all-targets -- -D warnings` 全部通过。

### [DONE] T5000b4aR Review：确认共享 cache 已脱离 `MainCodegen` 的函数级状态
- 重点：
  - layout / suspend-analysis cache 是否都已收口到编译单元级共享上下文；
  - 是否还残留“每个 `MainCodegen` 各自维护一份 cache”的路径；
  - 这一步是否降低了后续 function / effect emitter 上下文拆分的耦合面。
- 验收：
  - 后续 `T5000b4b` / `T5000b4c` 可以不再同时搬运 layout / analysis cache 的字段。
- 依赖：T5000b4a
- 完成记录（2026-04-25）：
  - 已复核 `crates/scoopc/src/llvm/codegen/mod.rs`，确认 `SharedCodegenCaches` 现由 `CompilationUnitCodegenCx` 持有，`MainCodegen` 不再直接定义 `known_fun_call_suspend_cache`、`type_layout_cache`、`option_niche_cache`、`enum_cg_layout_cache`、`class_init_layout_cache`、`pack_field_indices` 六类 cache 字段；
  - 已复核 `crates/scoopc/src/llvm/emit.rs`，确认实现代码中仍只有一个 `CompilationUnitCodegenCx::new(...)` 构造入口；顶层声明、reachable top-level function body 发射与入口 `main` lowering 统一经 `fresh_main_codegen()` 进入，而 effect-call wrapper、closure body、object init 等 nested lowering 统一经 `fresh_child_codegen()` 进入，都会复用同一编译单元级 `shared_caches`；
  - 已复核 `crates/scoopc/src/llvm/codegen/layout.rs`、`ty.rs` 与 `effect/state_machine_plan.rs`，确认 layout / suspend-analysis 相关 cache 访问现全部通过 `self.shared_caches` 读写，没有残留“每个 `MainCodegen` 自带一份 cache 容器”的路径；其中 `cg_enum_layout(...)` 继续返回从共享 cache 克隆出的 layout，`packed-field` 索引回填也稳定写回共享 cache，避免把 `RefCell` 借用或缓存所有权继续泄漏到后续 lowering；
  - review 结论：后续 `T5000b4b` / `T5000b4c` 可以只聚焦 function/body 状态与 effect emitter 专属状态的收口，不再需要同时搬运 layout / analysis cache 字段；未发现需要插入到 `T5000b4b` 之前的新前置缺陷任务；
  - 已验证 `cargo test -p scoopc llvm::`、`cargo test --all`、`cargo clippy --all-targets -- -D warnings` 全部通过。

### [DONE] T5000b4b 拆出 `MainCodegen` 的 function/body 级上下文
- 范围：
  - 在共享 cache 脱离后，继续把 `MainCodegen` 内部明显属于单个函数 / body lowering 生命周期的状态收口到独立上下文，至少覆盖：
    - `env`
    - `extra_gc_root_slots` / `next_extra_gc_root_slot_id`
    - `loop_context_stack`
    - `return_context`
    - `current_fun_return_ty`
    - `current_sret_return_ptr`
    - `top_level_const_eval_stack`
  - 让通用 lowering helper 消费明确的 function/body 上下文，而不是继续直接读写 `MainCodegen` 上的混合字段。
- 验收：
  - `MainCodegen` 不再混放 module 级输入与 function/body 级运行时状态；
  - child-codegen / nested lowering 在保存与重建函数级状态时边界更明确。
- 依赖：T5000b4aR
- 完成记录（2026-04-25）：
  - 已在 `crates/scoopc/src/llvm/codegen/mod.rs` 中新增 `FunctionBodyCodegenCx`，统一收口 `env`、`extra_gc_root_slots` / `next_extra_gc_root_slot_id`、`loop_context_stack`、`return_context`、`current_fun_return_ty`、`current_sret_return_ptr` 与 `top_level_const_eval_stack` 七类函数 / body 生命周期状态；`MainCodegen` 当前改为持有独立 `function_cx`，不再把这些字段与编译单元级共享输入直接混放；
  - 已新增 `take_function_body_cx()` / `restore_function_body_cx()`，并把 `crates/scoopc/src/llvm/codegen/call/resume.rs` 中的 callee resume entry 生成路径，以及 `crates/scoopc/src/llvm/codegen/effect/state_machine_emitter.rs` 中 step/dispatch runtime function 发射入口，改为按整组 `function_cx` 保存/恢复函数级状态，而不是继续手动搬运 `env + loop + return` 多个字段；
  - `stmt.rs`、`control_flow.rs`、`gc.rs`、`class_ctor.rs`、`closure/mod.rs`、`object_init.rs`、`call/abi.rs`、`call/dispatch.rs`、`effect/mod.rs`、`effect/state_machine_plan.rs`、`effect/state_machine_emitter.rs`、`intrinsics/containers.rs`、`intrinsics/sysroot.rs` 与 `codegen/mod.rs` 的相关 helper / lowering 路径，现均已经由 `self.function_cx` 访问函数级状态，从而把通用 lowering 对函数 / body 上下文的依赖显式化；
  - 这一步保持了现有 lowering 语义与诊断边界不变，同时让后续 `T5000b4c` 可以只继续处理 effect emitter 专属状态，而无需再夹带普通函数级状态容器的拆分；
  - 已验证 `cargo fmt --all`、`cargo test -p scoopc llvm::`、`cargo test --all`、`cargo clippy --all-targets -- -D warnings` 全部通过。

### [DONE] T5000b4bR Review：确认 function/body 级上下文边界成立
- 重点：
  - function/body 生命周期字段是否已集中；
  - 是否仍有明显应属于函数级状态的字段残留在外层主上下文；
  - 这一步是否让 effect emitter 上下文的保存/恢复面继续收窄。
- 验收：
  - effect emitter 后续只需处理真正的 effect 专属状态，而不是再夹带普通函数级状态。
- 依赖：T5000b4b
- 完成记录（2026-04-25）：
  - 已复核 `crates/scoopc/src/llvm/codegen/mod.rs`，确认 `MainCodegen` 当前仅保留 `current_source_id`、`function_cx` 与 effect 专属状态；`env`、`extra_gc_root_slots` / `next_extra_gc_root_slot_id`、`loop_context_stack`、`return_context`、`current_fun_return_ty`、`current_sret_return_ptr` 与 `top_level_const_eval_stack` 已全部集中到 `FunctionBodyCodegenCx`，且未再发现 `self.env` / `self.return_context` / `self.current_fun_return_ty` 一类旧访问残留；
  - 已复核 `crates/scoopc/src/llvm/codegen/call/resume.rs` 与 `crates/scoopc/src/llvm/codegen/effect/state_machine_emitter.rs`，确认跨函数 / 跨 runtime-function 的保存恢复入口现已收敛为 `take_function_body_cx()` / `restore_function_body_cx()`；effect emitter 中剩余对 `return_context` / `current_fun_return_ty` 的若干保存恢复，仅用于同一 runtime function 内的局部语义覆写，不再是成片手工搬运普通函数级上下文；
  - 已确认 `effect_function_return_context`、`current_callee_suspend_plan`、`current_callee_resume_entry_fn`、`current_continuation_resume_replay`、`current_continuation_resume_replay_context`、`active_suspend_site_effect_outcome_capture` 与 `suspend_site_explicit_effect_outcomes` 继续准确对应下一条 `T5000b4c` 的 effect emitter 专属状态范围；`current_source_id` 则仍属 generic lowering / 诊断上下文，当前没有证据表明它是遗漏的函数局部运行态；
  - review 结论：function/body 生命周期边界已成立，effect emitter 后续可以只聚焦 effect 专属状态收口；未发现需要插入到 `T5000b4c` 之前的新前置缺陷任务；
  - 已验证 `cargo test -p scoopc llvm::`、`cargo test --all`、`cargo clippy --all-targets -- -D warnings` 全部通过。

### [DONE] T5000b4c 抽出 effect/state-machine emitter 专用上下文
- 范围：
  - 将 `effect/mod.rs` 与 `effect/state_machine_emitter.rs` 当前依赖的 effect 专属 lowering 状态收口为独立上下文，至少覆盖：
    - `effect_function_return_context`
    - `current_callee_suspend_plan`
    - `current_callee_resume_entry_fn`
    - `current_continuation_resume_replay`
    - `current_continuation_resume_replay_context`
    - `active_suspend_site_effect_outcome_capture`
    - `suspend_site_explicit_effect_outcomes`
  - 消除 state-machine emitter 中成片的“手动保存/恢复一串 `MainCodegen` 字段”模式。
- 验收：
  - effect/state-machine emitter 拥有清晰专用上下文；
  - `MainCodegen` 不再直接承载 effect emitter 的主要运行态。
- 依赖：T5000b4bR
- 完成记录（2026-04-25）：
  - 在 `crates/scoopc/src/llvm/codegen/mod.rs` 中新增 `EffectLoweringCodegenCx`，并按职责继续细分为 `CalleeSuspendLoweringCodegenCx`、`ContinuationResumeReplayCodegenCx` 与 `SuspendSiteEffectOutcomeCodegenCx`；原先平铺在 `MainCodegen` 上的 `effect_function_return_context`、`current_callee_suspend_plan`、`current_callee_resume_entry_fn`、`current_continuation_resume_replay`、`current_continuation_resume_replay_context`、`active_suspend_site_effect_outcome_capture` 与 `suspend_site_explicit_effect_outcomes` 已全部收口到该专用上下文；
  - `crates/scoopc/src/llvm/codegen/effect/state_machine_emitter.rs` 的 step / dispatch runtime function 发射入口现已改为整组 `take_effect_lowering_cx()` / `restore_effect_lowering_cx()` 交换 effect 专属状态；`step` / `dispatch` return bridge 改经 `with_effect_function_return_context(...)` 安装，`SuspendCall` / object-init / runtime-raise boundary 与 ordinary callee fresh path 改经 `with_active_suspend_site_effect_outcome_capture(...)` 覆写局部 outcome capture，continuation replay 路径改经 `with_continuation_resume_replay(...)` 安装 replay token + payload 绑定，不再在 emitter 中成片手工保存/恢复多组 effect 字段；
  - `crates/scoopc/src/llvm/codegen/mod.rs` 的顶层函数 body lowering、`crates/scoopc/src/llvm/codegen/closure/mod.rs` 的 closure body lowering 与 `crates/scoopc/src/llvm/codegen/call/resume.rs` 的 callee resume dispatch 已统一改经 `with_callee_suspend_lowering(...)` 临时安装 ordinary callee suspend/resume lowering 状态；`crates/scoopc/src/llvm/codegen/effect/mod.rs` 及 `state_machine_emitter.rs` 的 effect 状态访问也已统一改成新的上下文 getter / helper；
  - 已验证 `cargo fmt --all`、`cargo test -p scoopc llvm::`、`cargo test --all`、`cargo clippy --all-targets -- -D warnings` 全部通过。

### [DONE] T5000b4cR Review：确认 effect/state-machine emitter 上下文边界成立
- 重点：
  - effect emitter 专属状态是否已集中；
  - state-machine emitter 是否仍大量直接操作外层主上下文；
  - 这一步是否已经为 `T5000b4R` 的 backend 边界 review 留出清晰结论。
- 验收：
  - 可以明确指出哪些状态仍属于 backend 的 generic lowering，哪些已是 effect emitter 自己的上下文。
- 依赖：T5000b4c
- 完成记录（2026-04-25）：
  - 已复核 `crates/scoopc/src/llvm/codegen/mod.rs` 中的 `EffectLoweringCodegenCx`、`CalleeSuspendLoweringCodegenCx`、`ContinuationResumeReplayCodegenCx` 与 `SuspendSiteEffectOutcomeCodegenCx`，确认 `effect_function_return_context`、ordinary callee suspend/resume、continuation replay 与 suspend-site explicit outcome 捕获状态均已集中到 effect 专用上下文；
  - 已复核 `crates/scoopc/src/llvm/codegen/effect/state_machine_emitter.rs`、`effect/mod.rs`、`call/resume.rs` 与 `closure/mod.rs`，确认 effect 专属状态的消费面现统一经 `take_effect_lowering_cx()` / `restore_effect_lowering_cx()`、getter 与 `with_*` helper 进入；除 `mod.rs` 外未再发现直接操作 `effect_cx` 内部字段的路径；
  - review 过程中发现并修复了一个既有边界泄漏：`state_machine_emitter.rs` 中仍残留 4 处手工保存/恢复 `function_cx.return_context` 与 `current_fun_return_ty = Never` 的模式，而且中间夹着会通过 `?` 提前返回的调用；现已在 `crates/scoopc/src/llvm/codegen/mod.rs` 中新增 `with_local_never_return_semantics(...)`，并将 ordinary callee replay、`Continuation.resume(...)` replay 与 handler arm body 等路径统一改经该 helper，确保无论成功还是失败都恢复函数级返回语义；
  - review 结论：effect emitter 自己的上下文现可明确界定为 `function_return_context`、callee suspend/resume lowering、continuation replay 与 suspend-site outcome 捕获；`current_source_id` 与 `FunctionBodyCodegenCx` 中的 env/loop/return/return-ty 等状态继续属于 backend 的 generic lowering / function-body 上下文，其中 `state_machine_emitter` 剩余的函数级返回语义覆写也已收口为统一 helper，而不再是 effect 专属字段成片手工保存/恢复；
  - 已验证 `cargo fmt --all`、`cargo test -p scoopc llvm::`、`cargo test --all`、`cargo clippy --all-targets -- -D warnings` 全部通过。

### [DONE] T5000b4R Review：确认 `MainCodegen` 的上下文分层已经成立
- 重点：
  - module / function / cache / effect emitter 四类职责是否已有稳定边界；
  - 是否仍有明显的中端分析继续依附在 backend 主上下文上；
  - 这一步是否已经为 `ProgramFacts` 抽离提供清楚入口。
- 验收：
  - 可以明确指出“哪些部分仍属于 backend”“哪些共享事实下一步应迁到 backend 之外”。
- 依赖：T5000b4cR
- 完成记录（2026-04-25）：
  - 已复核 `crates/scoopc/src/llvm/codegen/mod.rs` 中的 `CompilationUnitCodegenCx`、`SharedCodegenCaches`、`FunctionBodyCodegenCx`、`EffectLoweringCodegenCx` 与 `MainCodegen::{fresh_main_codegen,fresh_child_codegen}`，确认编译单元级只读输入/共享 cache、函数体生命周期状态与 effect emitter 专属运行态现已形成稳定分层：共享 cache 不再挂在 `MainCodegen` 上随 child-codegen 重建，effect 专属状态也不再平铺在主上下文字段里；
  - 已复核 `crates/scoopc/src/llvm/codegen/effect/state_machine_emitter.rs`、`call/resume.rs`、`closure/mod.rs` 与 `object_init.rs` 的 runtime-function / child-codegen 入口，确认 step/dispatch runtime function 通过 `take_function_body_cx()` / `restore_function_body_cx()` 与 `take_effect_lowering_cx()` / `restore_effect_lowering_cx()` 成组切换，ordinary callee resume entry 则只重置函数级上下文；当前没有发现新的 effect/runtime-function 状态越界回灌到 generic lowering；
  - review 同时确认“仍附着在 backend 主上下文上的共享事实”已经清晰收敛到下一条 `T5000c` 的入口：
    - `crates/scoopc/src/llvm/codegen/effect/state_machine_plan.rs` 中的 `HandlePlanContext::from_codegen(...)` 仍直接从 `MainCodegen` 抽取 ctor/object/property/type/local metadata；
    - 同文件中的 `ensure_known_fun_body_may_outward_effect_cache(...)` / `known_fun_body_may_outward_effect_map(...)` 仍在 LLVM codegen 内构造 higher-order suspendability 事实；
    - `crates/scoopc/src/llvm/codegen/mod.rs` 与 `state_machine_plan.rs` 仍各自维护一套 concrete-type / field-type 恢复 helper，说明这部分 shared facts 尚未 backend-agnostic；
    - `crates/scoopc/src/effect_step_summary.rs` 已直接 `include!` 复用 `state_machine_plan.rs` 的纯分析实现，进一步说明这些事实层已经有 backend 外消费者；
  - review 结论：`MainCodegen` 的 module / function / cache / effect emitter 四类职责边界已成立；下一步要迁出 backend 的不再是“更多 runtime lowering 状态”，而是 `ProgramFacts` / `EffectAnalysisCtx` / shared side tables 这类分析事实。当前未发现需要插入到 `T5000bR` 之前的新前置缺陷任务；
  - 已验证 `cargo test -p scoopc llvm::`、`cargo test --all`、`cargo clippy --all-targets -- -D warnings` 全部通过。

### [DONE] T5000bR Review：确认 LLVM codegen 已收口到“只做 backend lowering”的方向
- 重点：
  - 本轮拆分是否只是“把大文件拆成更多大文件”，还是确实拉直了 backend 边界；
  - 是否仍有明显的中端分析继续依附在 `MainCodegen` 上；
  - 拆分后是否为后续 `ProgramFacts` 抽离和 MIR 迁移留出了清晰入口。
- 验收：
  - 可以明确指出“哪些部分仍属于 backend”“哪些部分下一步要迁出”，且边界比改动前更清楚。
- 依赖：T5000b4R
- 完成记录（2026-04-25）：
  - 已复核 `crates/scoopc/src/llvm/codegen/` 当前主题边界：`call/`、`intrinsics/`、`closure/`、`class_ctor.rs`、`enum_lowering.rs`、`object_init.rs`、`effect/`、`gc.rs`、`runtime_abi.rs` 等已各自承接稳定 backend lowering 主题；`crates/scoopc/src/llvm/codegen/mod.rs` 当前主要保留 `CompilationUnitCodegenCx` / `MainCodegen` 共享上下文、顶层初始化/访问、字面量/聚合值/成员访问/运算符/类型转换等 generic lowering，以及 GC-sensitive spill/root/sret/return helper 与通用 lvalue bridge，说明本轮拆分不是简单把一个大文件机械切成更多大文件；
  - 已确认仍滞留在 LLVM backend 内、且下一步必须迁出的 shared facts/analysis side tables 主要集中在 `crates/scoopc/src/llvm/codegen/effect/state_machine_plan.rs`：
    - `HandlePlanContext::from_codegen(...)` 仍直接从 `MainCodegen` 采集 ctor/object/property/type/local metadata；
    - `ensure_known_fun_body_may_outward_effect_cache(...)` / `known_fun_body_may_outward_effect_map(...)` 仍在 codegen 内拼装 `SuspendCallProgramFacts` 并缓存 higher-order suspendability 事实；
    - `crates/scoopc/src/llvm/codegen/mod.rs` 与 `state_machine_plan.rs` 仍各自维护 concrete-type / field-type 恢复 helper，说明 concrete type、receiver exactness 与 field specialization 相关事实尚未统一迁出 backend；
  - 已再次确认 `crates/scoopc/src/effect_step_summary.rs` 直接 `include!` 复用 `state_machine_plan.rs` 的纯分析实现，证明 effect planning/shared facts 已经存在 backend 外消费者，因此后续 `T5000c` 的方向应是抽离 `ProgramFacts` / `EffectAnalysisCtx`，而不是继续把更多分析逻辑留在 LLVM codegen 内；
  - review 过程中顺手修复了一个既有文档错配：`crates/scoopc/src/llvm/codegen/mod.rs` 顶部注释此前仍写“下一步 T5000b4”，现已改为准确指向 `T5000c` 的 shared-facts 抽离入口；
  - review 结论：LLVM codegen 已明显朝“只做 backend lowering”的方向收口，backend 主题边界比改动前更清楚；当前剩余问题已明确收敛到 `ProgramFacts` / `EffectAnalysisCtx` / shared side tables 抽离，没有发现需要插到 `T5000c` 之前的新前置缺陷任务。

### T5000c 抽离 backend-agnostic 的 `ProgramFacts` / `EffectAnalysisCtx` / shared side tables
- 说明：
  - 经核对，`T5000c` 当前同时覆盖 shared facts 数据结构抽离、effect analysis 上下文收口、以及 planning / direct-step summary 消费面的迁移，单轮过大；
  - 现按稳定边界拆成以下子任务，先收口 `ProgramFacts`，再抽 `EffectAnalysisCtx`，最后迁移共享分析消费者并清理 `include!` 耦合。

### [DONE] T5000c1 抽出 backend-agnostic 的 `ProgramFacts` 数据结构与统一 builder
- 范围：
  - 新增独立于 LLVM backend 的 `ProgramFacts` 数据结构，至少统一承接：
    - ctor / continuation resume call-site facts；
    - top-level value / function return / object property / struct field / class field type facts；
    - class super-key、object / property / top-level immutable value FQN sets。
  - 从 HIR lowering 产物统一构造 `ProgramFacts`，供 LLVM codegen 共享上下文、effect/state-machine planner 与测试 helper 复用；
  - 消除 `HandlePlanContext::from_codegen(...)`、`ensure_known_fun_body_may_outward_effect_cache(...)`、`state_machine_segments.rs` / `state_machine_transform.rs` 测试 helper 对 `SuspendCallProgramFacts` 的重复现场拼装。
- 验收：
  - `CompilationUnitCodegenCx` 持有由 HIR lowering 统一构造的 `ProgramFacts`，而不是在 LLVM codegen 内重建同类 side tables；
  - `HandlePlanContext` 与 known-fun suspendability cache 已改为复用同一份 `ProgramFacts`；
  - 行为与诊断边界保持不变。
- 依赖：T5000bR
- 完成记录（2026-04-25）：
  - 新增 `crates/scoopc/src/program_facts.rs` 与 `lib.rs` 模块入口，定义 backend-agnostic `ProgramFacts`，统一承接 ctor / continuation resume call-site、top-level value / function return / object property / struct/class field type、class super-key、object/property/top-level immutable value 集合等共享 facts，并由 `ProgramFacts::from_lowered(&hir::LoweredHir)` 一次性构造；
  - `crates/scoopc/src/llvm/emit.rs` 现会在进入 LLVM backend 前基于 lowering 结果构造共享 `Rc<ProgramFacts>`，`crates/scoopc/src/llvm/codegen/mod.rs` 中的 `CompilationUnitCodegenCx` 现持有该 shared facts，而不是继续保存一组只为 effect analysis 服务的 backend 专有 side tables；
  - `crates/scoopc/src/llvm/codegen/effect/state_machine_plan.rs` 中的 `HandlePlanContext`、`SuspendCallAnalysis`、`ensure_known_fun_body_may_outward_effect_cache(...)` 与 higher-order function-value suspendability 查询，现已统一复用同一份 `ProgramFacts`；原 `SuspendCallProgramFacts` 临时拼装结构已删除；
  - `crates/scoopc/src/llvm/codegen/effect/state_machine_segments.rs` 与 `state_machine_transform.rs` 的测试 helper 也已改为从 `LoweredHir` 统一构造 `ProgramFacts`，不再各自复制一份 facts 拼装逻辑；
  - 本轮同时修复了一个既有无告警构建问题：`crates/scoopc/src/effect_step_summary.rs` 在 `--no-default-features` 路径下直接 `include!` 整个 `state_machine_plan.rs` 会暴露大量 intentional dead-code / unused-import warnings；现已把告警边界收口在 `effect_step_summary.rs` 自身，保持当前共享语义不变并恢复无告警构建。
  - 已验证 `cargo fmt --all`、`cargo test -p scoopc llvm::`、`cargo test -p scoopc --no-default-features`、`cargo test --all`、`cargo clippy --all-targets -- -D warnings` 全部通过。

### [DONE] T5000c1R Review：确认 `ProgramFacts` 已成为 backend-agnostic 的共享 side table
- 重点：
  - `ProgramFacts` 是否已脱离 LLVM builder / module / GC ABI 依赖；
  - 是否还残留多处重复拼装 program facts 的路径；
  - 这一步是否为后续 `EffectAnalysisCtx` 抽离提供了稳定输入边界。
- 验收：
  - 后续 `T5000c2` 可直接在 `ProgramFacts` 之上继续收口 analysis context，而不再先清理 facts 来源。
- 依赖：T5000c1
- 完成记录（2026-04-25）：
  - 已复核 `crates/scoopc/src/program_facts.rs` 与 `crates/scoopc/src/lib.rs`，确认 `ProgramFacts` 结构与 builder 只依赖 HIR lowering / `TypeId` side tables，不依赖 LLVM builder、module、GC ABI 或 backend runtime helper，因而已形成 backend-agnostic 的共享 facts 层；
  - 已复核 `crates/scoopc/src/llvm/emit.rs`、`crates/scoopc/src/llvm/codegen/mod.rs`、`crates/scoopc/src/llvm/codegen/effect/state_machine_plan.rs`、`state_machine_segments.rs` 与 `state_machine_transform.rs`，确认生产路径与 effect 测试 helper 都统一经 `ProgramFacts::from_lowered(&hir::LoweredHir)` 构造 facts，没有残留第二套 `SuspendCallProgramFacts` 或手写 `HashMap` / `HashSet` 现场拼装；
  - review 过程中暴露并修复了一个既有共享性回退：`SuspendCallAnalysis::handle_may_suspend_outward(...)` 原先会把借来的 `ProgramFacts` 整体 `clone` 后重新包成 `Rc` 传给 nested `HandlePlanContext`；现已把 `SuspendCallAnalysis` 及相关测试 helper 统一改为持有并传递共享 `Rc<ProgramFacts>`，从而恢复 nested-handle suspendability 分析对同一份 side table 的复用，而不是重复复制整表；
  - review 结论：`ProgramFacts` 的来源已经稳定收口为 lowering -> shared builder -> `Rc<ProgramFacts>`，后续 `T5000c2` 可以直接聚焦 `HandlePlanContext::from_codegen(...)`、known local metadata、synthetic symbol/source-path 等 analysis context 取数边界，而不必再回头清理 facts 来源；
  - 已验证 `cargo fmt --all`、`cargo test -p scoopc llvm::`、`cargo test -p scoopc --no-default-features`、`cargo test --all`、`cargo clippy --all-targets -- -D warnings` 全部通过。

### [DONE] T5000c2 抽出 backend-agnostic 的 `EffectAnalysisCtx` 与 shared local metadata
- 范围：
  - 将当前 `HandlePlanContext` 中与 effect/state-machine 分析相关、但不应继续依赖 `MainCodegen` 的上下文收口为独立 `EffectAnalysisCtx`，至少覆盖：
    - known fun/local effect facts；
    - known local metadata；
    - synthetic symbol allocator 状态；
    - source-path / call-site 关联上下文。
  - 替换当前“分析从 `MainCodegen` 反取 env/local metadata”的主路径，使 planning / summary 可在 backend 外复用同一份 analysis context。
- 验收：
  - `HandlePlanContext::from_codegen` 这类由 backend 上下文直接喂给中端分析的路径开始消失；
  - planning / suspendability summary 进入统一 `EffectAnalysisCtx + ProgramFacts` 输入形态。
- 依赖：T5000c1R
- 完成记录（2026-04-25）：
  - 新增 `crates/scoopc/src/effect_analysis.rs` 与 `crates/scoopc/src/lib.rs` 模块入口，抽出 backend-agnostic 的 `EffectAnalysisCtx`、`KnownLocalMetadata`，并统一收口 known local metadata 收集、synthetic symbol allocator、source-path / call-site 关联上下文；
  - `crates/scoopc/src/llvm/codegen/effect/state_machine_plan.rs` 现将 `HandlePlanContext` 收口为 `EffectAnalysisCtx` 别名，`SuspendCallAnalysis` 改为直接消费共享 analysis context，而不再平铺保存 `known_fun_effects`、`known_local_metadata`、`current_source_path` 与 `ProgramFacts`；
  - `HandlePlanContext::from_codegen(...)` 已消失；LLVM backend 现仅通过 `MainCodegen::effect_analysis_ctx()` 把当前函数 env 投影成共享 `EffectAnalysisCtx`，再供 `build_unified_lowering_contract(...)`、ordinary callee suspend planning 与 higher-order function-value suspendability 查询复用；
  - `state_machine_segments.rs` 与 `state_machine_transform.rs` 的测试 helper 已改为统一复用 `collect_effect_analysis_context_for_fun(...)`，不再各自手工拼装同类 analysis context；
  - `direct_step_analysis_context_for_handle(...)` 也已改为构造统一 `EffectAnalysisCtx + ProgramFacts` 输入形态，后续 `T5000c3` 可以继续直接迁移共享分析消费者，而不必再先清理 local metadata / source-path / synthetic symbol 的来源边界；
  - 已验证 `cargo fmt --all`、`cargo test -p scoopc llvm::`、`cargo test -p scoopc --no-default-features`、`cargo test --all`、`cargo clippy --all-targets -- -D warnings` 全部通过。

### [DONE] T5000c2R Review：确认 `EffectAnalysisCtx` 已脱离 LLVM backend 现场取数
- 重点：
  - `EffectAnalysisCtx` 是否真的 backend-agnostic；
  - 是否还残留“必须通过 `MainCodegen` 才能做分析”的强耦合路径；
  - local metadata / synthetic symbol / source-path 上下文是否已形成稳定输入边界。
- 验收：
  - 后续共享分析消费者迁移时，不再需要继续从 backend 主上下文回捞 analysis state。
- 依赖：T5000c2
- 完成记录（2026-04-26）：
  - 已复核 `crates/scoopc/src/effect_analysis.rs`，确认 `EffectAnalysisCtx` / `KnownLocalMetadata` 只依赖 `hir`、`TypeId`、`ProgramFacts`、`PathBuf` 与标准库容器/内部可变性，不依赖 LLVM builder、module、GC ABI 或 runtime helper 类型，因此 analysis context 本体已形成 backend-agnostic 边界；
  - 已复核 `crates/scoopc/src/llvm/codegen/effect/state_machine_plan.rs`，确认 `HandlePlanContext` 当前仅是 `EffectAnalysisCtx` 别名，`SuspendCallAnalysis`、`HandlePlanBuilder` 与 direct-step summary helper 都经共享 context 访问 known fun/local effects、known local metadata、synthetic symbol allocator、source-path / call-site 与 `ProgramFacts`，不再平铺保存或从 backend 现场回捞这些 analysis state；
  - 已复核 `MainCodegen::effect_analysis_ctx()` 只承担 LLVM backend -> shared context 的单向投影；同时 `collect_effect_analysis_context_for_fun(...)` 已为 backend 外构造提供稳定入口，`state_machine_segments.rs` 与 `state_machine_transform.rs` 的测试 helper 现统一经该入口复用 analysis context，不再手工拼装平行上下文；
  - review 未发现需要插入到 `T5000c3` 之前的新前置缺陷任务；`effect_step_summary.rs` 对 `state_machine_plan.rs` 的 `include!` 复用仍是已在 `T5000c3` 显式跟踪的下一步工作，但它当前已不需要通过 backend 主上下文回捞 analysis state；
  - 已验证 `cargo fmt --all --check`、`cargo test -p scoopc llvm::`、`cargo test -p scoopc --no-default-features`、`cargo test --all`、`cargo clippy --all-targets -- -D warnings` 全部通过。

### T5000c3 迁移 effect/state-machine planning 与 direct-step summary 到 shared facts / analysis 层
- 说明：
  - 经核对，当前 `T5000c3` 同时包含 shared analysis 源文件归属迁移、`effect_step_summary.rs` 的 backend `include!` 清理，以及 concrete-type / field-type / receiver exactness 共享 helper 的消费方向收口，单轮过大；
  - 现按稳定边界拆成以下子任务，先迁 shared planning / summary 源文件，再继续拉直 concrete-type / receiver exactness 相关 helper 的依赖方向。

### [DONE] T5000c3a 抽出共享 `effect_state_machine_analysis.rs` 源文件并清理 backend `include!`
- 范围：
  - 将当前 `llvm/codegen/effect/state_machine_plan.rs` 中不依赖 LLVM builder / module 的 pure analysis 主体迁到 crate 根的共享源文件，供 backend planning 与 `effect_step_summary.rs` 共同复用；
  - 让 `llvm/codegen/effect/state_machine_plan.rs` 退回 backend 壳层 / 薄入口，不再作为 shared analysis 的实际归属路径；
  - 清理 `effect_step_summary.rs` 对 backend 源文件的 `include!` 复用。
- 验收：
  - shared planning / direct-step summary 源文件归属已脱离 `llvm/codegen/effect/`；
  - `effect_step_summary.rs` 不再 `include!` LLVM backend 源文件；
  - 现有 planning / direct-step summary 行为与测试保持一致。
- 依赖：T5000c2R
- 完成记录（2026-04-26）：
  - 原 `crates/scoopc/src/llvm/codegen/effect/state_machine_plan.rs` 的 pure analysis 主体已迁到新的 crate 根共享源文件 `crates/scoopc/src/effect_state_machine_analysis.rs`；
  - `crates/scoopc/src/llvm/codegen/effect/state_machine_plan.rs` 现已收口为薄包装，仅负责把共享分析源码 `include!` 回 backend 的 `unified_state_machine_skeleton` 模块可见性作用域；
  - `crates/scoopc/src/effect_step_summary.rs` 已改为直接复用 `effect_state_machine_analysis.rs`，不再依赖 backend 路径下的 `state_machine_plan.rs`；
  - 已验证 `cargo fmt --all --check`、`cargo test -p scoopc llvm::`、`cargo test -p scoopc --no-default-features`、`cargo test --all`、`cargo clippy --all-targets -- -D warnings` 全部通过。

### [DONE] T5000c3aR Review：确认 shared planning / summary 源文件已脱离 backend 路径
- 重点：
  - pure analysis 主体是否已经从 `llvm/codegen/effect/state_machine_plan.rs` 迁出；
  - backend 当前是否只保留薄入口，而不是继续充当 shared analysis 的归属层；
  - `effect_step_summary.rs` 是否已改为依赖 shared 源文件而非 backend 文件。
- 验收：
  - 后续 `T5000c3b` 可以只继续处理 concrete-type / receiver exactness helper 的消费方向，而不必再回头处理 shared source ownership。
- 依赖：T5000c3a
- 完成记录（2026-04-26）：
  - 已复核 `crates/scoopc/src/effect_state_machine_analysis.rs`、`crates/scoopc/src/llvm/codegen/effect/state_machine_plan.rs`、`crates/scoopc/src/effect_step_summary.rs` 与 `crates/scoopc/src/llvm/codegen/effect/mod.rs`，确认 handle planning、higher-order suspendability summary、direct-step summary 与相关测试主体当前统一归属到 crate 根的 shared 源文件 `effect_state_machine_analysis.rs`，不再把 `state_machine_plan.rs` 当作 shared analysis 的实际归属层；
  - 已确认 `crates/scoopc/src/llvm/codegen/effect/state_machine_plan.rs` 当前只剩薄包装：文件本身仅负责把 `../../../effect_state_machine_analysis.rs` `include!` 回 backend `unified_state_machine_skeleton` 的局部可见性作用域，backend 壳层不再直接承载 pure analysis 主体；
  - 已搜索 `crates/scoopc/src` 中对 `state_machine_plan.rs` / `effect_state_machine_analysis.rs` 的文本级复用路径，确认 `crates/scoopc/src/effect_step_summary.rs` 现直接 `include!("effect_state_machine_analysis.rs")`，仓库中已不存在 non-LLVM consumer 再通过 backend 路径复用 shared analysis 源文件的残留；
  - review 结论：shared planning / direct-step summary 源文件归属已经脱离 backend 路径，后续 `T5000c3b` 可以只继续处理 concrete-type / field-type / receiver exactness helper 的消费方向；未发现需要插入到 `T5000c3b` 之前的新前置缺陷任务；
  - 已验证 `cargo fmt --all --check`、`cargo test -p scoopc llvm::`、`cargo test -p scoopc --no-default-features`、`cargo test --all`、`cargo clippy --all-targets -- -D warnings` 全部通过。

### [TODO] T5000c3b 收口 concrete-type / field-type / receiver exactness 共享 helper 的消费方向
- 范围：
  - 将 effect/state-machine planning 中的 concrete-type / field-type / receiver exactness 相关共享 helper 收口到 backend-agnostic 的 shared analysis / facts 归属层；
  - 拉直 `llvm/codegen/mod.rs` 与 shared planning / summary 对这组 helper 的消费方向，为后续 MIR / summary 复用同一层事实做准备。
- 验收：
  - planning / direct-step summary 与 backend generic lowering 不再各自维护一套同类 helper；
  - concrete type、receiver exactness 与 field specialization 相关事实已可由 shared 层统一提供。
- 依赖：T5000c3aR

### [TODO] T5000c3bR Review：确认 concrete-type / receiver exactness helper 的依赖方向已拉直
- 重点：
  - 是否还残留 planning / summary 与 backend generic lowering 各自维护一套同类 helper；
  - shared facts / analysis 层是否已经承接 concrete type / field type / receiver exactness 的共同输入；
  - 后续 MIR / summary 消费面是否可以直接复用这层事实，而不必重新从 backend 现场回捞。
- 验收：
  - `T5000c3R` 可以基于统一 shared helper 边界做总复核。
- 依赖：T5000c3b

### [TODO] T5000c3R Review：确认共享分析消费者已脱离 LLVM backend 源文件依赖
- 重点：
  - shared facts / analysis 层是否已经覆盖 planning 与 direct-step summary 的共同输入；
  - 是否还残留对 `state_machine_plan.rs` 文本级复用或 backend helper 的强耦合；
  - concrete type / receiver exactness / field specialization 相关 helper 的依赖方向是否已经拉直。
- 验收：
  - `T5000cR` 可以基于清晰的 backend-agnostic facts / analysis 边界做总复核。
- 依赖：T5000c3bR

### [TODO] T5000cR Review：确认共享事实层已经脱离 LLVM backend 依赖方向
- 重点：
  - `ProgramFacts` / `EffectAnalysisCtx` 是否真的 backend-agnostic；
  - 是否还残留“必须通过 `MainCodegen` 才能做分析”的强耦合路径；
  - 该层是否已经足够支撑 MIR、summary 与 effect planning 的共同消费。
- 验收：
  - 后续 MIR 任务可以在不依赖 LLVM builder / module / GC ABI 细节的前提下推进。
- 依赖：T5000c3R

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
