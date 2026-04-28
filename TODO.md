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

### [DONE] T5000c3b 收口 concrete-type / field-type / receiver exactness 共享 helper 的消费方向
- 范围：
  - 将 effect/state-machine planning 中的 concrete-type / field-type / receiver exactness 相关共享 helper 收口到 backend-agnostic 的 shared analysis / facts 归属层；
  - 拉直 `llvm/codegen/mod.rs` 与 shared planning / summary 对这组 helper 的消费方向，为后续 MIR / summary 复用同一层事实做准备。
- 验收：
  - planning / direct-step summary 与 backend generic lowering 不再各自维护一套同类 helper；
  - concrete type、receiver exactness 与 field specialization 相关事实已可由 shared 层统一提供。
- 依赖：T5000c3aR
- 完成记录（2026-04-26）：
  - 新增 `crates/scoopc/src/expr_facts.rs`，将基于 `TypeStore + ProgramFacts + local type lookup` 的 concrete-type / field-type / call-result 解析统一收口为 backend-agnostic shared resolver `ExprFactResolver`，不再让 LLVM generic lowering 与 effect/state-machine shared analysis 维持平行实现；
  - `crates/scoopc/src/program_facts.rs` 已补充 `top_level_value_ty`、`object_property_ty`、`fun_return_ty` 与 `resolve_nominal_field_ty` 等最小查询 helper，把 top-level value、object property、struct/class field 与 class-super 递归查询集中到 shared facts 层；
  - `crates/scoopc/src/effect_state_machine_analysis.rs` 中 planning 的 `resolve_plan_*` concrete-type helper 与 `SuspendCallAnalysis` 内部的同类 helper 已删除并统一改为调用 `ExprFactResolver`；`crates/scoopc/src/llvm/codegen/mod.rs` 中原本独立维护的 `resolve_member_access_concrete_type` / `resolve_*_field_concrete_type` / `resolve_call_result_type` 也已收口为对 shared resolver 的薄包装；
  - 阶段结论：planning / direct-step summary 与 backend generic lowering 已不再各自维护一套同类 helper，receiver exactness / field specialization 所需的 concrete-type 事实现统一由 shared `ProgramFacts` + `expr_facts` 层提供；
  - 已验证 `cargo fmt --all --check`、`cargo test -p scoopc llvm::tests::lowered_call_results_keep_concrete_types_for_local_bindings`、`cargo test -p scoopc direct_step_effect_rows_include_direct_effectful_call_after_escape_site`、`cargo test -p scoopc --no-default-features direct_step_effect_rows_include_direct_effectful_call_after_escape_site`、`cargo test -p scoopc --no-default-features`、`cargo test --all`、`cargo clippy --all-targets -- -D warnings` 全部通过。

### [DONE] T5000c3bR Review：确认 concrete-type / receiver exactness helper 的依赖方向已拉直
- 重点：
  - 是否还残留 planning / summary 与 backend generic lowering 各自维护一套同类 helper；
  - shared facts / analysis 层是否已经承接 concrete type / field type / receiver exactness 的共同输入；
  - 后续 MIR / summary 消费面是否可以直接复用这层事实，而不必重新从 backend 现场回捞。
- 验收：
  - `T5000c3R` 可以基于统一 shared helper 边界做总复核。
- 依赖：T5000c3b
- 完成记录（2026-04-26）：
  - 已复核 `crates/scoopc/src/expr_facts.rs` 为 concrete-type / field-type / call-result 解析的唯一主体实现；`crates/scoopc/src/llvm/codegen/mod.rs` 与 `crates/scoopc/src/effect_state_machine_analysis.rs` 中仅剩向 shared resolver 注入 local type lookup 的薄包装，不再各自维护平行 helper；
  - 已复核 `crates/scoopc/src/program_facts.rs` 中的 `top_level_value_ty`、`object_property_ty`、`fun_return_ty` 与 `resolve_nominal_field_ty` 已覆盖 shared resolver 所需的共同输入，effect planning 与 backend generic lowering 现统一经 `ProgramFacts + ExprFactResolver` 读取 top-level/object/field/return 事实，而不是回到 LLVM backend 现场拼装；
  - 已全文检索 `resolve_member_access_concrete_type`、`resolve_*_field_concrete_type`、`resolve_call_result_type` 等旧 helper 名称，确认主体实现只剩 `expr_facts.rs` 一处；effect planning 的 `resolve_plan_expr_concrete_type` 与 `SuspendCallAnalysis::resolve_expr_concrete_type` 仅保留为注入 `known_local_metadata` 的轻量桥接；
  - review 同时确认 `MainCodegen::top_level_value_ty` 等剩余 backend 查询只服务于 lowering 期 top-level value/function-value 路径，不再承担 shared concrete-type / receiver exactness helper 的职责；当前未发现需要插入到 `T5000c3R` 之前的新前置缺陷任务；
  - 已验证 `cargo fmt --all --check`、`cargo test -p scoopc llvm::tests::lowered_call_results_keep_concrete_types_for_local_bindings`、`cargo test -p scoopc direct_step_effect_rows_include_direct_effectful_call_after_escape_site`、`cargo test -p scoopc --no-default-features direct_step_effect_rows_include_direct_effectful_call_after_escape_site`、`cargo test --all`、`cargo clippy --all-targets -- -D warnings` 全部通过。

### [DONE] T5000c3R Review：确认共享分析消费者已脱离 LLVM backend 源文件依赖
- 重点：
  - shared facts / analysis 层是否已经覆盖 planning 与 direct-step summary 的共同输入；
  - 是否还残留对 `state_machine_plan.rs` 文本级复用或 backend helper 的强耦合；
  - concrete type / receiver exactness / field specialization 相关 helper 的依赖方向是否已经拉直。
- 验收：
  - `T5000cR` 可以基于清晰的 backend-agnostic facts / analysis 边界做总复核。
- 依赖：T5000c3bR
- 完成记录（2026-04-26）：
  - 已复核 `crates/scoopc/src/lib.rs`、`crates/scoopc/src/effect_step_summary.rs`、`crates/scoopc/src/effect_state_machine_analysis.rs` 与 `crates/scoopc/src/llvm/codegen/effect/state_machine_plan.rs`，确认非 LLVM consumer 当前直接复用 crate 根的 shared 源文件 `effect_state_machine_analysis.rs`，而 `state_machine_plan.rs` 已收口为 backend 局部可见性薄包装；仓库内对 shared analysis 源文件的文本级复用现只剩这两条显式入口，不再存在 non-LLVM consumer 经由 backend 路径复用 shared source 的残留；
  - 已复核 `crates/scoopc/src/program_facts.rs`、`crates/scoopc/src/effect_analysis.rs` 与 `crates/scoopc/src/expr_facts.rs`，确认 planning 与 direct-step summary 的共同输入已统一落在 `ProgramFacts` / `EffectAnalysisCtx` / `ExprFactResolver`：top-level/object/field/return 事实、known local metadata、known local function suspendability 与 synthetic symbol/source-path 上下文均由 shared 层提供，不再依赖 backend helper 现场回捞；
  - 已确认 `crates/scoopc/src/effect_state_machine_analysis.rs` 中剩余 `MainCodegen` 相关入口全部位于 `#[cfg(feature = "llvm")] impl MainCodegen` 内，只作为 backend 调用 shared analysis 的薄接缝；direct-step summary 与非 LLVM 测试辅助路径继续只消费 shared 函数和 shared context，没有新增对 LLVM backend 类型或 backend 文件路径的强耦合；
  - review 结论：共享分析消费者已脱离 LLVM backend 源文件依赖，`T5000cR` 可以基于 backend-agnostic 的 facts / analysis 边界做总复核；当前未发现需要插入到 `T5000cR` 之前的新前置缺陷任务；
  - 已验证 `cargo check -p scoopc --lib`、`cargo fmt --all --check`、`cargo test -p scoopc llvm::tests::lowered_call_results_keep_concrete_types_for_local_bindings`、`cargo test -p scoopc --no-default-features direct_step_effect_rows_include_direct_effectful_call_after_escape_site`、`cargo test -p scoopc --no-default-features`、`cargo test --all`、`cargo clippy --all-targets -- -D warnings` 全部通过。

### [DONE] T5000cR Review：确认共享事实层已经脱离 LLVM backend 依赖方向
- 重点：
  - `ProgramFacts` / `EffectAnalysisCtx` 是否真的 backend-agnostic；
  - 是否还残留“必须通过 `MainCodegen` 才能做分析”的强耦合路径；
  - 该层是否已经足够支撑 MIR、summary 与 effect planning 的共同消费。
- 验收：
  - 后续 MIR 任务可以在不依赖 LLVM builder / module / GC ABI 细节的前提下推进。
- 依赖：T5000c3R
- 完成记录（2026-04-26）：
  - 已复核 `crates/scoopc/src/program_facts.rs`、`crates/scoopc/src/effect_analysis.rs` 与 `crates/scoopc/src/expr_facts.rs`，确认三者当前只依赖 HIR / `TypeStore` / `ProgramFacts` / 本地 metadata 等 shared 输入，不直接引用 `inkwell`、`crate::llvm`、`MainCodegen`、GC ABI 或 LLVM builder/module 细节；`ProgramFacts::from_lowered(...)` 也继续只从 lowered HIR side tables 一次性构造共享事实，而不是从 backend emitter 现场回捞。
  - 已复核 `crates/scoopc/src/effect_state_machine_analysis.rs` 与 `crates/scoopc/src/effect_step_summary.rs`，确认 handle planning、higher-order suspendability summary 与 direct-step summary 的主体实现继续运行在 `EffectAnalysisCtx` / `ProgramFacts` / `ExprFactResolver` 之上；文件中唯一 `MainCodegen` 相关入口位于 `#[cfg(feature = "llvm")] impl MainCodegen` 的薄包装接缝，而 `collect_known_fun_call_suspendability(...)`、`direct_step_analysis_context_for_handle(...)`、`compute_escape_continuation_direct_step_effect_rows_for_handle_in_program(...)` 等 shared 分析入口均可在不构造 LLVM backend 上下文的前提下独立工作。
  - 已复核 `crates/scoopc/src/llvm/emit.rs` 与 `crates/scoopc/src/llvm/codegen/mod.rs`，确认 backend 侧现统一在 lowering 后一次性构造 `Rc<ProgramFacts>` 并注入 `CompilationUnitCodegenCx`；generic lowering 对 concrete-type / field-type 的恢复也只经 `ExprFactResolver` 这类 shared helper 消费共享事实，而不是在 backend 现场重建平行 side tables。
  - review 过程中顺手修复了一个既有文档错配：`crates/scoopc/src/llvm/codegen/mod.rs` 顶部注释此前仍写“下一步 T5000c”，现已改为准确描述 `T5000c` 已完成 shared facts 抽离、后续转向 `T5000d+` 让 early MIR / summary 直接复用同一层共享事实。
  - review 结论：共享事实层已经脱离 LLVM backend 依赖方向，后续 `T5000d` 可以在不依赖 LLVM builder / module / GC ABI 细节的前提下推进 generic early MIR / ANF template；当前未发现需要插入到 `T5000d` 之前的新前置缺陷任务。
  - 已验证 `cargo fmt --all --check`、`cargo check -p scoopc --lib`、`cargo test -p scoopc llvm::tests::lowered_call_results_keep_concrete_types_for_local_bindings`、`cargo test -p scoopc --no-default-features direct_step_effect_rows_include_direct_effectful_call_after_escape_site`、`cargo test -p scoopc --no-default-features`、`cargo test --all`、`cargo clippy --all-targets -- -D warnings` 全部通过。

### T5000d 扩展现有 MIR，形成最小 generic early MIR / ANF template
- 说明：
  - 经核对，当前 `crates/scoopc/src/mir/mod.rs` 虽已具备 CFG / locals / 最小 `Perform` / `Handle` 占位，但 `crates/scoopc/src/mir/lower.rs` 对普通 `Call` 仍统一产出 `Todo("call lowering pending")`，`Perform` 也尚未承载稳定的 payload / dispatch metadata，而 `Continuation.resume` / member dispatch 仍未在 MIR 上显式出现。
  - 原任务单轮过大，现按“普通调用主线 → 动态分派 / resume → control-transfer / provenance 收口”的依赖顺序拆成以下子任务；所有子任务均保持 backend-agnostic，不把 LLVM 细节倒灌到 MIR。

### [DONE] T5000d1 为 MIR 引入显式普通调用节点，并落地 `DirectCall / ClosureCall / FunValueCall`
- 范围：
  - 在 MIR 中加入显式普通调用节点与参数承载形状，不再让这三类调用统一退化为 `Todo(...)`；
  - 将以下调用主线 lowering 为稳定的 ANF 形状：
    - 顶层 / 已静态唯一确定的直接调用 → `DirectCall`
    - 已知 closure value 调用 → `ClosureCall`
    - 其余函数值调用基线 → `FunValueCall`
  - 打通 callable value 的最小 provenance 基线，使 closure/object-like callable 值经 local 传播后仍可在 MIR 上区分为 `ClosureCall` 而不是重新退化成模糊 `Call`。
- 验收：
  - MIR dump / fixtures 能显式区分 `DirectCall`、`ClosureCall`、`FunValueCall`；
  - 这三类调用不再出现通用 `"call lowering pending"` 占位；
  - 调用实参按求值顺序先降到 operand / local，再进入 MIR 调用节点。
- 依赖：T5000cR
- 完成记录（2026-04-26）：
  - 已在 `crates/scoopc/src/mir/mod.rs` 中新增 MIR 级 `CallArg`、`CallKind::{Direct, Closure, FunValue}` 与 `Rvalue::Call`，把普通调用从通用 `Todo(...)` 提升为显式普通调用节点；
  - 已在 `crates/scoopc/src/mir/lower.rs` 中实现普通调用 lowering：顶层静态调用降为 `DirectCall`，已知 closure value 调用降为 `ClosureCall`，其余函数值调用降为 `FunValueCall`；调用实参现统一先按求值顺序 lowering 为 operand/local，再写入 MIR 调用节点；
  - 实现过程中暴露并修复了两个既有阻塞点：
    - `dump-hir` / `dump-mir` 路径里，顶层函数值作为普通调用实参时，`ExpectedExpr` 的旧“非数组字面量直接早退”会吞掉 `value_ty` hint，导致 `apply(id, 2)` 这类 callable 实参无法合成为 closure；现已在 `crates/scoopc/src/hir/lower/expr.rs` 中补上一般 `value_ty` 透传与顶层函数值 expected-type fallback；
    - closure 临时值在 dump 路径中常先落成 `Any` local，导致 `MakeClosure -> local -> local` 传播后丢失 closure provenance；现已在 `crates/scoopc/src/mir/lower.rs` 中把 callable provenance 跟踪改成对已知 closure 来源独立保留，不再要求中间 local 先有函数类型；
  - 已新增 `tests/fixtures/mir/direct_and_fun_value_call.{scoop,mir}`，并更新 `closure_non_capture.mir`、`closure_capture_val.mir`、`closure_capture_var.mir`，确认 direct / closure / fun-value 三类调用都能在 MIR golden 中显式出现。
  - 已验证 `cargo fmt --all`、`cargo test --all`、`cargo clippy --all-targets -- -D warnings` 全部通过。

### [DONE] T5000d1R Review：确认普通调用主线已从 HIR 语法形状收口为显式 MIR call kind
- 重点：
  - `DirectCall / ClosureCall / FunValueCall` 是否已经在 MIR 上显式区分；
  - callable value provenance 是否足以支撑后续 closure / higher-order 分析，而不要求再回到 HIR 语法猜测；
  - 当前改动是否仍保持 backend-agnostic，没有混入 LLVM lowering 细节。
- 验收：
  - 后续 `VirtualCall / InterfaceCall / Resume` 可直接建立在统一调用节点之上，而不是再改一套平行表示。
- 依赖：T5000d1
- 完成记录（2026-04-26）：
  - 已复核 `crates/scoopc/src/mir/mod.rs`、`crates/scoopc/src/mir/lower.rs`、`crates/scoopc/src/hir/lower/expr.rs` 与相关 MIR fixtures，确认 `CallArg`、`CallKind::{Direct, Closure, FunValue}`、`Rvalue::Call` 已构成统一的普通调用表示；`DirectCall / ClosureCall / FunValueCall` 在 MIR golden 中已显式区分，不再依赖 HIR `Call` / `Closure` 语法形状回猜。
  - 已确认 callable provenance 现由 `callable_value_origins` 基线跟踪：`MakeClosure` 经 local 传播后仍能在 `closure_non_capture.mir`、`closure_capture_val.mir`、`closure_capture_var.mir` 中稳定保持为 `ClosureCall`；而无法恢复为唯一 closure 目标的函数值调用则保守落为 `FunValueCall`，符合后续 higher-order 分析可消费的最小结构事实。
  - 已确认普通调用主线保持 backend-agnostic：`crates/scoopc/src/mir/*` 未依赖 `crate::llvm` / `inkwell`，MIR `CallKind` 只表达语言级 direct/closure/fun-value 语义；当前残留的 `Todo(...)` 分支仅针对 `T5000d2` 之后才会接入的 member dispatch / ctor / 非函数 callee guardrail，不再是这三类普通调用的通用占位。
  - review 结论：普通调用主线已经从 HIR 语法形状收口为显式 MIR call kind，后续 `T5000d2` 可直接在同一调用节点层级上补 `VirtualCall / InterfaceCall / Resume`，无需再引入平行表示。
  - 已验证 `cargo fmt --all --check`、`cargo run -p scoop -- test --fixtures tests/fixtures/mir`、`cargo test --all`、`cargo clippy --all-targets -- -D warnings` 全部通过。

### [DONE] T5000d2 在 MIR 中显式表达 `VirtualCall / InterfaceCall / Resume`
- 范围：
  - 将剩余依赖 member dispatch 的调用主线从 HIR `MemberAccess` 形状提升为显式 `VirtualCall` / `InterfaceCall`；
  - 为这些调用补上后续优化所需的最小 receiver / dispatch metadata；
  - 将 typecheck 已确认的 `Continuation.resume` 调用点提升为显式 `Resume`，不再只靠 side table + 更晚 lowering 识别。
- 验收：
  - MIR 不再需要通过 `MemberAccess` callee 形状隐式推断“这是 virtual/interface 调用”；
  - `Continuation.resume` 在 MIR 上有稳定落点，可供后续 escaping / effect planning 直接消费。
- 依赖：T5000d1R
- 完成记录（2026-04-26）：
  - `crates/scoopc/src/mir/mod.rs` 新增 `DispatchMetadata`、`ResumeMetadata`，并将 `CallKind` 扩展为 `Direct / Closure / FunValue / Virtual / Interface / Resume`；MIR 调用节点现在可直接表达动态分派与 continuation resume，而不再回退到 HIR `MemberAccess` 形状推断。
  - `crates/scoopc/src/mir/lower.rs` 新增 `MirLoweringFacts`，统一消费 typed/shared HIR facts 中的 class vtable / interface dispatch 目标与 `Continuation.resume` side table，并在 `lower_call_expr` 中显式产出 `Virtual`、`Interface`、`Resume` 调用。
  - 已修复 dump 路径的既有阻塞点：`crates/scoopc/src/hir/lower/mod.rs` 新增 `lower_typed_for_dump(...)`，`crates/scoopc/src/mir/lower.rs`、`crates/scoopc/src/monomorph/lower.rs`、`crates/scoopc/src/cone/pre_specialize.rs` 现统一走带 facts 的 typed lowering，从而让 late-bound member resolution、`Continuation.resume` 与 typed effect payload 信息都能进入 MIR dump / monomorph 路径。
  - 已新增 `tests/fixtures/mir/dispatch_and_resume_call.{scoop,mir}` 覆盖 `Virtual` / `Interface` / `Resume`，并更新现有 MIR goldens 以匹配 typed dump 的更精确类型信息；`crates/scoopc/src/monomorph/lower.rs` 还新增单测，确认实例化后仍保留 `Virtual` call kind。
  - 已修复新增 typed-lowering 错误变体导致的 `clippy::result_large_err`：`crates/scoopc/src/hir/lower/types.rs` 中相关 `HirLowerError` 变体已改为装箱并补齐 `From` 转换。
  - 已验证 `cargo fmt --all`、`cargo run -p scoop -- test --fixtures tests/fixtures/mir`、`cargo test -p scoopc monomorph::lower`、`cargo test -p scoop --test t1124_incremental_cone_run -- --nocapture`、`cargo test --all`、`cargo clippy --all-targets -- -D warnings` 全部通过。

### [DONE] T5000d2R Review：确认动态分派与 `Resume` 已成为 MIR 一等节点
- 重点：
  - `VirtualCall / InterfaceCall` 是否仍保持 backend-agnostic，而非退化成 vtable / itable 细节；
  - `Resume` 是否已脱离“普通调用 + side table”的隐式表示；
  - receiver / dispatch metadata 是否已经足够稳定，可供 devirt / escape analysis 使用。
- 验收：
  - 后续 pass 不必再回到 HIR `MemberAccess` 或 LLVM codegen 现场恢复这些控制转移形态。
- 依赖：T5000d2
- 完成记录（2026-04-26）：
  - 已复核 `crates/scoopc/src/mir/mod.rs` 与 `crates/scoopc/src/mir/lower.rs`，确认 `DispatchMetadata` 只保留 `owner_fqn`、`member_name`、`receiver_ty` 三类语言级事实；`CallKind::{Virtual, Interface}` 继续停留在 backend-agnostic 层，没有回退成 vtable slot / itable id / runtime thunk 等 LLVM 细节。
  - 已确认 `Continuation.resume` 在 MIR 上不再依赖“普通调用 + side table”隐式恢复：`MirLoweringFacts` 只作为 lowering 输入收口 typed/shared HIR 事实，`lower_call_expr` 会优先显式产出 `CallKind::Resume { continuation, resume }`，其中 `ResumeMetadata` 稳定携带 `continuation_ty` 与 `suspends_outward`，供后续 escape analysis / effect planning 直接消费。
  - review 过程中发现一个既有覆盖缺口：原有 `tests/fixtures/mir/dispatch_and_resume_call.{scoop,mir}` 只验证了 `ResumeMetadata.suspends_outward = false` 的 Pure continuation 场景，未覆盖 non-Pure continuation 的 outward-suspend 语义；本轮已补入 `resumeBoom` fixture，并确认 MIR dump 稳定产出 `suspends_outward: true`。
  - 已复核 `crates/scoopc/src/monomorph/lower.rs` 的 typed MIR lowering 入口与现有单测，确认实例化路径继续保留 `Virtual` call kind，不需要后续 pass 回到 HIR `MemberAccess` 或 LLVM codegen 现场重建动态分派形状。
  - review 结论：动态分派与 `Resume` 已成为 MIR 一等节点；后续 `T5000d3` 可直接在现有 `CallKind` / `Perform` / provenance 入口上继续收口 generic early MIR 形状，无需先插入新的前置缺陷任务。
  - 已验证 `cargo run -p scoop -- test --fixtures tests/fixtures/mir`、`cargo test -p scoopc monomorph::lower -- --nocapture`、`cargo test --all`、`cargo clippy --all-targets -- -D warnings` 全部通过。

### [DONE] T5000d3 收口 `Perform` / provenance / canonicalization 入口，为后续 pattern 与 operator materialization 提供正规化 MIR 形状
- 范围：
  - 将 `Perform` 扩展为显式承载已排序 payload / 调用点 metadata 的 MIR 节点；
  - 收口 early MIR 中与调用/控制转移相关的 provenance、control-flow 与 local-binding 形状，使其足以支撑后续 `when` / pattern lowering 与 operator-overload target materialization；
  - 清理剩余必须依赖 HIR 语法形状才能恢复的信息入口。
- 验收：
  - MIR 能稳定承载后续优化所需的调用形态与控制转移信息；
  - 后续 pass 不必再通过 HIR 语法形状或 LLVM codegen 现场推断来恢复这些信息。
- 依赖：T5000d2R
- 完成记录（2026-04-26）：
  - `crates/scoopc/src/mir/mod.rs` 已补齐 `TopLevelRef`、`MemberAccessMetadata`、`PerformArg`、`PerformMetadata`、`Pattern`、`PatternBindingStep`，并把 `Rvalue` 扩展为 `TopLevelRef` / `UnresolvedName` / `Unary` / `Binary` / `TypeCheck` / `Cast` / `MemberAccess` / `PatternMatch` / `PatternExtract` / `PerformResult` 等一等节点；`TerminatorKind::Perform` 现在显式携带 `metadata + args`。
  - `crates/scoopc/src/mir/lower.rs` 现已把 provenance 从“仅 callable”扩展为通用 `value_origins`，把顶层函数引用、member access、未解析 ctor 名、一元/二元运算、类型检查/转换、`when` pattern match/extract 与 `perform` payload canonicalization 全部在 early MIR 层落成结构化节点，不再依赖后续 pass 或 LLVM codegen 现场重建。
  - `crates/scoopc/src/hir/mod.rs` 与 `crates/scoopc/src/hir/lower/patterns.rs` 现已让 `WhenPat::IntLit` / `WhenPat::StringLit` 直接携带 `raw` / `value`；`crates/scoopc/src/llvm/codegen/control_flow.rs` 与 `crates/scoopc/src/llvm/codegen/mod.rs` 已改为消费这些正规化 pattern 字面量，而不是继续通过 span 回查源码。
  - `crates/scoopc/src/hir/lower/mod.rs` 与 `crates/scoopc/src/monomorph/lower.rs` 已补齐带 side table 的 typed lowering 透传，保证这些新 MIR 入口不仅在 `dump-mir` 路径可见，也能进入 monomorph 实例化路径。
  - 过程中发现并优先修复了一个既有 lowering 缺口：`when` 对“无 guard 的兜底 arm”会预分配永远不会执行的 `next_test_bb`，在 MIR dump 中留下 `unterminated` / 多余 CFG 残块；现已改为按 arm 形状懒分配 block，并让无 guard arm 直接在 match block 内继续 lowering。
  - 已更新 `tests/fixtures/mir/direct_and_fun_value_call.mir`、`tests/fixtures/mir/handle_perform.mir`、`tests/fixtures/mir/if_when.mir`，并新增 `tests/fixtures/mir/when_bind_guard.{scoop,mir}` 覆盖 `PatternMatch + PatternExtract + guard Binary` 路径；同时更新 `tests/fixtures/hir/control_flow.hir` 以匹配 `WhenPat::IntLit { raw }` 的新 HIR 形状。
  - 已验证 `cargo fmt --all`、`cargo run -p scoop -- test --fixtures tests/fixtures/mir`、`cargo test -p scoopc monomorph::lower -- --nocapture`、`cargo test --all`、`cargo clippy --all-targets -- -D warnings` 全部通过。

### [DONE] T5000d3R Review：确认 generic early MIR template 的调用与 control-transfer 入口已经成型
- 重点：
  - `Perform` / `Resume` / 各类 call kind 是否已形成统一、可扩展的 MIR 表达；
  - provenance / receiver / dispatch metadata 是否已经满足后续 monomorphization / summary / devirt 的最低要求；
  - 是否还残留“必须靠 HIR 语法或 backend 现场补猜”的关键信息。
- 验收：
  - `T5000dR` 可以只做总边界复核，而不需要再补基础表示层缺口。
- 依赖：T5000d3
- 完成记录（2026-04-26）：
  - 已复核 `crates/scoopc/src/mir/mod.rs`、`crates/scoopc/src/mir/lower.rs` 与 `crates/scoopc/src/monomorph/lower.rs`，确认 `CallKind::{Direct, Closure, FunValue, Virtual, Interface, Resume}` 与 `TerminatorKind::Perform` 已形成统一的调用 / control-transfer 入口；`TopLevelRef`、`MemberAccessMetadata`、`DispatchMetadata`、`ResumeMetadata`、`PerformMetadata`、`PerformArg` 与 `PerformResult` 已把后续 monomorphization / summary / devirt 需要的最小语言级事实显式落到 MIR，而没有回退到 LLVM vtable/itable/statepoint 细节。
  - 已确认 `MirLoweringFacts` 继续只承担 backend-agnostic 的 lowering 输入：dispatch 目标来自 class/interface shared tables，`Continuation.resume` 来自 typed/shared HIR call-site 标记，effect payload canonicalization 与 `when` binder type 也都经 side table 收口；下游 pass 无需再回到 HIR 语法或 backend 现场补猜这些信息。
  - review 过程中暴露并修复了一个既有 CFG 缺口：`crates/scoopc/src/mir/lower.rs` 中 `return` / `val` / `assign` 这类 statement wrapper 在子表达式 lowering 已通过 `Perform` 等 terminator 终结当前块后，仍会继续覆盖 terminator 或追加伪语句；现已统一在这些包装层检测 `current_is_terminated()` 并即时停止，恢复 return-position / initializer-position `Perform` 的正规 MIR 形状。
  - 已在 `crates/scoopc/src/monomorph/lower.rs` 新增单测 `monomorph_preserves_perform_metadata_and_arg_order_in_instantiated_body`，确认 generic 函数实例化后的 MIR 仍稳定保留 `Perform` terminator、payload canonicalization 顺序、`source_arg_index` 与 `PerformResult` provenance。
  - review 结论：generic early MIR template 的调用与 control-transfer 入口已经成型；`T5000dR` 可以进入总边界复核，无需再先补一轮表示层缺口。
  - 已验证 `cargo fmt --all`、`cargo test -p scoopc monomorph::lower -- --nocapture`、`cargo run -p scoop -- test --fixtures tests/fixtures/mir`、`cargo test --all`、`cargo clippy --all-targets -- -D warnings` 全部通过。

### [DONE] T5000dR Review：确认 generic early MIR / ANF template 的语义边界正确
- 重点：
  - MIR 是否仍保持 backend-agnostic；
  - 是否只表达“语言/运行时抽象事实”，没有提前混入 LLVM 落地细节；
  - generic template 与后续 monomorphic instance 的边界是否清楚。
- 验收：
  - 可以明确回答“这层 MIR 负责什么，不负责什么”，且它还没有越权承担 backend 细节。
- 依赖：T5000d3R
- 完成记录（2026-04-26）：
  - 已复核 `crates/scoopc/src/mir/mod.rs`、`crates/scoopc/src/mir/lower.rs`、`crates/scoopc/src/hir/lower/mod.rs`、`crates/scoopc/src/monomorph/lower.rs` 与 `crates/scoop/src/commands/dump_mir.rs`，确认 generic early MIR / ANF template 当前只承载显式 CFG、locals、ANF 风格 operand/materialization、以及 call / perform / resume / pattern / member-access 等语言级事实；没有混入 LLVM statepoint/address space/stackmap、mangled symbol、vtable slot / itable id、GC ABI 或 runtime thunk 细节。
  - review 过程中暴露并修复了一个既有边界泄漏：`crates/scoopc/src/hir/lower/mod.rs` 中 `lower_typed_for_dump(...)` 虽未传入 `monomorph_keys`，但共用的 compilation-unit lowering 入口仍会从初始 HIR body 做 fixed-point 扫描并 materialize standalone generic fun 的 `::<T...>` 实例，导致 `dump-mir` 把 generic template 与 monomorphic item 混在一起；现已抽出 `lower_for_compilation_unit_multi_files_internal(...)`，并让 dump 路径显式关闭该实例物化。
  - 已更新 `crates/scoopc/src/mir/mod.rs`、`crates/scoopc/src/mir/lower.rs` 与 `crates/scoop/src/commands/dump_mir.rs` 的顶部说明，明确 generic template 的职责上界，以及它与后续 monomorphized MIR instance 的分层关系。
  - 已新增回归单测 `mir::tests::dump_mir_keeps_generic_functions_as_templates_before_monomorphization`，确认 `dump-mir` 继续保留 generic fun 的裸 `fqn` 与 `TypeKind::Param`，不会提前输出 `::<...>` 实例；同时 `crates/scoopc/src/monomorph/lower.rs` 现有单测继续证明实例化后的 `dump-ir` 路径仍保留 `Virtual` / `Perform` 等结构化 MIR 语义。
  - review 结论：generic early MIR / ANF template 的语义边界现已清楚可答复，即“负责 backend-agnostic 的语言/运行时抽象事实模板，不负责实例身份物化与 backend 落地细节”；下一条可进入 `T5000e`。
  - 已验证 `cargo fmt --all`、`cargo test -p scoopc mir::tests::dump_mir_keeps_generic_functions_as_templates_before_monomorphization -- --nocapture`、`cargo run -p scoop -- test --fixtures tests/fixtures/mir`、`cargo test -p scoopc monomorph::lower -- --nocapture`、`cargo test --all`、`cargo clippy --all-targets -- -D warnings` 全部通过。

### T5000e 在 MIR 层实现 monomorphization / instance materialization

### [DONE] T5000e1 引入 `InstanceKey`，并把 `dump-ir` 路径迁到真正的 MIR template → instance materialization
- 范围：
  - 引入 backend-agnostic 的 `InstanceKey`，明确区分“typecheck 收集到的实例请求”与“实例本身的稳定身份”；
  - 在 MIR 侧实现单文件/调试路径可用的 generic template → monomorphic instance materializer，而不是继续对每个实例回到 HIR 重新 lowering；
  - 支持最小可用的 reachable-driven / on-demand fixed-point：至少覆盖 standalone generic direct call、实例内继续发现的 direct generic call，以及 nested closure family 的实例物化；
  - 建立 per-`InstanceKey` cache，保证同一实例只 materialize 一次。
- 验收：
  - `dump-ir` 输出的 monomorphic MIR instances 具有独立于 mangled symbol name 的 `InstanceKey`；
  - materialized instance body 内的 generic `DirectCall` 会改写为对应实例，而不是保留 generic template callee；
  - 现有 `monomorph` 调试回归改为验证“基于 generic MIR template 的实例化”，而不是“再做一次 HIR lowering”。
- 依赖：T5000dR
- 完成记录（2026-04-26）：
  - 新增 `crates/scoopc/src/mir/materialize.rs`，在 MIR 层引入 `TemplateKey` / `InstanceKey` / `MaterializedMir` 与 `materialize_for_dump(...)`，并基于 generic MIR template 实现单文件 dump 路径的 instance materialization；
  - `crates/scoopc/src/monomorph/lower.rs` 已收口为兼容薄包装；`crates/scoop/src/commands/dump_ir.rs` 改为直接打印新的 `MaterializedMir` Debug 视图，从而暴露实例键与实例文件；
  - 新 materializer 已覆盖 standalone generic direct-call fixed-point、nested closure family FQN/fn_ptr 重写、以及 per-`InstanceKey` cache；`MonomorphKey` 现明确退回为“typecheck 收集到的实例请求”语义；
  - 已新增/更新回归测试：`monomorph_collects_two_instances_for_id`、`monomorph_discovers_direct_call_fixed_point_in_mir_instances`、`monomorph_rewrites_nested_closure_family_fn_ptrs`、`monomorph_preserves_virtual_call_kind_in_instantiated_body`、`monomorph_preserves_perform_metadata_and_arg_order_in_instantiated_body`；
  - 已验证 `cargo fmt --all`、`cargo test -p scoopc monomorph::lower -- --nocapture`、`cargo test -p scoopc mir::tests::dump_mir_keeps_generic_functions_as_templates_before_monomorphization -- --nocapture`、`cargo test --all`、`cargo clippy --all-targets -- -D warnings` 全部通过。

### [DONE] T5000e1a 为 `dump-ir` materializer 补齐跨文件 / sysroot generic template 目录与正确的声明源身份
- 范围：
  - 修正 typecheck 收集的 `MonomorphKey.symbol`，让 imported / sysroot generic fun 使用真实 `decl_file` / `decl_span`，而不是把调用点文件误记为声明源；
  - 为 `dump-ir` 的 MIR materializer 建立至少覆盖输入源文件 + sysroot/imported generic fun 的 template catalog，避免 direct generic call 在单文件调试路径上因“只索引当前源文件 template”而失败；
  - 保持当前任务边界仍在 dump/debug 路径，不提前把 build/frontend 主路径的整体迁移混进来；编译单元主路径迁移仍留待 `T5000e2`。
- 验收：
  - `scoop dump-ir` 对 `scoop.core.print<T>`、`channelCreate<T>` 等外部 generic fun 的 direct call 能稳定 materialize 对应 MIR instance，而不是报 `missing_generic_template`；
  - 请求键中的声明源身份与 template lookup 已不再依赖“generic fun 必须定义在当前输入文件”这一错误假设。
- 依赖：T5000e1
- 完成记录（2026-04-26）：
  - 已在 `crates/scoopc/src/typecheck/lower.rs` 调整 `record_monomorph_call(...)` 签名，并在 `crates/scoopc/src/typecheck/expr/call.rs` 的全部泛型 direct/member/extension 调用路径上传入真实 `sig.decl_file`；`MonomorphKey.symbol` 不再把 imported / sysroot generic fun 误记成调用点文件声明。
  - 已重写 `crates/scoopc/src/mir/materialize.rs` 的 dump/debug 前端准备流程：`materialize_for_dump(...)` 现在会为当前输入文件、`session.sysroot().compilable_source_paths` 中的可编译 sysroot 源，以及 `sysroot` 声明文件建立统一 index / resolve / typecheck / lowering 上下文，再用整组文件的 generic HIR/MIR template 驱动实例化，而不再只 lower 当前源文件。
  - `collect_generic_template_infos(...)` 现已按整组 prepared files 收集 template catalog，并纳入 declaration-only generic fun；因此 `scoop.unsafe.stackAlloc<T>` 这类只在 sysroot 声明文件里出现的 generic fun 也能生成稳定的 `TemplateKey` / `InstanceKey`，不再触发 `missing_generic_template`。
  - 由于 `sysroot/print.scoop`、`sysroot/task.scoop` 等可编译 sysroot 源依赖 `stdlib/*.scoop`，本轮一并把 dump/debug support sources 扩到 `stdlib + compilable sysroot`；这是为了让外部 template 在调试路径上可 resolve/typecheck/lower，并未把 `T5000e2` 的编译单元主路径迁移提前并入。
  - 已新增回归测试 `monomorph_materializes_compilable_sysroot_generic_template` 与 `monomorph_materializes_declaration_only_sysroot_generic_template`，分别覆盖 `scoop.core.print::<Int>` 与 `scoop.unsafe.stackAlloc::<Int>` 的实例化。
  - 已验证 `cargo fmt --all`、`cargo test -p scoopc monomorph::lower -- --nocapture`、`cargo run -q -p scoop -- dump-ir <tmp print case>`、`cargo run -q -p scoop -- dump-ir <tmp stackAlloc case>`、`cargo test --all`、`cargo clippy --all-targets -- -D warnings` 全部通过。

### [DONE] T5000e1aR Review：确认 dump-ir 单文件路径的 template identity 已脱离“仅当前源文件”假设
- 重点：
  - imported / sysroot generic fun 的请求键是否指向真实声明源；
  - dump-ir template catalog 是否已覆盖调试路径上可达的外部 generic template，而不是继续只扫当前源文件；
  - 该补丁是否仍保持 e1 边界，即只修 dump/debug materializer，不提前把 e2 的 build/frontend 主路径一起耦合进来。
- 验收：
  - 单文件/调试路径上的 template identity 已足够稳定，不再因外部 generic direct call 而直接报缺模板。
- 依赖：T5000e1a
- 完成记录（2026-04-26）：
  - review 过程中先暴露并修复了一个现存 fixed-point 缺口：当 entry 文件中的 generic fun 实例体继续 direct-call 外部 generic fun（例如 `wrap<T>(value: T) { print(value) }` + `wrap(1)`）时，`dump-ir` 先前会错误 seed 出 `scoop.core.print::<T>` 这类非具体实例请求，并在 `wrap::<Int>` 体内保留 generic `callee_fqn: "scoop.core.print"`；现已在 `crates/scoopc/src/mir/materialize.rs` 中引入基于 `(fqn, decl_file, decl_span)` 请求键与归一化签名的 canonical template 选择，优先收口到 body-bearing root，并把 template family 缩到“当前 root + 其 lambda family”，从而恢复外部 generic direct-call 的具体实例 fixed-point；
  - `seed_requests(...)` 现会过滤仍含 type-param / effect-param 的非具体实例请求，避免把 generic template body 的 typecheck 请求误当成 monomorphic roots；外部 generic direct-call 的具体实例改由 materialized instance body 的 fixed-point 发现路径补齐；
  - 已新增回归测试 `monomorph_rewrites_external_generic_calls_to_concrete_instances`，确认 `wrap::<Int>` 的 body 会把 `print(value)` 重写到 `scoop.core.print::<Int>`，且 materializer 不再输出 `scoop.core.print::<T>` 这类模板参数实例；CLI 复现 `cargo run -q -p scoop -- dump-ir <tmp wrap/print case>` 也已确认输出中的 `callee_fqn` 为 `scoop.core.print::<Int>`；
  - 已复核 `record_monomorph_call(...)` 的 `decl_file/decl_span` 传递链、dump/debug prepared-files template catalog、以及新的 canonical root/family 逻辑，确认本轮改动仍局限在 `dump-ir` materializer 与调试回归，没有提前把 `T5000e2` 的编译单元 frontend/build 主路径接线并入；
  - review 结论：单文件/调试路径上的 template identity 已脱离“generic template 必须定义在当前输入文件”与“声明/实现双份 generic fun 会破坏 fixed-point”这两类错误假设，下一条可进入 `T5000e1b`；
  - 已验证 `cargo fmt --all`、`cargo test -p scoopc monomorph::lower -- --nocapture`、`cargo test -p scoopc mir::tests::dump_mir_keeps_generic_functions_as_templates_before_monomorphization -- --nocapture`、`cargo run -q -p scoop -- dump-ir <tmp wrap/print case>`、`cargo test --all`、`cargo clippy --all-targets -- -D warnings` 全部通过。

### [DONE] T5000e1b 让 `InstanceKey` / dump-ir materializer 正确承载 effect-row 实参
- 范围：
  - 让 monomorph 请求收集记录 `<eff ...>` 实参，不再把 `eff_args` 留空，也不再因为 `type_args` 为空而跳过 effect-only generic fun 的实例请求；
  - 让 generic HIR/MIR template 与 MIR materializer 保留并应用 effect-row 参数绑定，避免 `<eff E>` 在 dump 路径中退化为默认 row 或 `Any`；
  - 让 `InstanceKey` 的显示名、实例缓存与 direct-call fixed-point 发现共同区分 `eff_args`，避免同一 type args 下不同 effect 实例发生身份碰撞。
- 验收：
  - effect-only generic fun 在 `dump-ir` 路径上会生成对应实例，而不是返回空实例集；
  - 同一 generic template 在相同 type args、不同 `eff_args` 下拥有可区分的实例身份与 materialized callee。
- 依赖：T5000e1aR
- 完成记录（2026-04-26）：
  - 已在 `crates/scoopc/src/ast/mod.rs`、`crates/scoopc/src/typecheck/lower.rs` 与 `crates/scoopc/src/typecheck/expr/call.rs` 接通 `eff_args` side table：top-level function value / call binding 与 `MonomorphKey` 请求现在都会记录真实 effect-row 实参和声明源身份；effect-only generic fun 不再因 `type_args.is_empty()` 被漏掉，显式 `<eff ...>` 也会优先进入实例请求。
  - 已在 `crates/scoopc/src/parser/expr.rs` 修复表达式级 `<eff ...>` lookahead / type-apply 扫描，使 `forward<eff Boom>` 这类写法不会再被误当成比较表达式；`crates/scoopc/src/typecheck/expr/call.rs` 现能从 `TypeApply` callee 和 top-level function value 路径提取显式 `eff_arg`。
  - 已在 `crates/scoopc/src/typecheck/lower.rs`、`crates/scoopc/src/typecheck/expr/{entry,stmt}.rs` 把 effect-row 形参绑定改为 marker-preserving 语义；`<eff E>` 在 typecheck / HIR lowering / template substitution 路径上不再塌缩成默认 `Pure`，而是保留为可替换的 effect-row 参数。
  - 已在 `crates/scoopc/src/hir/{mod.rs,lower/mod.rs,lower/util.rs,lower/expr.rs}` 与 `crates/scoopc/src/mir/materialize.rs` 完成 effect-row 模板闭环：
    - HIR generic template 会保留 effect-row 参数 marker；
    - `InstanceKey`、`instance_fqn(...)`、site binding、instance substitution、effect-row substitution、direct-call fixed-point 与 per-instance cache 现在都区分 `eff_args`；
    - top-level function value / direct call 都能把同 type args、不同 effect row 的实例 materialize 成不同 callee。
  - `crates/scoopc/src/mir/materialize.rs` 本轮还顺手收口了 `DumpMaterializeRequestSet` / `RewriteContext`，消除了 `clippy` 的 `too_many_arguments` 与 `collapsible_if` 问题；`crates/scoopc/src/typecheck/lower.rs` 的 `record_top_level_fun_call_binding(...)` 也已改成直接接收结构体绑定，避免继续堆参数。
  - 已新增回归测试：
    - `monomorph_materializes_effect_only_generic_instance`
    - `monomorph_distinguishes_same_type_args_with_different_effect_rows`
    - `monomorph_rewrites_top_level_fun_value_effect_instance`
  - 已验证：
    - `cargo fmt --all`
    - `cargo check -p scoopc`
    - `cargo clippy --all-targets -- -D warnings`
    - `cargo test -p scoopc monomorph::lower -- --nocapture`
    - `cargo test --all`
    - 全部通过。

### [DONE] T5000e1b0a 修复 effect-generic extension/member direct-call 的 request binding 与显式 type-apply 透传
- 范围：
  - 让 `x.ext<eff E>()` / `obj.method<T>()` 这类 call-callee 位置的 `TypeApply` 在 HIR lowering 中继续走 extension/member direct-call 降糖，而不是退回成员值 / `FunValue` 路径；
  - 让 extension/member direct-call 的 typecheck side table 写回 `TopLevelFunCallBinding { decl_file, decl_span, type_args, eff_args }`，使 materializer fixed-point 能恢复具体实例请求；
  - 让扩展函数与直连成员方法的签名收集 / 调用点推断不再在 request binding 阶段丢掉 `eff_param` 与显式 `eff_arg`。
- 验收：
  - effect-generic extension call 在 `dump-ir` 路径中会把 nested direct call 重写到带 `eff_args` 的 concrete callee，而不是退回 `Pure` 或 generic FQN；
  - extension/member direct-call 路径的 request binding 现在都能携带真实 `eff_args`。
- 依赖：T5000e1b
- 完成记录（2026-04-26）：
  - 已在 `crates/scoopc/src/hir/lower/expr.rs` 新增 `transparent_call_callee(...)`，把 call 位置的 `TypeApply` 视为透明外壳；`x.forward<eff E>()` 之类 extension/member 调用不再错误落到 `MemberAccess + FunValue` 路径；
  - 已在 `crates/scoopc/src/typecheck/expr/call.rs` 为 extension/member direct-call 补齐 `record_top_level_fun_call_binding(...)`，并让扩展调用的 single-candidate / overload 两条路径优先消费显式 `eff_arg`；`eff_args` 现在会一路进入 request binding，而不是在 direct-call fixed-point 前被塌缩成 `Pure`；
  - 已在 `crates/scoopc/src/typecheck/expr/ops.rs` 让直连成员方法签名收集保留 `eff_param`、默认 effect binding、`param_*_eff_base` 与 `param_eff_row_var_subst`，从而不再在 typecheck 阶段先天丢失成员方法的 effect-row 事实；
  - 已在 `crates/scoopc/src/mir/materialize.rs` 扩展 generic template catalog，使其能够为 type-body / companion object 内的 generic member fun 建立 request lookup key 与 canonical signature groundwork，而不会继续只看顶层 `fun`；
  - 已新增回归测试 `monomorph_rewrites_effect_generic_extension_call_to_concrete_instance`，确认 effect-generic extension call 会 materialize 到 `forward::<eff fixtures.monomorph.Boom>`，且 nested call 会重写成 concrete direct callee；
  - 继续 probing 时暴露出新的更深阻塞：`cargo run -q -p scoop -- dump-mir <member-effect-generic case>` 仍把 type declarations 输出为 `Todo { kind: "type" }`，generic MIR file 里没有 `fixtures.monomorph.Box.forward` root，因此 member method 的 monomorphic materialization 仍需后续前置任务 `T5000e1b0b` 补齐；本条任务只负责修正 request binding / call-site 透传层面的 `eff_args` 缺口。
  - 已验证：
    - `cargo fmt --all`
    - `cargo check -p scoopc`
    - `cargo test -p scoopc monomorph_ -- --nocapture`
    - `cargo clippy --all-targets -- -D warnings`
    - 全部通过。

### [DONE] T5000e1b0a1 修复 effect-generic member direct-call 对 lambda 实参的 overload matching / `eff_arg` 推断闭环
- 范围：
  - 修复 `class Box() { fun <eff E = Pure> lift(f: () -> Int / E): Int / E { ... } }` 这类成员方法在 typed receiver 路径上的 direct-call typecheck；
  - 让 `val box: Box = Box(); box.lift({ perform Boom.ping(); 1 })` 不再在 member direct-call 分支因 `NoMatchingOverload` 提前丢弃候选；
  - 确保 member direct-call 在 lambda expected-context typecheck、`eff_arg` 推断、`instantiate_eff_row_var_in_sig_types(...)` 与最终 assignability 检查之间形成和顶层/扩展函数一致的闭环，而不是只收集了 `param_*_eff_base` / subst facts 却没真正消费成功。
- 验收：
  - 上述 typed receiver + lambda case 能通过 typecheck；
  - 对应 `TopLevelFunCallBinding` / monomorph key 会保留非 `Pure` 的 `eff_args`；
  - 不回退当前已验证通过的 extension/member 显式 `<eff E>` direct-call 路径。
- 依赖：T5000e1b0a
- 完成记录（2026-04-26）：
  - 继续定位后确认，阻塞 `box.lift({ perform Boom.ping(); 1 })` 的真正既有缺口并不是 member overload matcher 本身，而是 spec-correct 的显式 `perform` 语法尚未进入 parser 表达式前缀：`perform Boom.ping()` 先被落成 `StmtKind::Missing`，导致 lambda expected-context typecheck 提前报 `block expression（missing stmt）`，成员候选因此被误丢弃；
  - 已在 `crates/scoopc/src/parser/expr.rs` 为 `perform E.op(...)` 补齐前缀解析，并把它按 effect-op call 的源码级语法糖接入现有 typecheck/HIR lowering 主线，从而不再要求把该 case 改写成非 spec 代表形状；
  - 已新增 `typecheck::expr::infer::tests::member_direct_call_infers_effect_row_from_lambda_with_explicit_perform`，确认 typed receiver 成员 direct-call 现在能从显式 `perform` 的 lambda body 推断出非 `Pure` `eff_arg`；
  - 已新增 `mir::materialize::tests::dump_materialization_inputs_keep_eff_args_for_member_direct_call_binding_from_lambda`，确认对应 `TopLevelFunCallBinding` / monomorph key 都会保留非 `Pure` 的 `eff_args`；
  - 已回归 `typed_hir_keeps_effect_generic_member_type_apply_on_direct_call_path` 与 `monomorph_rewrites_effect_generic_extension_call_to_concrete_instance`，确认显式 `<eff E>` member direct-call 与 extension direct-call 路径没有回退；
  - 已验证：
    - `cargo fmt --all`
    - `cargo test -p scoopc member_direct_call_infers_effect_row_from_lambda_with_explicit_perform -- --nocapture`
    - `cargo test -p scoopc dump_materialization_inputs_keep_eff_args_for_member_direct_call_binding -- --nocapture`
    - `cargo test -p scoopc typed_hir_keeps_effect_generic_member_type_apply_on_direct_call_path -- --nocapture`
    - `cargo test -p scoopc monomorph_rewrites_effect_generic_extension_call_to_concrete_instance -- --nocapture`
    - `cargo test --all`
    - `cargo clippy --all-targets -- -D warnings`
    - 全部通过。

### [DONE] T5000e1b0a1R Review：确认 member direct-call 已真正消费 lambda-derived effect-row facts
- 重点：
  - member direct-call 分支是否已经不再“先按默认 `Pure` expected type 过滤掉 lambda 实参候选，再去推 `eff_arg`”；
  - `collect_member_method_signatures_from_index(...)` 产出的 `eff_param` / `param_fn_effect_eff_base` / `param_nominal_eff_eff_base` / `param_eff_row_var_subst` 是否已被调用点闭环消费，而不是停留在 side table；
  - typed receiver / lambda 推断路径修复后，显式 `<eff E>` 与 extension direct-call 回归是否仍保持通过。
- 验收：
  - effect-generic member method 的 lambda 实参路径已经具备与顶层/扩展函数相同的 `eff_arg` 推断能力。
- 依赖：T5000e1b0a1
- 完成记录（2026-04-26）：
  - 已复核 `crates/scoopc/src/typecheck/expr/call.rs` 的 member direct-call 单候选与多候选路径，确认两条路径都会先对 lambda 实参做 expected-context typecheck，再消费 `param_nominal_eff_eff_base` / `param_fn_effect_eff_base` 推断 `eff_arg`，随后通过 `instantiate_eff_row_var_in_sig_types(...)` 回填签名并执行最终 assignability 检查，而不是在默认 `Pure` 形态下提前淘汰候选；
  - 已确认 `collect_member_method_signatures_from_index(...)` 产出的 `eff_param`、`param_*_eff_base` 与 `param_eff_row_var_subst` 并未停留在 side table：member direct-call 分支会直接消费这些事实来决定 receiver 是否依赖 `E`、从 lambda / nominal 参数提取 effect-row 增量，并完成实例化后的 receiver/arg 复检；
  - 已新增回归测试 `typecheck::expr::infer::tests::member_direct_call_overload_keeps_effect_generic_lambda_candidate_alive`，确认在存在其它成员重载候选时，`box.lift({ perform Boom.ping(); 1 })` 仍不会因默认 `Pure` expected type 而过早丢弃 effect-generic lambda 候选；
  - 已回归 `member_direct_call_infers_effect_row_from_lambda_with_explicit_perform`、`dump_materialization_inputs_keep_eff_args_for_member_direct_call_binding{,_from_lambda}`、`typed_hir_keeps_effect_generic_member_type_apply_on_direct_call_path` 与 `monomorph_rewrites_effect_generic_extension_call_to_concrete_instance`，确认 typed receiver / lambda 推断修复后，显式 `<eff E>` 与 extension direct-call 路径仍保持通过；
  - 已验证：
    - `cargo fmt --all`
    - `cargo test -p scoopc member_direct_call_ -- --nocapture`
    - `cargo test -p scoopc typed_hir_keeps_effect_generic_member_type_apply_on_direct_call_path -- --nocapture`
    - `cargo test -p scoopc monomorph_rewrites_effect_generic_extension_call_to_concrete_instance -- --nocapture`
    - `cargo test --all`
    - `cargo clippy --all-targets -- -D warnings`
    - 全部通过。

### [DONE] T5000e1b0aR Review：确认 extension/member direct-call 已不再在 request binding 阶段丢失 `eff_args`
- 重点：
  - call-callee 位置的 `TypeApply` 是否已不会把 extension/member call 重新打回成员值 / `FunValue` 路径；
  - extension/member direct-call 的 `TopLevelFunCallBinding` 是否已稳定携带 `decl_file` / `decl_span` / `type_args` / `eff_args`；
  - 直连成员方法签名收集是否已保留 `eff_param` 与 effect-row subst facts，而不是继续在 typecheck 入口处先天丢失。
- 验收：
  - extension/member direct-call 在 request binding 与调用点正规化层面已经具备与顶层函数相同的 `eff_args` 承载能力。
- 依赖：T5000e1b0a1R
- 完成记录（2026-04-26）：
  - 已复核 `crates/scoopc/src/hir/lower/expr.rs`、`crates/scoopc/src/typecheck/expr/call.rs` 与 `crates/scoopc/src/typecheck/expr/ops.rs`：call-callee 位置的 `TypeApply` 现会继续走 member / extension direct-call 主线；顶层 / member / extension direct-call 的单候选与多候选路径都会写回带 `decl_file` / `decl_span` / `type_args` / `eff_args` 的 `TopLevelFunCallBinding`；`collect_member_method_signatures_from_index(...)` 继续保留并输出 `eff_param`、`param_*_eff_base` 与 `param_eff_row_var_subst`；
  - review 过程中额外暴露并修复了一个既有 safe-call 缺口：nullable receiver 的 direct member call 先前在 `member.resolved == None` 时不会做 late resolution，`box?.forward()` / `box?.forward<eff E>()` 会在 `dump-mir` 路径报 `callee_not_callable`；现已在 `crates/scoopc/src/typecheck/expr/call.rs` 扩大 direct member late resolution 触发条件，并在 `crates/scoopc/src/hir/lower/expr.rs` 让 `TypeApply(SafeMemberAccess(...))` 继续命中 `lower_safe_call_expr(...)`，不再把 safe-call 误丢到普通 callee lowering；
  - 已新增 `hir::lower::tests::typed_hir_lowers_safe_member_type_apply_as_safe_direct_call`，确认 safe member direct-call + `TypeApply` 会被 typed HIR lowering 保持在 safe-call desugar + direct-call 主线；已新增 `mir::materialize::tests::dump_materialization_inputs_keep_eff_args_for_extension_direct_call_binding`，直接锁定 extension direct-call 的 `TopLevelFunCallBinding` / monomorph key 会保留非 `Pure` `eff_args`；
  - 已回归 `dump_materialization_inputs_keep_eff_args_for_member_direct_call_binding{,_from_lambda}`、`typed_hir_keeps_effect_generic_member_type_apply_on_direct_call_path`、`monomorph_rewrites_effect_generic_extension_call_to_concrete_instance`，并额外用 CLI 复现确认 `/tmp/safe_member_plain.scoop` 与 `/tmp/safe_member_type_apply.scoop` 的 `dump-mir` 路径均不再报 `callee_not_callable`；
  - 已验证：
    - `cargo fmt --all`
    - `cargo test -p scoopc typed_hir_ -- --nocapture`
    - `cargo test -p scoopc dump_materialization_inputs_keep_eff_args_for_ -- --nocapture`
    - `cargo test -p scoopc monomorph_rewrites_effect_generic_extension_call_to_concrete_instance -- --nocapture`
    - `cargo run -q -p scoop -- dump-mir /tmp/safe_member_plain.scoop`
    - `cargo run -q -p scoop -- dump-mir /tmp/safe_member_type_apply.scoop`
    - `cargo test --all`
    - `cargo clippy --all-targets -- -D warnings`
    - 全部通过。

### [DONE] T5000e1b0b 让 generic MIR template / dump-ir 收录 type-body generic member fun roots
- 范围：
  - 让 `dump-mir` / generic MIR lowering 把 type-body / companion object 内的 generic member fun 作为真正的 MIR template root 发射，而不是继续把整段 type decl 记成 `Todo { kind: "type" }`；
  - 让 materializer 能为 `Owner.method` 这类 type-body generic fun 找到对应 root/family，并完成 member direct-call 的 monomorphic fixed-point；
  - 用用户态回归覆盖 `class Box { fun <eff E> forward() }` + `wrap<eff Boom>(box.forward<eff E>())` 这类路径，确认 member method 也能生成带 `eff_args` 的 concrete callee。
- 验收：
  - `cargo run -q -p scoop -- dump-ir <member-effect-generic case>` 不再报 `missing_mir_root_for_template`；
  - type-body generic member fun 能进入 `InstanceKey` / materializer 闭环。
- 依赖：T5000e1b0aR
- 完成记录（2026-04-26）：
  - 已在 `crates/scoopc/src/mir/lower.rs` 扩展 dump 路径 MIR lowering 入口，使其同时 lowering `hir::File` 顶层 item 与 `member_funs` side table；type/object 顶层 `Todo` 占位仍保留，但 type-body / companion object member fun 现在会额外发射真实 MIR root；
  - 已在 `crates/scoopc/src/mir/materialize.rs` 把 `lowered_hir.member_funs` 接入 generic MIR lowering，materializer 的 template root 匹配、family 收集与 member direct-call fixed-point 现已覆盖 `Owner.method` 这类 type-body generic member fun；
  - 已在 `crates/scoopc/src/cone/pre_specialize.rs` 对齐新的 MIR lowering 入口，预特化单函数路径显式传入空 `member_funs` 切片，避免旧调用面编译失败；
  - 已新增回归测试 `mir::lower::tests::dump_mir_emits_type_body_generic_member_fun_roots` 与 `mir::materialize::tests::materialize_for_dump_handles_type_body_generic_member_fun_roots`，分别锁定 generic MIR root 发射与 `InstanceKey` / concrete callee 闭环；
  - 已用 CLI 复现确认 `/tmp/t5000e1b0b_member_root.scoop` 的 `dump-mir` 输出现在包含 `fixtures.monomorph.Box.forward` root，`dump-ir` 不再报 `missing_mir_root_for_template`，并会 materialize `fixtures.monomorph.Box.forward::<eff fixtures.monomorph.Boom>`；
  - 已验证：
    - `cargo fmt --all`
    - `cargo test -p scoopc type_body_generic_member_fun_roots -- --nocapture`
    - `cargo test -p scoopc dump_materialization_inputs_keep_eff_args_for_member_direct_call_binding -- --nocapture`
    - `cargo run -q -p scoop -- dump-ir /tmp/t5000e1b0b_member_root.scoop`
    - `cargo run -q -p scoop -- dump-mir /tmp/t5000e1b0b_member_root.scoop`
    - `cargo test --all`
    - `cargo clippy --all-targets -- -D warnings`
    - 全部通过。

### [DONE] T5000e1b0bR Review：确认 type-body generic member fun 已进入 generic MIR template → instance materialization 主线
- 重点：
  - `dump-mir` 是否已为 generic member fun 发射真实 MIR root，而不是停留在 `Todo { kind: "type" }`；
  - template catalog / canonical lookup / instance cache 是否已经同时覆盖顶层 fun、extension fun 与 type-body member fun；
  - member method 的 effect-row 实参是否真正进入 concrete instance identity，而不是只在 call-site binding 层保存。
- 验收：
  - `InstanceKey` / materializer 对 member method 的支持与顶层/扩展函数处于同一层次，`T5000e1bR` 可以继续做总复核。
- 依赖：T5000e1b0b
- 完成记录（2026-04-26）：
  - review 先暴露并修复了一个真实前置缺陷：`dump-mir` / `dump-ir` 的 typed dump 路径原本会把 `TypeName.member()` 的 companion dispatch receiver 当成普通值表达式去 typecheck，导致 `Box.forward()` / `Box.forward<eff E>()` 直接报 `scoop::typecheck::unsupported_expr`（`ident（未 resolve）`）；现已在 `crates/scoopc/src/typecheck/expr/call.rs` 把 unresolved type receiver 收口为 companion object nominal receiver，并在 `crates/scoopc/src/hir/lower/expr.rs` 把 companion member direct-call 改写为携带显式 companion singleton receiver 的顶层 direct-call；
  - 已新增 `hir::lower::tests::typed_hir_lowers_companion_member_type_apply_as_direct_call`、`mir::lower::tests::dump_mir_emits_companion_generic_member_fun_roots`、`mir::materialize::tests::materialize_for_dump_distinguishes_companion_member_fun_effect_instances`，分别锁定 companion member direct-call 降糖、generic MIR root 发射，以及同一 companion member fun 在不同 effect row 下的独立 `InstanceKey` / concrete MIR instance；
  - 复核后确认 template catalog / canonical lookup / instance cache 现已在同一层处理顶层 fun、extension fun、type-body member fun 与 companion member fun；`Box.Companion.forward::<eff Boom>` / `::<eff Zap>` 会作为不同实例身份稳定 materialize；
  - `dump-mir` 的 member-fun 发射范围现与 `member_funs` side table 对齐，因此补充更新了 `tests/fixtures/mir/dispatch_and_resume_call.mir`，以反映 type/effect/interface body member fun 作为真实 MIR root 输出的现状；
  - 已用 CLI 复现确认：
    - `/tmp/t5000e1b0br_companion_plain.scoop` 的 `cargo run -q -p scoop -- dump-mir ...` 不再报 unresolved ident，并会把 `Box.forward()` 降成 `fixtures.review.Box.Companion.forward` 的 direct call；
    - `/tmp/t5000e1b0br_companion_member.scoop` 的 `cargo run -q -p scoop -- dump-mir ...` / `dump-ir ...` 现在都会通过，并会 materialize `fixtures.review.Box.Companion.forward::<eff fixtures.review.Boom>` 与 `::<eff fixtures.review.Zap>`；
  - 已验证：
    - `cargo fmt --all`
    - `cargo test -p scoopc companion -- --nocapture`
    - `cargo run -q -p scoop -- dump-mir /tmp/t5000e1b0br_companion_plain.scoop`
    - `cargo run -q -p scoop -- dump-mir /tmp/t5000e1b0br_companion_member.scoop`
    - `cargo run -q -p scoop -- dump-ir /tmp/t5000e1b0br_companion_member.scoop`
    - `cargo run -q -p scoop -- test --fixtures tests/fixtures/mir`
    - `cargo test --all`
    - `cargo clippy --all-targets -- -D warnings`
    - 全部通过。

### [DONE] T5000e1bR Review：确认 effect-row 实参已成为 `InstanceKey` / materializer 的一等维度
- 重点：
  - `eff_args` 是否已经从 typecheck 请求一路进入 `InstanceKey`、template substitution、instance cache 与 Debug 输出；
  - HIR/MIR template 是否仍然保存 effect-row 参数语义，而不是在 lowering 时提前塌缩成默认 row / `Any`；
  - effect-only generic fun 与“同 type args 不同 effect row”的实例是否都能稳定区分。
- 验收：
  - `InstanceKey` 与 dump-ir materializer 的 effect-row 维度已经闭环，e1R 可以继续审查总体边界。
- 依赖：T5000e1b0bR
- 完成记录（2026-04-26）：
  - 已复核 `crates/scoopc/src/monomorph/mod.rs`、`crates/scoopc/src/typecheck/lower.rs`、`crates/scoopc/src/ast/mod.rs`、`crates/scoopc/src/hir/lower/{mod.rs,expr.rs}` 与 `crates/scoopc/src/mir/materialize.rs`，确认 `MonomorphKey` 请求、`TopLevelFunCallBinding` / `TopLevelFunValueRef`、`SiteInstanceBinding`、`InstanceKey`、`instance_fqn(...)`、`build_instance_substitution(...)`、`substitute_type_and_effect_params_in_effect_row(...)` 与 `materialized: HashMap<InstanceKey, ...>` 缓存都把 `eff_args` 当作实例身份的一部分处理，而不是只在调用点 side table 中短暂保存；
  - 已确认 HIR/MIR template 仍保留 effect-row 参数语义：`crates/scoopc/src/hir/lower/mod.rs` 继续通过 `push_effect_row_param_placeholder(...)` 把 `<eff E>` lowering 为 `EFFECT_ROW_PARAM_DECL_FILE` marker，`lower_effect_row_expr(...)` 与 `substitute_type_and_effect_params_in_effect_row(...)` 会在模板阶段保留、在实例化阶段再展开该 marker，没有把 row 提前塌缩成默认 `Pure` 或 `Any`；
  - 已复核 effect-only generic fun、相同 type args 不同 effect row、top-level function value、extension/member/lambda-derived member、type-body member 与 companion member 路径的现有回归，确认这些路径都会稳定区分非 `Pure` `eff_args`，并在 materialized MIR 中生成不同的 concrete callee / `InstanceKey`；
  - 已用 `wrap<Int, eff Boom>` / `wrap<Int, eff Zap>` 的 CLI probe 复核 `dump-ir` 用户可见输出，确认 `MaterializedMir.instance_keys` 会同时输出两个 distinct `InstanceKey`，且 `file.items` 中会 materialize `review.e1br.wrap::<Int, eff review.e1br.Boom>` 与 `::<Int, eff review.e1br.Zap>` 两个不同实例；
  - review 结论：effect-row 实参已经成为 `InstanceKey` / materializer 的一等维度，当前未发现需要插入到 `T5000e1R` 之前的新前置缺陷任务；下一条可进入 `T5000e1R Review：确认 InstanceKey 与 dump-ir materializer 的边界正确`。
  - 已验证：
    - `cargo test -p scoopc monomorph_materializes_effect_only_generic_instance -- --nocapture`
    - `cargo test -p scoopc monomorph_distinguishes_same_type_args_with_different_effect_rows -- --nocapture`
    - `cargo test -p scoopc monomorph_rewrites_top_level_fun_value_effect_instance -- --nocapture`
    - `cargo test -p scoopc dump_materialization_inputs_keep_eff_args_for_extension_direct_call_binding -- --nocapture`
    - `cargo test -p scoopc dump_materialization_inputs_keep_eff_args_for_member_direct_call_binding -- --nocapture`
    - `cargo test -p scoopc dump_materialization_inputs_keep_eff_args_for_member_direct_call_binding_from_lambda -- --nocapture`
    - `cargo test -p scoopc materialize_for_dump_handles_type_body_generic_member_fun_roots -- --nocapture`
    - `cargo test -p scoopc materialize_for_dump_distinguishes_companion_member_fun_effect_instances -- --nocapture`
    - `cargo test -p scoopc monomorph::lower -- --nocapture`
    - `cargo run -q -p scoop -- dump-ir <tmp review case>`
    - `cargo test --all`
    - `cargo clippy --all-targets -- -D warnings`
    - 全部通过。

### [DONE] T5000e1R Review：确认 `InstanceKey` 与 dump-ir materializer 的边界正确
- 重点：
  - `InstanceKey` 是否已与旧的 `MonomorphKey` 请求键语义分离；
  - `dump-ir` 是否已经改为消费 generic MIR template，而不是继续把 HIR 重新 lower 成实例；
  - per-`InstanceKey` cache 与 fixed-point 发现策略是否足够稳定，可作为编译单元接线前的基础。
- 验收：
  - 单文件/调试路径上的实例身份、模板索引与实例缓存已经明确落在 MIR 层，而不是旧 `monomorph` HIR 调试路径。
- 依赖：T5000e1bR
- 完成记录（2026-04-26）：
  - review 先暴露并修复了一个真实边界泄漏：`crates/scoopc/src/mir/materialize.rs` 仍通过 `hir::lower_for_compilation_unit_multi_files_with_type_env(...)` 构造 typed HIR，而该入口会继续启用 HIR 层的 standalone generic fun / owner-specialized member fun `::<...>` 实例物化；这与“dump-ir 消费 generic MIR template，再由 MIR materializer 负责实例化”的验收边界直接冲突。
  - 已在 `crates/scoopc/src/hir/lower/mod.rs` 新增 `lower_generic_for_compilation_unit_multi_files_with_type_env(...)` 与 `CompilationUnitLoweringOptions`，显式区分“完整编译单元 lowering”和“generic template only lowering”；`lower_typed_for_dump(...)` 与 `crates/scoopc/src/mir/materialize.rs` 现都关闭 HIR 层 standalone/member generic 实例物化，只保留 generic template roots。
  - 已新增 `mir::materialize::tests::generic_mir_template_for_dump_stays_free_of_hir_level_instances`，确认 typed HIR 与 generic MIR template 输入均不再混入 `::<...>` standalone/member roots；并新增 `mir::materialize::tests::materialize_for_dump_dedups_repeated_instance_requests`，确认 per-`InstanceKey` cache 会对重复请求去重，同一实例只 materialize 一次。
  - 已复核 `crates/scoopc/src/monomorph/{mod.rs,lower.rs}` 与 `crates/scoop/src/commands/dump_ir.rs`，确认 `MonomorphKey` 现只保留 typecheck 请求键语义，`dump-ir` 入口直接进入 `mir::materialize_for_dump(...)`，`monomorph::lower_for_dump(...)` 仅剩兼容薄包装；单文件/调试路径上的实例身份、模板索引与实例缓存均已收口到 MIR 层。
  - review 结论：`InstanceKey` 已与旧 `MonomorphKey` 请求键语义分离；`dump-ir` 现在消费 generic MIR template 而不是 HIR eager 实例；per-`InstanceKey` cache 与 fixed-point 发现策略已足够稳定，可继续进入 `T5000e2`。
  - 已验证：
    - `cargo fmt --all`
    - `cargo test -p scoopc generic_mir_template_for_dump_stays_free_of_hir_level_instances -- --nocapture`
    - `cargo test -p scoopc materialize_for_dump_dedups_repeated_instance_requests -- --nocapture`
    - `cargo test -p scoopc monomorph::lower -- --nocapture`
    - `cargo test -p scoopc mir::tests::dump_mir_keeps_generic_functions_as_templates_before_monomorphization -- --nocapture`
    - `cargo test -p scoopc materialize_for_dump_handles_type_body_generic_member_fun_roots -- --nocapture`
    - `cargo test -p scoopc materialize_for_dump_distinguishes_companion_member_fun_effect_instances -- --nocapture`
    - `cargo test --all`
    - `cargo clippy --all-targets -- -D warnings`
    - 全部通过。

### T5000e2 把编译单元 frontend/build 路径的 instance collection / materialization 迁到 MIR 层
- 经核对，`T5000e2` 当前同时覆盖：
  - 抽出可复用的“编译单元 typechecked facts -> generic MIR template -> `InstanceKey` 集合”主线；
  - 扩展 MIR instance collection，使其覆盖 owner/nominal specialization，而不只是 dump 路径上的 standalone generic fun；
  - 把 build / single-file LLVM frontend 从 `MonomorphKey` + HIR `collect_generic_*_instantiations(...)` 主路径切换到 MIR，并给仍消费 HIR 的 codegen 保留兼容输入。
- 另外，现有主路径已暴露真实问题：`scoop build --emit-llvm` 对 `wrap<Int, eff Boom>` / `wrap<Int, eff Zap>` 这类“相同 type args、不同 effect-row”的调用会坍缩成同一个 `@"wrap::<Int>"` 符号，说明编译单元 build 路径的实例身份仍未正确纳入 `eff_args`。
- 因此该任务拆成 `T5000e2a`～`T5000e2c` 三个实现子任务与对应 review，按顺序推进。

### [DONE] T5000e2a 抽出编译单元级 MIR materialization 输入/instance-set API
- 范围：
  - 从当前 dump-only `materialize_for_dump(...)` 中抽出一条可复用的“基于既有 typechecked compilation-unit facts 进行 generic MIR lowering + materialization”的内部 API；
  - 该 API 必须直接消费现成的 `Index`、`TypeEnv`、`TypeStore`、`MonomorphKey` 与 AST side table，而不是重新跑一次 parse/resolve/typecheck；
  - 先锁定编译单元级 `InstanceKey` 集合对跨文件 template identity 与 `eff_args` 维度的稳定承载，为后续 build/frontend 接线提供可复用基础。
- 验收：
  - `mir/materialize.rs` 不再只有 dump-only 入口，而是拥有可复用的编译单元 materialization 主线；
  - 基于同一组 typechecked inputs，能够得到稳定的 `InstanceKey` 集合，并区分相同 type args 下的不同 effect rows。
- 依赖：T5000e1R
- 完成记录（2026-04-26）：
  - 已在 `crates/scoopc/src/mir/materialize.rs` 新增 `materialize_compilation_unit_from_typechecked_inputs(...)`，将 generic HIR lowering、generic MIR lowering、template catalog 构建、AST side table site binding 收集与 `InstanceKey` materialization 收口为可复用的编译单元内部 API；
  - `materialize_for_dump(...)` 现已退回为 dump-only 前端准备包装，真正的“既有 typechecked compilation-unit facts -> monomorphic MIR instances”主线转由上述新 API 承接；
  - `collect_generic_template_infos(...)` 现已直接消费通用 `(&SourceFile, &ast::File)` compilation-unit 视图，而不再绑定 `PreparedDumpFile` 包装；AST `TopLevelFunCallBinding` / `TopLevelFunValueRef` 的 site binding 收集也已收口到 `collect_site_instance_bindings(...)`；
  - 已新增 `mir::materialize::tests::typechecked_compilation_unit_materialization_distinguishes_same_type_args_with_different_effect_rows`，直接锁定基于同一组 typechecked inputs 的编译单元 materialization 会保留 `wrap::<Int, eff Boom>` 与 `wrap::<Int, eff Zap>` 两个不同实例身份；
  - 本轮刻意未修改 build / single-file LLVM frontend 的主接线，`scoop build` 路径上 effect-row 实例身份坍缩问题仍继续由后续 `T5000e2c` 跟踪修复。

### [DONE] T5000e2aR Review：确认编译单元级 materialization API 已脱离 dump-only 包装
- 重点：
  - 新 API 是否真正复用了既有 typechecked compilation-unit facts，而不是重新构造一套 dump 专用前端；
  - 跨文件 template identity / `eff_args` 维度是否已经在该层稳定可见；
  - 是否为后续 build/frontend 接线留下了直接入口，而不是继续把主逻辑埋在 dump 包装里。
- 验收：
  - 编译单元 materialization 的可复用入口已经成立，可直接作为 `T5000e2b` / `T5000e2c` 的基础。
- 依赖：T5000e2a
- 完成记录（2026-04-26）：
  - review 先暴露并修复了一个真实边界泄漏：`materialize_compilation_unit_from_typechecked_inputs(...)` 仍只对 `files_to_lower` 收集 `TopLevelFunCallBinding` / `TopLevelFunValueRef` 并生成 generic HIR/MIR template；这在 dump 包装的 `compilation_unit == files_to_lower` 形状下可工作，但对后续 build/frontend 的典型形状（完整 `compilation_unit` + 子集请求源文件）会遗漏跨文件 generic template 的 MIR root 与 helper 文件内的 site binding，不能作为稳定复用入口。
  - `crates/scoopc/src/mir/materialize.rs` 现已改为始终基于完整 `compilation_unit` 收集 site binding 并生成 generic HIR/MIR template；实例请求的裁剪只由调用方传入的 `monomorph_keys` 决定，不再通过排除 template 提供者文件来“间接筛选”。
  - `crates/scoopc/src/mir/mod.rs` 现新增模块级 `pub(crate)` 包装入口 `materialize_compilation_unit_from_typechecked_inputs(...)`，并让 `materialize_for_dump(...)` 与 review 测试都经由该入口调用，确认编译单元 materialization API 已真正穿出 dump-only 包装边界，而不是继续埋在私有 `mir::materialize` 模块内部。
  - 已新增 `mir::materialize::tests::typechecked_compilation_unit_materialization_keeps_cross_file_effect_roots_when_request_sources_are_subset`，锁定“helper 文件定义 effect-generic `wrap/id`，main 文件仅贡献实例请求”时，编译单元 materialization 仍会保留跨文件 `wrap::<eff Boom>` 与 helper 内嵌套调用触发的 `id::<eff Boom>` 两个 concrete root。
  - review 结论：新 API 现已真正复用既有 typechecked compilation-unit facts；跨文件 template identity、`eff_args` 与 site binding 语义已经在该层稳定可见；后续 `T5000e2b` / `T5000e2c` 可以直接基于这一入口继续推进。
  - 已验证：
    - `cargo test -p scoopc typechecked_compilation_unit_materialization_keeps_cross_file_effect_roots_when_request_sources_are_subset -- --nocapture`
    - `cargo fmt --all`
    - `cargo test --all`
    - `cargo clippy --all-targets -- -D warnings`
    - 全部通过。

### [DONE] T5000e2b 让编译单元 MIR instance collection 覆盖 owner/nominal specialization
- 范围：
  - 扩展 MIR template / instance collection，使其不只覆盖独立泛型函数，还能表达 generic owner 下 member fun / getter 所需的 owner-specialized instance identity；
  - 收口当前 HIR `collect_generic_member_fun_instantiations(...)` 通过扫描 `TypeStore` 承担的 owner/nominal specialization 发现职责；
  - 保持 reachable-driven / on-demand，不退回“看到某个具体 nominal 类型就全量 eager clone 全部成员”的旧模式。
- 验收：
  - owner/nominal specialization 的实例集合已可在 MIR 层表达与收集；
  - HIR 不再是 generic owner member specialization 的主发现入口。
- 依赖：T5000e2aR
- 完成记录（2026-04-26）：
  - `crates/scoopc/src/typecheck/expr/call.rs` 现已在 member direct-call 记录 `MonomorphKey` / `TopLevelFunCallBinding` 时，把 generic owner 的 concrete `type_args` 以前缀形式并入请求键，统一形成 `owner args + fun args` 的实例身份；对应注释也已同步到 `crates/scoopc/src/monomorph/mod.rs` 与 `crates/scoopc/src/typecheck/lower.rs`。
  - `crates/scoopc/src/mir/materialize.rs` 的 generic template catalog 现已把 owner type params 纳入 `type_param_names` 与 signature key，generic owner member fun / getter 因而能成为 MIR `InstanceKey` 的一等维度，而不再只覆盖 standalone generic fun。
  - 为了保持 request-driven / on-demand，本轮没有回退到扫描 `TypeStore` 的 eager 发现模式；相反，编译单元 materializer 新增 request-root direct-call seeding，会从请求源文件对应的 generic root 函数中扫描 MIR `CallKind::Direct`，补种 owner-specialized getter 与 nested direct-call 实例请求。
  - 这意味着编译单元级 MIR instance collection 现在已经可以独立发现并 materialize owner-specialized member/getter 实例，不再依赖 HIR `collect_generic_member_fun_instantiations(...)` 作为该 collection 语义的主发现入口；build/frontend 仍在使用的 HIR eager materialization 主路径继续留给后续 `T5000e2c` 收口。
  - 已新增 `mir::materialize::tests::typechecked_compilation_unit_materialization_handles_owner_specialized_effect_generic_member_calls` 与 `mir::materialize::tests::typechecked_compilation_unit_materialization_seeds_owner_specialized_getter_from_request_roots`，分别锁定 owner-specialized effect-generic member call 与 getter seeding 的编译单元 materialization 语义。
  - 过程中还暴露出一个已存在质量问题：`MirInstanceMaterializer::new(...)` 命中 `clippy::too_many_arguments`。现已通过新增 `MaterializerConstructionInputs` 收口构造输入并修复，保持本轮无告警验收门。
- 已验证：
  - `cargo fmt --all`
  - `cargo clippy --all-targets -- -D warnings`
  - `cargo test -p scoopc typechecked_compilation_unit_materialization_handles_owner_specialized_effect_generic_member_calls -- --nocapture`
  - `cargo test -p scoopc typechecked_compilation_unit_materialization_seeds_owner_specialized_getter_from_request_roots -- --nocapture`
  - `cargo test --all`
  - 全部通过。

### [DONE] T5000e2bR Review：确认 owner/nominal specialization 已进入 MIR instance collection 语义
- 重点：
  - generic owner member/getter 的实例身份是否已被 MIR 层建模；
  - 是否仍有大块 owner-specialized instance discovery 依赖 HIR 扫描 `TypeStore`；
  - 新 collection 是否仍保持 reachable-driven / on-demand。
- 验收：
  - owner/nominal specialization 已成为 MIR instance collection 的一部分，而不是 HIR 副产物。
- 依赖：T5000e2b
- 完成记录（2026-04-26）：
  - review 先暴露并修复了一个真实 reachable-driven 缺口：`crates/scoopc/src/mir/materialize.rs` 原先只会扫描请求源文件中的非泛型 root，以及后续真正被 materialize 的 generic family；若 owner-specialized getter/member 只出现在跨文件非泛型 helper 中，或 generic instance 再调用非泛型 helper 后才触发，则该实例不会进入 MIR request 集合。
  - `crates/scoopc/src/mir/materialize.rs` 现已新增 `CallableBodyInfo` 与可达 non-generic body lookup，并保留完整 `TopLevelFunCallBinding` 索引；request-root seeding 与 generic instance rewrite 现在都会沿 direct-call 可达图继续扫描非泛型 helper/body，而不是只停在请求根文件本体。
  - generic owner member/getter 的实例身份继续由 MIR 层建模：owner args + fun args / `eff_args` 仍经 `typecheck -> MonomorphKey / TopLevelFunCallBinding -> materializer request/site binding -> InstanceKey` 主线传递；owner-specialized getter 在没有 AST call binding 的情况下，也会通过 MIR direct-call + receiver concrete type 推导进入实例集合。
  - `HIR` 的 `collect_generic_member_fun_instantiations(...)` 目前仍存在，但只服务后续 `T5000e2c` 尚未切换的 build/frontend 主路径；当前 MIR collection 本身已经不再依赖 HIR `TypeStore` 扫描来发现 owner-specialized member/getter 实例。
  - 已新增 `mir::materialize::tests::typechecked_compilation_unit_materialization_reaches_owner_specialized_getter_through_cross_file_non_generic_helper` 与 `mir::materialize::tests::typechecked_compilation_unit_materialization_reaches_owner_specialized_getter_through_non_generic_helper_called_by_generic_instance`，分别锁定“请求根跨文件 helper”与“generic instance 经由非泛型 helper”两条 reachable-driven 回归路径。
  - review 结论：owner/nominal specialization 现已成为 MIR instance collection 的一部分，且 collection 继续保持 reachable-driven / on-demand，没有退回到“按 `TypeStore` 中出现的 nominal 实例全量 eager clone 成员”的旧模式。
- 已验证：
  - `cargo test -p scoopc typechecked_compilation_unit_materialization_reaches_owner_specialized_getter_through_cross_file_non_generic_helper -- --nocapture`
  - `cargo test -p scoopc typechecked_compilation_unit_materialization_reaches_owner_specialized_getter_through_non_generic_helper_called_by_generic_instance -- --nocapture`
  - `cargo fmt --all`
  - `cargo test --all`
  - `cargo clippy --all-targets -- -D warnings`
  - 全部通过。

### [DONE] T5000e2c 让 build / single-file LLVM frontend 消费 MIR instance collection，并收口 HIR eager materialization 主路径
- 范围：
  - 将 build / single-file LLVM frontend 当前依赖的 `MonomorphKey` + HIR `collect_generic_fun_instantiations(...)` / `collect_generic_member_fun_instantiations(...)` 主路径切换到 MIR instance collection / materialization；
  - 给当前仍消费 HIR 的 LLVM codegen 提供兼容输入，但该输入必须以 MIR 产出的实例集合为来源，而不是继续让 HIR 自己做主发现；
  - 修复编译单元 build 路径对“相同 type args、不同 effect rows”实例身份的坍缩问题。
- 验收：
  - 编译单元级 monomorphic instance 集合已由 MIR 层生成并缓存，并被 build/frontend 主路径消费；
  - HIR lowering 不再作为 standalone generic fun 与 owner-specialized member fun 实例生成的主入口；
  - `wrap<Int, eff Boom>` / `wrap<Int, eff Zap>` 这类实例在 build/frontend 主路径上不会再坍缩成同一个符号身份。
- 依赖：T5000e2bR
- 完成记录（2026-04-26）：
  - `crates/scoop/src/commands/build.rs` 与 `crates/scoopc/src/llvm/frontend.rs` 现已统一改为调用 `hir::lower_for_compilation_unit_multi_files_via_mir_instance_collection(...)`，build / single-file LLVM frontend 都先复用 typechecked compilation-unit facts 做 MIR materialization，再生成当前 LLVM 仍消费的 HIR 兼容输入；
  - `crates/scoopc/src/hir/lower/mod.rs` / `util.rs` 已新增“显式 `InstanceKey` 集驱动”的 HIR 兼容 lowering：top-level generic fun、owner-specialized member fun/getter 的 concrete `FunDecl` 只按 MIR 产出的实例集合生成，`HIR` 不再通过 `MonomorphKey` / `TypeStore` 扫描承担主实例发现职责；
  - 本轮同时修复了两个真实前置缺口，避免 build/frontend 主路径在 async/task 场景下退回 generic 签名：
    - `crates/scoopc/src/mir/materialize.rs` 现会继续扫描 non-generic request root 创建出来的 closure body，并为所有带 body 的 MIR fun 建立按 FQN 的可达索引，使 async lowering 生成的匿名 lambda 也能继续贡献实例请求；
    - `crates/scoopc/src/mir/lower.rs` 现会在占位式 `Handle` / `Perform` terminator 后为剩余源码语句落一个孤立 continuation block，保住 `await` 之后的 `println(...)`、`__task_step_ready(...)` 等 direct-call 形状，避免 generic MIR 在 placeholder terminator 处把后半段语句截断；
  - 新增回归覆盖：
    - `llvm::tests::single_file_frontend_keeps_distinct_effect_row_generic_instances`
    - `llvm::tests::single_file_frontend_reaches_async_task_helper_instances_through_perform_continuations`
    - `mir::materialize::tests::typechecked_compilation_unit_materialization_reaches_generic_calls_through_non_generic_async_closure_body`
  - 验证结果：
    - `cargo fmt --all`
    - `cargo test -p scoopc single_file_frontend_keeps_distinct_effect_row_generic_instances -- --nocapture`
    - `cargo test -p scoopc single_file_frontend_reaches_async_task_helper_instances_through_perform_continuations -- --nocapture`
    - `cargo test -p scoopc typechecked_compilation_unit_materialization_reaches_generic_calls_through_non_generic_async_closure_body -- --nocapture`
    - `cargo test -p scoopc async_task_resume_ir_does_not_replay_original_await_site -- --nocapture`
    - `cargo test -p scoopc async_task_resume_replay_ir_terminates_step_fn_on_active_effect -- --nocapture`
    - `cargo test -p scoopc async_task_ir_uses_ordinary_scoop_task_helpers_not_legacy_runtime_abi -- --nocapture`
    - `cargo test -p scoopc single_file_minimal_ir_supports_handled_async_await -- --nocapture`
    - `cargo test -p scoopc task_step_ir_uses_ordinary_scoop_definition_not_legacy_poll_abi -- --nocapture`
    - `cargo test -p scoopc task_step_ir_uses_seqcst_atomic_claim_and_trap_without_mutex -- --nocapture`
    - `cargo test -p scoopc thread_join_statepoint_preserves_live_gc_locals -- --nocapture`
    - `cargo test --all`
    - `cargo clippy --all-targets -- -D warnings`
    - 全部通过。

### [DONE] T5000e2cR Review：确认 build/frontend 主路径已切到 MIR instance collection
- 重点：
  - build / single-file LLVM frontend 是否已消费 MIR 产出的实例集合；
  - HIR lowering 是否已经退出主实例发现职责；
  - effect-row 与跨文件 template identity 是否在编译单元主路径上保持稳定。
- 验收：
  - 编译单元 build/frontend 路径的实例收集与实例身份已经由 MIR 层主导。
- 依赖：T5000e2c
- 完成记录（2026-04-26）：
  - 已复核 `crates/scoop/src/commands/build.rs` 与 `crates/scoopc/src/llvm/frontend.rs`，确认 build / single-file LLVM frontend 现都直接调用 `hir::lower_for_compilation_unit_multi_files_via_mir_instance_collection(...)`；主路径不再通过 `lower_for_compilation_unit_multi_files_with_type_env(...)` 让 HIR 自己承担实例发现。
  - 已复核 `crates/scoopc/src/hir/lower/mod.rs` / `util.rs`，确认 compilation-unit lowering 已显式区分 `LegacyEagerHir`、`ExplicitMirInstances` 与 `GenericTemplateOnly` 三种模式；在 build/frontend 使用的 `ExplicitMirInstances` 模式下，top-level generic fun、owner-specialized member fun/getter 的 concrete `FunDecl` 只按 MIR 产出的 `InstanceKey` 集生成，HIR 仅做兼容 lowering，不再扫描 `MonomorphKey` / `TypeStore` 作为主实例发现入口。
  - 已复核 `crates/scoopc/src/mir/materialize.rs`，确认编译单元主路径仍以完整 compilation unit 建立 template catalog、call binding 与 callable-body 索引，再仅用 request source 子集做 request-root seeding；这保证了跨文件 template identity 仍由 MIR materializer 统一主导，而不会因为 build/frontend 只请求部分源文件而退回到局部 HIR eager clone。
  - 已确认仓库中旧的 `lower_for_compilation_unit_multi_files_with_type_env(...)` 调用面仅剩 `crates/scoopc/src/llvm/codegen/effect/state_machine_transform.rs` 的测试 helper，用于 effect/state-machine transform 的 typed-lowering 测试构造，不属于 build/frontend 主路径，因此不构成 `T5000e2cR` 的阻塞缺陷。
  - review 结论：本轮未发现需要插入到 `T5000e2cR` 之前的新前置缺陷任务；build/frontend 编译单元主路径的实例收集、实例身份与 HIR 兼容实例生成均已切换到 MIR instance collection 主导。
- 已验证：
  - `cargo test -p scoop build_frontend_keeps_distinct_effect_row_generic_instances -- --nocapture`
  - `cargo test -p scoopc single_file_frontend_ -- --nocapture`
  - `cargo test -p scoopc typechecked_compilation_unit_materialization_reaches_generic_calls_through_non_generic_async_closure_body -- --nocapture`
  - `cargo test --all`
  - `cargo clippy --all-targets -- -D warnings`
  - 全部通过。

### [DONE] T5000e2R Review：确认编译单元级 monomorphization 已脱离 HIR eager materialization
- 重点：
  - 是否仍有大块单态化逻辑残留在 HIR lowering；
  - 跨文件 template identity / cache key 是否稳定；
  - 实例收集是否仍保持 reachable-driven / on-demand，而不是退回全量 eager clone。
- 验收：
  - 编译单元主路径上的实例身份、收集与物化已经归属于 MIR 层。
- 依赖：T5000e2cR
- 完成记录（2026-04-26）：
  - 已复核 `crates/scoop/src/commands/build.rs` 与 `crates/scoopc/src/llvm/frontend.rs`，确认 build/frontend 与 single-file LLVM frontend 都经 `lower_for_compilation_unit_multi_files_via_mir_instance_collection(...)` 进入主路径；仓库中旧的 `lower_for_compilation_unit_multi_files(...)` / `lower_for_compilation_unit_multi_files_with_type_env(...)` 现仅剩测试与测试 helper 调用，不再承担 production monomorphization 主职责；
  - 已复核 `crates/scoopc/src/mir/materialize.rs` 与 `crates/scoopc/src/hir/lower/util.rs`，确认跨文件 template identity 继续由 `TemplateKey { fqn, source_path, decl_span }`、canonical template 选择与 `request_templates` 统一主导；HIR compatibility lowering 只按显式 `InstanceKey` 集恢复当前 LLVM codegen 仍需要的 monomorphic fun/member，不再回退到 legacy HIR eager 路径承担实例发现语义；
  - review 过程中发现并修复了一个既有文档错配：`crates/scoop/src/commands/build.rs` 中 `FrontendOutput::monomorph_keys` / `typecheck_types` 与 monomorph key 收集处的注释仍把它们描述为 HIR eager lowering 输入；现已改为准确描述 MIR materialization request seeds 与 HIR compatibility lowering 的关系；
  - 已新增 `crates/scoop/src/commands/build.rs` 中的 `build_frontend_does_not_eager_materialize_unused_owner_specialized_getter`，锁定“`TypeStore` 中出现 `Box<String>` 不应让 build frontend 额外产出 `Box.doubled::<String>`”的回归面；
  - review 结论：编译单元主路径上的实例身份、收集与物化已经归属于 MIR 层；当前未发现需要插入到 `T5000e3` 之前的新前置缺陷任务。
- 已验证：
  - `cargo test -p scoop build_frontend_ -- --nocapture`
  - `cargo test -p scoopc single_file_frontend_ -- --nocapture`
  - `cargo test -p scoopc typechecked_compilation_unit_materialization_ -- --nocapture`
  - `cargo test --all`
  - `cargo clippy --all-targets -- -D warnings`
  - 全部通过。

### T5000e3 收口当前仍阻塞 monomorphic MIR instance 成为主输入的 program-boundary / sysroot / LLVM codegen 路径
- 说明：
  - 经核对，`T5000e3` 当前同时覆盖四块仍会把旧边界继续拖回主线的收口工作：
    - panic/trap 路径仍通过 `scoop.process.exit(3)` 暴露给 sysroot/task 与 fixtures，缺少 bottom-typed 的统一 `panic` intrinsic；
    - 一批早期试验性的 sysroot surface（`scoop.channels`、`scoop.test`、`scoop.env`、`scoop.fs`、`scoop.io`、`scoop.net`、`scoop.path`、`scoop.time`、`scoop.process`）仍把未定型 API、对应 lowering 与 fixtures 留在主线；
    - 程序边界仍只接受零参数 `main`，而 argv surface 还经待删除的 `scoop.process.args()` 暂挂；
    - LLVM codegen 仍在按 mangled FQN 现场猜测 monomorphized target。
  - 因此该阶段拆成 `T5000e3a`～`T5000e3d` 四个实现子任务与对应 review，按顺序推进。

### [DONE] T5000e3a 在 corelib / `scoop.core` 中新增 `panic` intrinsic，并收口当前直接 abort 路径
- 范围：
  - 在 corelib（当前为 `scoop.core` sysroot surface）新增 `panic` intrinsic，返回类型固定为 `Nothing`；
  - 当前阶段允许 `panic` 的 runtime 落点继续实现为 `exit(3)` / 等价的立即终止路径，但该细节必须隐藏在 intrinsic / runtime 边界后面，而不是继续暴露成用户或 fixture 直接调用的 surface；
  - 把 sysroot/task 与其它“语义上就是 panic/trap”的现存路径从 `exit(3)` 改为 `panic(...)`；
  - 清理 fixtures 中直接使用 `exit(...)` 的写法；需要断言失败/崩溃路径时，应改为通过 `panic` 或更合适的语言语义表达。
- 验收：
  - 代码库中所有“语义上是 panic”的路径都经由 `scoop.core.panic`，而不是散落的 `scoop.process.exit(3)`；
  - `panic` 的返回类型为 `Nothing`，可被类型检查与控制流分析视为 bottom；
  - 任意 fixture 中都不再出现 `exit(...)` 调用。
- 依赖：T5000e2R
- 完成记录（2026-04-26）：
  - `sysroot/core.scoop` 已新增 `panic(message: String): Nothing`；LLVM codegen / runtime ABI / C runtime 已新增 `scoop_panic` 入口，当前 runtime 仍可在边界后把它落到 `exit(3)` / 等价立即终止路径；
  - `sysroot/task.scoop` 中语义上属于 panic/trap 的 `exit(3)` 已改为 `panic(...)`；fixture 中直接 `exit(...)` 的写法已清理，`std_process_args_exit_basic.scoop` 现只覆盖 argv 行为，并新增 `tests/fixtures/run-pass/core_panic_intrinsic_basic.scoop` 覆盖 bottom-typed panic surface；
  - `tests/fixtures/build/task_atomic_claim_no_mutex_llvm.scoop` 现断言 trap 路径走 `@scoop_panic`，`SCOOP_FULL_SPEC.md` 与 `SCOOP_RUNTIME.md` 已同步更新 panic surface/runtime 口径；
  - 收尾验证中先后修复了两个既有编译器阻塞：`crates/scoopc/src/typecheck/expr/entry.rs` 中 object member `fun` 未进入 expr typecheck 主线导致的 `object_member_call_basic.scoop` codegen 回归，以及 `crates/scoopc/src/hir/lower/expr.rs` 中 imported/sysroot `FunSig` expected-type hint 误用 caller source 导致的 `std_channels_basic.scoop` UTF-8 foreign-span panic；
  - 已验证 `cargo test -p scoopc mir::materialize --no-fail-fast`、`cargo test -p scoopc llvm:: --no-fail-fast`、`cargo test --all`、`cargo clippy --all-targets -- -D warnings`、`cargo run -p scoop -- test` 全部通过，其中完整 fixture suite 输出 `fixtures: ok (1214)`。

### [DONE] T5000e3aR Review：确认 panic/trap 语义已统一收口到 `Nothing`-typed intrinsic
- 重点：
  - `panic` 是否稳定落在 corelib / `scoop.core`，而不是新增另一层 process-style 过渡 surface；
  - 返回类型是否确实是 `Nothing`，没有为了兼容旧路径退回 `Unit`；
  - fixtures 与 sysroot/task 中是否已经清干净 `exit(...)` 的直接使用。
- 验收：
  - 之后若 runtime 想把 panic 从 `exit(3)` 改成更强的 trap / abort 语义，不需要再回改用户 surface 与 fixtures。
- 依赖：T5000e3a
- 完成记录（2026-04-26）：
  - 已复核 `sysroot/core.scoop`、`sysroot/process.scoop` 与 `sysroot/task.scoop`：`panic(message: String): Nothing` 稳定留在 `scoop.core`，而 `scoop.process` 只保留显式 process-control surface，没有新增第二层 process-style fatal trap 过渡 API；
  - 已复核 `crates/scoopc/src/llvm/codegen/call/dispatch.rs`、`crates/scoopc/src/llvm/codegen/intrinsics/builtin.rs`、`crates/scoopc/src/llvm/codegen/runtime_abi.rs` 与 `runtime/c/scoop_runtime.c`：`scoop.core.panic` 统一走专门 lowering / runtime symbol，codegen 端返回 `CgValue::never()`，未为兼容旧路径回退成 `Unit`；
  - 已全文检索 `tests/fixtures/**` 与 `sysroot/task.scoop` 中的直接 `exit(...)` 调用，确认用户/fixture 面已清理干净；`tests/fixtures/build/task_atomic_claim_no_mutex_llvm.scoop` 继续断言 trap path 发射 `@scoop_panic`；
  - 已验证 `cargo test -p scoopc llvm:: --no-fail-fast`、`cargo run -p scoop -- test`、`cargo test --all`、`cargo clippy --all-targets -- -D warnings` 全部通过，其中 fixture suite 输出 `fixtures: ok (1214)`；
  - review 结论：panic/trap 语义已经收口到 `Nothing`-typed core intrinsic；后续若 runtime 把实现从 `exit(3)` 切到更强的 trap/abort，只需调整 runtime 边界，不需要回改用户 surface、fixtures 或 `sysroot/task` 语义。

### [DONE] T5000e3b 删除待重做的早期 sysroot 模块与对应 tests/fixtures
- 范围：
  - 从 sysroot 中移除以下待重做 surface：`scoop.channels`、`scoop.test`、`scoop.env`、`scoop.fs`、`scoop.io`、`scoop.net`、`scoop.path`、`scoop.time`；
  - 删除对应的 LLVM lowering、stdlib bridge、typecheck/run-pass/runtime fixture 与文档口径，不保留“先 deprecated 但继续可用”的兼容层；
  - `scoop.process` 的移除与 argv surface 迁移绑在下一条 `T5000e3c` 中完成，避免在主线中留下“先删 argv、后补 entry args”的额外临时缺口。
- 验收：
  - 上述模块不再存在于 sysroot、prelude/import 可见面、fixtures 与 docs 中；
  - 相关测试与 fixture 要么删除，要么改写到仍受支持的新主线，不留下死路径；
  - 文档能明确表达这些 surface 是“先移除、后重设计重做”，而不是仍受支持但未完整实现。
- 依赖：T5000e3aR
- 完成记录（2026-04-26）：
  - 已从 `sysroot/` 中删除 `scoop.channels`、`scoop.test`、`scoop.env`、`scoop.fs`、`scoop.io`、`scoop.net`、`scoop.path`、`scoop.time`，同时删除 `stdlib/test.scoop`，并同步移除 `crates/scoopc/src/llvm/codegen/call/dispatch.rs`、`crates/scoopc/src/llvm/codegen/intrinsics/sysroot.rs`、`crates/scoopc/src/llvm/codegen/runtime_abi.rs`、`crates/scoopc/src/llvm/codegen/runtime_symbols.rs`、`runtime/c/scoop_runtime.c`、`runtime/c/scoop_runtime_api.h` 与 `crates/scoop_runtime/build.rs` 中只服务这些 surface 的 lowering / ABI / runtime 实现；
  - 已删除对应 run-pass / typecheck fixture 与 golden；仍需保留语义覆盖的用例已迁回当前主线：`stdlib_*` smoke fixture 改为本地 `assert*` helper + `require(...)`，`gc_pin_unpin_move_stress_matrix.scoop` 改用 fixture `ENV:` 指令承载 GC stress 配置，3 条 delegated-property 并发回归已改为基于 `scoop.sync` + `scoop.thread` 的同步实现；
  - 文档与历史记录已同步收口：`SCOOP_RUNTIME.md` 更新 sysroot/runtime 维护边界，`SCOOP_FULL_SPEC.md` 移除了 `scoop.io.*` 示例，`PLAN.md` / `TODO.md` 中旧 `scoop.channels.channelCreate::<Int>` 示例已改为仍存在的 `scoop.unsafe.stackAlloc::<Int>`；
  - 收尾验证中顺手修复了三个暴露出的既有问题：`crates/scoop/src/commands/build.rs` 里硬编码的已删 fixture 已改为 `std_sync_basic.scoop`，`runtime/c/scoop_runtime.c` 中 3 个删除 surface 后遗留的未使用符号已清理，`tests/fixtures/hir/**/*.hir` 与 `tests/fixtures/mir/**/*.mir` 已按新的 `TypeId` 编号重生快照；
  - 已验证 `cargo fmt --all`、`cargo test -p scoopc monomorph::lower -- --nocapture`、`cargo run -p scoop -- test`、`cargo test --all`、`cargo clippy --all-targets -- -D warnings` 全部通过，其中完整 fixture suite 输出 `fixtures: ok (1197)`。

### [DONE] T5000e3bR Review：确认 sysroot surface 已实质缩回到仍承诺维护的最小集合
- 重点：
  - 被列入本轮移除名单的模块是否真的从 sysroot / fixtures / docs 中消失；
  - 是否还残留只为了兼容旧 fixture 而保留的 lowering / prelude 特判；
  - 当前保留下来的 sysroot surface 是否已经与“近期继续维护”的范围一致。
- 验收：
  - 后续 std/runtime 重设计可以在干净边界上重做，而不是继续背着旧 API 壳。
- 依赖：T5000e3b
- 完成记录（2026-04-26）：
  - 已复核 `sysroot/` 当前仅保留 `collections.scoop`、`core.scoop`、`delegates.scoop`、`print.scoop`、`process.scoop`、`string.scoop`、`sync.scoop`、`task.scoop`、`thread.scoop`、`unsafe.scoop`；被列入移除名单的 `channels/env/fs/io/net/path/test/time` 均已不再出现在 sysroot 目录中。
  - 已全文检索仓库中对 `scoop.channels`、`scoop.test`、`scoop.env`、`scoop.fs`、`scoop.io`、`scoop.net`、`scoop.path`、`scoop.time` 的剩余引用：运行时/LLVM codegen/fixture 主路径中未再发现兼容性 lowering、runtime ABI 导出或 prelude 特判；剩余命中只存在于显式说明“已移除”的状态注记、未来设计文档或 fixture 注释。
  - review 过程中发现并修复了 3 处既有文档错配：`PLATFORM_API_SURFACE_AUDIT.md` 仍把已删除 platform surface 列为现行模块，`STDLIB_COMPLETENESS.md` 仍把这些已删除 surface 与 `stdlib/test.scoop` 记为 `DONE/DECL-ONLY`，`STDLIB_DESIGN.md` 的目标模块树缺少“这不是当前 shipped surface”的状态说明；现均已回写为与 `T5000e3b` 后边界一致的口径。
  - 已复核当前仍保留并近期承诺维护的最小 sysroot surface 与文档口径一致：通用/核心边界为 `scoop.core`、`scoop.unsafe`、`scoop.collections`、`scoop.delegates`、`scoop.string`、`scoop.print`、`scoop.task`，平台相关 surface 仅剩 `scoop.thread`、`scoop.sync` 与过渡期的 `scoop.process`；其中 `scoop.process` 将由下一条 `T5000e3c` 以扩展后的 `main` 程序边界替换（允许零参数/单个 `Array<String>` 参数，返回 `Unit` 或 `Int`）。
  - 已验证 `cargo run -p scoop -- test`（`fixtures: ok (1197)`）、`cargo test --all`、`cargo clippy --all-targets -- -D warnings` 全部通过。
  - review 结论：sysroot surface 已实质缩回到当前仍承诺维护的最小集合，后续 std/runtime 重设计可以在干净边界上推进；未发现需要插入到 `T5000e3c` 之前的新前置缺陷任务。

### [DONE] T5000e3c 扩展程序边界 `main` 签名以直接承载 argv，并移除 `scoop.process`
- 范围：
  - 将 executable entry `main` 的允许签名扩展为以下四种且仅以下四种：
    - `fun main(): Unit / Pure!`
    - `fun main(): Int / Pure!`
    - `fun main(args: Array<String>): Unit / Pure!`
    - `fun main(args: Array<String>): Int / Pure!`
  - 规定 `args` 直接承载 native 程序边界收到的完整 `argv`，包含 `argv[0]`（可执行文件名/路径）；这不是 Kotlin/Java 风格的“仅用户参数”约定；
  - 规定返回 `Unit` 的 `main` 在正常返回时默认进程退出码为 `0`；返回 `Int` 的 `main` 在正常返回时将该值作为进程退出码；`panic(...)` 等其它提前终止路径可以使用不同退出码，但它们不属于 `main` 的正常返回 contract；
  - 在 driver / frontend / typecheck / docs 中同步这一 entry contract，使零参数/单个 `Array<String>` 参数、`Unit`/`Int` 返回的 `main` 都能成为合法程序边界；
  - 移除 `scoop.process` sysroot surface（包括 `args()` 与旧 `exit(...)`），并删除/改写对应 tests/fixtures。
- 验收：
  - `main` 的 program-boundary contract 已明确且文档化：参数仅接受零参数或单个 `Array<String>` 两种形状，返回类型仅接受 `Unit` 或 `Int`，并继续强制 `Pure!`；
  - 正常返回 `Unit` 的 `main` 默认退出码为 `0`；正常返回 `Int` 的 `main` 将该值作为进程退出码；其它提前终止路径仍由各自语义负责；
  - 运行时传入的完整 `argv` 可稳定到达 `main(args)`，并保留 `argv[0]`；
  - 代码库中不再存在 `scoop.process` 模块、`args()` API 与相关 fixture 依赖。
- 依赖：T5000e3bR
- 完成记录（2026-04-26）：
  - 已在 driver / frontend / typecheck / HIR lowering / LLVM entry lowering 上统一收口 `main` program-boundary contract：合法形状稳定为零参数或单个 `Array<String>` 参数，返回类型仅允许 `Unit` 或 `Int`，并继续要求 `Pure!`；
  - 已修复一个阻塞本任务的既有真实缺陷：typecheck 先前不会把未显式标注返回类型的 `fun` 推断结果回写到 typed HIR 可消费的 side table，导致 typed lowering 与 `main` 签名校验会把这类函数错误地回退成 `Any` / `Unit`；现已新增并接通推断返回类型 side table，使 `main` 的推断返回类型在 typed HIR 与 entry-point 校验中一致生效；
  - runtime / codegen / driver 主路径现已把 native 程序边界收到的完整 `argv`（包含 `argv[0]`）直接传给 `main(args)`，并按 contract 将正常返回的 `Unit` 映射为退出码 `0`、正常返回的 `Int` 映射为进程退出码；
  - `sysroot/process.scoop` 已删除，相关 runtime ABI / lowering / fixture 主路径已迁移到新的 entry-point argv contract；`SCOOP_FULL_SPEC.md`、`SCOOP_RUNTIME.md` 与受影响的 fixture / golden 已同步更新；
  - 本轮收尾还修正了一个过时 fixture 断言：`tests/fixtures/typecheck/entry_point_main_param_not_array_string_is_error.scoop` 现改为匹配当前实际诊断中的完整限定类型名 `scoop.core.Array<Int>`。
  - 已验证 `target/debug/scoop test --fixtures tests/fixtures/typecheck`（`fixtures: ok (395)`）、`target/debug/scoop test --fixtures tests/fixtures/runtime_gc`（`fixtures: ok (24)`）、`target/debug/scoop test --fixtures tests/fixtures/run-pass`（`fixtures: ok (394)`）、`target/debug/scoop test --fixtures tests/fixtures/spec_doctest`（`fixtures: ok (1)`）、`cargo run -p scoop -- test`（`fixtures: ok (1201)`）、`cargo test --all`、`cargo clippy --all-targets -- -D warnings` 全部通过。

### [DONE] T5000e3cR Review：确认 entry-point argv contract 已替代临时 `scoop.process` surface
- 重点：
  - `main` 的合法签名集合是否已经稳定收口为“两种参数形状 × 两种返回类型”的四种约定；
  - `Unit` 返回的 `main` 是否在正常返回时稳定映射到默认退出码 `0`，而 `Int` 返回的 `main` 是否把返回值作为退出码；
  - argv 是否已经从程序边界直接进入，而不是继续绕经单独的 process sysroot API；
  - `SCOOP_FULL_SPEC.md`、`SCOOP_RUNTIME.md`、README/相关入口文档与 fixtures 是否已经同步。
- 验收：
  - 程序边界 contract 与文档口径已一致；之后不需要再为 `scoop.process.args()` 保留兼容语义。
- 依赖：T5000e3c
- 完成记录（2026-04-26）：
  - 已复核 `crates/scoopc/src/typecheck/expr/stmt.rs`、`crates/scoopc/src/llvm/emit.rs`、`crates/scoopc/src/llvm/codegen/mod.rs` 与 `runtime/c/scoop_runtime.c`：executable `main` 的合法形状已稳定收口为零参数或单个 `Array<String>` 参数、返回 `Unit` 或 `Int`；LLVM 入口继续保留 `main(argc, argv)` 的 C ABI，并仅在 `main(args)` 形状下通过 `scoop_entry_argv_array(argc, argv)` 把完整 native argv（含 `argv[0]`）注入程序边界；正常返回 `Unit` 映射退出码 `0`，正常返回 `Int` 映射为进程退出码；
  - review 过程中发现并修复了 4 处既有文档/注释错配：`STDLIB_COMPLETENESS.md` 仍把 `scoop.process` 记为当前 DONE surface，`PLATFORM_API_SURFACE_AUDIT.md` 仍把 `scoop.process` 列为现行平台模块，`README.md` 尚未说明新的 executable `main` argv / exit-code contract，`STDLIB_DESIGN.md` 中 `scoop.process` 条目缺少“future target 而非 current shipped surface”的明确说明；另已顺手修正 `crates/scoopc/src/typecheck/expr/stmt.rs` 中 entry-point effect row 注释仍写成 `Pure` 的口误；
  - 已在 `crates/scoopc/src/llvm/tests.rs` 新增 `minimal_main_ir_with_array_string_args_calls_entry_argv_helper`，并让现有 `minimal_main_ir_contains_main_and_ret0` 断言零参数 `main` 不会错误接入 `scoop_entry_argv_array`，从 IR 层直接锁定 entry argv helper 的接线边界；
  - 已验证 `cargo fmt --all`、`cargo test -p scoopc minimal_main_ir_ -- --nocapture`、`cargo run -p scoop -- run tests/fixtures/run-pass/std_process_args_exit_basic.scoop -- foo bar`、`cargo run -p scoop -- run tests/fixtures/run-pass/entry_main_args_int_exit_basic.scoop -- foo bar`（退出码 `3`）、`cargo run -p scoop -- test --fixtures tests/fixtures/typecheck`（`fixtures: ok (395)`）、`cargo test --all`、`cargo clippy --all-targets -- -D warnings` 全部通过；
  - review 结论：entry-point argv / exit-code contract 已实质替代临时 `scoop.process` surface；现行文档与 fixtures 已统一收口到程序边界 `main`，后续无需再为 `scoop.process.args()` 保留兼容语义。

### [DONE] T5000e3d 让 LLVM codegen 消费已实例化 target identity，并删除现场猜测 monomorphized target 的主路径
- 范围：
  - 移除/收口 LLVM codegen 中按 mangled FQN 现场重定向 generic callee 的主职责；
  - 让 codegen 通过已实例化的 callee 事实 / instance identity 进入目标，而不是继续做 `try_resolve_monomorphized_*` 式推断；
  - 确保后续 summary / devirt / inline / effect planning 消费的是 monomorphic MIR instances。
- 验收：
  - LLVM codegen 不再以“现场根据 mangled FQN 重定向目标”为主路径承担 monomorphization；
  - 后续中端优化面向的主要输入已经是 monomorphic MIR instances。
- 依赖：T5000e3cR
- 完成记录（2026-04-27）：
  - `HirLowering` / `LoweringInputs` / compilation-unit lowering 现新增 `materialize_direct_call_targets` 开关：compilation-unit 与 via-MIR-instance lowering 开启，`lower_for_dump` / `lower_typed_for_dump` / generic-template-only 路径关闭，确保只有可 codegen 的 HIR 会把非 intrinsic direct-call target 物化为最终实例 FQN；
  - standalone / member / extension direct-call lowering 现统一回放 typecheck 写回的 `TopLevelFunCallBinding`，对非 intrinsic 且 type/effect 实参都已 concrete 的调用直接写入稳定实例 FQN；async task helper 等内部 top-level helper 调用也经 `call_top_level_fun_with_type_args` 走同一套物化规则；
  - LLVM backend 已删除 `try_resolve_monomorphized_member_fqn` / `try_resolve_monomorphized_standalone_fun_fqn` 与相关现场推断 helper；`call/dispatch.rs` 只保留一个窄的 direct-call template-FQN 归一化入口，专供 sysroot / builtin special-case、vtable / itable 等仍按模板名建模的路径消费，普通静态 direct-call 继续直接用完整实例 FQN 命中 `fun_index`；
  - 实现过程中暴露并修复了一个当前任务范围内的既有 backend 缺口：HIR 把 `scoop.core.println::<T>` 这类 generic sysroot direct-call 物化为实例 FQN 后，LLVM special-case dispatch 仍只按模板名分派，导致 builtin lowering 被绕过；现已通过窄归一化 helper 收口为“special-case 看模板名、普通静态调用看实例名”的稳定边界；
  - 已新增 `compilation_unit_via_mir_instances_materializes_non_intrinsic_direct_call_targets`、`typed_hir_dump_keeps_generic_direct_calls_as_template_targets` 与 `lowered_hir_codegen_accepts_materialized_generic_sysroot_direct_calls` 回归测试，分别锁定 compilation-unit lowering、typed dump / generic-template lowering 与 LLVM builtin dispatch 的边界；
  - 已验证 `cargo test -p scoopc`、`cargo test --all`、`cargo clippy --all-targets -- -D warnings` 全部通过。

### [DONE] T5000e3dR Review：确认 LLVM backend 已退出单态目标猜测主职责
- 重点：
  - LLVM codegen 是否已经直接消费显式 instance identity，而不是继续依赖 backend 符号名推断；
  - 是否还残留大块 `try_resolve_monomorphized_*` / mangled-FQN 导向的现场补救逻辑；
  - monomorphic MIR instance 是否已经成为 summary / devirt / inline / effect planning 的共同输入。
- 验收：
  - backend 只消费已物化实例，而不是继续承担 monomorphization 的最后一公里语义。
- 依赖：T5000e3d
- 完成记录（2026-04-27）：
  - 已复核 `crates/scoopc/src/hir/lower/{mod,expr}.rs`、`crates/scoopc/src/llvm/codegen/call/dispatch.rs`、`crates/scoopc/src/llvm/frontend.rs` 与 `crates/scoopc/src/mir/materialize.rs`：codegen 主路径上的 standalone / member / extension direct-call 已统一回放 typecheck `TopLevelFunCallBinding`，并在 via-MIR compilation-unit lowering 中按 `InstanceKey` 物化实例 target；`prepare_single_file_codegen_unit(...)` 也已稳定走 `lower_for_compilation_unit_multi_files_via_mir_instance_collection(...)`。
  - 已全文检索 LLVM backend 中 `try_resolve_monomorphized_*` / mangled-FQN 现场重定向逻辑：`codegen/mod.rs` 中旧 helper 已删除，普通静态 direct-call 只剩完整实例 FQN 命中 `fun_index` 的主路径；剩余模板名消费已收口为 `call/dispatch.rs` 的 sysroot / builtin special-case、vtable / itable slot 识别，以及 `emit.rs` 中围绕 generic member reachability 的窄兜底路径，未再承担 ordinary static direct-call 的单态目标解析语义。
  - 已在 `crates/scoopc/src/llvm/tests.rs` 新增 `frontend_codegen_consumes_materialized_generic_direct_call_instances`，从 via-MIR frontend lowering 与 LLVM IR 双层断言 `fixtures.t5000e3dr.id::<Int>` / `fixtures.t5000e3dr.Box.memberId::<Int>` 在进入 backend 前后都保持实例身份，且 IR 不会回退到 template target 符号。
  - 已验证 `cargo test -p scoopc compilation_unit_via_mir_instances_materializes_non_intrinsic_direct_call_targets -- --nocapture`、`cargo test -p scoopc typed_hir_dump_keeps_generic_direct_calls_as_template_targets -- --nocapture`、`cargo test -p scoopc lowered_hir_codegen_accepts_materialized_generic_sysroot_direct_calls -- --nocapture`、`cargo test -p scoopc frontend_codegen_consumes_materialized_generic_direct_call_instances -- --nocapture`、`cargo fmt --all`、`cargo test --all`、`cargo clippy --all-targets -- -D warnings` 全部通过。
  - review 结论：LLVM backend 已退出 ordinary static direct-call 的单态目标猜测主职责；后续 `T5000e3R` / summary / devirt / inline / effect planning 可以把 MIR `InstanceKey` 与已物化实例 HIR 作为稳定前置边界继续推进。

### [DONE] T5000e3R Review：确认 monomorphization 与 program-boundary / sysroot 收口已形成稳定前置边界
- 重点：
  - `InstanceKey` 是否真正独立于 backend 符号名；
  - 是否仍有大量单态化职责遗留在 LLVM codegen；
  - `panic` / entry-point argv / sysroot surface 收口后，是否还残留会把旧 process/sysroot 约定重新长回来的兼容层；
  - 实例收集 / 缓存策略是否已经考虑 `-O0` / debug build 成本。
- 验收：
  - monomorphization 的主语义与主数据结构已经明确属于 MIR，而不是 HIR 或 LLVM codegen；
  - 后续 summary / devirt / inline 可以建立在当前 program-boundary 与 sysroot 最小契约之上，而不是继续背着旧 surface。
- 依赖：T5000e3dR
- 完成记录（2026-04-27）：
  - 已复核 `crates/scoopc/src/mir/{mod,materialize.rs}`、`crates/scoopc/src/monomorph/mod.rs`、`crates/scoopc/src/hir/lower/mod.rs`、`crates/scoopc/src/llvm/{frontend.rs,codegen/call/dispatch.rs}` 与相关回归测试：`InstanceKey` 仍由 `TemplateKey + type_args + eff_args` 构成，实例身份先在 MIR 层建立，再由 HIR 兼容输出与 LLVM backend 消费；LLVM backend 侧未重新长回按 backend 符号名承担 monomorphization 主语义的路径。
  - review 过程中暴露并修复了一个既有前端边界缺口：single-file LLVM frontend 之前会让 stdlib/helper support sources 一并收集 `monomorph_keys`，并默认把所有 lowering 输入文件都当作 request roots；这会把 support source 中未被入口触达的 generic 调用错误提升为实例收集种子。现已在 `crates/scoopc/src/hir/lower/mod.rs` 新增 `lower_for_compilation_unit_multi_files_via_mir_instance_collection_with_request_sources(...)`，显式分离“参与 lowering 的文件集合”和“允许贡献实例请求的 request roots”，并在 `crates/scoopc/src/llvm/frontend.rs` 中收紧为仅入口源文件收集 `monomorph_keys`、support sources 继续参与 lowering/codegen 但不再作为实例请求根。
  - 已在 `crates/scoopc/src/mir/materialize.rs` 新增 `typechecked_compilation_unit_materialization_skips_unreachable_generic_requests_from_non_request_sources`，锁定 support source 仍进入 HIR 兼容输出、但其 helper-only generic 调用不会被 request-root 语义错误物化；同时复跑 `frontend_codegen_consumes_materialized_generic_direct_call_instances` 与 `single_file_frontend_keeps_distinct_effect_row_generic_instances`，确认前端经由 MIR 的实例物化主链仍保持正确。
  - 已重新核对 program-boundary / sysroot 收口现状：现行 surface 继续以 `scoop.core.panic` 与 executable `main(args?) -> Unit|Int` contract 为准，未再发现把旧 `scoop.process` / 早期 sysroot 约定重新长回生产路径的兼容层；实例收集与缓存方面，`collect_request_root_fun_keys(...)`、`collect_hir_direct_call_instance_requests(...)`、`instance_request_is_concrete(...)` 以及 `queued/materialized/declaration_only_instances` 去重缓存，现已与单文件 frontend 的 request-root 语义对齐，`-O0` / debug build 路径不会再被 support-source helper-only generic 调用平白放大。
  - 已验证 `cargo fmt --all`、`cargo test -p scoopc typechecked_compilation_unit_materialization_skips_unreachable_generic_requests_from_non_request_sources -- --nocapture`、`cargo test -p scoopc frontend_codegen_consumes_materialized_generic_direct_call_instances -- --nocapture`、`cargo test -p scoopc single_file_frontend_keeps_distinct_effect_row_generic_instances -- --nocapture`、`cargo test --all`、`cargo clippy --all-targets -- -D warnings` 全部通过。
  - review 结论：monomorphization 的主语义、主实例身份与主缓存边界已稳定收口到 MIR/request-root 语义；program-boundary 与 sysroot 最小契约也已稳定，后续 `T5000f` 的 per-instance summary 可以直接建立在当前边界之上。

### [DONE] T5000f 建立 per-instance summary 基础设施
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
- 依赖：T5000e3R
- 完成记录（2026-04-27）：
  - 新增 `crates/scoopc/src/mir/summary.rs`，在 `MaterializedMir` 上挂载 `MaterializedMirSummaries` stable side table，并为每个 `InstanceKey` 产出 `InstanceSummary { body_known, size_cost, recursive_scc, may_outward_effect, may_allocate_closure, param_use_summaries, result_provenance }`；
  - 本轮先修复了一个直接阻塞 `result_provenance` / 参数逃逸判断的既有 MIR 边界缺口：`crates/scoopc/src/mir/mod.rs` 的 `TerminatorKind::Return` 现改为显式携带 `Option<Operand>` 返回值，`crates/scoopc/src/mir/lower.rs` 与 `crates/scoopc/src/mir/materialize.rs` 已同步改为保留和重写返回 operand，不再让 summary 依赖“返回前最后一个临时 local”的隐式约定；
  - summary 计算当前以 materialized MIR 为输入：对 body-known materialized callable 做 CFG 上的局部 provenance / 参数使用 dataflow、direct-call graph/SCC 计算与 outward-effect fixed-point；对 declaration-only instance 则基于声明签名给出 `body_known = false` 的保守 summary，而不是在 codegen 查询时现场重建；
  - `param_use_summaries` v1 已覆盖 `Unused` / `ValueOnly` / `DirectCallOnly` / `Escapes` 四态，`result_provenance` v1 已覆盖 `Unit`、`Param`、`DirectFunction`、`KnownClosure`、`TopLevelValue`、`PerformResult`、`Join` 与 `Unknown`，可直接作为后续 devirt / inline / escape analysis 的共同输入；
  - 已新增 5 个 summary 回归测试，分别锁定“summary 按实例身份而不是按函数名工作”“函数值参数 `DirectCallOnly`”“经返回逃逸的参数”“已知 closure 返回值与 closure allocation”“declaration-only instance 的保守 summary”；
  - 已验证 `cargo fmt --all`、`cargo test -p scoopc mir::summary -- --nocapture`、`cargo test -p scoopc`、`cargo test --all`、`cargo clippy --all-targets -- -D warnings` 全部通过。

### [DONE] T5000fR Review：确认 summary 已按单态实例而不是按函数名工作
- 重点：
  - summary 是否真正挂在 monomorphic instance 上；
  - 是否还残留“按函数名一份 summary，再在 codegen 现场补类型”的做法；
  - summary 计算与缓存是否已经具备后续多轮迭代的可扩展性。
- 验收：
  - summary 的层次归属与 identity 已足够稳定，可以直接喂给 devirt / inline。
- 依赖：T5000f
- 完成记录（2026-04-27）：
  - review 首先暴露并修复了一个既有实例 identity 缺口：`crates/scoopc/src/mir/materialize.rs` 里的 `instance_fqn()` 之前只把 `TemplateKey` 投影成 `template.fqn + type/effect args`，会让“同名 generic overload + 相同实例化实参”落到同一个 materialized root symbol；现已新增 canonical template → stable overload suffix 映射，仅在同一 `template.fqn` 存在多个 canonical overload 时追加 `$overload$<stable-hash>`，并把后缀放在 `::<args>` 之后，从而既保持 per-instance root symbol 单射，又不破坏现有按 `template_fqn::<...>` 前缀做 base-FQN 查询的逻辑；
  - 已复核 `MaterializedMirSummaries` 的对外缓存边界仍是 `InstanceKey -> InstanceSummary`，没有重新长回“按函数名缓存一份 summary，再在 codegen 现场补类型”的路径；`crates/scoopc/src/mir/summary.rs` 内部的 direct-call graph / SCC / outward-effect fixed-point 仍以 materialized family symbol 建图，但在 root projection 重新变成 injective 之后，不同 overload 实例不会再在 pending summaries 中相互覆盖或错误共享递归/effect 结论；
  - 已新增 `mir::summary::tests::overloaded_generic_instances_keep_distinct_summary_identity`，锁定两个同名 generic overload 在相同 `Int` 实例化下会产生不同 root symbol、保留 `template_fqn::<Int>` 前缀，并分别得到 `ResultProvenance::Param(0)` / `Param(1)` 的 summary；
  - 已验证 `cargo fmt --all`、`cargo test --all`、`cargo clippy --all-targets -- -D warnings` 全部通过；
  - review 结论：summary 的层次归属、导出 identity 与缓存边界现已稳定，可直接作为 `T5000g` 的共享输入。

### [DONE] T5000g 在 MIR 层实现通用 devirtualization
- 范围：
  - 对所有 `VirtualCall` / `InterfaceCall` 统一做 receiver exactness / target-set shrinking；
  - 只要静态 target set 为 singleton，就改写为 `DirectCall`；
  - backend 只消费已分类完成的调用节点，不再负责主要去虚化判定。
- 验收：
  - 去虚化规则对“所有 receiver 类型已知且 target singleton 的 class/interface 调用”统一成立；
  - 不依赖 `Iterator.next()` / `Iterator.hasNext()` 等任何特定函数名。
- 依赖：T5000fR
- 完成记录（2026-04-27）：
  - 新增 `crates/scoopc/src/devirtualize.rs`，统一承接 exact-receiver 判定与 dispatch target shrinking；当前同时消费 `known_receiver_subclasses`、class vtable 与 interface itable 事实，并在 exact class receiver 没有 vtable slot 的情况下回退到 `owner.member` singleton target，覆盖 final/non-override class member 这类原先会卡在 `VirtualCall` 的路径；
  - `crates/scoopc/src/hir/lower/{mod,expr}.rs` 现已把 dispatch-call side table、known-subclass/vtable/itable 事实与 `devirtualize_dispatch_calls` 开关接入 explicit MIR instance lowering：exact receiver 的 class/interface dispatch 会直接 materialize 为 `DirectCall` target，非 exact receiver 仍保留 `DispatchCallSiteIndex` 供后续 MIR lowering 输出 `VirtualCall` / `InterfaceCall`；
  - `crates/scoopc/src/mir/{lower,materialize}.rs` 现已统一把 HIR side tables 显式收口到 `MirLoweringFacts`，并在 instance materialization 阶段对剩余 `VirtualCall` / `InterfaceCall` 做同一套 devirtualization；同时修复了一个直接阻塞实例发现的既有缺口：materialized direct-call FQN 与 `TopLevelFunCallBinding` / `TopLevelFunValueRef` 不再要求字符串完全相等，`foo::<...>` 会正确回落到对应 template 的 site binding，而不是误丢 instance request；
  - `crates/scoopc/src/cone/pre_specialize.rs` 已补上新的 `lower_fun_with_type_bindings_and_mir_facts(...)` 路径，确保 HIR dispatch/effect/when side tables 在预特化单函数 lowering 中不会因为旧 MIR facts 构造接口消失而掉线；
  - 已新增/更新 monomorph 回归，分别锁定“exact virtual receiver -> DirectCall”“存在已知子类时仍保留 `VirtualCall`”“`where T: Interface` 在实例化到 concrete receiver 后 `InterfaceCall -> DirectCall`”；同时修复并复跑 owner-specialized effect-generic member / top-level fun value effect instance 的 materialization 回归。
  - 已验证 `cargo fmt --all`、`cargo test -p scoopc`、`cargo test --all`、`cargo run -p scoop -- test`（`fixtures: ok (1201)`）、`cargo clippy --all-targets -- -D warnings` 全部通过。

### [DONE] T5000gR Review：确认 devirtualization 已经是结构驱动而不是热点特判
- 重点：
  - 规则是否对所有符合条件的调用统一生效；
  - 是否仍保留 backend 侧的目标猜测或名字特判；
  - 与后续 inline 的接口是否足够自然。
- 验收：
  - `InterfaceCall -> DirectCall`、`VirtualCall -> DirectCall` 已是 MIR 层统一改写，而不是 codegen 侧例外路径。
- 依赖：T5000g
- 完成记录（2026-04-27）：
  - review 首先暴露并修复了一个既有 backend 边界问题：`crates/scoopc/src/llvm/codegen/call/dispatch.rs` 之前仍按 class/interface member FQN 猜测“这是不是 dispatch call”，class vtable 路径甚至还保留了 `try_devirtualize_class_vtable_call_target_impl(...)` 的 backend 内部去虚化分支；现已把 `LoweredHir.dispatch_call_sites` 接入 `CompilationUnitCodegenCx`，并让 class/interface dispatch lowering 仅在当前 call site 被显式标记为 `Virtual` / `Interface` 时才走 vtable/itable，backend 不再对已 directized 的调用重新猜目标或再次去虚化；
  - review 进一步暴露并修复了一个被旧 backend 猜测路径长期遮住的 HIR 缺口：`crates/scoopc/src/hir/lower/stmt.rs` 的 custom-iterator `for` 语法糖此前手工拼出了 `iterator()/next()` 的 top-level call，但没有同步写入 `dispatch_call_sites`；现已改为复用与普通 member-call 一致的 synthetic dispatch 分类/去虚化逻辑，使 `for_in_custom_iterator_basic` 这类 interface-driven iterator 协议不再依赖 backend 按 FQN 兜底；
  - 已新增 LLVM 回归 `via_mir_direct_class_call_is_not_reinterpreted_as_vtable_dispatch` 与 `via_mir_direct_interface_default_call_is_not_reinterpreted_as_itable_dispatch`，分别锁定“via-MIR 已 directized 的 class/interface 调用不会在 backend 被重新识别成 vtable/itable dispatch”；同时复跑并恢复 `tests/fixtures/run-pass/for_in_custom_iterator_basic.scoop` 的端到端行为；
  - 已验证 `cargo fmt --all`、`cargo test -p scoopc llvm:: -- --nocapture`、`cargo run -p scoop -- test`（`fixtures: ok (1201)`）、`cargo test --all`、`cargo clippy --all-targets -- -D warnings` 全部通过。

### [DONE] T5000h0a 为 build / single-file frontend 产物保留 `MaterializedMir` / summaries，而不是在 `instance_keys` 后丢弃
- 范围：
  - 调整 build / single-file frontend 入口使用的 lowering 产物，使 production 主路径返回值中稳定保留 `MaterializedMir.file`、`types`、`instance_keys` 与 `summaries`；
  - 保持现有 HIR 兼容 lowering 继续服务当前 LLVM codegen / side tables，但要明确它在这条路径上的角色是“兼容输入”，而不是唯一 frontend 产物；
  - 为 build / single-file 两条 production 入口补测试，锁定“materialized body / summary 已进入主路径产物”。
- 验收：
  - build / single-file frontend 调用点不再在 `instance_keys` 后丢弃 `MaterializedMir`；
  - production 入口返回值中可以直接读到 materialized callable body 集合与 `MaterializedMir.summaries`；
  - 现有基于 HIR 兼容输入的 LLVM frontend / build 行为不回退。
- 依赖：T5000gR
- 完成记录（2026-04-27）：
  - `crates/scoopc/src/hir/lower/types.rs` 中的 `LoweredHir` 已新增 production 用 `materialized_mir()` / `materialized_mir_mut()` 挂点；该挂点在 dump/legacy eager HIR lowering 继续保持为空，只在 via-MIR production 主路径上保留 canonical `MaterializedMir`；
  - `crates/scoopc/src/hir/lower/mod.rs` 的 `lower_for_compilation_unit_multi_files_via_mir_instance_collection_with_request_sources(...)` 现已不再在 `instance_keys` 后丢弃 `MaterializedMir`，而是把 `file/types/instance_keys/summaries` 连同现有 HIR 兼容输出一起返回；同时补充注释，明确 HIR 在这条路径上的角色是“兼容输入”，而不是唯一 frontend 产物；
  - `crates/scoopc/src/llvm/frontend.rs` 与 `crates/scoop/src/commands/build.rs` 已补记新边界说明；`crates/scoopc/src/llvm/tests.rs` 与 `crates/scoop/src/commands/build.rs` 的 production 回归现已锁定 single-file/build 两条入口都能直接观察 materialized callable body 集合，并对每个 `InstanceKey` 读取 `MaterializedMir.summaries`；
  - 已验证 `cargo fmt --all`、`cargo test -p scoopc single_file_frontend_keeps_distinct_effect_row_generic_instances -- --nocapture`、`cargo test -p scoop build_frontend_keeps_distinct_effect_row_generic_instances -- --nocapture`、`cargo test --all`、`cargo run -p scoop -- test`（`fixtures: ok (1201)`）、`cargo clippy --all-targets -- -D warnings` 全部通过。

### [DONE] T5000h0aR Review：确认 production frontend 已稳定保留 materialized MIR / summary 产物
- 重点：
  - build / single-file 主路径是否都保留了 `MaterializedMir.file` 与 `summaries`；
  - HIR 兼容 lowering 是否已经被明确收口为“side tables / 兼容输入”，而不是继续被默认视为唯一 frontend 产物；
  - 是否为后续 canonical body/pass 视图留出了稳定挂点。
- 验收：
  - `T5000h0b` 可以直接在 production 产物上建立 canonical materialized-body / summary 视图，而不需要再改回“重新 materialize 一次”的接口。
- 依赖：T5000h0a
- 完成记录（2026-04-27）：
  - 已复核 `crates/scoopc/src/hir/lower/mod.rs::lower_for_compilation_unit_multi_files_via_mir_instance_collection_with_request_sources(...)`，确认 via-MIR production 主路径会先 materialize compilation unit，再把完整 `MaterializedMir` 挂回 `LoweredHir`，而不是像旧路径那样只消耗 `instance_keys`；
  - 已复核 `crates/scoopc/src/llvm/frontend.rs::prepare_single_file_codegen_unit(...)` 与 `crates/scoop/src/commands/build.rs::lower_main_hir_for_build(...)` 两条 production 入口，确认它们都直接返回带 `LoweredHir::materialized_mir()` 的 lowering 产物，没有额外重新组装 `LoweredHir` 并把 canonical MIR / summaries 丢掉；
  - 已复核 `crates/scoopc/src/hir/lower/types.rs` 上 `materialized_mir()` / `materialized_mir_mut()` 的边界说明，确认 HIR 兼容 lowering 已被明确收口为当前 LLVM codegen 所需的兼容输入与 side tables，而 canonical materialized body / summary 挂点则稳定保留在 production 产物上；
  - review 过程中未发现需要插入到 `T5000h0b` 之前的新前置缺陷任务；结论是下一步可直接在现有 production 产物上建立 canonical materialized callable body / summary 视图，而不需要回到“消费侧重新 materialize 一次”的接口；
  - 已验证 `cargo test -p scoopc single_file_frontend_keeps_distinct_effect_row_generic_instances -- --nocapture`、`cargo test -p scoop build_frontend_keeps_distinct_effect_row_generic_instances -- --nocapture`、`cargo test --all`、`cargo run -p scoop -- test`（`fixtures: ok (1201)`）、`cargo clippy --all-targets -- -D warnings` 全部通过。

### [DONE] T5000h0b 建立 production 可复用的 materialized callable body / summary 视图
- 范围：
  - 在 production frontend 产物上抽出“按 materialized callable body 身份索引”的稳定视图，供后续 MIR rewrite / summary / codegen 接线复用；
  - 让后续 pass 可以明确地区分：
    - HIR 兼容 side tables；
    - canonical materialized body / summary 输入；
  - 不在这一步把 LLVM codegen 全面改写到 MIR body，但要把可复用视图和查询边界先收口出来。
- 验收：
  - production 主路径存在稳定的 materialized-body / summary 查询入口；
  - 后续 MIR pass 不需要重新扫描 `MaterializedMir.file` 或回退到 `instance_keys` 字符串集合才能落地。
- 依赖：T5000h0aR
- 完成记录（2026-04-27）：
  - 新增 `crates/scoopc/src/mir/callables.rs`，建立 canonical `MaterializedCallableView` / `MaterializedCallableFamilyView` 查询层，并在 materializer 产物上保留 `InstanceKey -> root_fqn / callable family` 的稳定 side table，避免 production 消费侧继续手扫 `MaterializedMir.file.items` 或自行拼 `instance_keys + summaries.get(...)`；
  - `crates/scoopc/src/mir/materialize.rs` 中的 `MaterializedMir` 现已原生暴露 `callable_view()`，而 `crates/scoopc/src/hir/lower/types.rs` 中的 `LoweredHir` 新增 `materialized_callable_view()`，明确区分 raw `materialized_mir()` 与 production 主路径应优先消费的 canonical callable body / summary 视图；
  - `crates/scoop/src/commands/build.rs` 与 `crates/scoopc/src/llvm/tests.rs` 的 production 回归现已直接通过新 view 断言 root body / owner instance / family summary，而不再只验证“原始 MIR 被保留下来”；
  - 实现/验证过程中暴露并修复了一个既有边界问题：body-less `FunDecl`（例如 declaration-only surface）此前会被新视图误当成“有 body 的 callable”；现已把 view 与 family side table 收紧为只索引 `FunDecl.body.is_some()` 的真实 callable body，并过滤“同一 `InstanceKey` 已 materialize 后又被 declaration-only 输入重复覆盖”的路径，避免 `body_known` summary 与 root body 可见性不一致；
  - 已新增 `mir::callables::tests::callable_view_keeps_overloaded_generic_roots_distinct`，锁定 overloaded generic 在 view 中仍保留 distinct root/body/summary 身份；
  - 已验证 `cargo fmt --all`、`cargo test -p scoopc callable_view_keeps_overloaded_generic_roots_distinct -- --nocapture`、`cargo test -p scoopc single_file_frontend_keeps_distinct_effect_row_generic_instances -- --nocapture`、`cargo test -p scoop build_frontend_keeps_distinct_effect_row_generic_instances -- --nocapture`、`cargo test --all`、`cargo clippy --all-targets -- -D warnings`、`cargo run -p scoop -- test`（`fixtures: ok (1201)`）全部通过。

### [DONE] T5000h0bR Review：确认 canonical materialized-body / summary 视图边界成立
- 重点：
  - canonical 视图是否确实以 materialized body / summary 为中心，而不是换个名字继续包 `instance_keys`；
  - 查询接口是否足够直接支撑后续 MIR rewrite 与 codegen 接线；
  - 是否仍存在“需要在消费侧重复扫描 `MaterializedMir.file`”的边界泄漏。
- 验收：
  - `T5000h0c` 可以直接消费该视图接线 production/codegen 主路径，而不是再拼一套 ad-hoc lookup。
- 依赖：T5000h0b
- 完成记录（2026-04-27）：
  - 已复核 `crates/scoopc/src/mir/callables.rs`、`crates/scoopc/src/mir/materialize.rs` 与 `crates/scoopc/src/hir/lower/types.rs`，确认 canonical 查询入口现在稳定落在 `MaterializedMir::callable_view()` / `LoweredHir::materialized_callable_view()`，并以 `InstanceKey -> family -> root body / callable bodies / per-instance summary` 为中心组织，而不是继续把消费方绑在 `instance_keys` + raw side table 拼装上；
  - 已复核 materializer 出口构造的 `MaterializedCallableFamilies`，确认真实 callable body family 与 declaration-only instance summary 已在产出阶段分流：有 body 的实例会保留 root/body 列表，declaration-only 实例则只保留 family/summary 身份，`root_body()` 与 `summary().body_known` 的边界保持一致；
  - 已复核 production 消费面与回归：`crates/scoop/src/commands/build.rs`、`crates/scoopc/src/llvm/tests.rs` 现都能直接经由 canonical callable view 读取 root body、反查 owner instance 并读取 family summary，不需要重复扫描 `MaterializedMir.file.items` 才能完成这些查询；
  - review 未发现需要插入到 `T5000h0c` 之前的新前置缺陷；结论是下一步可以直接让 LLVM build / single-file entry 显式接入 materialized body / pass 视图，而不需要再补一层 ad-hoc lookup；
  - 已验证 `cargo test -p scoopc callable_view_keeps_overloaded_generic_roots_distinct -- --nocapture`、`cargo test -p scoopc single_file_frontend_keeps_distinct_effect_row_generic_instances -- --nocapture`、`cargo test -p scoop build_frontend_keeps_distinct_effect_row_generic_instances -- --nocapture`、`cargo test --all`、`cargo run -p scoop -- test`（`fixtures: ok (1201)`）、`cargo clippy --all-targets -- -D warnings` 全部通过。

### [DONE] T5000h0c 让 LLVM build / single-file entry 显式接入 materialized body / pass 视图
- 范围：
  - 调整 build / single-file LLVM entry 的输入边界，使其显式拿到 materialized callable body / summary / 后续 pass 产物视图，而不是隐式退回只看 HIR 兼容 body；
  - 为后续 MIR rewrite 留下稳定插入点，避免再把同一套优化抄回 HIR lowering；
  - 保持现有 codegen 功能与测试通过，不在这一步额外混入 summary-driven inlining 规则本身。
- 验收：
  - production/codegen 主路径已经有明确的 materialized body / pass 产物输入面；
  - 后续 `T5000h` 可以在该输入面上接入 MIR rewrite，而不是继续靠 HIR lowering workaround。
- 依赖：T5000h0bR
- 完成记录（2026-04-27）：
  - 新增 `crates/scoopc/src/mir/pass_view.rs`，建立 canonical `MaterializedMirPassView`；`crates/scoopc/src/mir/materialize.rs` 与 `crates/scoopc/src/hir/lower/types.rs` 现分别通过 `MaterializedMir::pass_view()` / `LoweredHir::materialized_pass_view()` 暴露 production/codegen 主路径应显式消费的 materialized body / summary / 后续 MIR pass 查询面，而不再只保留 raw `materialized_mir()`；
  - `crates/scoopc/src/llvm/emit.rs` 现新增 production-only LLVM entry：`emit_minimal_main_ir_from_production_lowered_hir(...)` 以及对应的 `*_to_file_from_production_lowered_hir_with_entry_with_opt_level(...)`；这些入口统一要求 `LoweredHir` 显式携带 `materialized_pass_view()`，若调用方只提供 legacy/测试 lowering，则返回结构化错误 `MissingMaterializedPassView`，不再静默退回只看 HIR 兼容 body；
  - single-file LLVM 路径 `build_minimal_main_module(...)` 与 build 主路径 `crates/scoop/src/commands/build.rs` 中的 `--emit-llvm/--emit-obj/--emit-asm`/link 前 object 生成，现均已切到上述 production-only entry；`crates/scoopc/src/llvm/codegen/mod.rs` 中的 `CompilationUnitCodegenCx` 也开始显式保留 `materialized_pass_view` 作为后续 MIR rewrite / inlining 的稳定接缝，而不是让这层输入再次隐式消失在 frontend 与 backend 之间；
  - 已新增/更新回归：`crates/scoopc/src/llvm/tests.rs` 中 `frontend_codegen_consumes_materialized_generic_direct_call_instances` 现直接走 production codegen entry，另新增 `production_codegen_entry_rejects_lowered_hir_without_materialized_pass_view`；`crates/scoop/src/commands/build.rs` 中新增 `build_production_codegen_entry_consumes_materialized_pass_view`，直接锁定 build frontend 产物可被 production emit 入口消费并保留实例身份；
  - 已验证 `cargo test -p scoopc llvm::tests -- --nocapture`、`cargo test -p scoop build_ -- --nocapture`、`cargo test --all`、`cargo clippy --all-targets -- -D warnings`、`cargo run -p scoop -- test`（`fixtures: ok (1201)`）全部通过。

### [DONE] T5000h0cR Review：确认 production 主路径已经真正消费 materialized MIR body / pass 产物
- 重点：
  - build / single-file frontend 是否已经不再把 MIR 仅当作 instance collection 发现器；
  - materialized MIR body / summary / 后续 pass rewrite 是否有稳定的 production 消费面；
  - 是否避免退回“在 HIR lowering 再抄一份等价优化”的 workaround。
- 验收：
  - 生产主路径已经具备直接承接 MIR inlining rewrite 的边界，`T5000h` 不再被 dump-only 输出边界阻塞。
- 依赖：T5000h0c
- 完成记录（2026-04-27）：
  - 已复核 `crates/scoopc/src/llvm/emit.rs`、`crates/scoopc/src/llvm/frontend.rs`、`crates/scoop/src/commands/build.rs`、`crates/scoopc/src/hir/lower/types.rs`、`crates/scoopc/src/mir/pass_view.rs` 与 `crates/scoopc/src/llvm/tests.rs`，确认 single-file `build_minimal_main_module(...)` 与 build 的 `--emit-llvm/--emit-obj/--emit-asm`/link 前 object 生成，现都必须经由 `LoweredCodegenEntry::from_production_lowered_hir(...)` 或对应 production-only emit 入口显式消费 `LoweredHir::materialized_pass_view()`；
  - 已全文检索 non-test 调用面，确认 production 主路径不再调用 legacy `emit_minimal_main_*_from_lowered_hir*` / `build_main_module_from_lowered_hir(...)` 入口；这些 legacy 入口现仅保留给测试与通用 helper，未重新长回 build / single-file frontend；
  - 已确认 `crates/scoopc/src/llvm/codegen/mod.rs` 中 `CompilationUnitCodegenCx` 继续显式保留 `materialized_pass_view`，`build_main_module_from_codegen_entry(...)` 也以 `debug_assert_eq!` 锁定该边界是否随 production 入口一并进入 codegen 编译单元，因此后续 `T5000h` 可以在这层直接接入 MIR rewrite / inlining，而不必把等价逻辑回抄到 HIR lowering；
  - review 未发现需要插入到 `T5000h` 之前的新前置缺陷任务；结论是 production 主路径已经具备直接承接 materialized MIR body / pass 产物的稳定边界；
  - 已验证 `cargo test -p scoopc production_codegen_entry_rejects_lowered_hir_without_materialized_pass_view -- --nocapture`、`cargo test -p scoopc frontend_codegen_consumes_materialized_generic_direct_call_instances -- --nocapture`、`cargo test -p scoop build_production_codegen_entry_consumes_materialized_pass_view -- --nocapture`、`cargo test --all`、`cargo run -p scoop -- test`（`fixtures: ok (1201)`）、`cargo clippy --all-targets -- -D warnings` 全部通过。

### [DONE] T5000h0d 把 `MaterializedMirPassView` 扩展为可承载 pass-rewritten callable body / summary 的稳定产物层
- 范围：
  - 把 raw `MaterializedMir` 与“当前 MIR pass 链对外暴露的 canonical callable body / summary”显式分层；
  - 为后续 summary-driven inlining 产出的 rewritten callable body、更新后的 per-instance summary 与必要的 family/root 映射提供稳定 side table / view；
  - 保持 raw materialization 仍可作为调试/回归载体，避免把 pass rewrite 结果隐式回写成“所有调用方都只能看到的唯一状态”。
- 验收：
  - `LoweredHir::materialized_pass_view()` 能稳定暴露“当前 pass 后”的 callable body / summary，而不是只能包裹 raw `MaterializedMir`；
  - 后续 `T5000h` 无需把 inline 结果回抄到 HIR lowering，也不必破坏 raw materialization 与 pass 产物的边界。
- 依赖：T5000h0cR
- 完成记录（2026-04-27）：
  - `crates/scoopc/src/mir/pass_view.rs` 现新增 `MaterializedMirPassArtifacts`，把 canonical pass 产物层中的 callable body、per-instance summary 与 family/root 映射显式收口为独立 side table；`MaterializedMirPassView` 不再只是 raw `MaterializedMir` + callable view 的只读薄包装；
  - `crates/scoopc/src/mir/materialize.rs` 中的 `MaterializedMir` 现会在构造 raw materialization 后同步初始化 `pass_artifacts`，并通过 `pass_artifacts()` / `pass_artifacts_mut()` 暴露后续 MIR pass 应写入的 canonical pass 输出层，而不是继续直接覆写 raw `file` / `summaries`；
  - `crates/scoopc/src/mir/pass_view.rs` 现新增 `MaterializedPassCallableView` / `MaterializedPassCallableFamilyView`，让 `LoweredHir::materialized_pass_view()` 能稳定查询 pass 后的 callable body / summary / owner/family 身份；raw `MaterializedMir::callable_view()` 则继续只反映 materialization 原始产物；
  - 已新增回归：
    - `mir::pass_view::tests::pass_view_keeps_rewritten_body_and_summary_separate_from_raw_materialized_mir`
    - `mir::pass_view::tests::pass_view_can_override_family_mapping_without_mutating_raw_materialization`
    以上测试分别锁定“pass body/summary override 不会隐式覆盖 raw materialization”以及“family/root 映射可在 pass side table 中独立重写”；
  - 已验证 `cargo fmt --all`、`cargo test -p scoopc mir::pass_view -- --nocapture`、`cargo test --all`、`cargo run -p scoop -- test`（`fixtures: ok (1201)`）、`cargo clippy --all-targets -- -D warnings` 全部通过。

### [DONE] T5000h0dR Review：确认 pass view 已成为可承载 rewrite 后 callable body / summary 的 canonical pass 输出层
- 重点：
  - pass view 是否已经不再只是 raw `MaterializedMir` 的薄包装；
  - rewritten callable body / summary / family 身份是否有稳定查询入口；
  - 是否避免让后续 pass 通过覆盖 raw materialized MIR 或回退到 HIR side table 来传递结果。
- 验收：
  - `T5000h0e` 可以直接消费该 pass 产物层接线 production codegen，而不必重新发明另一套 pass 输出表示。
- 依赖：T5000h0d
- 完成记录（2026-04-27）：
  - 已复核 `crates/scoopc/src/mir/pass_view.rs`、`crates/scoopc/src/mir/materialize.rs`、`crates/scoopc/src/mir/callables.rs`、`crates/scoopc/src/mir/summary.rs`、`crates/scoopc/src/hir/lower/types.rs` 以及 production 侧 `crates/scoopc/src/llvm/emit.rs` / `crates/scoopc/src/llvm/codegen/mod.rs` / `crates/scoopc/src/llvm/frontend.rs` / `crates/scoop/src/commands/build.rs` 的接线，确认 `MaterializedMirPassView` 当前已经显式建立在 `MaterializedMirPassArtifacts` 之上，而不是继续复用 raw `MaterializedMir` 的只读薄包装；
  - review 过程中首先暴露并修复了一个既有一致性缺陷：`crates/scoopc/src/mir/callables.rs` 中 `MaterializedCallableFamilies::replace_family(...)` 之前只在 debug 下用 `debug_assert!` 防止 callable 跨实例迁移；一旦 release 构建里把某个 callable 重挂到另一个 family，旧 family 的 `callable_fqns` 会静默残留该 symbol，导致同一 callable 同时出现在两个 family。现已改为在重写 family 时同步从旧 owner 的 `callable_fqns` 中移除迁出的 symbol，并对输入 `callable_fqns` 做稳定去重，确保 canonical pass 输出层在 release/debug 下都保持单一归属；
  - 已新增回归 `mir::pass_view::tests::pass_view_rehomes_callable_across_families_without_leaving_duplicate_membership`，锁定“pass family 重写可跨实例迁移 callable，且不会在旧 family 留下重复成员记录”这一边界；结合 `pass_view_keeps_rewritten_body_and_summary_separate_from_raw_materialized_mir` 与 `pass_view_can_override_family_mapping_without_mutating_raw_materialization`，现已覆盖 body/summary override、family side-table 重写、以及跨 family rehome 三类核心行为；
  - 复核结论：pass view 现在已经提供稳定的 rewritten callable body / summary / family 查询面，后续 pass 不需要通过覆盖 raw `MaterializedMir` 或回退到 HIR side table 来传递结果；下一条 `T5000h0e` 可以直接消费这层 canonical pass 输出表示；
  - 已验证 `cargo fmt --all`、`cargo test -p scoopc mir::pass_view -- --nocapture`、`cargo test --all`、`cargo run -p scoop -- test`（`fixtures: ok (1201)`）、`cargo clippy --all-targets -- -D warnings` 全部通过。

### T5000h0e 让 production LLVM codegen 真正消费 pass-rewritten callable body / summary，而不是只携带 `materialized_pass_view`
- 说明：
  - 经核对，`T5000h0e` 同时包含 pass-backed reachability / summary 接线、production callable body-presence 选择，以及后续真正 lowered pass MIR body 的 LLVM 发射路径，单轮过大；
  - 现先把 production codegen 的 callable / summary 查询面切到 pass view，再补齐 pass-rewritten MIR body lowering，避免继续把 `materialized_pass_view` 只当成未消费的边界标记。

### [DONE] T5000h0e1 让 production reachability / body-presence / effect summary 查询优先消费 pass view
- 范围：
  - 让 production reachability 在扫描 materialized callable 时优先读取 `materialized_pass_view` 中的 canonical pass body，而不是总是回退到 HIR `FunDecl.body`；
  - 让 production callable body 发射至少按 pass view 中“当前 callable 是否仍有 canonical body”决定，而不是只看 HIR body 是否存在；
  - 让 known fun outward-effect / suspendability cache 优先使用 pass view 中的 per-instance summary；
  - 不在本子任务实现完整 MIR body -> LLVM lowering，但必须避免 pass view 已删除/声明为 body-unknown 的 callable 仍被 production codegen 静默按 HIR body 发射。
- 验收：
  - pass view 中移除某个 reachable callable body 后，production codegen 不再继续发射该 callable 的 HIR body；
  - effect/suspend 查询路径可从 pass summary 读取 `may_outward_effect`；
  - 后续子任务可以在同一输入面上补齐真正的 pass-rewritten MIR body lowering。
- 依赖：T5000h0dR
- 完成记录（2026-04-27）：
  - `crates/scoopc/src/llvm/reachability.rs` 现会在扫描 pass-visible callable 时优先读取 `MaterializedMirPassView` 中的 canonical MIR body，并从 MIR `Direct` / closure fn-ptr / top-level ref 等结构事实恢复 reachability 输入；未被 pass view 控制的入口、support HIR 与 legacy 测试路径继续走原有 HIR 扫描；
  - `crates/scoopc/src/llvm/emit.rs` 的 reachable body 发射现会检查 pass view 中 callable body 是否仍存在；pass side table 已移除或声明为 body-unknown 的 callable 不再静默按 HIR body 发射；
  - `crates/scoopc/src/mir/pass_view.rs` 现记录哪些 instance summary 是由 pass 显式覆盖的；`crates/scoopc/src/effect_state_machine_analysis.rs` 的 known fun outward-effect / suspendability cache 只消费这些显式 override，避免把初始 raw materialized summary 提前当作完整 effect/state-machine 事实；
  - 已新增 LLVM 回归 `production_codegen_body_emission_observes_pass_view_body_presence` 与 `production_codegen_suspendability_observes_overridden_pass_summary`，分别锁定 body-presence 与 overridden summary 两条 consumption 边界；
  - 验证过程中曾发现初版把 raw materialized summary 直接覆盖 HIR/effect 分析会破坏 async task resume replay IR；已收口为“只有 pass 显式 override 的 summary 才抢占 known suspendability cache”，并复跑相关回归；
  - 已验证 `cargo fmt --all`、`cargo test -p scoopc llvm::tests -- --nocapture`、`cargo test -p scoopc --no-default-features`、`cargo test --all`、`cargo clippy --all-targets -- -D warnings`、`cargo run -p scoop -- test`（`fixtures: ok (1201)`）全部通过。

### [DONE] T5000h0e2 补齐 pass-rewritten MIR callable body 的 production LLVM lowering
- 范围：
  - 为 `MaterializedMirPassView` 中存在 canonical pass body 的 callable 建立实际 LLVM body lowering / HIR-compatible bridge；
  - 确保 MIR pass rewrite 对 callable body 内容的修改会直接影响 production build / single-file LLVM 输出；
  - 清理 `T5000h0e1` 后仍必须依赖 HIR-compatible body 的剩余路径。
- 验收：
  - 后续 `T5000h` 产出的 inline 后 MIR body 能直接成为 production LLVM body 发射输入，而不是只影响 reachability 或 summary side table；
  - production 主路径不需要把等价 inlining 逻辑回抄到 HIR lowering。
- 依赖：T5000h0e1
- 完成记录（2026-04-27）：
  - `crates/scoopc/src/mir/pass_view.rs` 现会记录哪些 callable body 由 MIR pass 显式覆盖或移除，使 production codegen 能区分 raw materialization body 与真正 pass-rewritten body；
  - 新增 `crates/scoopc/src/llvm/codegen/mir_body.rs`，为显式覆盖的 pass MIR callable body 建立 production LLVM lowering 入口，当前覆盖 inlining 主线所需的 HIR-compatible MIR 子集：local / constant operand、direct call、primitive unary / binary、基础 CFG、return / goto / condbr / unreachable；
  - `crates/scoopc/src/llvm/emit.rs` 的 reachable body 发射在遇到显式覆盖的 pass body 时改走 `codegen_top_level_mir_fun(...)`，未被 pass 改写的 raw materialization body 继续走现有 HIR 兼容路径；遇到尚未具备明确 production lowering 语义的 pass MIR 节点会返回结构化 `UnsupportedMainBody`，不会静默回退 HIR body；
  - 新增回归 `llvm::tests::production_codegen_lowers_overridden_pass_mir_body`，将 `wrap::<Int>` 的 pass MIR direct-call target 从 `id::<Int>` 改为 `replacement`，确认 production LLVM 中 `wrap::<Int>` 的函数体直接观察到 pass-rewritten MIR body；
  - 已验证 `cargo fmt --all`、`cargo test -p scoopc production_codegen_lowers_overridden_pass_mir_body -- --nocapture`、`cargo test -p scoopc llvm::tests -- --nocapture`、`cargo test -p scoopc --no-default-features`、`cargo test --all`、`cargo run -p scoop -- test`（`fixtures: ok (1201)`）、`cargo clippy --all-targets -- -D warnings` 全部通过。

### [DONE] T5000h0eR Review：确认 production codegen 已真正切到 pass-rewritten callable body / summary 输入面
- 重点：
  - reachability / callable body 选择 / effect-suspend 查询是否已经真正消费 pass view，而不是只传递该参数；
  - 是否还残留“为了让优化生效，只能再把等价逻辑抄回 HIR/codegen”的 workaround；
  - pass 后 callable body 身份是否已成为 production codegen 可直接观察到的 canonical 边界。
- 验收：
  - `T5000h` 可以在 MIR 层实现 rewrite 并直接影响 production 主路径，而不是停留在 dump-only 输出。
- 依赖：T5000h0e2
- 完成记录（2026-04-27）：
  - 已复核 `crates/scoopc/src/llvm/emit.rs`，确认 production 入口会把 `materialized_pass_view` 传入 reachability，reachable body emission 会按 pass view body-presence 决定是否发射，并且显式 pass-overridden callable 会进入 `codegen_top_level_mir_fun(...)` 而不是回退 HIR body；
  - 已复核 `crates/scoopc/src/llvm/reachability.rs`，确认 pass-visible callable 的 reachability 扫描读取 canonical pass MIR body，可从 MIR direct call / closure fn-ptr / top-level ref 等结构事实恢复可达输入；
  - 已复核 `crates/scoopc/src/effect_state_machine_analysis.rs`，确认 known fun outward-effect / suspendability cache 只消费 pass 显式 override 的 summary，不会把 raw materialized summary 提前当成完整后端 effect 事实；
  - 已复核 `crates/scoopc/src/mir/pass_view.rs` 与 `crates/scoopc/src/llvm/codegen/mir_body.rs`，确认 raw materialized body / summary 与 pass-overridden body / summary 已分层，显式 pass body rewrite 可直接改变 production LLVM body，unsupported pass MIR 节点会结构化报错而非静默走 HIR workaround；
  - review 过程中发现并修复一个既有边界不一致：`codegen_top_level_mir_fun(...)` 原先在调用 `build_fun_callee_suspend_plan(...)` 后才切换 `current_source_id`，现已改为与 HIR lowering 一致，先切到当前函数源文件再做 suspend-plan 检查，避免跨文件 pass-overridden callable 的 effect/suspend 分析使用入口源文件上下文；
  - 额外验证了 `member_call_virtual_dispatch_override_basic.scoop` 与 `member_call_interface_dispatch_basic.scoop` 的 production build 和运行输出，确认 pass-view reachability 接线未破坏 vtable / itable 端到端路径；
  - 已验证 `cargo fmt --all`、`cargo test -p scoopc production_codegen_ -- --nocapture`、`cargo test -p scoopc mir::pass_view -- --nocapture`、`cargo test -p scoopc --no-default-features`、`cargo test --all`、`cargo run -p scoop -- test`（`fixtures: ok (1201)`）、`cargo clippy --all-targets -- -D warnings` 全部通过。

### T5000h 在 MIR 层实现 summary-driven inlining
- 拆分记录（2026-04-27）：
  - 经核对，原任务同时包含 pass-visible direct-call rewrite、caller-side provenance 覆盖面，以及 `DirectCallOnly` 高阶参数摊平，单轮过大；
  - 拆为 `T5000h1` / `T5000h2` / `T5000h3` 三个可独立验收的子任务，保持原验收目标不变；
  - 不允许按函数名白名单触发；所有收益必须来自 MIR 结构、per-instance summary 与 provenance。

### [DONE] T5000h1 在 pass-visible MIR callable body 上实现保守 small direct-call inlining
- 范围：
  - 在 `MaterializedMirPassView` 可见的 callable body 上运行 MIR inlining pass；
  - eligibility 来自 per-instance summary：`body_known`、非递归、小体量；
  - 先覆盖普通小 direct call 的边界消除，不按函数名白名单触发；
  - inline 结果写入 `MaterializedMirPassArtifacts` 的 rewritten body / summary，让 production LLVM 主路径直接观察。
- 验收：
  - 类似 `wrap<Int> -> id<Int>` 的 materialized wrapper body 在 production LLVM 中不再保留被内联的小 direct-call 边界；
  - raw materialized MIR 与 pass rewritten body 继续分层；
  - unsupported MIR 节点保持结构化边界，不静默回退 HIR workaround。
- 依赖：T5000h0eR
- 完成记录（2026-04-27）：
  - 新增 `crates/scoopc/src/mir/inline.rs`，在 pass-visible monomorphic callable roots 上运行保守 summary-driven inlining；eligibility 由 `body_known`、非递归、`size_cost <= 16` 与 straight-line MIR body 结构决定，不按函数名白名单触发；
  - materialization 返回前会运行 `run_summary_driven_inlining(...)`，将 rewritten callable body 与保守更新后的 summary 写入 `MaterializedMirPassArtifacts`，raw `MaterializedMir.file` / raw callable view 保持不变；
  - 新增 `summarize_pass_rewritten_fun(...)`，为 rewritten body 生成 pass summary，并保留上一版 outward-effect / recursion 上界，避免单体重算少看跨函数 effect fixed point；
  - 新增 MIR 回归确认 `wrap<Int> -> id<Int>` 的 pass body 已移除 direct call、raw MIR 未被覆盖，且非 `id/wrap` 命名的 `project/shell` 形状同样可被结构性内联；
  - 新增 LLVM 回归 `production_codegen_observes_summary_driven_mir_direct_call_inlining`，确认 production LLVM 中 `wrap::<Int>` 不再调用 `id::<Int>`；
  - 验证过程中发现并修复既有 TypeStore 边界问题：pass MIR body local `TypeId` 属于 `MaterializedMir.types`，production MIR body lowering 现从 `MaterializedMirPassView::materialized().types` 读取 MIR local type，并在 aggregate 需要时映射回 codegen TypeStore；
  - 已验证 `cargo fmt --all`、`cargo test -p scoopc mir::inline -- --nocapture`、`cargo test -p scoopc production_codegen_observes_summary_driven_mir_direct_call_inlining -- --nocapture`、`cargo test -p scoopc llvm::tests -- --nocapture`、`cargo test -p scoopc mir:: -- --nocapture`、`cargo test -p scoopc --no-default-features`、`cargo test --all`、`cargo run -p scoop -- test`（`fixtures: ok (1201)`）、`cargo clippy --all-targets -- -D warnings` 全部通过。

### [DONE] T5000h2 让 caller-side MIR pass 能安全覆盖 request-root / non-generic callable body
- 范围：
  - 为后续 call-site provenance inlining 提供 caller body rewrite 边界；
  - 只在 pass 明确改写且 production MIR lowering 覆盖该 body 子集时，把 request-root / non-generic callable body 写入 pass artifacts；
  - 避免把所有 non-generic HIR 兼容 body 无条件并入 pass view，防止 reachability 与 effect 查询边界扩大过快。
- 验收：
  - call-site provenance 优化可以改写真实 caller body，而不是只能改写 generic callee family；
  - 未被 pass 改写的 non-generic body 继续走现有 HIR 兼容 lowering。
- 依赖：T5000h1
- 完成记录（2026-04-27）：
  - `MaterializedMir` 现在保留 request-root 可达的 non-generic caller MIR body 作为 caller-side pass 候选输入；这些候选不会默认写入 `MaterializedMirPassArtifacts`，因此不会无条件出现在 pass view；
  - non-generic caller 候选在记录前会复用 materializer 的 site binding / instance FQN 重写逻辑，使 caller body 中的 generic direct-call target 与 pass-visible monomorphic callee identity 对齐；
  - `run_summary_driven_inlining(...)` 现在除了 pass-visible monomorphic callable roots，也可以改写 caller-side non-generic 候选；只有实际发生 rewrite 且 body 保持在当前 production MIR body lowering 支持的结构子集内时，才会写入 pass artifacts；
  - entry `main` 仍由专用 HIR `codegen_main_exit_code` 路径降低，当前不会发布 pass MIR override，避免 reachability 观察到 production entry lowering 尚未消费的 body；
  - production LLVM reachability / body emission 现在会识别没有 instance owner 的显式 pass body override，从而能扫描并降低真正被改写的 non-generic caller body；
  - 新增 MIR 回归 `caller_side_inlining_publishes_only_rewritten_non_generic_body`，确认 caller body 可被结构性 inline 改写、未改写 non-generic body 不进入 pass view；
  - 新增 LLVM 回归 `production_codegen_observes_caller_side_mir_inlining_for_non_generic_body` 与 `production_reachability_scans_overridden_non_generic_pass_body`，分别确认 production LLVM 消费 caller-side rewritten MIR body，以及 reachability 会扫描 ownerless pass override 中新增的 direct call；
  - 已验证 `cargo fmt --all`、`cargo test -p scoopc mir::inline -- --nocapture`、`cargo test -p scoopc production_codegen_observes_caller_side_mir_inlining_for_non_generic_body -- --nocapture`、`cargo test -p scoopc production_reachability_scans_overridden_non_generic_pass_body -- --nocapture`、`cargo test -p scoopc mir::pass_view -- --nocapture`、`cargo test -p scoopc llvm::tests -- --nocapture`、`cargo test -p scoopc mir:: -- --nocapture`、`cargo test -p scoopc --no-default-features`、`cargo test --all`、`cargo clippy --all-targets -- -D warnings`、`cargo run -p scoop -- test`（`fixtures: ok (1201)`）全部通过。

### [DONE] T5000h3 接入 `DirectCallOnly` + known provenance 的高阶 wrapper 摊平
- 范围：
  - 使用 `ParamUseSummary::DirectCallOnly` 与 call-site known provenance 驱动内联；
  - 支持 known direct function / known closure provenance，把 wrapper 内部函数值参数调用摊平成结构化 direct MIR call；
  - 覆盖 `map` / `filter` / `forEach` 类 wrapper 形状的结构性收益，不按函数名或库函数白名单触发。
- 验收：
  - 高阶 wrapper 函数内对函数值参数的调用边界来自 summary / provenance 被摊平；
  - `@Inline` 仍只是 hint，不是主机制；
  - codegen 不再继续承担“内联后才能去掉的额外高层调用边界”。
- 依赖：T5000h2
- 完成记录（2026-04-27）：
  - `crates/scoopc/src/mir/inline.rs` 现为 summary-driven inlining pass 增加 block-local callable provenance，识别 `TopLevelRef`、`MakeClosure`、`Use` 传播，以及 direct-call result summary 中的 `DirectFunction` / `KnownClosure` / `Param` 简单来源；
  - direct-call callee 的 `ParamUseSummary::DirectCallOnly` 参数现在必须在调用点具备 known provenance 才会触发高阶 wrapper 摊平；展开 callee body 时，对这些参数的 `FunValue` 调用会被重写为结构化 `DirectCall` 或 `ClosureCall`，不再靠函数名或库函数白名单触发；
  - 对源码层顶层函数值先降成 non-capturing closure wrapper 的现有形态，inliner 会在 MIR pass 层识别“只转发到 direct function 且实参一一对应”的 closure，并把该 provenance 归一化为 direct function；随后移除由此变成死代码的 `TopLevelRef` / `MakeClosure` pass artifact，保证 production 可发布 body 不携带无用 closure 构造；
  - caller-side pass 发布边界仍由 `pass_publishable_caller_body(...)` 保守把关：direct-function provenance 的高阶 wrapper 可以进入 production LLVM lowering，普通 known closure provenance 目前先在 MIR rewrite 层收缩为结构化 `ClosureCall`，不会绕过 production MIR body 支持集；
  - 新增 MIR 回归 `direct_call_only_param_with_direct_function_provenance_flattens_wrapper`，确认 `DirectCallOnly + provenance` 驱动 caller-side wrapper 摊平并继续消除具体 direct function 的小调用边界；
  - 新增 MIR 回归 `direct_call_only_param_with_known_closure_provenance_rewrites_to_closure_call`，确认 known closure provenance 会把 wrapper 内部模糊 `FunValue` 调用收缩为结构化 `ClosureCall`；
  - 新增 LLVM 回归 `production_codegen_observes_direct_call_only_provenance_wrapper_flattening`，确认 production LLVM 消费 provenance-driven caller rewrite，`caller` 不再调用高阶 wrapper 或被传入的具体 direct function；
  - 已验证 `cargo fmt --all`、`cargo test -p scoopc mir::inline -- --nocapture`、`cargo test -p scoopc production_codegen_observes_direct_call_only_provenance_wrapper_flattening -- --nocapture`、`cargo test -p scoopc --no-default-features`、`cargo test --all`、`cargo run -p scoop -- test`（`fixtures: ok (1201)`）、`cargo clippy --all-targets -- -D warnings` 全部通过。

### [DONE] T5000hR Review：确认 inlining 已走 summary / structure 路线
- 重点：
  - 是否仍有特定函数名 hard-code；
  - `DirectCallOnly` 与 provenance 是否真的在驱动高阶内联；
  - `@Inline` 是否仍只是 hint，而不是主机制。
- 验收：
  - 内联主路径已经是结构驱动；没有退回“为几个库函数特判”的方案。
- 依赖：T5000h3
- 完成记录（2026-04-27）：
  - 已复核 `crates/scoopc/src/mir/inline.rs` 与 `crates/scoopc/src/mir/summary.rs`，确认 summary-driven inlining 的触发条件来自 `body_known`、非递归、小体量、straight-line body、`ParamUseSummary::DirectCallOnly` 与 call-site callable provenance；未发现 `map` / `filter` / `forEach` 或其它库函数名白名单；
  - 已确认 `DirectCallOnly` 与 known provenance 是高阶 wrapper 摊平的必要条件：direct function provenance 可把 wrapper 内部 `FunValue` 调用改写为 `DirectCall`，known closure provenance 只收缩为结构化 `ClosureCall`，不会绕过当前 production MIR body lowering 支持集；
  - 已确认 `@Inline` 没有进入本轮 inlining 主路径；当前自动优化由 MIR summary / provenance 与 `OptLevel` 控制；
  - review 过程中修复了半成品中暴露的真实问题：`run_summary_driven_inlining(...)` 原先无条件运行，现改为 `OptLevel::enables_summary_driven_mir_inlining()` 控制，`O0` 不发布 summary-driven rewritten body，`O1+` 保持现有 inlining pass；同时把 build / single-file frontend 的 opt-level 传入 MIR materialization，新增回归 `production_codegen_respects_mir_inlining_opt_level_gate` 锁定 `O0` / `O2` 差异；
  - review 同时修正了过宽的 opt-level gate：`T5000g` 已建立的 exact / singleton dispatch directization 属于当前必要 call classification 与 instance discovery，`O0` 仍保留该路径，避免 `member_call_devirt_final_receiver_direct_call.scoop` 与 owner-specialized effect-generic member materialization 回归；`OPTIMIZATION.md` 已同步补充这一边界；
  - 已清理新增 opt-level API 的 clippy 问题，将 request-source + opt-level 组合为 `MirInstanceCollectionOptions`，避免扩散超长参数列表；
  - 已验证 `cargo fmt --all`、`cargo test -p scoopc production_codegen_respects_mir_inlining_opt_level_gate -- --nocapture`、`cargo test -p scoopc mir::inline -- --nocapture`、`cargo test -p scoopc llvm::tests -- --nocapture`、`cargo test -p scoopc --no-default-features`、`cargo test --all`、`cargo clippy --all-targets -- -D warnings`、`cargo run -p scoop -- test`（`fixtures: ok (1201)`）全部通过。

### T5000i 加入 continuation / closure escaping analysis，并把 effect/state-machine planning 迁到正确边界
- 说明：
  - 经核对，原任务同时包含 MIR 逃逸事实层、non-escaping closure 简化、continuation escape 分析消费，以及 effect/state-machine planning 迁出 LLVM backend 四类改动，单轮过大；
  - 现拆成以下实现子任务，先建立 MIR pass 产物层可消费的 escape facts，再让后续 simplification 与 effect planning 迁移共享同一事实来源。

### [DONE] T5000i1 建立 MIR-level closure / continuation escape facts side table
- 范围：
  - 新增 backend-agnostic 的 MIR escape analysis 产物，覆盖最保守的：
    - `MakeClosure` 产生的 closure value 是否只被本地直接调用；
    - `Continuation.resume(...)` 的 continuation local 是否只被本地 resume；
    - 被返回、传参、装入 aggregate/capture box、或进入未建模 MIR 节点时统一视为 escaping / unknown。
  - 将结果挂到 `MaterializedMirPassArtifacts` / `MaterializedMirPassView`，作为 production MIR pass 产物层的稳定 side table。
  - 受优化等级控制：`-O0` 不运行新增 escape analysis，`O1+` 在 summary-driven inlining 之后发布 facts。
- 验收：
  - pass view 能按 callable FQN 查询 closure / continuation escape facts；
  - non-escaping 与 escaping 的最小 closure / continuation case 有单元测试覆盖；
  - `O0` 与 `O1+` 的 escape facts 发布行为有回归覆盖。
- 依赖：T5000hR
- 完成记录（2026-04-28）：
  - 新增 `crates/scoopc/src/mir/escape.rs`，以 backend-agnostic MIR pass 分析 closure / continuation escape facts；当前保守覆盖 `MakeClosure` 本地 direct closure call、`Continuation.resume(...)` 本地 resume、返回/传参/aggregate/capture-box/未建模 MIR 的 escaping 或 unknown 分类；
  - `MaterializedMirPassArtifacts` / `MaterializedMirPassView` 现发布 `MaterializedEscapeFacts`，可按 callable FQN 查询 `CallableEscapeFacts`，后续 closure simplification 与 effect planning 可复用同一 side table；
  - 新增 `OptLevel::enables_mir_escape_analysis()`，materialization 在 summary-driven inlining 之后、且仅 `O1+` 运行 escape analysis；`O0` 保持不发布新增 facts；
  - 新增单元测试覆盖 non-escaping / escaping closure、non-escaping / escaping continuation，以及 production pass view 的 `O0` / `O2` 发布差异；
  - 已验证 `cargo fmt --all`、`cargo test -p scoopc mir::escape -- --nocapture`、`cargo test -p scoopc mir:: -- --nocapture`、`cargo test -p scoopc llvm::tests -- --nocapture`、`cargo test -p scoopc --no-default-features`、`cargo test --all`、`cargo clippy --all-targets -- -D warnings`、`cargo run -p scoop -- test`（`fixtures: ok (1201)`）全部通过。

### [DONE] T5000i1P1 修复 build frontend MIR request roots 对 support sources 过宽的问题
- 范围：
  - 按最新提交记录的 `ISSUES.md` P1 优先处理 build frontend 过度物化问题；
  - 单文件 build 与 single-file LLVM frontend 对齐：只有用户入口源贡献 initial `MonomorphKey` 与 request-root 可达扫描；
  - cone build 先采用保守 consumer-cone 全 source request roots 策略，但 stdlib / sysroot support sources 不再贡献 initial monomorph seeds。
- 验收：
  - `scoop build` frontend 不再对 stdlib / sysroot support sources 调用 `check_file_exprs_with_monomorph_keys(...)`；
  - `lower_main_hir_for_build(...)` 显式传入 request roots，而不是把全部 `files_to_lower` 自动视为 request roots；
  - 单文件与 cone 两条 build frontend 路径都有回归覆盖。
- 依赖：最新提交 `ISSUES.md` P1 / T5000i1
- 完成记录（2026-04-28）：
  - `crates/scoop/src/commands/build.rs` 新增 build-input 级 `mir_request_source_paths()` / `is_mir_request_source_index(...)`，集中表达 production build 的 MIR request-root 策略；
  - typecheck 阶段现只对 request sources 收集 monomorph keys；support sources 仍完整普通 typecheck，后续继续参与 lowering / fun index；
  - `lower_main_hir_for_build(...)` 已改用 `lower_for_compilation_unit_multi_files_via_mir_instance_collection_with_request_sources(...)`，将 request roots 显式传给 MIR materializer；
  - 新增 `build_frontend_single_file_request_roots_exclude_stdlib_support_sources` 与 `build_frontend_cone_request_roots_exclude_stdlib_support_sources`；
  - `ISSUES.md` 已把该 P1 标记为已修复，并保留后续 source-aware monomorph request / reachable-block 口径收口建议；
  - 已验证 `cargo fmt --all --check`、`cargo test -p scoop build_frontend_ -- --nocapture`、`cargo test -p scoopc mir::materialize -- --nocapture`、`cargo test --all`、`cargo clippy --all-targets -- -D warnings`、`cargo run -p scoop -- test`（`fixtures: ok (1201)`）全部通过。

### [DONE] T5000i1P2 为 monomorph request 增加 call-site source，并让 materializer seed 过滤非 request source
- 范围：
  - 按最新提交 `ISSUES.md` 仍开放的 P1 继续收口：`MonomorphKey` 只能表示实例身份，不能表达请求来源；
  - 新增带来源的 request wrapper，记录 request source 与 call span；
  - production build / single-file LLVM frontend 与 MIR materializer seed 主路径改为消费 source-aware request；
  - support source 中即使存在被收集到的 request，也不能在不属于 request roots 时成为 initial seed。
- 验收：
  - `MonomorphKey` 不再被当作 production materializer 的唯一初始请求输入；
  - `seed_requests(...)` 能按 `request_source_paths` 过滤 `MonomorphRequest`；
  - 有回归覆盖 support-source request 被过滤，以及 request-source request 正常生效。
- 依赖：最新提交 `ISSUES.md` P1 / T5000i1P1
- 完成记录（2026-04-28）：
  - 新增 `MonomorphRequest { key, request_source_path, call_span }`，`MonomorphKey` 继续只表达实例身份；
  - typecheck 新增 `check_file_exprs_with_monomorph_requests(...)`，`TypeLowering::record_monomorph_call(...)` 现在保留当前源文件路径与调用点 span；
  - build frontend 与 single-file LLVM frontend 已改为收集并传递 `MonomorphRequest`，HIR via-MIR lowering 与 MIR materializer 主入口也已切到 source-aware request；
  - `MirInstanceMaterializer::seed_requests(...)` 现在按 `request_sources` 过滤 initial monomorph seeds，避免 support source 中的请求绕过 request-root 过滤；
  - 新增 `materializer_filters_initial_monomorph_requests_by_call_site_source`，确认 support source request 在 main-only request roots 下被过滤，而同一 request 来自 request source 时仍正常物化；
  - `ISSUES.md` 已把该 P1 标记为已修复；
  - 已验证 `cargo check -p scoopc`、`cargo test -p scoopc --no-run`、`cargo test -p scoopc materializer_filters_initial_monomorph_requests_by_call_site_source -- --nocapture`、`cargo test -p scoopc mir::materialize -- --nocapture`、`cargo test -p scoop --no-run`、`cargo test -p scoop build_frontend_ -- --nocapture`、`cargo fmt --all --check`、`cargo test --all`、`cargo clippy --all-targets -- -D warnings`、`cargo run -p scoop -- test`（`fixtures: ok (1201)`）通过。

### [DONE] T5000i1P3 修复 production MIR request roots 仍按源文件级过度物化的问题
- 范围：
  - 按最新提交 `ISSUES.md` 的第一个未修复 P2，新增 entry-main rooted materialization mode；
  - production build / single-file LLVM frontend 不再把 request source 内全部 callable 当作实例 root；
  - `main` / export entry points 可达函数体内的 request 仍可作为 seed，未从入口触达的同源 helper 不应 materialize；
  - dump / 调试路径继续保留 source-file rooted 模式，避免改变调试入口语义。
- 验收：
  - 单文件 build 中，同一源文件未从 `main` 触达的 generic helper 不会被物化；
  - cone build 中，consumer cone 内非入口源文件的未触达 generic helper 不会被物化；
  - 从 entry-main 真实触达的 generic/effect-row/member getter 实例仍会保留；
  - 零参数顶层 direct call 在 MIR 中不再误落为 `Todo("dispatch receiver lowering pending")`。
- 依赖：T5000i1P2
- 完成记录（2026-04-28）：
  - 新增 `MaterializeRequestRootMode` 与 `MaterializeCompilationUnitOptions`，source-file rooted 模式保留给 dump / 旧测试路径，production build / single-file LLVM frontend 显式选择 entry-main rooted 模式；
  - `collect_request_root_fun_keys(...)` 现按模式区分：source-file 模式保持原行为，entry-main 模式只从选定 `main` 与 `Index` 中声明的 export entry points 出发；
  - entry-main 模式下，initial `MonomorphRequest` seed 还必须位于已扫描到的 entry 可达函数体内，避免同一 request source 中未触达 helper 的泛型请求直接成为实例 root；block 级精确过滤仍留给下一条 P4；
  - HIR-only synthetic direct-call fallback 现按实际扫描到的 reachable MIR function body 消费，不再预先只扫初始 source roots；这保留了 async task lowering 里 `__task_step_ready<T>` 等 HIR synthetic generic helper 的实例发现能力；
  - production build 的单文件 / cone 回归已覆盖未触达 helper 不再 materialize，同时旧的 effect-row 与 owner-specialized getter build 回归已改成从 `main` 真实触达被测实例；
  - 修复过程中暴露并一并修复了零参数顶层 direct call 的 MIR lowering bug：`entry()` 现在稳定 lowering 为 `CallKind::Direct`，新增 `tests/fixtures/mir/direct_zero_arg_call.{scoop,mir}` 锁定该行为；
  - 修复 full fixture 验证暴露的 async task 回归：MIR direct-call instance inference 现在可从赋值目标 local 的结果类型推断只出现在返回类型中的 type 参数，配合 reachable HIR fallback 恢复 `async_fun_task_runtime_basic.scoop` 的 `__task_step_ready::<Int>` 实例；
  - `ISSUES.md` 已把对应 P2 标记为已修复；
  - 已验证 `cargo fmt --all --check`、`cargo test -p scoop build_frontend_ -- --nocapture`、`cargo test -p scoopc mir::materialize -- --nocapture`、`cargo run -p scoop -- test --fixtures tests/fixtures/mir`，并单独确认 `cargo run -p scoop -- run tests/fixtures/run-pass/async_fun_task_runtime_basic.scoop` 通过。

### [DONE] T5000i1P4 让 materializer request-root 可达扫描使用 MIR reachable-block 过滤
- 范围：
  - 按最新提交 `ISSUES.md` 剩余 P2，修复 `scan_reachable_non_generic_fun(...)` 当前直接遍历 `body.blocks` 全量 block 的口径；
  - 与 LLVM reachability 对齐：`body.reachable_blocks()` 成功时只扫描可达 blocks，失败时再保守扫描全部 blocks；
  - entry-main 模式中按 reachable function span 放行 initial request 的临时粗粒度边界，也应同步收口到 reachable block / reachable statement 级别。
- 验收：
  - MIR 不可达 block 中的 generic direct-call 不会产生额外实例，除非 CFG reachable-block 计算失败触发保守回退；
  - materializer 与 LLVM reachability 对 MIR block 可达性的判断口径一致；
  - 有覆盖不可达 block generic call 的回归。
- 依赖：T5000i1P3
- 完成记录（2026-04-28）：
  - 新增 `reachable_body_block_indices(...)`，request-root 可达扫描现与 LLVM reachability 对齐：优先使用 `Body::reachable_blocks()`，CFG 验证失败时保守回退扫描全部 block；
  - `scan_reachable_non_generic_fun(...)` 不再遍历全量 `body.blocks`，只从可达 block 中收集 generic direct-call / top-level ref 实例；
  - entry-main 模式下 initial `MonomorphRequest` seed 的 fallback 已从“可达函数 span”缩小到“可达语句 span”，不再让同一函数内不可达 block 的 request 通过函数级粗粒度放行；
  - request-root caller-side pass candidate rewrite 同步改为只重写可达 block，避免不可达 block 在 rewrite 阶段绕过扫描过滤并 enqueue 泛型实例；
  - 修复过程中暴露并一并修复了既有 MIR CFG 边界问题：`TerminatorKind::Handle` 此前没有把 handler body / arms / finally 作为 CFG successor 暴露，导致 `reachable_blocks()` 会把语义上可执行的 handle 内部 block 判为不可达；现已为 handle terminator 增加保守 successor targets，并更新 `handle_perform.mir` golden；
  - 完整 fixture 验证继续暴露顶层 immutable `val` initializer 的可达性缺口：入口函数读取的顶层值会在运行时 lazy init，其 initializer 中的 generic call 也必须参与实例请求过滤；materializer 现在在可达 MIR `TopLevelRef` 命中顶层 immutable value 时，递归标记该 initializer span 及其引用的顶层值 initializer span；
  - 新增 `request_root_scan_ignores_generic_calls_in_unreachable_mir_blocks`，手动向 `main` 的 MIR 追加结构不可达的 `id<Int>` direct-call，确认不会进入 initial requests、不会产生 instance key，也不会物化 callable body；
  - `ISSUES.md` 已把对应 P2 标记为已修复；
  - 已验证 `cargo fmt --all --check`、`cargo test -p scoopc request_root_scan_ignores_generic_calls_in_unreachable_mir_blocks -- --nocapture`、`cargo test -p scoopc production_codegen_suspendability_observes_overridden_pass_summary -- --nocapture`、`cargo test -p scoopc mir::tests:: -- --nocapture`、`cargo test -p scoopc mir::materialize -- --nocapture`、`cargo test -p scoop build_frontend_ -- --nocapture`、`cargo run -p scoop -- test --fixtures tests/fixtures/mir`、`cargo run -p scoop -- test --fixtures tests/fixtures/run_pass_cone/cross_file_generic_top_level_val_basic`、`cargo test --all`、`cargo clippy --all-targets -- -D warnings` 通过。

### [DONE] T5000i1P5 让 production LLVM body emission 默认消费 materialized MIR body
- 范围：
  - 按最新提交 `ISSUES.md` 剩余 P2，收口 production emit 仍主要走 HIR 兼容 body 的半切换边界；
  - 当前 `LoweredHir::materialized_pass_view()` 已存在，reachability / body presence / summary 查询已消费 pass view，但普通 body emission 仍在未 override 时回退 HIR；
  - 需要让 production LLVM lowering 主路径默认消费 materialized MIR callable body / pass-rewritten body，并仅在明确不支持的声明或边界上保守保留 HIR 兼容路径。
- 验收：
  - production LLVM codegen 不再只在 pass override 时才走 MIR body bridge；
  - materialized MIR body / summary / pass view 成为普通 callable emission 的 canonical 输入面；
  - 现有 HIR 兼容 body 不再掩盖 materialized MIR 与最终 codegen 可达集合之间的不一致。
- 依赖：T5000i1P4
- 完成记录（2026-04-28）：
  - production body emission 现在通过 `canonical_materialized_callable_body(...)` 读取 `MaterializedMirPassView` 中的 canonical callable body；materialized instance 的 raw body 与显式 pass-rewritten body 都可进入 MIR bridge，不再只有 `callable_body_is_overridden(...)` 时才走 `codegen_top_level_mir_fun(...)`；
  - 对未被 pass override 的 raw materialized body，新增 bridge 支持性预检：当前 MIR bridge 已支持的纯 scalar / direct-call / 基础控制流形状默认走 MIR；effect/state-machine body、函数值 `TopLevelRef`、closure/fun-value/dynamic dispatch、tuple/member/capture/pattern/perform 等尚未支持的 MIR 节点继续走 HIR 兼容发射边界；
  - 显式 pass override 不走上述 HIR 兼容回退：若 pass 发布了当前 bridge 仍不支持的 MIR body，仍会暴露结构化 `UnsupportedMainBody`，避免把 pass rewrite 静默吞回 HIR；
  - 新增回归 `production_codegen_lowers_raw_materialized_mir_body_without_pass_override`，确认 O0 下未被 pass override 的 `wrap::<Int>` raw materialized MIR body 也通过 MIR bridge 发射，并直接消费 materialized `id::<Int>` call target；
  - 已验证 `cargo fmt --all --check`、`cargo test -p scoopc production_codegen -- --nocapture`、`cargo test -p scoopc llvm::tests -- --nocapture`、`cargo test -p scoopc mir::materialize -- --nocapture`、`cargo test -p scoop build_frontend_ -- --nocapture`、`cargo test --all`、`cargo clippy --all-targets -- -D warnings` 通过。

### [DONE] T5000i2 基于 escape facts 接入最小 non-escaping closure simplification
- 范围：
  - 消费 `T5000i1` 的 MIR escape facts；
  - 仅对已证明 non-escaping、body-known、调用形状可保持语义的 closure value 做最保守简化；
  - 不通过函数名或 fixture 形状特判。
- 验收：
  - closure simplification 只读取 pass-view escape facts，不重新在 LLVM codegen 现场推断；
  - escaping / unknown closure 不被错误简化。
- 依赖：T5000i1, T5000i1P1, T5000i1P2, T5000i1P3, T5000i1P4, T5000i1P5
- 完成记录（2026-04-28）：
  - 新增 `crates/scoopc/src/mir/closure_simplify.rs`，在 MIR materialization pass 链中消费 `MaterializedMirPassView::escape_facts()`；当前只简化已证明 `NonEscaping`、同一 callable 内 direct closure call 次数为 1、`Unit` env、body-known 且 closure body 可直线展开的最小形状；
  - pass 不在 LLVM codegen 现场重新推断 escape 状态，也不依赖函数名或 fixture 形状；escaping / unknown closure 保守不改写；
  - materialization 现在于 `O1+` escape analysis 后运行 non-escaping closure simplification，若发生改写则刷新 escape facts，并为被改写实例重算 summary；
  - 修复 closure MIR lowering 既有缺口：表达式 lambda body 正常完成时显式生成 `Return(Some(body_result))`，从而让 body-known closure 有可保持语义的 MIR body；
  - 验证过程中发现并修复一个前置 MIR 类型缺口：比较 / 相等 / 逻辑二元表达式的 MIR 结果 local 现在明确为 `Bool`，避免 raw materialized MIR bridge 在 `generic_fun_recursion.scoop` 这类基础控制流实例上看到 `Any` 分支条件；
  - 新增单元测试覆盖 non-escaping closure 被简化、escaping closure 不被简化、`O0` 无 escape facts 时不简化，以及 generic template 中比较条件 local 为 `Bool`；
  - 已验证 `cargo fmt --all`、`cargo test -p scoopc mir::closure_simplify -- --nocapture`、`cargo test -p scoopc mir::escape -- --nocapture`、`cargo test -p scoopc mir::inline -- --nocapture`、`cargo test -p scoopc mir::lower::tests::dump_mir_types_comparison_condition_as_bool_in_generic_template -- --nocapture`、`cargo run -p scoop -- test --fixtures tests/fixtures/mir`、`cargo run -p scoop -- run tests/fixtures/run-pass/generic_fun_recursion.scoop`、`cargo test -p scoopc production_codegen -- --nocapture`、`cargo test -p scoopc llvm::tests -- --nocapture`、`cargo test -p scoopc --no-default-features`、`cargo test --all`、`cargo clippy --all-targets -- -D warnings`、`cargo run -p scoop -- test`（`fixtures: ok (1202)`）全部通过。

### [DONE] T5000i3 让 continuation escaping analysis 进入 effect/state-machine planning 输入面
- 范围：
  - 将 `T5000i1` 的 continuation escape facts 与既有 `ProgramFacts` / `EffectAnalysisCtx` 对接；
  - effect/state-machine planning 可以消费 “local resume only / escaping / unknown” 级别的 continuation 事实；
  - 暂不改变 backend emitter ABI，只迁移分析输入边界。
- 验收：
  - planning 层不再需要从 `MainCodegen` 现场推断 continuation 是否逃逸；
  - continuation facts 的缺失路径保持保守 unknown，不改变现有运行语义。
- 依赖：T5000i2
- 完成记录（2026-04-28）：
  - `ContinuationEscapeFact` 现在记录 `resume_call_spans`，使 MIR escape facts 能按 HIR `CallSite` 稳定投影到 effect/state-machine planning 输入面；
  - `crates/scoopc/src/effect_analysis.rs` 新增 backend-agnostic `ContinuationEscapeState::{LocalResumeOnly, Escaping, Unknown}` 与 call-site keyed `ContinuationEscapeFacts`，并挂入 `EffectAnalysisCtx`；缺失 pass view、缺失 callable FQN 或缺失 call-site fact 时统一保守返回 `Unknown`；
  - shared / production analysis context 现在从 `MaterializedMirPassView::escape_facts()` 为当前 callable 投影 continuation facts，nested handle suspendability analysis 继承同一 side table；
  - `SuspendSitePlan` 与 state-machine segment suspend-site side table 现在记录 `Continuation.resume` hidden suspend site 的 continuation escape 状态，并把该状态纳入 planning/segment structural signature；本轮不改变 backend emitter ABI；
  - 修复接入回归暴露的 MIR escape 精度缺口：`TerminatorKind::Handle`、handle body/arm/finally exit `Todo`、以及 `Rvalue::Todo("handle result pending")` 均是已结构化暴露的 handle 占位，不再错误降级为 unknown；其它未建模 `Todo` 继续保守 unknown；
  - 新增单元测试覆盖 facts 缺失时为 `Unknown`、本地 `Continuation.resume` 投影为 `LocalResumeOnly`、传参逃逸 continuation 投影为 `Escaping`，并确认 handle planning 把这些状态记录到 suspend site；
  - 已验证 `cargo fmt --all --check`、`cargo test -p scoopc continuation_escape -- --nocapture`、`cargo test -p scoopc escaping_continuation_facts_enter_handle_planning_input -- --nocapture`、`cargo test -p scoopc mir::escape -- --nocapture`、`cargo test -p scoopc llvm::codegen::effect -- --nocapture`、`cargo test -p scoopc --no-default-features`、`cargo test --all`、`cargo clippy --all-targets -- -D warnings`、`cargo run -p scoop -- test`（`fixtures: ok (1202)`）全部通过。

### [DONE] T5000i4 迁移 `state_machine_plan / segments / transform` 到 MIR + shared facts 边界
- 范围：
  - 把 effect/state-machine 的 planning / segments / transform 主分析入口迁出 LLVM codegen 语义边界；
  - 新入口依赖 MIR pass view、`ProgramFacts`、`EffectAnalysisCtx` 与 escape facts；
  - LLVM backend 只保留 emitter 与必要的 backend lowering 合同。
- 验收：
  - effect/state-machine planning 不再以 `MainCodegen` 为主要输入上下文；
  - backend 不再承担 effect middle-end 主分析责任。
- 依赖：T5000i3
- 完成记录（2026-04-28）：
  - 新增 `crates/scoopc/src/effect/mod.rs` 与 `crates/scoopc/src/effect/state_machine/{mod,analysis,segments,transform}.rs`，把 shared effect analysis、state-machine planning skeleton 与 no-LLVM step summary 统一收口到 `crate::effect` 目录模块；`crates/scoopc/src/lib.rs` 不再继续声明散落在 crate root 的 `effect_analysis` / `effect_step_summary` 入口。
  - `crates/scoopc/src/effect/state_machine/analysis.rs` 现直接承载 shared `CalleeSuspendPlan` / `SuspendCallAnalysis` / known-fun suspendability helper，并删除原先混在 shared analysis 文件末尾的 `MainCodegen` impl；这些 backend bridge 入口现迁入新文件 `crates/scoopc/src/llvm/codegen/effect/state_machine_bridge.rs`。
  - LLVM backend 已删除 `crates/scoopc/src/llvm/codegen/effect/state_machine_plan.rs` wrapper 与 backend-local `unified_state_machine_skeleton`；`state_machine_emitter.rs` 现在直接消费 `crate::effect::state_machine` 的 unified contract / frame schema / state-machine types，`effect/step_summary.rs` 也直接复用同一 shared analysis，而不是继续 `include!` LLVM-side planning 文件。
  - `llvm/codegen/effect/mod.rs` 现只保留 emitter、bridge 与必要的 lowering 辅助；effect/state-machine 的 planning / segments / transform 主分析入口已迁到 shared MIR + facts 边界，LLVM backend 不再承担 effect middle-end 主分析责任。
  - 已验证 `cargo fmt --all --check`、`cargo test -p scoopc llvm::codegen::effect -- --nocapture`、`cargo test -p scoopc --no-default-features`、`cargo test --all`、`cargo clippy --all-targets -- -D warnings`、`cargo run -p scoop -- test`（`fixtures: ok (1202)`）全部通过。

### [DONE] T5000iR Review：确认 effect middle-end 已从 LLVM backend 语义边界迁出
- 重点：
  - `state_machine_plan / segments / transform` 是否已经脱离 LLVM codegen 主职责；
  - effect planning 是否真正依赖 MIR 与 shared facts，而不是依赖 backend context；
  - closure / continuation escape 分析是否与 summary / call kind / provenance 形成统一体系。
- 验收：
  - LLVM backend 只剩 emitter 与 backend lowering，而不再承担 effect middle-end 的主分析责任。
- 依赖：T5000i
- 完成记录（2026-04-28）：
  - 已复核 `crates/scoopc/src/effect/**`、`crates/scoopc/src/effect/analysis.rs` 与 `crates/scoopc/src/effect/step_summary.rs`；shared effect 模块现只依赖 HIR/MIR、`ProgramFacts`、`TypeStore` 与 shared metadata，未再直接引用 `MainCodegen`、`crate::llvm`、`inkwell` 或其它 LLVM backend 语义类型，说明 `state_machine plan / segments / transform` 已稳定离开 LLVM codegen 主职责；
  - 已复核 `crates/scoopc/src/llvm/codegen/effect/state_machine_bridge.rs` 与 `state_machine_emitter.rs`；backend 侧现在由 bridge 负责把 `ProgramFacts`、`MaterializedMirPassView` summary/escape facts 与函数级局部 metadata 注入 `EffectAnalysisCtx`，emitter 则只消费 `crate::effect::state_machine::UnifiedHandleLoweringContract` 发射 LLVM IR，职责边界已收口为 bridge + emitter + 必要 lowering helper；
  - 已复核 `crates/scoopc/src/effect/state_machine/analysis.rs`、`crates/scoopc/src/mir/pass_view.rs`、`crates/scoopc/src/mir/summary.rs` 与 `crates/scoopc/src/mir/inline.rs` 的接缝：known-fun suspendability 现在直接读取 pass-view summary override 的 `may_outward_effect`，continuation escape 通过 `ContinuationEscapeFacts::from_pass_view_for_callable(...)` 按 call site 投影进 `EffectAnalysisCtx`，说明 closure / continuation escape 与 summary / call kind / provenance 已通过同一 MIR pass facts 层进入 planning，而不是回退到 backend 现场重新推断；
  - 已验证 `cargo test -p scoopc llvm::codegen::effect -- --nocapture`、`cargo test -p scoopc --no-default-features`、`cargo test --all`、`cargo clippy --all-targets -- -D warnings` 全部通过；
  - review 结论：未发现需要插入到 `T5000j` 之前的新前置缺陷任务；LLVM backend 当前只剩 emitter 与 backend lowering，effect middle-end 已迁出 LLVM backend 语义边界。

### T5000j 扩展覆盖面，并继续跟踪 safepoint / `mem2reg` 方向
- 说明：
  - 当前任务同时覆盖 `when/pattern`、operator-overload target materialization、更多 higher-order / closure / object-init / top-level-init 场景，以及 safepoint/root-pressure 跟踪，单轮过大；
  - probing 已确认本阶段最靠前的既有边界泄漏仍是 operator-overload target 仍在 LLVM backend 现场决定：
    - `crates/scoopc/src/llvm/emit.rs` 仍为 struct member methods 做 eager inclusion；
    - `crates/scoopc/src/llvm/codegen/mod.rs` 仍在 `codegen_binary(...)` 现场决定 user-defined binary / `compareTo` overload；
    - 同主题下还暴露出 unary operator overload 已被 typecheck 接受，但尚未进入 production codegen / materialization 主线；
  - 因此先按“operator target 边界 → pattern/when 覆盖 → 更多 higher-order/init 场景 → safepoint 观测”顺序拆分。
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

### T5000j1 收口 operator-overload target 的 typed HIR / generic MIR 主线
- 说明：
  - probing 进一步确认 `compareTo` 型比较与普通 unary/binary operator 不能在同一轮机械合并：
    - arithmetic/bitwise/unary overload 可以直接重写为显式 direct-call；
    - 但 `< <= > >=` 的 user-defined `compareTo` 路径还需要一个“调用结果与 0 比较”的稳定表示，而当前 HIR/MIR 的整数字面量节点并不承载可直接合成的 `0` 常量值；
  - 因此先拆成 `T5000j1a` / `T5000j1b`，避免在未补齐表示层前提下宣称同一轮完整收口。

### [DONE] T5000j1a 把 unary 与 arithmetic/bitwise/shifts operator overload target 收口到 typed HIR / generic MIR direct-call 主线
- 范围：
  - 让 unary `~` 与 user-defined binary arithmetic/bitwise/shifts operator overload，在 typecheck 阶段写回统一的 direct-call binding / monomorph request；
  - 让 HIR lowering 把这些 operator site 收口为显式顶层 direct-call 形状，并让 generic/effect-row/owner specialization 继续走现有 materialized target identity 主线；
  - 让 MIR lowering / reachability / production LLVM body emission 直接消费这些 explicit direct-call target；若 `llvm/emit.rs` 仍需为剩余 `compareTo` 路径保留最小 eager inclusion，范围必须显式缩到只覆盖尚未迁出的比较路径。
- 验收：
  - unary/binary arithmetic/bitwise/shifts operator-overload target identity 不再主要由 LLVM `codegen_binary` / reachability 现场猜测；
  - generic owner method / effect-row aware operator overload 会进入正常的 monomorph/materialization / summary / direct-call 主线；
  - production regression 能证明上述 operator overload 已通过 MIR/reachability 主线触达真实 callee，而不是靠 backend eager inclusion 托底。
- 依赖：T5000iR
- 完成记录（2026-04-28）：
  - `crates/scoopc/src/typecheck/expr/ops.rs` 现已为 unary `~` 与 user-defined arithmetic/bitwise/shifts operator overload 统一记录 `TopLevelFunCallBinding` / monomorph request，且 unary overload 的 binding span 已从 operand 修正到外层一元表达式；
  - `crates/scoopc/src/hir/lower/expr.rs` 现会把上述 operator site 改写成显式顶层 `ExprKind::Call`，继续复用已有 direct-call binding 与 materialized target identity；generic owner specialization 与默认 eff-arg 都沿现有主线进入 HIR/MIR；
  - `crates/scoopc/src/llvm/emit.rs` 已把仅为 operator overload 保留的 eager inclusion 缩到 `compareTo` 比较路径，不再把整类 struct member methods 一起托底带入 reachable 集；
  - 新增 `crates/scoopc/src/mir/materialize.rs` 回归，验证 operator overload binding / monomorph key 会保留 owner specialization 的 `Int` type arg 与非 `Pure` 的默认 eff-arg；新增 `crates/scoopc/src/llvm/tests.rs` production regression，验证 `~` / `+` / `<<` 已经作为 direct call 进入 typed HIR 与 LLVM IR，且未使用的 `Mask.minus` 不会再因 eager inclusion 混入 IR；
  - 已验证 `cargo fmt --all`、`cargo test -p scoopc`、`cargo test --all`、`cargo clippy --all-targets -- -D warnings`、`cargo run -p scoop -- test` 全部通过。

### [DONE] T5000j1b 收口 user-defined `compareTo` 比较 target，并删除剩余 struct member eager inclusion
- 范围：
  - 为 `< <= > >=` 经 `compareTo` 的用户态比较补齐稳定的 typed HIR / generic MIR 表示；
  - 让这类比较也进入 direct-call target / monomorph / reachability 主线，并删除 `llvm/emit.rs` 中剩余仅为 operator overload 保留的 struct member eager inclusion。
- 验收：
  - user-defined comparison 不再依赖 LLVM backend 现场决定 `compareTo` 目标；
  - `llvm/emit.rs` 不再需要为 operator overload 保留 struct member eager inclusion。
- 依赖：T5000j1a
- 完成记录（2026-04-28）：
  - `crates/scoopc/src/typecheck/expr/ops.rs` 现已为 `< <= > >=` 的 user-defined `compareTo` 比较统一记录 `TopLevelFunCallBinding`、monomorph request 与默认 eff-arg，不再只把站点类型化为 `Bool`；
  - `crates/scoopc/src/hir/lower/expr.rs`、`crates/scoopc/src/mir/lower.rs` 与 `crates/scoopc/src/mir/mod.rs` 现已把这类比较收口为“显式 direct-call + `SynthInt(0)` 的普通整数比较”；`crates/scoopc/src/llvm/codegen/mir_body.rs` 也已补齐 `SynthInt` 常量发射；
  - `crates/scoopc/src/hir/lower/mod.rs`、`crates/scoopc/src/hir/lower/types.rs`、`crates/scoopc/src/hir/mod.rs` 与 `crates/scoopc/src/cone/pre_specialize.rs` 现已统一携带 `top_level_fun_call_sites` side table，使 generic MIR lowering / pre-specialize 都能直接消费 compareTo target identity；
  - 本轮同时修复了既有缺口：`crates/scoopc/src/typecheck/expr/stmt.rs` 之前不会对 statement-position `if` 条件做 `infer`，导致 `if (lhs < rhs)` 里的 compareTo 站点不写回 typed side table；现已补齐条件表达式推导，真实 fixture 路径不再漏记；
  - `crates/scoopc/src/llvm/emit.rs` 已删除剩余仅为 operator overload 保留的 struct member eager inclusion；新增 `crates/scoopc/src/mir/lower.rs`、`crates/scoopc/src/mir/materialize.rs`、`crates/scoopc/src/llvm/tests.rs` 回归，覆盖 `if` 条件 compareTo、typed HIR binding、owner specialization / eff-arg 保留与 production LLVM reachability；
  - 已验证 `cargo fmt --all`、`cargo test -p scoopc`、`cargo test --all`、`cargo clippy --all-targets -- -D warnings`、`cargo run -p scoop -- test` 全部通过。

### [DONE] T5000j1R Review：确认 operator-overload target 已脱离 LLVM backend 现场物化
- 重点：
  - operator-overload target 是否已在 typed HIR / generic MIR 主线中显式化；
  - `llvm/emit.rs` 是否已经去掉仅为 operator overload 保留的 struct member eager inclusion；
  - unary/binary/`compareTo` 的 owner specialization / materialized target identity 是否仍沿现有 `InstanceKey` / direct-call 主线工作。
- 验收：
  - 可以明确说出 operator-overload target 的来源边界，并证明 reachability / production codegen 不再依赖 backend 现场猜目标。
- 依赖：T5000j1b
- 完成记录（2026-04-28）：
  - review 确认 operator-overload / `compareTo` target identity 的来源边界已前移到 typecheck：`typecheck/expr/ops.rs` 写回 `TopLevelFunCallBinding` / monomorph request，`hir/lower/expr.rs` 将 unary/binary operator-overload 与 `compareTo` 比较改写为显式顶层 direct-call 或 `direct-call + SynthInt(0)` 的普通整数比较，`mir/lower.rs` / `mir/materialize.rs` / `llvm/reachability.rs` 继续沿显式 `CallKind::Direct` 与 side table 主线消费这些 target；
  - review 确认 `llvm/emit.rs` 已删除仅为 operator-overload 保留的 struct member eager inclusion；production reachability 现在只扫描 entry `main` 的已改写 typed HIR 和 canonical materialized MIR body，不再依赖 backend 现场补猜 operator-overload callee；
  - review 同时确认 entry `main` 与 raw-MIR HIR-compat body emission 虽仍可落回 HIR 表达式 lowering，但这些路径消费的已经是 typed HIR 中显式改写后的 direct-call 形状，因此 compareTo / operator-overload target 不再由 `llvm/codegen` 现场决定；
  - 已验证 `cargo test -p scoopc compare_to -- --nocapture`、`cargo test -p scoopc operator_overload -- --nocapture`、`cargo test --all`、`cargo clippy --all-targets -- -D warnings`、`cargo run -p scoop -- test`（`fixtures: ok (1202)`）全部通过。

### [DONE] T5000j2 扩展 `when` / pattern 到 production MIR body / summary 主线
- 范围：
  - 在 `T5000d3` 已正规化 MIR `PatternMatch` / `PatternExtract` 的基础上，继续把非 effect 的 `when` / pattern 场景推进到 production MIR body / summary 主线；
  - 减少这类 body 因 MIR 节点不支持而退回 HIR-compatible emission 的覆盖空洞。
- 验收：
  - `when` / pattern 的更多常见结构可直接被 MIR reachability / body emission / summary 消费，而不是重新落回 HIR 现场解释。
- 依赖：T5000j1R
- 完成记录（2026-04-29）：
  - `crates/scoopc/src/llvm/codegen/mir_body.rs` 现已把 raw materialized MIR body 的支持判定与实际 lowering 扩展到 `PatternMatch` / `PatternExtract`，可直接消费 wildcard / bind / rest / or / tuple / variant / literal / `is` 等常见非 effect `when` pattern 形状，而不是一律退回 HIR-compatible emission；
  - `crates/scoopc/src/llvm/emit.rs` 与 `crates/scoopc/src/llvm/reachability.rs` 现已把 raw non-generic pattern body 纳入 canonical materialized MIR body 选择与扫描主线，同时保留 declaration-only direct call 的 HIR-compatible fallback，以及 raw body 遇到 closure / virtual/interface/resume / perform/handle 等仍需 HIR 兼容扫描的保守边界；
  - 实现过程中暴露并修复了一个既有 ABI 回归：production MIR bridge 的参数绑定此前忽略了 ordinary param ABI 的 indirect GC aggregate 分支，导致 `Option<Option<String>>` 这类 `when`/pattern 参数会把 ABI 指针误当作 enum 原始值解释；`bind_mir_params` 现已按 ordinary param ABI 先 load 间接参数，再进入 MIR pattern lowering，`tests/fixtures/run-pass/option_nested_ref_no_nested_niche_basic.scoop` 的三态语义已恢复；
  - 已新增回归覆盖：
    - `crates/scoopc/src/llvm/tests.rs`：验证 declaration-only direct call raw body 继续退回 HIR-compatible emission、variant payload binder 的 `PatternExtract` 直接经 MIR bridge 发射、`when is Type` 复用运行期 `isa` lowering、generic pattern instance 在 pass view 中暴露 body-known summary、以及 indirect GC aggregate pattern param 会先按 ABI load 再匹配；
    - `crates/scoopc/src/llvm/tests.rs`：验证 raw non-generic pattern body 若仍需 HIR-compatible reachability 扫描（如 async helper 依赖定义），production reachability 仍会补齐 helper definition；
  - 已验证 `cargo fmt --all`、`cargo test -p scoopc production_codegen_ -- --nocapture`、`cargo test --all`、`cargo clippy --all-targets -- -D warnings`、`cargo run -p scoop -- test`（`fixtures: ok (1202)`）全部通过。

### [DONE] T5000j2R Review：确认 `when` / pattern 覆盖扩张仍沿 MIR 结构主线推进
- 重点：
  - 新覆盖是否建立在已有 `PatternMatch` / `PatternExtract` / provenance 结构之上；
  - 是否重新把 pattern 语义判断塞回 LLVM codegen。
- 验收：
  - pattern/when 覆盖扩张不依赖新的 backend 特判。
- 依赖：T5000j2
- 完成记录（2026-04-29）：
  - 已复核 `crates/scoopc/src/mir/mod.rs`、`crates/scoopc/src/mir/lower.rs`、`crates/scoopc/src/mir/materialize.rs` 与 `crates/scoopc/src/mir/summary.rs`，确认 `when` / pattern 语义继续以 backend-agnostic 的 `Pattern` / `PatternMatch` / `PatternExtract` / `PatternBindingStep` 结构存在于 generic MIR template、instance materialization 与 summary 主线上；本轮 production 覆盖扩张没有新增平行的 backend 专用 pattern 表示；
  - 已复核 `crates/scoopc/src/llvm/codegen/mir_body.rs`，确认新增逻辑只是在 raw materialized MIR bridge 中按既有 MIR 节点做支持判定与 LLVM lowering：variant payload 复用既有 `extract_matched_when_variant_field_value`，`when is Type` 复用既有运行期 `isa` / ref type-check helper，string / enum / tuple / literal pattern 也都以 MIR pattern 树递归消费；未发现重新按 HIR 语法形状、函数名或 ad-hoc backend 特判恢复 pattern 语义的路径；
  - 已复核 `crates/scoopc/src/llvm/emit.rs` 与 `crates/scoopc/src/llvm/reachability.rs`，确认它们只是把 raw non-generic pattern body 纳入 canonical materialized body 选择与扫描主线，同时继续通过 `raw_materialized_mir_body_requires_hir_compat_boundary` / `mir_fun_requires_hir_compat_scan` 对 declaration-only direct call、closure、virtual/interface/resume、perform/handle 等不支持形状保留 HIR-compatible fallback；扩张的是“哪些既有 MIR body 可直接走 production 主线”，不是把 pattern 分析职责倒灌回 backend；
  - review 过程中未发现需要前插到 `T5000j3` 之前的新既有缺陷任务；
  - 已验证 `cargo test -p scoopc production_codegen_ -- --nocapture`、`cargo test -p scoopc compare_to -- --nocapture`、`cargo test -p scoopc operator_overload -- --nocapture`、`cargo test --all`、`cargo clippy --all-targets -- -D warnings`、`cargo run -p scoop -- test`（`fixtures: ok (1202)`）全部通过。

### T5000j3 扩展更多 higher-order / closure / object-init / top-level-init 场景到 production MIR 主线
- 说明：
  - 经核对，当前任务同时包含 `top-level-init / object-init` 的 raw non-generic canonical body 扩张，以及 higher-order / closure 相关 pass-visible 形状扩张两类改动，单轮过大；
  - 现按“当前 MIR bridge 已支持的 init 形状”和“仍需依赖 summary / escape facts / caller-side rewrite 收口的 higher-order / closure 形状”拆成以下子任务，避免把两类边界问题混在同一轮实现。

### [DONE] T5000j3a 扩展 `top-level-init / object-init` 相关 raw non-generic body 到 production MIR 主线
- 范围：
  - 放宽 canonical raw non-generic callable body 选择，不再仅限 pattern candidate，而是覆盖当前 MIR bridge / reachability 已支持的 `top-level-init / object-init` 相关形状；
  - 为 production codegen 补齐 `top-level immutable init` / object init access 场景回归，确认这些函数可直接走 raw materialized MIR bridge；
  - 保持 closure / fun-value / `MakeClosure` / `Perform` / `Handle` 等仍未支持的形状继续留在 HIR-compatible fallback 边界。
- 验收：
  - `top-level-init / object-init` 新覆盖继续建立在 materialized MIR 主线之上；
  - 扩大 candidate 选择面后，不会错误把仍不支持的 higher-order / effect 形状推进到 production MIR body lowering。
- 依赖：T5000j2R
- 完成记录（2026-04-29）：
  - `crates/scoopc/src/llvm/codegen/mod.rs` 为 caller-side raw non-generic body 新增了按作用域收口的 canonical 选择 helper；production 入口现在只把两类 raw body 纳入 canonical materialized MIR 主线：既有 pattern body，以及通过 `TopLevelRef` 访问 `top_level_consts` / `top_level_immutable_values` / `top_level_vars` / `object_inits` 的 init 相关 body，不再把普通 arithmetic/direct-call helper 一并误推进 raw MIR bridge；
  - `crates/scoopc/src/llvm/emit.rs` 与 `crates/scoopc/src/llvm/reachability.rs` 现统一复用这条 candidate 选择边界；`reachability` 也同步扩展了 `top_level_vars` / `object_inits` 输入，并继续把 closure、fun-value、`Todo`、implicit tail-return 等 unsupported shape 保留在 HIR-compatible fallback；
  - `crates/scoopc/src/llvm/codegen/mir_body.rs` 现明确把 `Return { value: None }` 判为 raw MIR unsupported，避免 generic MIR 仍以隐式尾表达式约定表示返回值时，被 production bridge 错降成类型默认值；这次回归里暴露出的 `effect_escape_continuation_indirect_perform_statement_container_matrix.scoop` 与 `fun_call_add_basic.scoop` 都随之恢复；
  - `crates/scoopc/src/llvm/tests.rs` 已补齐 `top-level immutable init`、object value init、closure fallback、implicit tail-return fallback、non-init/non-pattern helper fallback 与 ctor-call `Todo` reachability fallback 回归，确认 `j3a` 的 candidate 放宽只覆盖目标 init 场景。

### [TODO] T5000j3aR Review：确认 init 场景扩张只是放宽 canonical MIR 覆盖，而非把分析责任倒灌回 backend
- 重点：
  - `top-level-init / object-init` 的新增 production 覆盖是否仍然只是消费既有 MIR body / reachability facts；
  - canonical raw body 选择放宽后，unsupported 形状是否仍稳定留在 HIR-compatible 边界。
- 验收：
  - 可以明确指出 init 场景新增覆盖依赖的是现有 materialized MIR / reachability 事实，而不是新的 backend 现场分析。
- 依赖：T5000j3a

### [TODO] T5000j3b 扩展更多 higher-order / closure 场景到 production MIR 主线
- 范围：
  - 补齐目前仍经常退回 HIR-compatible emission 的 higher-order / closure 场景；
  - 继续扩大 pass-visible materialized body / summary / escape facts 对 production codegen 的实际覆盖面。
- 验收：
  - higher-order / closure 新覆盖继续建立在 materialized MIR / summary / escape facts 之上，而不是重新把高阶分析长回 backend。
- 依赖：T5000j3aR

### [TODO] T5000j3bR Review：确认 higher-order / closure 场景扩张没有把分析责任倒灌回 backend
- 重点：
  - higher-order / closure 的新覆盖是否仍消费 shared facts / pass artifacts；
  - LLVM backend 是否只保留 lowering，而不是重新承担分析或 target-set 收缩职责。
- 验收：
  - 可以明确指出 production 主线新增的 higher-order / closure 覆盖依赖的是哪一层中端事实。
- 依赖：T5000j3b

### [TODO] T5000j3R Review：确认 higher-order / init 场景扩张没有把分析责任倒灌回 backend
- 重点：
  - higher-order / closure / object-init / top-level-init 的新覆盖是否仍消费 shared facts / pass artifacts；
  - LLVM backend 是否只保留 lowering，而不是重新承担分析或 target-set 收缩职责。
- 验收：
  - 可以明确指出 production 主线新增覆盖依赖的是哪一层中端事实。
- 依赖：T5000j3bR

### [TODO] T5000j4 建立 safepoint 数量 / roots 压力的可复验跟踪基线
- 范围：
  - 基于当前 inline / devirt / closure simplification / effect planning 主线，选定一组可复验 workload；
  - 记录调用边界减少后 safepoint 数量、roots 压力与后续 `mem2reg` 研究窗口的观察口径。
- 验收：
  - safepoint / root-pressure 变化有可复验结论，可供后续 GC / `mem2reg` 研究引用。
- 依赖：T5000j3R

### [TODO] T5000j4R Review：确认 safepoint / root-pressure 跟踪口径可持续复用
- 重点：
  - 观测方法是否可复验、可重跑，而不是一次性的手工结论；
  - 是否已经能回答“当前更值得继续减少调用边界，还是已经出现值得研究 `mem2reg` / register-root 的窗口”。
- 验收：
  - 后续 GC / `mem2reg` 研究可以直接复用本轮口径与 workload。
- 依赖：T5000j4

### [TODO] T5000jR Review：确认优化主线已形成可持续扩展的中端体系
- 重点：
  - 后续扩展是否仍沿 MIR / summary / structure 方向推进；
  - 是否重新出现“把新分析长回 LLVM codegen”的回退；
  - 是否已经为未来 C / JVM / CLR backend 预留了稳定消费边界。
- 验收：
  - 本轮结束后，优化主线已明确从“LLVM codegen 现场推断”转向“backend-agnostic 中端 + backend lowering 分层”。
- 依赖：T5000j4R
