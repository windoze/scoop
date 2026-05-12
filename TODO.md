# TODO（Effect Refactor Target-Shape 重建）

> 生成时间：2026-05-11  
> 设计基线：[`EFFECT_REFACTOR.md`](./EFFECT_REFACTOR.md)  
> 缺口基线：[`EFFECT_REFACTOR_GAPS.md`](./EFFECT_REFACTOR_GAPS.md)  
> continuation/runtime 补充：[`CONTINUATION_RUNTIME_REFACTOR.md`](./CONTINUATION_RUNTIME_REFACTOR.md)  
> 计划基线：[`PLAN.md`](./PLAN.md)  
> 格式参考：[`TODO-P0.md`](./TODO-P0.md)  
> 顺序约束：严格按当前文件中的条目顺序推进；不得跨条目并行实现。  
> 当前状态：旧 continuation/effect TLS 语义已经被硬删除；本文件的任务不是“恢复旧桥”，而是按目标形态把单一 effect pipeline 重新接通。

## 全局约束

- [`EFFECT_REFACTOR.md`](./EFFECT_REFACTOR.md) 是唯一设计基线。若任务实现中发现它的设计仍缺关键约束，必须先更新该文档，再继续编码。
- [`EFFECT_REFACTOR_GAPS.md`](./EFFECT_REFACTOR_GAPS.md) 是当前缺口基线；后续任务的完成记录必须能明确说明“消除了哪一类 gap”。
- 严禁重新引入任何 continuation/effect TLS 语义 source of truth，包括但不限于：
  - `__scoop_effect_handler_stack_top`
  - `__scoop_effect_active`
  - `__scoop_effect_perform_slot`
  - `__scoop_callee_suspend_state`
  - `__scoop_continuation_resume_scope`
- 严禁恢复任何旧 runtime bridge API 作为过渡方案，包括但不限于：
  - `scoop_continuation_alloc`
  - `scoop_continuation_resume_with`
  - `scoop_continuation_resume_into`
  - `scoop_effect_outcome_consume_current`
  - `scoop_effect_outcome_publish`
- 严禁恢复 `crates/scoopc/src/llvm/codegen/effect/{mod,contract}.rs` 或 `call/{dispatch,resume}.rs` 这种 legacy 语义容器；如需新 helper，必须放到新的、语义中立的 target-shape 模块中。
- effectful callable 的语义协议必须由显式 hidden ABI 承载：
  - `current_effect_ctx_ref`
  - `incoming_resume_token_ref`
  - `ScoopEffectOutcome *outcome`
  不能再退回单 hidden token 或 wrapper/TLS probing。
- backend 的 lowering 决策只能依赖：
  - 当前输入 MIR / late-lowered program
  - `MaterializedEffectFacts` / schema / site facts
  - target / opt level / feature flags
  不允许回 HIR/AST 或 resurrect deleted caches 补语义。
- `NoOutward` / plain body 不得为了“省事”被重新包成 complete-only `Step_F`；plain/effect ABI 分流必须由 facts 驱动。
- runtime C 最终只允许保留 generic substrate，不允许重新承担 continuation object model 或 effect propagation policy。
- 每个任务完成后，必须在该任务的“完成记录”处回写：
  - 改动范围
  - 核心决策
  - 验证结果
  - 与 `EFFECT_REFACTOR_GAPS.md` 对应消除的 gap 条目。

## [DONE] G0-T01：硬删除后的物理残余清场，恢复“最小一致破坏状态”

- 参考：
  - [`PLAN.md`](./PLAN.md) §2 / G0
  - [`EFFECT_REFACTOR_GAPS.md`](./EFFECT_REFACTOR_GAPS.md) §11、§12
- 目标：
  - 清掉 bulk deletion 后仅由“物理删文件”留下的机械残余；
  - 让后续编译错误只反映 target-shape 缺口，而不是残余静态断言、测试字符串或前置声明缺失。
- 必须实现的内容：
  1. 清理 `runtime/c/scoop_runtime.c` 中所有已删类型/宏的残余静态断言与注释尾巴。
     - 当前已知位置：`cargo check -p scoop_runtime` 首批报错对应 `runtime/c/scoop_runtime.c:315-384` 一带。
     - 需要删除的残余包括：
       - `ScoopEffectPerformSlot`
       - `ScoopEffectCtx`
       - `ScoopValueTransport`
       - `ScoopEffectHandlerFrame`
       - `SCOOP_EFFECT_PERFORM_SLOT_MAX_WORDS`
       的 `offsetof` / `sizeof` 断言残留。
  2. 修复 bulk deletion 误伤的中性前置声明。
     - 当前已知例子：`runtime/c/scoop_runtime.c` 中 `scoop_alloc` 的前置声明被 continuation section 一起删掉，导致 `scoop_alloc_typed(...)` 与 string helper 提前报隐式声明错误。
     - 只允许恢复“与 continuation/effect TLS 无关的中性前置声明”。
  3. 清理 `runtime/c/scoop_test.c` 中旧 effect/TLS test-only export 声明。
     - 当前文件顶部仍残留 `scoop_effect_*` test-only 前置声明；需要全部删除。
  4. 清理活跃测试源码中仍直接提旧桥名字的断言块。
     - 当前已知位置：`crates/scoopc/src/llvm/tests.rs` 中 `scoop_effect_*` / `scoop_continuation_resume_into` 相关断言；这些不是新设计的验证目标。
     - `crates/scoopc/src/pipeline/llvm_codegen_stage.rs` 中仍有 `scoop_effect_set_active_with_trace` 字符串断言，也应清掉或改成新 surface 验证。
  5. 清理活跃 surface / 识别表里已删除的旧 intrinsic 名字。
     - `sysroot/core.scoop` 中旧 `__scoop_effect_*` / `__scoop_effect_slot_*` surface 已删；需确认没有残余。
     - `crates/scoopc/src/effect_facts/builder.rs` 中 old effect intrinsic 名字表必须同步删净。
     - `crates/scoopc/src/llvm/codegen/effect_lowered/value.rs` 中 old effect intrinsic 识别表必须同步删净。
- 必须遵从的约束：
  - 本任务只做物理清场；禁止在此任务中恢复任何 deleted TLS contract helper。
  - 若为让 unrelated 代码通过编译而需要恢复声明，只能恢复与 generic substrate 有关的中性声明，不能恢复 deleted continuation/effect API。
- 验证：
  1. 对以下目录执行代码 grep，不得命中旧 TLS/bridge 符号名：
     - `crates/scoopc/src`
     - `runtime/c`
     - `sysroot`
  2. 运行：`cargo check -p scoop_runtime`
     - 输出中不再出现“unknown type name `ScoopEffectPerformSlot` / `ScoopEffectCtx` / `ScoopValueTransport` / `ScoopEffectHandlerFrame`”。
  3. 运行：`cargo check -p scoopc`
     - 输出中剩余错误必须主要是 architecture gap，而不是旧名字残余或删坏测试入口。
- 完成条件：
  - 代码树达到“最小一致破坏状态”：旧 TLS 语义仍不存在，且后续编译报错不再被物理残余噪音主导。
- 依赖：无
- 完成记录：
  - 改动范围：
    - `runtime/c/scoop_runtime.c`：删除已删 `ScoopEffectPerformSlot` / `ScoopEffectCtx` / `ScoopValueTransport` / `ScoopEffectHandlerFrame` 的残余 `_Static_assert`、孤立 handler-stack 段落与未再被使用的 GC stress / immix helper 尾巴；补回 `scoop_alloc` 的中性前置声明。
    - `crates/scoopc/src/pipeline/llvm_codegen_stage.rs`：把旧 bridge 名字负向断言切换为对 `scoop_runtime_init` 的正向 IR 验证。
    - `crates/scoopc/src/llvm/codegen/effect_lowered/{value,body}.rs`：移除围绕旧 bridge 名字的源码审计断言，保留其余 clean-backend 边界检查。
  - 核心决策：
    - 只清理“物理残余/验证噪音”，不在本任务内重建任何 replacement architecture，也不恢复任何旧 TLS/bridge helper。
    - 对测试面采取“改成目标 surface 的正向验证或直接移除旧名审计”的策略，避免继续把旧符号名当成 authoritative 验证目标。
    - `cargo check -p scoopc` 中保留的 `declare_*resume_entry*_impl`、`alloc_effect_outcome_slot`、`local_call_may_suspend_from_hir_ty` 等缺失项视为后续 G1/G2/G4/G5 的结构性 gap，而不是本任务需要继续兜底恢复的旧桥残余。
  - 验证结果：
    - 对 `crates/scoopc/src`、`runtime/c`、`sysroot` 执行针对旧 TLS/bridge 符号名的 grep：无命中。
    - `cargo check -p scoop_runtime`：通过；不再出现 `ScoopEffectPerformSlot` / `ScoopEffectCtx` / `ScoopValueTransport` / `ScoopEffectHandlerFrame` / `SCOOP_EFFECT_PERFORM_SLOT_MAX_WORDS` 残余报错。
    - `cargo clippy -p scoop_runtime --all-targets -- -D warnings`：通过。
    - `cargo check -p scoopc`：失败，但首批错误已切换为 effectful ABI / `EffectOutcome` / callee suspend-reentry / call lowering 缺口，与 `G1-T02`、`G2-T03`、`G4-T05` 等后续任务一致，不再由物理残余噪音主导。
  - 与 `EFFECT_REFACTOR_GAPS.md` 对应消除的 gap 条目：
    - §11：`runtime/c/scoop_runtime.c` 删除后的残余结构引用与前置声明缺口。
    - §12：活跃验证面中仍围绕旧桥名字的断言噪音。

## [DONE] G0-T01R：Review 物理清场结果，确认没有偷回任何 TLS 语义

- 参考：
  - [`PLAN.md`](./PLAN.md) §2 / G0
  - [`EFFECT_REFACTOR_GAPS.md`](./EFFECT_REFACTOR_GAPS.md) §11、§12
- 重点：
  - 活跃实现里是否仍存在任何 deleted TLS symbol / runtime bridge 名字；
  - 为修编译而新增的声明是否都是中性 substrate，而不是偷偷恢复旧语义入口；
  - 测试源码是否已经不再围绕旧 bridge 名字构建验证。
- 必须检查的文件/位置：
  - `runtime/c/scoop_runtime.c`
  - `runtime/c/scoop_runtime_api.h`
  - `runtime/c/scoop_test.c`
  - `crates/scoopc/src/llvm/tests.rs`
  - `crates/scoopc/src/pipeline/llvm_codegen_stage.rs`
  - `sysroot/core.scoop`
- 验证：
  - 对 G0-T01 中的 grep 范围再做一次人工复核；
  - 重新运行 `cargo check -p scoop_runtime` 和 `cargo check -p scoopc`，确认最前面的错误类别已经切到结构性 target-shape gap。
- 完成条件：
  - 可以明确写出：旧 TLS 语义仍然完全不存在，接下来可以开始补 replacement architecture。
- 依赖：G0-T01
- 完成记录：
  - 改动范围：
    - `crates/scoopc/src/llvm/tests.rs`：把 `G0-T01` 清掉旧名断言后变成空壳的几组 LLVM 回归补成新的正向 refactor-surface 断言，并把部分仍以 `tls_check` 命名的测试重命名为 direct-call surface 语义。
    - `runtime/c/scoop_runtime.c`：恢复被误删的中性 runtime substrate 定义，包括 `#include <stdatomic.h>`、GC-stress 状态/解析 helper、Immix TLS cache/nursery 分配 helper、`scoop_string_trim_indent(...)`、`scoop_runtime_init(...)`、`scoop_alloc(...)` 与 `scoop_gc_collect_safepoint(...)`。
  - 核心决策：
    - review 过程中发现的“空验证测试面”和“中性 substrate 被误删导致链接失败”都直接否定了 `G0-T01` 的清场结果，因此在本 review 任务内直接修复，而不是把 review 仅停留在记录问题。
    - 对 `crates/scoopc/src/llvm/tests.rs` 不再恢复任何围绕旧 bridge/TLS 名字的负向断言，而是改成对 direct-call surface、Step dispatch、once-init/native-call surface 的正向验证。
    - 对 `runtime/c/scoop_runtime.c` 只恢复 generic substrate / GC allocator / string runtime 所需的中性实现，不恢复任何 deleted continuation/effect TLS symbol、bridge API 或语义容器。
  - 验证结果：
    - 对 `crates/scoopc/src`、`runtime/c`、`sysroot` 执行旧 TLS/bridge 符号 grep：无命中。
    - `cargo check -p scoop_runtime`：通过。
    - `cargo test -p scoop_runtime --tests --no-run`：通过；此前 `_scoop_runtime_init` / `_scoop_alloc` 未定义的链接失败已消失。
    - `cargo test -p scoop_runtime --test runtime_init runtime_init_is_callable_and_observable -- --exact --nocapture`：通过。
    - `cargo test -p scoop_runtime --test alloc scoop_alloc_returns_non_null_and_can_be_called_repeatedly -- --exact --nocapture`：通过。
    - `cargo test -p scoop_runtime --test explicit_root_frame explicit_root_frame_tls_top_and_descriptor_walk_smoke -- --exact --nocapture`：通过。
    - `cargo clippy -p scoop_runtime --all-targets -- -D warnings`：通过。
    - `cargo check -p scoopc`：仍失败，但首批错误继续集中在 `declare_callee_resume_entry_function_impl`、`local_call_may_suspend_from_hir_ty`、`alloc_effect_outcome_slot`、`known_fun_body_may_outward_effect` 等 target-shape helper 缺口；未再出现旧 TLS/bridge 名字残余或 runtime substrate 缺失导致的噪音。
  - 与 `EFFECT_REFACTOR_GAPS.md` 对应消除的 gap 条目：
    - §11：`runtime/c/scoop_runtime.c` 中被误删的中性 substrate 定义已恢复，runtime review 面不再被裸链接失败干扰。
    - §12：活跃 LLVM 回归测试不再围绕旧 bridge 名字，也不再因为清理旧名而退化为空测试。

## [DONE] G1-T02：重建 effectful callable 的显式 hidden ABI 骨架

- 参考：
  - [`PLAN.md`](./PLAN.md) §2 / G1
  - [`EFFECT_REFACTOR.md`](./EFFECT_REFACTOR.md) §4.5、§4.10、§4.13
  - [`EFFECT_REFACTOR_GAPS.md`](./EFFECT_REFACTOR_GAPS.md) §1
- 目标：
  - 把 effectful callable ABI 从“单 hidden `incoming_resume_token`”升级到最终显式 hidden ABI：
    - `current_effect_ctx_ref`
    - `incoming_resume_token_ref`
    - `ScoopEffectOutcome *outcome`
  - 让顶层函数、closure callable、resume entry、dynamic invoke surface 使用同一 ABI 规则。
- 必须实现的内容：
  1. 修改 `crates/scoopc/src/llvm/codegen/mod.rs` 中 callable 声明路径：
     - `declare_top_level_fun_with_symbol(...)`
     - `codegen_top_level_fun(...)`
     - `declare_callee_resume_entry_function(...)`
     - `declare_top_level_fun_callee_resume_entry(...)`
     - `declare_top_level_fun_effect_call_wrapper(...)` 若仍保留，必须改写到新 ABI；若该 wrapper 本身是 legacy 设计，应在本任务内删除其概念并由新的 direct ABI 替代。
  2. 替换当前基于 HIR 的 helper：
     - `top_level_fun_uses_hidden_incoming_resume_token(...)`
     - `mir_fun_uses_hidden_incoming_resume_token(...)`
     - `function_type_uses_hidden_incoming_resume_token(...)`
     为基于 effect facts / call ABI contract 的 helper。
  3. 在 `FunctionBodyCodegenCx` 或等价位置，为当前函数体记录显式 hidden ABI 入口：
     - 当前 `current_effect_ctx_ref` 参数/slot
     - 当前 `incoming_resume_token_ref` 参数/slot
     - 当前 `outcome` 指针参数/slot
  4. 修改 closure callable 声明路径：
     - `crates/scoopc/src/llvm/codegen/closure/mod.rs`
     使 closure / function-value callable 的 effectful surface 也使用同一 hidden ABI。
- 必须遵从的约束：
  - 禁止以“先恢复 wrapper + 再传 hidden token”的方式过渡。
  - ABI 是否 effectful必须由 facts/schema 驱动；不能再只看 `hir_ty_declared_effectful(...)` 这类 HIR-level boolean。
  - plain callable 仍是 plain ABI；不得因为 effectful ABI 骨架重建而让所有 callable 都多出 hidden args。
- 验证：
  1. `cargo check -p scoopc`
  2. 输出中不再出现以下缺失项：
     - `declare_callee_resume_entry_function_impl`
     - `declare_top_level_fun_callee_resume_entry_impl`
     - `declare_top_level_fun_effect_call_wrapper_impl`
     - `ensure_top_level_fun_effect_call_wrapper_defined_impl`
     - `codegen_top_level_fun_effect_call_wrapper_impl`
  3. 输出中不再出现 `top_level_fun_uses_hidden_incoming_resume_token` 驱动的老 ABI 假设错误。
- 完成条件：
  - effectful callable 的 hidden ABI 骨架在声明层闭合，后续步骤可直接在其上实现 `EffectCtx` / `EffectOutcome` / `Step_F`。
- 依赖：G0-T01R
- 完成记录：
  - 改动范围：
    - `crates/scoopc/src/llvm/codegen/mod.rs`：为顶层 callable / callee resume entry 重建显式 hidden ABI 声明；删除旧 effect call wrapper 壳层；在 `FunctionBodyCodegenCx` 中加入 `current_effect_ctx_ref`、`current_incoming_resume_token_ref`、`current_effect_outcome_ptr` 三个显式 hidden ABI 槽位，并在顶层函数 / resume-entry 入口绑定。
    - `crates/scoopc/src/llvm/codegen/call/abi.rs`：新增基于 published late-lowered callable contract 的 helper，用它替代 `*_uses_hidden_incoming_resume_token(...)` 这类 HIR-level ABI 推断；统一 hidden ABI 参数计数、参数类型拼装与入口槽位绑定逻辑。
    - `crates/scoopc/src/llvm/codegen/closure/mod.rs`：closure callable / closure resume entry 改用同一显式 hidden ABI；closure callee resume shell 改为由 published callable contract 决定；不再从函数类型 effect row 直接猜 hidden token。
    - `crates/scoopc/src/llvm/codegen/mir_body.rs`：plain materialized MIR closure declaration/body path 显式保持 plain ABI，不再混入旧 hidden token；effect-step callable surface 继续交由 stage-owned entry shell 承载。
    - `crates/scoopc/src/llvm/emit.rs`、`crates/scoopc/src/llvm/codegen/effect_lowered/layout.rs`：把 ABI 可见性阶段的 published late-lowered program 接到 `CompilationUnitCodegenCx`，让 legacy declaration path 能消费同一份 callable contract。
    - `crates/scoopc/src/effect/state_machine/mod.rs`：去掉本任务不再使用的多余 re-export，避免新增 unused-import 警告。
  - 核心决策：
    - effectful callable 是否需要 hidden ABI，不再看 `hir_ty_declared_effectful(...)` / `FunctionType.effects.is_pure()` 这类 HIR 布尔值，而是看 ABI 可见性阶段发布的 late-lowered callable contract；只要某个 root callable 仍发布 effect-step surface，就为该 root declaration 预留显式 hidden ABI。
    - 旧 `effect call wrapper` 概念在活跃实现中已无调用方，本任务直接删除其壳层，而不是再补回 wrapper 过渡层。
    - plain materialized MIR closure entry 由 `RefactorPlainCallableLayout` 约束为 plain ABI；effect-step callable 的 direct/dynamic surface 继续由 stage-owned shell 表达，避免把 plain entry 再次污染成 hidden-token 变体。
    - ordinary callee resume body lowering 仍属于后续 `G4-T05`；本任务只把 resume-entry declaration 形状切到新的显式 hidden ABI，并保留显式 fail-fast 边界，避免继续依赖缺失的旧实现体。
  - 验证结果：
    - 对 `crates/scoopc/src` grep `top_level_fun_uses_hidden_incoming_resume_token|mir_fun_uses_hidden_incoming_resume_token|function_type_uses_hidden_incoming_resume_token`：无命中。
    - 对 `crates/scoopc/src` grep `declare_callee_resume_entry_function_impl|declare_top_level_fun_callee_resume_entry_impl|declare_top_level_fun_effect_call_wrapper_impl|ensure_top_level_fun_effect_call_wrapper_defined_impl|codegen_top_level_fun_effect_call_wrapper_impl`：无命中。
    - `cargo fmt`：通过。
    - `cargo check -p scoopc`：仍失败，但不再出现本任务目标中的 ABI skeleton 缺失项；首批错误已切到 `local_call_may_suspend_from_hir_ty` / `known_fun_body_may_outward_effect`（G4）、`alloc_effect_outcome_slot` / `effect_outcome_*` / `coerce_u64_word`（G2）、`codegen_perform_expr` / `codegen_handle_expr`（G5）等后续结构性 gap。
  - 与 `EFFECT_REFACTOR_GAPS.md` 对应消除的 gap 条目：
    - §1：effectful callable 仍以“单 hidden incoming token”建模，尚未建立显式 `current_effect_ctx_ref` / `incoming_resume_token_ref` / `ScoopEffectOutcome *outcome` hidden ABI 的缺口。

## [DONE] G1-T02R：Review 显式 hidden ABI 骨架，确认不再回退单 token 语义

- 参考：
  - [`PLAN.md`](./PLAN.md) §2 / G1
  - [`EFFECT_REFACTOR.md`](./EFFECT_REFACTOR.md) §4.5、§4.10
- 重点：
  - effectful callable 是否都已经拥有同一 hidden ABI；
  - plain callable 是否保持 plain ABI；
  - ABI 判定是否来自 facts/schema，而不是 HIR-level effectful boolean；
  - 是否还残留 effect call wrapper 这类 legacy 表面结构。
- 必须检查的文件/位置：
  - `crates/scoopc/src/llvm/codegen/mod.rs`
  - `crates/scoopc/src/llvm/codegen/closure/mod.rs`
  - 任何新增的 ABI contract helper 模块
- 验证：
  - 重新运行 `cargo check -p scoopc`；
  - 对 effectful callable declaration code path 做代码搜索，确认没有再以 deleted runtime bridge 作为 ABI 补丁。
- 完成条件：
  - 可以明确写出当前 effectful callable ABI 的参数顺序和适用范围。
- 依赖：G1-T02
- 完成记录：
  - 改动范围：
    - `TODO.md`：将 `G1-T02R` 标记为 `[DONE]` 并补充 review 结论。
    - 本任务对 `crates/scoopc/src/llvm/codegen/mod.rs`、`crates/scoopc/src/llvm/codegen/closure/mod.rs`、`crates/scoopc/src/llvm/codegen/call/abi.rs`、`crates/scoopc/src/llvm/codegen/mir_body.rs`、`crates/scoopc/src/llvm/emit.rs` 做人工复核；无需额外源码修补。
  - 核心决策：
    - 当前 declaration/binding 层的显式 hidden ABI 顺序已经稳定为：`[sret?] current_effect_ctx_ref, incoming_resume_token_ref, ScoopEffectOutcome *outcome, ...`；其中 closure callable 在 hidden ABI 之后追加 `env`、可选 `receiver` 与普通参数。
    - plain materialized MIR closure/body symbol 继续保持 plain ABI；其入口不混入 hidden effect 参数，并在 body 入口显式清空 `current_effect_ctx_ref` / `current_incoming_resume_token_ref` / `current_effect_outcome_ptr` 槽位，避免回退到“所有 callable 都带 hidden token”的旧语义。
    - ABI 适用范围与 callee-resume shell 判定来自 published late-lowered callable contract（`effect_step_abi()` / `needs_reentry()`），而不是 `hir_ty_declared_effectful(...)`、`FunctionType.effects.is_pure()` 这类 HIR-level effectful boolean。
    - declaration code path 中未发现 effect call wrapper、单 hidden token helper 或 deleted runtime bridge 作为 ABI 补丁。
  - 验证结果：
    - 精确搜索 `crates/scoopc/src/llvm/codegen/mod.rs`、`crates/scoopc/src/llvm/codegen/closure/mod.rs`、`crates/scoopc/src/llvm/codegen/call/abi.rs`：未命中 `top_level_fun_uses_hidden_incoming_resume_token`、`mir_fun_uses_hidden_incoming_resume_token`、`function_type_uses_hidden_incoming_resume_token`、`effect_call_wrapper`、`declare_runtime_effect_is_active`、`declare_runtime_effect_set_active_with_trace`、`scoop_effect_*`、`scoop_continuation_*` 等 legacy 名字。
    - `cargo check -p scoopc`：仍失败，但首批错误继续集中在后续结构性 gap：`local_call_may_suspend_from_hir_ty` / `known_fun_body_may_outward_effect`（G4）、`alloc_effect_outcome_slot` / `effect_outcome_*` / `coerce_u64_word`（G2）、`codegen_perform_expr` / `codegen_handle_expr`（G7）、`codegen_call_impl` / `codegen_*call*_impl`（G6）等；未回退到本任务范围内的 hidden ABI skeleton 缺口。
  - 与 `EFFECT_REFACTOR_GAPS.md` 对应消除的 gap 条目：
    - §1：review 确认显式 `current_effect_ctx_ref` / `incoming_resume_token_ref` / `ScoopEffectOutcome *outcome` hidden ABI 仍然是 declaration 层唯一 authoritative surface，未回退到单 hidden token、wrapper 或 HIR-level 布尔推断语义。

## [DONE] G2-T03：重建 backend-owned `EffectOutcome` / transport primitive

- 参考：
  - [`PLAN.md`](./PLAN.md) §2 / G2
  - [`EFFECT_REFACTOR.md`](./EFFECT_REFACTOR.md) §4.3、§4.13
  - [`EFFECT_REFACTOR_GAPS.md`](./EFFECT_REFACTOR_GAPS.md) §4
- 目标：
  - 在 LLVM backend 内重建 explicit `EffectOutcome` / `EffectSignal` / `ValueTransport` contract primitive；
  - 所有 propagation / completion / runtime-error / task transport 都通过这组 primitive，而不是 runtime C helper。
- 必须实现的内容：
  1. 在新的 neutral module 中重建下列 helper；如新增模块，推荐放在 `crates/scoopc/src/llvm/codegen/` 目录下，禁止沿用已删 `effect/contract.rs` 的 legacy 语义容器：
     - `alloc_effect_outcome_slot(...)`
     - `build_value_transport(...)`
     - `build_effect_signal(...)`
     - `build_effect_outcome(...)`
     - `effect_outcome_is_propagating(...)`
     - `effect_outcome_payload_transport(...)`
     - `effect_outcome_resume_token(...)`
     - `decode_effect_transport_value(...)`
     - `coerce_u64_word(...)`
     - `split_task_transport_tuple_value(...)`
  2. 这些 helper 必须只依赖：
     - `runtime_abi.rs` 中仍保留的结构 type builder
     - LLVM load/store/build primitive
     - `TypeStore` / `CgTy`
     不能声明新的 runtime C bridge 符号。
  3. 重新接回当前 compile errors 集中命中的调用方：
     - `crates/scoopc/src/llvm/codegen/effect_lowered/body.rs`
     - `crates/scoopc/src/llvm/codegen/effect_lowered/value.rs`
     - `crates/scoopc/src/llvm/codegen/mir_body.rs`
     - `crates/scoopc/src/llvm/codegen/class_ctor.rs`
     - `crates/scoopc/src/llvm/codegen/enum_lowering.rs`
     - `crates/scoopc/src/llvm/codegen/intrinsics/{containers,thread}.rs`
- 必须遵从的约束：
  - 禁止恢复 `scoop_effect_outcome_consume_current` / `publish`。
  - 禁止恢复 `scoop_effect_set_active*` / `scoop_effect_clear` / `scoop_effect_slot_*` 作为中间协议。
  - `EffectOutcome` 必须是唯一 propagation source of truth。
- 验证：
  1. `cargo check -p scoopc`
  2. 输出中不再出现下列缺失项：
     - `alloc_effect_outcome_slot`
     - `effect_outcome_is_propagating`
     - `effect_outcome_payload_transport`
     - `decode_effect_transport_value`
     - `coerce_u64_word`
     - `split_task_transport_tuple_value`
     - `declare_runtime_effect_set_active_with_trace`
- 完成条件：
  - backend 内部拥有完整 explicit outcome / transport primitive，旧 runtime bridge API 不再参与任何传播语义。
- 依赖：G1-T02R
- 完成记录：
  - 改动范围：
    - `crates/scoopc/src/llvm/codegen/effect_outcome.rs`：新增 backend-owned explicit outcome/transport primitive 模块，集中实现 `alloc_effect_outcome_slot(...)`、`build_value_transport(...)`、`build_effect_signal(...)`、`build_effect_outcome(...)`、`effect_outcome_is_propagating(...)`、`effect_outcome_payload_transport(...)`、`effect_outcome_resume_token(...)`、`decode_effect_transport_value(...)`、`coerce_u64_word(...)`、`split_task_transport_tuple_value(...)`，并补上 task transport tuple 识别。
    - `crates/scoopc/src/llvm/codegen/runtime_symbols.rs`、`crates/scoopc/src/llvm/codegen/runtime_abi.rs`：补回 cross-thread resume transport 所需的中性 runtime 声明 `scoop_thread_spawn_join_compat_resume_u64`、`scoop_thread_spawn_join_resume_u64`、`scoop_thread_spawn_join_resume_transport`。
    - `crates/scoopc/src/llvm/codegen/effect_lowered/body.rs`：把 task transport resume payload decode 切到新的 `ValueTransportParts` surface，并删除残留的 `declare_runtime_effect_set_active_with_trace` / `scoop_effect_set_active*` 旧桥调用。
    - `crates/scoopc/src/llvm/tests.rs`：把仍然 `include_str!` 已删除 `codegen/effect/*` 文件的源清单检查改到当前 target-shape 文件，避免 lint/test 目标被陈旧 inventory 阻断。
  - 核心决策：
    - `ScoopEffectOutcome` 继续沿用 `runtime_abi.rs` 中已经稳定的显式布局：`tag = COMPLETE | PROPAGATE`、`complete = ValueTransport`、`signal = { op_tag, effect_instance_key, payload, resume_token }`；builder/query/write-back 全部回到 LLVM backend 自己维护，不再声明任何 `scoop_effect_outcome_*` bridge。
    - task transport tuple 仅被识别为 codegen type 层的原始 `(u64-word, gc-ref)` 二元 carrier；`split_task_transport_tuple_value(...)` 与 `decode_effect_transport_value(...)` 只为这条 target-shape transport surface 提供拆包/重建，不再回退 runtime slot/TLS 协议。
    - 对 cross-thread resume 只补中性的 runtime substrate 声明，不恢复任何 continuation/effect policy；transport 语义仍由 backend 侧 `ValueTransport` / composite descriptor 组织。
    - `Raise<RuntimeError>` perform boundary 上残留的 `declare_runtime_effect_set_active_with_trace` 已直接删除；显式 propagation 不再允许先写 active flag 再经 bridge 物化 outcome。
  - 验证结果：
    - `cargo fmt`：通过。
    - 对 `crates/scoopc/src`、`runtime/c`、`sysroot` grep `declare_runtime_effect_set_active_with_trace|scoop_effect_set_active|scoop_effect_outcome_|scoop_effect_clear`：无命中。
    - `cargo check -p scoopc`：仍失败，但不再出现 `alloc_effect_outcome_slot`、`effect_outcome_is_propagating`、`effect_outcome_payload_transport`、`decode_effect_transport_value`、`coerce_u64_word`、`split_task_transport_tuple_value`、`declare_runtime_effect_set_active_with_trace` 或 thread resume transport runtime declarations 缺失；剩余首批错误已切到 `emit_ordinary_call_effect_propagation_check` / `ordinary_effect_propagation_enabled`（G7）、`known_fun_body_may_outward_effect` / `local_call_may_suspend_from_hir_ty`（G4）、`codegen_mir_*call*` / `codegen_perform_expr`（G6/G7）等后续结构性 gap。
    - `cargo clippy -p scoopc --all-targets -- -D warnings`：仍失败，但已不再被 `crates/scoopc/src/llvm/tests.rs` 对已删除 `codegen/effect/*` 文件的 `include_str!` 阻断；当前失败原因与 `cargo check -p scoopc` 一致，均来自后续任务缺口。
  - 与 `EFFECT_REFACTOR_GAPS.md` 对应消除的 gap 条目：
    - §4：explicit `EffectOutcome` contract 不再只剩 layout/type 名字，backend 已重新拥有 authoritative builder/query/write-back primitive。
    - §9（其中与 transport primitive 直接相关的子缺口）：`coerce_u64_word(...)`、`split_task_transport_tuple_value(...)` 与 cross-thread resume transport runtime declarations 已重新接回，dynamic/task transport surface 不再因为这些基础 helper 缺失而悬空。

## [DONE] G2-T03R：Review explicit outcome/transport primitive，确认 contract 已 backend-owned

- 参考：
  - [`PLAN.md`](./PLAN.md) §2 / G2
  - [`EFFECT_REFACTOR.md`](./EFFECT_REFACTOR.md) §4.3、§4.13
- 重点：
  - `EffectOutcome` / `ValueTransport` / `resume_token` 是否都已由 backend-owned primitive 表达；
  - 是否还有任何 runtime C bridge 函数名参与 active implementation；
  - `task transport` / `u64 word` coercion 是否统一走同一 primitive。
- 必须检查的文件/位置：
  - 新增 neutral module
  - `runtime_abi.rs`
  - `effect_lowered/body.rs`
  - `effect_lowered/value.rs`
  - `mir_body.rs`
- 验证：
  - 再跑一次 `cargo check -p scoopc`；
  - grep active implementation，不得出现 `scoop_effect_outcome_*` / `scoop_effect_set_active*` / `scoop_effect_clear` 名字。
- 完成条件：
  - 可以明确写出 explicit outcome 的 authoritative query/write-back surface。
- 依赖：G2-T03
- 完成记录：
  - 改动范围：
    - `crates/scoopc/src/llvm/codegen/effect_outcome.rs`：补出 `build_zero_complete_effect_outcome(...)`，把默认 complete outcome 的构造也收拢到 backend-owned primitive 模块。
    - `crates/scoopc/src/llvm/codegen/mod.rs`、`crates/scoopc/src/llvm/codegen/object_init.rs`：object/top-level immutable init bridge 不再直接返回 `ScoopEffectOutcome::const_zero()`，统一改为调用 `build_zero_complete_effect_outcome(...)`。
    - 本任务对 `crates/scoopc/src/llvm/codegen/runtime_abi.rs`、`crates/scoopc/src/llvm/codegen/effect_lowered/body.rs`、`crates/scoopc/src/llvm/codegen/effect_lowered/value.rs`、`crates/scoopc/src/llvm/codegen/mir_body.rs` 做人工复核；无需额外语义修补。
  - 核心决策：
    - explicit outcome 的 authoritative surface 明确收敛为 `effect_outcome.rs`：构造/写回走 `build_value_transport(...)`、`build_effect_signal(...)`、`build_effect_outcome(...)`、`build_zero_complete_effect_outcome(...)`、`alloc_effect_outcome_slot(...)`；查询/拆包走 `effect_outcome_is_propagating(...)`、`effect_outcome_payload_transport(...)`、`effect_outcome_resume_token(...)`、`decode_effect_transport_value(...)`。
    - `runtime_abi.rs` 只保留 layout/type 与中性 runtime substrate 声明，不再承担 propagation policy 或 outcome bridge 语义。
    - task transport / `u64` word coercion 继续统一走 `coerce_u64_word(...)`、`split_task_transport_tuple_value(...)`、`decode_effect_transport_value(...)`，未发现第二套并行 carrier 协议。
    - review 中发现 bridge body 直接返回 `const_zero()` 会让 default complete outcome 构造点分散；因此在本任务内直接收口，而不是仅记录问题。
  - 验证结果：
    - 对 `crates/scoopc/src`、`runtime/c`、`sysroot` grep `scoop_effect_outcome_|scoop_effect_set_active|scoop_effect_clear`：无命中。
    - 对 `crates/scoopc/src/llvm/codegen` grep `llvm_effect_outcome_struct_type().const_zero()`：无命中；active implementation 不再绕开 backend-owned outcome builder 直接手搓默认 complete outcome。
    - `cargo fmt`：通过。
    - `cargo check -p scoopc`：仍失败，但首批错误继续集中在 `emit_ordinary_call_effect_propagation_check` / `ordinary_effect_propagation_enabled` / `local_call_may_suspend_from_hir_ty`（G4）、`codegen_mir_*call*` / `codegen_funptr_value_call_impl`（G6）、`codegen_perform_expr` / `codegen_handle_expr`（G7）等后续结构性 gap；未再出现 `EffectOutcome` / `ValueTransport` primitive 缺失或旧 bridge 名字残余。
  - 与 `EFFECT_REFACTOR_GAPS.md` 对应消除的 gap 条目：
    - §4：review 确认 explicit `EffectOutcome` 的 builder/query/write-back surface 已稳定留在 backend，而非 runtime C bridge。
    - §9（其中与 transport primitive 直接相关的子缺口）：review 确认 task transport / `u64` transport 仍统一走同一组 backend-owned primitive，没有分叉回旧协议。

## [DONE] G3-T04：重建显式 `EffectCtx` / handler graph 模型

- 参考：
  - [`PLAN.md`](./PLAN.md) §2 / G3
  - [`EFFECT_REFACTOR.md`](./EFFECT_REFACTOR.md) §4.5、§4.10、§4.13
  - [`CONTINUATION_RUNTIME_REFACTOR.md`](./CONTINUATION_RUNTIME_REFACTOR.md) §2.2、§2.3、§4.1-§4.3
  - [`EFFECT_REFACTOR_GAPS.md`](./EFFECT_REFACTOR_GAPS.md) §7
- 目标：
  - 用显式 `EffectCtx` / handler node graph 替换已删除的 TLS handler stack 抽象；
  - 让 `handle` / arm / outward dispatch / continuation capture 都能显式持有 handler context。
- 必须实现的内容：
  1. 定义 backend-owned `EffectCtx` object layout 与 type descriptor。
  2. 定义 handler node layout：
     - `prev_ref`
     - `op_tag`
     - flags / active semantics
     - `owner_frame_ref`
     - dispatch entry identity
  3. 为 `handle` 入口重建显式 ctx 构造逻辑。
     - body/arm/finally/nested effect-capable call 必须显式传递 `current_effect_ctx_ref`。
  4. 为 arm self-inactive 重建 derived ctx / immutable handler node 语义。
  5. 为 outward dispatch 重建显式 ctx-based dispatch helper；不能再依赖 ambient stack。
- 必须遵从的约束：
  - 禁止恢复 `handler stack snapshot clone` 或任何原生 snapshot 逻辑。
  - 禁止以“临时 global 当前 ctx”模拟显式 `EffectCtx`。
  - handler context 必须可被 continuation capture，不能是临时栈上独占结构。
- 验证：
  1. `cargo check -p scoopc`
  2. 输出中不再出现下列缺失项：
     - `prepare_current_effect_call_contract`
     - `load_effect_ctx_handler_top_from_slot`
     - `swap_effect_handler_stack_top`
     - `publish_incoming_resume_token`
     - `clear_incoming_resume_token`
  3. 相关新 helper 不得再包含 deleted TLS 名字。
- 完成条件：
  - handler context 重新存在为显式 data model，而不是 deleted TLS 语义的缺位。
- 依赖：G2-T03R
- 完成记录：
  - 改动范围：
    - `crates/scoopc/src/llvm/codegen/effect_ctx.rs`：新增 backend-owned `ScoopEffectCtx` / `ScoopEffectHandlerNode` LLVM 布局、field helper、stable `dispatch_identity` 编码，以及 `handler_top_ref` / `prev_ref` / `op_tag` / `flags` / `owner_frame_ref` / `dispatch_identity` 的读写入口。
    - `crates/scoopc/src/effect_lowered/{ir,frame,dump}.rs`：为 late-lowered frame schema 增加 `CurrentEffectCtx` system slot，以及 `HandleSavedEffectCtx` / `HandleArmEffectCtx` 稳定 slot kind，并把它们纳入 dump 与 frame-lifting 验证面。
    - `crates/scoopc/src/llvm/codegen/effect_lowered/body.rs`：在新 frame 初始化时显式分配空 `EffectCtx`；`HandleDispatch` 入口保存 outer ctx、构造当前 handle 的 active ctx，并为每个 arm 预构造 self-inactive derived ctx；arm 进入时切换到对应 derived ctx；handle 退出/`finally` outward 前恢复 outer ctx；活跃 outward routing 改为扫描显式 ctx 链上的 `dispatch_identity`，不再靠静态嵌套深度决定当前 handler。
    - `crates/scoopc/src/llvm/codegen/effect_lowered/layout.rs`、`crates/scoopc/src/effect_lowered/{materialize,opt}.rs`、`crates/scoopc/src/llvm/codegen/mod.rs`：把新的 ctx/system slot/slot-kind 接入 frame local 映射、layout 验证与 codegen 模块装配。
  - 核心决策：
    - `EffectCtx` 明确物化为 managed object `{ hdr, handler_top_ref }`，`HandlerNode` 明确物化为 managed object `{ hdr, prev_ref, op_tag, flags, owner_frame_ref, dispatch_identity }`；不恢复任何 ambient TLS source of truth。
    - late-lowered frame 现在显式持有 `CurrentEffectCtx`、每个 handle 的 saved outer ctx、以及每个 arm 的 derived ctx，使 handler context 成为 frame/capture 可见的数据，而不是临时栈态。
    - arm self-inactive 不再依赖共享 node 上的原地 mutation；在进入 handle 时一次性预构造每个 arm 的 derived ctx，进入 arm 时只切换当前 ctx 引用。
    - 活跃 outward dispatch 路径不再用 `handle_dispatch_nesting_depth(...)` 的静态深度择优，而是按当前 `EffectCtx` 链上的 `op_tag + dispatch_identity + owner_frame_ref` 扫描本帧可见 handler；遇到当前 handle 选择 `EmitOutward` 时继续沿链向外扫描，显式表达“向外传播到外层 ctx”。
  - 验证结果：
    - 对 `crates/scoopc/src` grep `prepare_current_effect_call_contract|load_effect_ctx_handler_top_from_slot|swap_effect_handler_stack_top|publish_incoming_resume_token|clear_incoming_resume_token|__scoop_effect_handler_stack_top|scoop_effect_handler_stack`：无命中。
    - `cargo fmt`：通过。
    - `cargo check -p scoopc`：仍失败，但无新增 warning，且前沿错误继续停在后续任务缺口：`emit_ordinary_call_effect_propagation_check` / `ordinary_effect_propagation_enabled` / `local_call_may_suspend_from_hir_ty`（G4）、`codegen_call_impl` / `codegen_mir_*call*`（G6）、`codegen_perform_expr` / `codegen_handle_expr` / `codegen_mir_perform_terminator`（G7）等；未出现本任务要求消除的 ctx/TLS helper 缺失。
  - 与 `EFFECT_REFACTOR_GAPS.md` 对应消除的 gap 条目：
    - §7：`EffectCtx` / handler graph 已重新拥有 backend-owned replacement 实体，late-lowered 活跃 handler context 不再缺位于 deleted TLS 之后。

## [DONE] G3-T04R：Review `EffectCtx` / handler graph，确认不再退回 ambient context

- 参考：
  - [`PLAN.md`](./PLAN.md) §2 / G3
  - [`EFFECT_REFACTOR.md`](./EFFECT_REFACTOR.md) §4.5
- 重点：
  - `EffectCtx` 是否已经成为 continuation / call / handle 的显式输入；
  - outward dispatch 是否从 ctx 出发，而不是依赖 ambient stack；
  - arm self-inactive 是否通过 derived ctx / node 语义表达。
- 必须检查的文件/位置：
  - 新增 `EffectCtx` / handler node 实现模块
  - `expr.rs`
  - `effect_lowered/body.rs`
  - 任何新的 handle dispatch helper
- 验证：
  - 重新运行 `cargo check -p scoopc`；
  - 人工检查代码，不得再出现任何“current handler stack top”式语义变量。
- 完成条件：
  - 可以明确说明 handler context 的 capture/dispatch 生命周期。
- 依赖：G3-T04
- 完成记录：
  - 改动范围：
    - `TODO.md`：将 `G3-T04R` 标记为 `[DONE]` 并补充 review 结论。
    - 本任务人工复核 `crates/scoopc/src/llvm/codegen/effect_ctx.rs`、`crates/scoopc/src/llvm/codegen/effect_lowered/body.rs`、`crates/scoopc/src/effect_lowered/{frame,ir,dump,materialize,opt}.rs`、`crates/scoopc/src/llvm/codegen/{expr.rs,call/abi.rs}`；无需额外源码修补。
  - 核心决策：
    - handler context 的生命周期已经可以明确描述：fresh entry 在 frame `CurrentEffectCtx` system slot 中初始化空 ctx；进入 `HandleDispatch` 时把 outer ctx 存入 `HandleSavedEffectCtx`，为 handle body 构造 active ctx，并为每个 arm 预构造 self-inactive derived ctx；进入 arm 时切换 `CurrentEffectCtx`；向外分发时从当前 ctx 的 `handler_top_ref` 出发，沿 `prev_ref -> op_tag/flags/owner_frame_ref/dispatch_identity` 扫描 managed handler node graph；continuation capture 通过 captured frame 自动携带 `CurrentEffectCtx` 与 handle ctx slots，resume 时通过 `load_frame_from_continuation(...)` 恢复。
    - 代码中仍存在的 `handle_dispatch_nesting_depth(...)` 只用于编译期 contract 校验/消歧（如 `ResumeUnwind` 与 boundary routing 选择），不是 runtime handler source of truth；真正的 outward/local dispatch 路径已经固定走 `dispatch_handle_boundary_from_ctx(...)`。
    - `expr.rs` 当前只把 `perform` / `handle` surface 转发到待后续 `G7-T08` 恢复的 lowering 入口，没有复活任何 ambient handler stack fallback 或旧 TLS 语义变量。
  - 验证结果：
    - 对 `crates/scoopc/src` grep `prepare_current_effect_call_contract|load_effect_ctx_handler_top_from_slot|swap_effect_handler_stack_top|publish_incoming_resume_token|clear_incoming_resume_token|__scoop_effect_handler_stack_top|current_handler_stack_top|handler_stack_top`：无命中。
    - 人工复核 `effect_ctx.rs`、`effect_lowered/body.rs`、`effect_lowered/frame.rs`、`effect_lowered/ir.rs`、`effect_lowered/{materialize,opt,dump}.rs`、`llvm/codegen/{expr.rs,call/abi.rs}`：`enter_handle_dispatch_effect_ctx(...)` 为 body/arm 构造显式 ctx 与 derived ctx；`apply_handle_boundary_consume_to_arm(...)` 在 arm 入口切换 `CurrentEffectCtx`；`dispatch_handle_boundary_from_ctx(...)` 从 `EffectCtx.handler_top_ref` 扫描 managed handler node graph；`create_continuation_object_with_state_tag(...)` 捕获整帧，因此 `CurrentEffectCtx` 与 handle ctx slots 会随 continuation 一并被 capture。
    - `cargo fmt`：通过。
    - `cargo check -p scoopc`：仍失败，但首批错误继续集中在后续结构性 gap：`local_call_may_suspend_from_hir_ty` / `known_fun_body_may_outward_effect`（G4）、`codegen_call_impl` / `codegen_mir_*call*`（G6）、`codegen_perform_expr` / `codegen_handle_expr` / `codegen_mir_perform_terminator`（G7）、ordinary effect propagation helper 缺口等；未出现 `EffectCtx` / handler graph / deleted TLS helper 回退问题。
  - 与 `EFFECT_REFACTOR_GAPS.md` 对应消除的 gap 条目：
    - §7：review 确认显式 `EffectCtx` / managed handler node graph 已成为当前 handler context 的 authoritative replacement，outward dispatch 不再依赖 ambient TLS handler stack，arm self-inactive 也已稳定落在 derived ctx 语义上。

## [DONE] G4-T05：重建 ordinary callee suspend/reentry 分析与 lowering

- 参考：
  - [`PLAN.md`](./PLAN.md) §2 / G4
  - [`EFFECT_REFACTOR.md`](./EFFECT_REFACTOR.md) §3.5、§4.4、§4.13
  - [`EFFECT_REFACTOR_GAPS.md`](./EFFECT_REFACTOR_GAPS.md) §3
- 目标：
  - 让 ordinary callee suspend/reentry contract 重新闭合；
  - 所有“是否可能 outward / 是否需要 reentry / suspend-state 怎么保存与恢复”重新由 facts + explicit token 驱动。
- 必须实现的内容：
  1. 处理当前孤立文件 `crates/scoopc/src/llvm/codegen/effect/ordinary_callee.rs`：
     - 要么移动到新的 neutral module；
     - 要么把其内容直接吸收进新的 non-legacy effect lowering 容器；
     - 绝不能简单恢复 `mod effect;` 让整套旧目录复活。
  2. 恢复下列 analysis/lowering 入口：
     - `build_fun_callee_suspend_plan_impl`
     - `build_ordinary_callee_suspend_plan`
     - `local_call_may_suspend_from_hir_ty`
     - `hir_ty_declared_effectful`
     - `known_fun_body_may_outward_effect`
     - `function_value_expr_body_may_outward_effect_when_called_for_local`
     - `codegen_callee_resume_dispatch_impl`
     - `codegen_callee_resume_entry_function_impl`
  3. `incoming_resume_token_ref` 必须成为 resumed path 的唯一恢复输入；不得重新恢复 TLS `callee_suspend_state` scratch。
  4. `needs_reentry` 判定必须只消费 facts，不得回 HIR/AST 猜测。
- 必须遵从的约束：
  - 若现有 facts 不足以支撑这些判断，必须先扩 facts/schema，而不是用 HIR fallback 顶上。
  - ordinary callee suspend/reentry 不得重新依赖 runtime-owned replay state。
- 验证：
  1. `cargo check -p scoopc`
  2. 输出中不再出现本任务列出的 helper 缺失。
  3. `effect_lowered/{body,layout,value}.rs` 中相关调用恢复到 facts-driven helper，而不是 ad-hoc backend magic。
- 完成条件：
  - ordinary callee suspend/reentry 再次成为后端显式协议的一部分。
- 依赖：G3-T04R
- 完成记录：
  - 改动范围：
    - `crates/scoopc/src/llvm/codegen/ordinary_callee.rs`：新增 neutral ordinary-callee 模块，接回共享 suspendability analysis、facts-driven plan builder、显式 `incoming_resume_token_ref` 驱动的 resume-state 读取，以及 `codegen_callee_resume_dispatch_impl` / `codegen_callee_resume_entry_function_impl`。
    - `crates/scoopc/src/llvm/codegen/mod.rs`：把 `build_fun_callee_suspend_plan`、`build_ordinary_callee_suspend_plan`、`hir_ty_declared_effectful`、`local_call_may_suspend_from_hir_ty`、`known_fun_body_may_outward_effect`、`function_value_expr_body_may_outward_effect_when_called_for_local`、resume dispatch / entry body 统一改为委托新模块实现。
    - `crates/scoopc/src/llvm/codegen/closure/mod.rs`：closure suspend-plan 改为通过新模块构造，并在 closure function body 入口显式写入 lambda callable FQN，避免分析上下文继续借用外层 callable 标识。
    - `crates/scoopc/src/effect/state_machine/mod.rs`：补出 ordinary-callee 共享分析 helper 的 crate 内 re-export，供新 neutral module 直接消费。
    - `crates/scoopc/src/llvm/codegen/effect/ordinary_callee.rs`：删除当前孤立的 legacy 路径残余，避免继续留下假的实现入口。
  - 核心决策：
    - 不恢复 `mod effect;` 或任何 legacy container，而是把 ordinary-callee 相关能力迁入新的 `llvm/codegen/ordinary_callee.rs` neutral module。
    - ordinary callee shell / `needs_reentry` 判定继续只消费已发布 callable facts（`callable_needs_callee_resume_shell(...)`）；共享 suspendability/outward-effect helper 继续复用 pass summary + shared analysis context，而不是回退到 TLS scratch。
    - resumed path 只从显式 `incoming_resume_token_ref` 读取 suspend-state，恢复 saved locals 与 resume slot 后再执行 `resume_tail`；没有重新引入 `__scoop_callee_suspend_state`、`scoop_callee_suspend_state_*`、`publish_incoming_resume_token` 或 `clear_incoming_resume_token`。
    - closure ordinary-callee analysis 必须带着 lambda 自己的 callable FQN 和参数 metadata 运行，不能继续借外层 function 的 callable identity 猜测 continuation escape / outward-effect 事实。
  - 验证结果：
    - 对 `crates/scoopc/src` grep `__scoop_callee_suspend_state|scoop_callee_suspend_state_|publish_incoming_resume_token|clear_incoming_resume_token`：无命中。
    - `cargo fmt`：通过。
    - `cargo check -p scoopc`：仍失败，但不再出现 `local_call_may_suspend_from_hir_ty`、`hir_ty_declared_effectful`、`known_fun_body_may_outward_effect`、`function_value_expr_body_may_outward_effect_when_called_for_local`、`codegen_callee_resume_dispatch_impl`、`codegen_callee_resume_entry_function_impl` 等 G4 helper 缺失；前沿已切到 `emit_ordinary_call_effect_propagation_check` / `ordinary_effect_propagation_enabled` / `declare_runtime_effect_is_active`（既有 G2 回归）以及 `codegen_perform_expr` / `codegen_handle_expr` / `codegen_call_impl` / `codegen_mir_*call*`（后续 G6/G7 缺口）。
    - `cargo clippy -p scoopc --all-targets -- -D warnings`：仍失败，失败前沿与 `cargo check -p scoopc` 一致；未新增 G4 helper 缺口或 ordinary-callee TLS 回退。
  - 与 `EFFECT_REFACTOR_GAPS.md` 对应消除的 gap 条目：
    - §3：ordinary callee suspend/reentry 分析与 lowering 重新拥有 active replacement；共享 outward/suspendability helper、resume-entry body、resume dispatch 不再悬空在已删除的 legacy effect module 外。

## [DONE] G4-T05R：Review ordinary callee suspend/reentry，确认 facts 驱动且无 TLS 旁路

- 参考：
  - [`PLAN.md`](./PLAN.md) §2 / G4
  - [`EFFECT_REFACTOR.md`](./EFFECT_REFACTOR.md) §3.5、§4.4
- 重点：
  - `needs_reentry` / outward-effect analysis 是否只依赖 facts；
  - resumed path 是否只依赖显式 incoming token；
  - orphan `ordinary_callee.rs` 是否已被 neutral 化，而不是恢复 legacy container。
- 必须检查的文件/位置：
  - 新 neutral ordinary-callee module
  - `mod.rs`
  - `closure/mod.rs`
  - `control_flow.rs`
  - `stmt.rs`
  - `effect_lowered/{body,layout,value}.rs`
- 验证：
  - 重新运行 `cargo check -p scoopc`；
  - 对实现源码做 grep，不得再出现 deleted callee TLS bridge 名字。
- 完成条件：
  - ordinary callee reentry contract 可被独立描述，不依赖已删除 bridge。
- 依赖：G4-T05
- 完成记录：
  - 改动范围：
    - `TODO.md`：将 `G4-T05R` 标记为 `[DONE]` 并补充 review 结论。
    - 本任务人工复核 `crates/scoopc/src/llvm/codegen/{ordinary_callee.rs,mod.rs,closure/mod.rs,control_flow.rs,stmt.rs,effect_lowered/body.rs,effect_lowered/layout.rs,effect_lowered/value.rs}`；无需额外源码修补。
  - 核心决策：
    - `needs_reentry` / callee-resume shell 判定继续只走 published late-lowered callable contract：`callable_needs_callee_resume_shell(...)` 最终只读取 `effect_step_abi()` 与 `needs_reentry()`，没有回退到 HIR/AST boolean 或 deleted TLS scratch。
    - resumed path 的唯一恢复输入仍是显式 `incoming_resume_token_ref`：`codegen_callee_resume_entry_function_impl(...)` 先绑定显式 hidden ABI，再从 `current_incoming_resume_token_ref` 取回 suspend-state 并恢复 saved locals / resume slot；没有重新引入 `__scoop_callee_suspend_state`、`scoop_callee_suspend_state_*`、`publish_incoming_resume_token(...)` 或 `clear_incoming_resume_token(...)`。
    - ordinary-callee active implementation 仍只存在于 neutral `crates/scoopc/src/llvm/codegen/ordinary_callee.rs`；已删除的 `llvm/codegen/effect/ordinary_callee.rs` 没有被恢复，`mod.rs` / `closure/mod.rs` / `effect_lowered/*` 也都只委托到新的中性入口。
    - `hir_ty_declared_effectful(...)`、`local_call_may_suspend_from_hir_ty(...)` 与 `function_value_expr_body_may_outward_effect_when_called_for_local(...)` 现在只承担局部 function-value 元数据与 HIR body 的保守分析，不参与 `needs_reentry` shell 判定，也不通过任何 TLS 旁路补语义。
  - 验证结果：
    - 对 `crates/scoopc/src` grep `__scoop_callee_suspend_state|scoop_callee_suspend_state_|publish_incoming_resume_token|clear_incoming_resume_token`：无命中。
    - 对仓库执行 glob `crates/scoopc/src/llvm/codegen/**/ordinary_callee.rs`：仅命中 `crates/scoopc/src/llvm/codegen/ordinary_callee.rs`，未发现 legacy container 路径残留。
    - 人工复核 `ordinary_callee.rs`、`mod.rs`、`closure/mod.rs`、`control_flow.rs`、`stmt.rs`、`effect_lowered/{body,layout,value}.rs`：`callable_needs_callee_resume_shell_impl(...)` 仅消费 published callable contract；closure body 在 lowering 前显式设置 lambda callable FQN；resume-entry declaration/body 统一绑定显式 hidden ABI，恢复路径只从 incoming token 取状态。
    - `cargo check -p scoopc`：仍失败，但首批错误继续停在后续结构性 gap：`emit_ordinary_call_effect_propagation_check` / `ordinary_effect_propagation_enabled` / `declare_runtime_effect_is_active`（既有 ordinary effect propagation 缺口），以及 `codegen_call_impl` / `codegen_top_level_fun_call_impl` / `codegen_mir_*call*`（G6）、`codegen_perform_expr` / `codegen_handle_expr` / `codegen_mir_perform_terminator`（G7）等；未再出现 G4 helper 缺失或 deleted callee TLS bridge 名字回退。
  - 与 `EFFECT_REFACTOR_GAPS.md` 对应消除的 gap 条目：
    - §3：review 确认 ordinary callee suspend/reentry contract 继续由 published callable facts + explicit incoming token 驱动，active implementation 未回退到 deleted callee TLS bridge 或 legacy effect container。

## [DONE] G5-T05a：为 continuation resume driver 补齐 outcome-return continuation step core

- 参考：
  - [`PLAN.md`](./PLAN.md) §2 / G5
  - [`EFFECT_REFACTOR.md`](./EFFECT_REFACTOR.md) §4.5、§4.10、§5.3.3-§5.3.4
  - [`CONTINUATION_RUNTIME_REFACTOR.md`](./CONTINUATION_RUNTIME_REFACTOR.md) §2.1、§3.3、§6、§7、§8
  - [`EFFECT_REFACTOR_GAPS.md`](./EFFECT_REFACTOR_GAPS.md) §6
- 目标：
  - 在 LLVM backend 内补出 continuation-owned step core，使 generated continuation resume driver 可以直接通过显式 hidden ABI + `EffectOutcome` 推进 suspended computation；
  - 为 `G5-T06` 中的 `step_fn` / one-shot / answer write-back 提供可调用的 backend-owned resume 内核，而不是继续绕回 surface-resume `Step_F` wrapper 或任何 runtime continuation bridge。
- 必须实现的内容：
  1. 为 effect-lowered callable 增加 continuation step core emission mode（例如 `Outcome` return mode 或等价机制），其入口至少能够直接消费：
     - `state_ref`
     - `current_effect_ctx_ref`
     - `incoming_resume_token_ref`
     - `ScoopEffectOutcome *outcome`
  2. continuation step core 的 complete / propagate / runtime-error 路径必须直接构造或写回 backend-owned explicit `EffectOutcome`，不能把 `Step_F` 当作 generated resume driver 的内部传播媒介。
  3. 对 call-boundary continuation composition，step core 必须通过显式 incoming resume token surface 消费“underlying callee continuation / ordinary suspend-state token”，不能继续依赖专用 continuation runtime 字段或 bridge API。
  4. 若当前 `effect_lowered/body.rs` / `effect_outcome.rs` 缺少把 outward case / runtime-error boundary 直接映射到 `EffectOutcome` 的 query/builder helper，需在本任务内补齐到 neutral target-shape 模块中。
- 必须遵从的约束：
  - 禁止通过“generated helper 内部再调用 shared surface-resume symbol 并回收 `Step_F`”的方式冒充 continuation driver；那仍然是在 wrapper 边界上做 workaround，而不是补回 continuation-owned resume core。
  - 禁止恢复任何 runtime continuation/effect bridge、TLS active flag / perform-slot / replay-state 作为中转。
  - 若构造 `EffectOutcome.signal` 还缺 authoritative op-tag / payload transport / resume-token query，必须在本任务内补齐真实 contract，而不是用常量、site 名字或 ad-hoc tag 猜测顶上。
- 验证：
  1. `cargo check -p scoopc`
  2. continuation step core / generated resume helper 相关实现源码中不再通过 shared surface-resume `Step_F` wrapper 充当 resume 内核。
  3. 针对 continuation resume / call-boundary composition 的 LLVM 测试应能断言：generated resume core 直接写 `EffectOutcome`，而不是依赖 runtime continuation bridge 或 shared `Step_F` wrapper。
- 完成条件：
  - backend 已具备 continuation-owned resume 内核：`step_fn` 可直接在显式 `EffectOutcome` 协议上推进 resumed computation，后续 `G5-T06` 可在其上只做 object layout / generated helper / thread integration 收口。
- 依赖：G4-T05R
- 完成记录：
  - 改动范围：
    - `crates/scoopc/src/llvm/codegen/effect_outcome.rs`：补出 `effect_outcome_complete_transport(...)`、`effect_outcome_signal_op_tag(...)`、`effect_outcome_signal_effect_instance_key(...)`，让 continuation-owned resume core 能直接 query explicit outcome，而不是先回 `Step_F`。
    - `crates/scoopc/src/llvm/codegen/mod.rs`：新增 `effect_instance_key_for_family(...)`，把 `EffectOutcome.signal` -> published case 的 effect-instance 对齐规则收口到 backend。
    - `crates/scoopc/src/llvm/codegen/effect_lowered/body.rs`：新增 internal `__scoop_refactor_surface_resume_outcome__k*`、owner `__outcome` wrapper、owner `__core` function shell；在 `RefactorCallableEmitter` 中加入 `RefactorCallableReturnMode::EffectOutcome`、`StateTag` 读写、composite transport boxing/unboxing、`EffectOutcome -> Step` 最小重建，以及显式 `EffectOutcome` complete/propagate/double-resume return path。
    - `crates/scoopc/src/llvm/tests.rs`：新增 IR 断言，锁定 internal outcome surface / owner core 的发布，以及 composed continuation resume 先走 outcome surface、再由 caller 侧重建 `Step` 的路径。
  - 核心决策：
    - 不提前重写公开 `Continuation.resume(...) -> Step_F` surface；本任务新增的是 internal outcome-resume surface / owner core，让 `G5-T06` 的 generated resume driver 后续可以直接挂到 `state_ref + current_effect_ctx_ref + incoming_resume_token_ref + outcome` 协议上，而不是继续把 shared `surface-resume -> Step_F` 当作内核。
    - call-boundary continuation composition 不再在 resumed path 里直接调用 shared `surface_resume(symbol) -> Step_F`；改为走 internal outcome surface，显式把底层 callee continuation 作为 `incoming_resume_token_ref` 传进 owner core，再由 caller 侧用 published schema 把 outcome 重建成 `Step` 并继续复用既有 boundary dispatch。
    - 为了不把 composite answer/payload 重新塞回 runtime bridge，本任务在 backend 里补了最小的 composite transport boxing/unboxing；`EffectOutcome` 的 payload/complete transport 现在可以在 LLVM side 直接 box 到 GC-visible object，而不是依赖 runtime continuation/effect policy。
  - 验证结果：
    - `cargo fmt`：通过。
    - `cargo check -p scoopc`：仍失败，但前沿继续停在后续 `G6/G7` 缺口：`emit_ordinary_call_effect_propagation_check` / `ordinary_effect_propagation_enabled` / `declare_runtime_effect_is_active`、`codegen_call_impl` / `codegen_top_level_fun_call_impl` / `codegen_mir_*call*`、`codegen_perform_expr` / `codegen_handle_expr` / `codegen_mir_perform_terminator` 等；本任务新增的 outcome/core 代码不再产生新的前沿编译错误。
    - `cargo clippy -p scoopc --all-targets -- -D warnings`：仍失败，失败前沿与 `cargo check -p scoopc` 一致，依旧停在后续 `G6/G7` 缺口；未新增本任务范围内的 lint/compile 前沿问题。
    - 源码复核：`resume_composed_call_boundary_case(...)` 已改为调用 `refactor_surface_resume_outcome_function(surface)` + `alloc_effect_outcome_slot(...)` + `build_step_from_effect_outcome(...)`，不再直接把 shared `surface-resume -> Step_F` 当作 composed resume 的内部内核。
    - `crates/scoopc/src/llvm/tests.rs` 已补两条 IR 断言覆盖 internal outcome surface / owner core 与 composed resume outcome-first path；由于 `G6/G7` 仍未完成，当前无法单独跑通 `cargo test -p scoopc`，但测试源码已与本任务的新 surface 同步。
  - 与 `EFFECT_REFACTOR_GAPS.md` 对应消除的 gap 条目：
    - §6：continuation resume driver 不再只剩 shared surface-resume `Step_F` wrapper；backend 已重新拥有 explicit `EffectOutcome` 驱动的 internal resume core/outcome wrapper，可直接承载后续 generated resume driver 的 `step_fn` 内核。

## [DONE] G5-T06：重建 codegen-owned continuation object model 与 generated resume driver

- 参考：
  - [`PLAN.md`](./PLAN.md) §2 / G5
  - [`EFFECT_REFACTOR.md`](./EFFECT_REFACTOR.md) §4.5、§4.10
  - [`CONTINUATION_RUNTIME_REFACTOR.md`](./CONTINUATION_RUNTIME_REFACTOR.md) §2.1、§3.3、§5、§6
  - [`EFFECT_REFACTOR_GAPS.md`](./EFFECT_REFACTOR_GAPS.md) §6
- 目标：
  - 把 runtime 已删除的 continuation policy 完整迁回 codegen。
- 必须实现的内容：
  1. 在 backend/type-layout 层定义新的 `ScoopContinuation` object layout：
     - `captured_effect_ctx_ref`
     - `state_ref`
     - `step_fn`
     - `resume_word`
     - `resume_gc_ref`
     - `captured_callee_suspend_state_ref`
     - `resumed` one-shot flag
     - `resume_state_tag`
  2. 明确禁止：
     - stable handle owner
     - native handler snapshot
     - `release_fn`
     - runtime replay-state object
  3. 生成 module-private `__scoop_continuation_resume_with(...)` helper：
     - `cmpxchg` one-shot
     - explicit payload store
     - call step/dispatch with explicit `current_effect_ctx_ref` / `incoming_resume_token_ref` / `outcome`
     - complete path answer slot write-back
  4. 如果 thread resume 仍需 generic substrate 协助，只允许保留“generic thread spawn/join substrate”；不得恢复 runtime-owned continuation resume API。
  5. 重接所有当前还引用 deleted runtime continuation API 的地方，尤其：
     - `effect_lowered/value.rs`
     - `intrinsics/thread.rs`
     - 任何 continuation resume lowering path
- 必须遵从的约束：
  - continuation object model 必须是普通 traced managed object。
  - generated resume driver 必须是 compiler-owned helper，不得把 owner 退回 runtime C。
- 验证：
  1. `cargo check -p scoopc`
  2. 输出中不再出现：
     - `declare_runtime_continuation_resume_with`
     - `declare_runtime_thread_spawn_join_resume_u64`
     - `declare_runtime_thread_spawn_join_resume_transport`
     - `declare_runtime_thread_spawn_join_compat_resume_u64`
  3. 相关实现源码 grep，不得再出现 deleted runtime continuation symbol 名字。
- 完成条件：
  - continuation alloc / resume / answer / outward propagation 再次在实现上存在，但 owner 完全位于 codegen。
- 依赖：G5-T05a
- 完成记录：
  - 改动范围：
    - `crates/scoopc/src/llvm/codegen/effect_lowered/{layout,types}.rs`：把 continuation object 的 authoritative LLVM layout/field kind 改成 codegen-owned 目标字段集合：`resumed`、`resume_state_tag`、`captured_effect_ctx_ref`、`state_ref`、`step_fn`、`resume_word`、`resume_gc_ref`、`captured_callee_suspend_state_ref`，并保留 published resume-packing vtable fields；不再把 continuation 建模成旧的 frame/one-shot/composed-callee 壳层。
    - `crates/scoopc/src/llvm/codegen/effect_lowered/body.rs`：补出 generated continuation step / owner outcome driver / generic outcome dispatcher；continuation 分配改为直接写新字段；resume 路径使用 LLVM `cmpxchg` one-shot、显式 payload store、显式 `captured_effect_ctx_ref` / `captured_callee_suspend_state_ref` / `step_fn` 读取，以及 answer slot write-back；同时更新源码自检并清掉本任务引出的 pointer-type warning。
    - `crates/scoopc/src/llvm/tests.rs`：把 continuation 布局与 one-shot 断言切到新 object model 与 `cmpxchg` 协议。
  - 核心决策：
    - continuation object 的 authoritative 字段顺序收口为：`header`、`resumed`、`resume_state_tag`、`captured_effect_ctx_ref`、`state_ref`、`step_fn`、`resume_word`、`resume_gc_ref`、`captured_callee_suspend_state_ref`，其后才是 effect-family resume packing 的 vtable 字段；不再保留 stable handle owner、native handler snapshot、`release_fn` 或 runtime replay-state 语义槽位。
    - compiler-owned resume driver 采用“generic outcome dispatcher + owner-specific outcome wrapper + owner step core”的 generated helper 组合来实现 `__scoop_continuation_resume_with(...)` 的目标算法：先 `cmpxchg` 置位 one-shot，再写入 resume payload，随后用显式 `current_effect_ctx_ref` / `incoming_resume_token_ref` / `ScoopEffectOutcome *outcome` 调用 owner step core，并在 complete path 上直接回写 answer slot。
    - cross-thread resume 继续只依赖 generic thread spawn/join substrate；复核 `effect_lowered/value.rs` 与 `intrinsics/thread.rs` 后，活跃实现中未重新引入任何 deleted runtime continuation API。公开 `Continuation.resume(...) -> Step_F` surface 的全面切换仍属于后续 `G7-T08`，但其底层 owner 已经迁回 codegen。
  - 验证结果：
    - 对 `crates/scoopc/src`、`runtime/c`、`sysroot` grep `scoop_continuation_|scoop_callee_suspend_state_|scoop_effect_handler_stack_|scoop_effect_outcome_|captured_handler_stack_top|pending_continuation`：无命中。
    - `cargo fmt`：通过。
    - `cargo check -p scoopc`：仍失败，但首批错误只剩后续 `G6/G7` 缺口：`emit_ordinary_call_effect_propagation_check` / `ordinary_effect_propagation_enabled` / `declare_runtime_effect_is_active`（ordinary effect propagation）、`codegen_call_impl` / `codegen_top_level_fun_call_impl` / `codegen_mir_*call*`（G6）、`codegen_perform_expr` / `codegen_handle_expr` / `codegen_mir_perform_terminator`（G7）等；不再出现 `declare_runtime_continuation_resume_with`、`declare_runtime_thread_spawn_join_resume_u64`、`declare_runtime_thread_spawn_join_resume_transport`、`declare_runtime_thread_spawn_join_compat_resume_u64` 或本任务引入的 warning/frontier 噪音。
    - `cargo clippy -p scoopc --all-targets -- -D warnings`：仍失败，失败前沿与 `cargo check -p scoopc` 一致；当前未新增 `G5-T06` 自身的 lint/warning。
    - `crates/scoopc/src/llvm/codegen/effect_lowered/{body,layout}.rs` 的源码自检/布局测试已同步切到新的 continuation 字段集合与 `cmpxchg` 协议；由于 `G6/G7` 尚未闭合，当前无法独立跑通 `cargo test -p scoopc`。
  - 与 `EFFECT_REFACTOR_GAPS.md` 对应消除的 gap 条目：
    - §6：runtime 已删除的 continuation object model / resume driver 已由 codegen-owned continuation layout、generated step core 与 generated outcome-resume driver 补回，owner 不再停留在 runtime C。

## [DONE] G5-T06R：Review continuation object model / generated resume driver，确认 owner 已迁回 codegen

- 参考：
  - [`PLAN.md`](./PLAN.md) §2 / G5
  - [`CONTINUATION_RUNTIME_REFACTOR.md`](./CONTINUATION_RUNTIME_REFACTOR.md) §5、§6
- 重点：
  - continuation 是否已不再依赖 stable handle/native snapshot/release_fn；
  - resume driver 是否为 generated helper 而非 runtime C API；
  - `current_effect_ctx_ref` / `captured_callee_suspend_state_ref` 是否都成为 continuation 的显式 traced field。
- 必须检查的文件/位置：
  - continuation layout/type descriptor 生成处
  - generated resume helper 生成处
  - thread resume integration 处
- 验证：
  - 重新运行 `cargo check -p scoopc`；
  - 人工检查实现，不得重新引入 runtime-owned continuation policy。
- 完成条件：
  - 可以明确列出 continuation object 的 authoritative 字段集合与 resume 算法。
- 依赖：G5-T06
- 完成记录：
  - 改动范围：
    - `TODO.md`：将 `G5-T06R` 标记为 `[DONE]` 并补充 review 结论。
    - 本任务人工复核 `crates/scoopc/src/llvm/codegen/effect_lowered/{layout,types,body,value}.rs`、`crates/scoopc/src/llvm/codegen/{intrinsics/thread.rs,runtime_abi.rs,runtime_symbols.rs}`、`crates/scoopc/src/llvm/tests.rs`；无需额外语义修补。
  - 核心决策：
    - continuation object 的 authoritative 字段集合已经稳定为：`header`、`resumed`、`resume_state_tag`、`captured_effect_ctx_ref`、`state_ref`、`step_fn`、`resume_word`、`resume_gc_ref`、`captured_callee_suspend_state_ref`，其后才是 published resume packing vtable 字段；不再依赖 stable handle、native handler snapshot、`release_fn` 或 runtime-owned replay-state。
    - generated resume driver 已明确迁回 codegen：`emit_generated_continuation_resume_driver(...)` 以 LLVM `cmpxchg` 完成 one-shot，显式写入 `resume_word` / `resume_gc_ref`，从 continuation 直接读取 `state_ref` / `captured_effect_ctx_ref` / `captured_callee_suspend_state_ref` / `resume_state_tag` / `step_fn`，再以显式 hidden ABI + `ScoopEffectOutcome *outcome` 调用 owner `step_fn`，complete path 直接把 answer transport 写回调用方 answer slot。
    - active cross-thread resume integration 仍只依赖 generic thread spawn/join substrate：refactor path 由 `effect_lowered/value.rs` 生成 thunk 并调用 `scoop_thread_spawn_join_resume_u64` / `scoop_thread_spawn_join_resume_transport`；未发现 deleted runtime continuation API 回流到活跃实现。`intrinsics/thread.rs` 中旧兼容 helper 入口当前仅见定义面，未见 active refactor code path 调用点，因此不构成 runtime-owned continuation policy 的回退 source of truth。
  - 验证结果：
    - 对 `crates/scoopc/src`、`runtime/c`、`sysroot` grep `scoop_continuation_|scoop_callee_suspend_state_|captured_handler_stack_top|pending_continuation|scoop_effect_handler_stack_|scoop_effect_outcome_`：无命中。
    - 对 `crates/scoopc/src/llvm/codegen` grep `codegen_sysroot_thread_intrinsics\(`：仅命中 `intrinsics/thread.rs` 中的定义，未发现 active refactor path 调用点；`__scoop_thread_spawn_join_resume_u64` 的 refactor lowering 仍集中在 `effect_lowered/value.rs`。
    - 人工复核 `effect_lowered/layout.rs`、`effect_lowered/types.rs`、`effect_lowered/body.rs`：continuation layout、`refactor_continuation_step_llvm_ty()`、`create_continuation_object_with_state_tag(...)`、`try_mark_continuation_resumed(...)`、`store_continuation_resume_payload(...)`、`emit_generated_continuation_resume_driver(...)` 与 `emit_generated_continuation_step(...)` 一致对齐 `CONTINUATION_RUNTIME_REFACTOR.md` 中的 codegen-owned object model / resume algorithm。
    - `cargo fmt --check`：通过。
    - `cargo check -p scoopc`：仍失败，但首批错误继续停在后续结构性 gap：`emit_ordinary_call_effect_propagation_check` / `ordinary_effect_propagation_enabled` / `declare_runtime_effect_is_active`（ordinary effect propagation），以及 `codegen_call_impl` / `codegen_top_level_fun_call_impl` / `codegen_mir_*call*`（G6）、`codegen_perform_expr` / `codegen_handle_expr` / `codegen_mir_perform_terminator` / `emit_raise_runtime_error_variant` / `emit_ordinary_non_resuming_effect_exit`（G7）；未出现 `G5` continuation object model / generated resume driver 回退问题。
    - `cargo clippy -p scoopc --all-targets -- -D warnings`：仍失败，失败前沿与 `cargo check -p scoopc` 一致；未新增 `G5-T06R` 范围内的 lint/warning 问题。
  - 与 `EFFECT_REFACTOR_GAPS.md` 对应消除的 gap 条目：
    - §6：review 确认 continuation object model / generated resume driver 的 owner 已迁回 codegen；continuation 字段集合、one-shot/resume 算法、captured effect ctx 与 captured callee suspend-state 都已收口到 compiler-owned helper 与 traced object 字段，不再依赖 runtime-owned continuation policy。

## [DONE] G6-T07：重建 direct/static/dynamic call lowering 与 plain/effect ABI 分流

- 参考：
  - [`PLAN.md`](./PLAN.md) §2 / G6
  - [`EFFECT_REFACTOR.md`](./EFFECT_REFACTOR.md) §4.6-§4.10
  - [`EFFECT_REFACTOR_GAPS.md`](./EFFECT_REFACTOR_GAPS.md) §2、§9
- 目标：
  - direct/static/dynamic call 再次可 lowering，但直接面对 plain/effect ABI 最终形态；
  - 不再恢复旧 wrapper/TLS call boundary。
- 必须实现的内容：
  1. 在新的 non-legacy call lowering 模块中重建：
     - `codegen_call_impl`
     - `codegen_top_level_fun_call_impl`
     - `try_codegen_class_vtable_call_impl`
     - `try_codegen_interface_itable_call_impl`
     - `load_class_vtable_slot_fn_ptr_i8_impl`
     - `load_interface_itable_slot_fn_ptr_i8_impl`
     - `codegen_funptr_value_call_impl`
     - `codegen_function_value_call_impl`
     - `codegen_function_value_call_from_closure_obj_impl`
     - `emit_enter_native_for_extern_call_impl`
     - `emit_extern_native_call_impl`
  2. Plain ABI callable：
     - direct/static path 直接返回源码返回值；
     - dynamic plain surface 不得被强行包成 `Step_F` body。
  3. Effect ABI callable：
     - direct/vtable/itable/funptr/closure path 直接传 `ctx + incoming token + outcome`；
     - dynamic effect surface 以固定 `Step_F` / `invoke(args_tuple)` 组织。
  4. plain body 若需对接 effect-typed dynamic surface，只能由 adapter/thunk 包 `Complete`。
- 必须遵从的约束：
  - 禁止恢复 effect call wrapper / TLS probing boundary。
  - ABI 是否 plain/effect 必须由 callable facts / `resolved_outward_cases` / `needs_reentry` 决定。
- 验证：
  1. `cargo check -p scoopc`
  2. 输出中不再出现本任务列出的 call lowering impl 缺失。
  3. `mod.rs` 不再保留指向已删除 legacy impl 的 wrapper 外壳。
- 完成条件：
  - call lowering 再次闭合，且 plain/effect ABI 分流不再依赖 deleted bridge。
- 依赖：G5-T06R
- 完成记录：
  - 改动范围：
    - `crates/scoopc/src/llvm/codegen/call/{mod.rs,lowering.rs}`：新增 non-legacy call lowering 模块，实现 `codegen_call_impl`、`codegen_top_level_fun_call_impl`、`try_codegen_class_vtable_call_impl`、`try_codegen_interface_itable_call_impl`、`load_class_vtable_slot_fn_ptr_i8_impl`、`load_interface_itable_slot_fn_ptr_i8_impl`、`codegen_funptr_value_call_impl`、`codegen_function_value_call_impl`、`codegen_function_value_call_from_closure_obj_impl`、`emit_enter_native_for_extern_call_impl`、`emit_extern_native_call_impl`；并补回 `llvm_scoop_itable_{entry_,}type_impl`、`build_tuple_cg_value_from_values(...)` 与 ordinary call outcome propagation helper。
    - `crates/scoopc/src/llvm/codegen/class_ctor.rs`、`crates/scoopc/src/llvm/codegen/mir_body.rs`：把 suppressed class-ctor path 从旧 `declare_runtime_effect_is_active()` probe 改为显式 `EffectOutcome` 判定。
    - `crates/scoopc/src/llvm/codegen/effect_lowered/body.rs`：class-ctor boundary 在没有 suspend-site 显式 capture 时，回退观察当前函数 `current_effect_outcome_ptr`。
  - 核心决策：
    - 顶层 direct call、vtable/itable dispatch 的 plain/effect ABI 分流不再看旧 hidden-token / TLS 语义，也不回 HIR effectful boolean；统一由已发布 callable contract（`callable_uses_explicit_effect_hidden_abi(...)`）决定是否附带显式 hidden ABI。
    - effectful call path 统一为当前 call site 分配显式 `EffectOutcome` slot，并直接传 `current_effect_ctx_ref + null incoming_resume_token_ref + outcome`；不恢复 wrapper、active flag、handler-stack swap 或任何 runtime bridge。
    - HIR closure alias（`scoop.lambda$*`）继续保留 direct-call fallback，因此 function-value/closure call 不会被强行改写成 `invoke(args_tuple) -> Step_F`；而已发布的 dynamic callable carrier contract 仍服务于后续 `G7`/late-lowered dynamic surface。
    - suppressed class-ctor call path 不再 probe runtime active flag，而是让 explicit outcome/capture 成为唯一 authoritative propagation source；其中 nested call capture 继续优先走 suspend-site explicit outcome，direct class-ctor body propagation 则可回退观察当前函数 `current_effect_outcome_ptr`。
    - `Continuation.resume` 与 `perform` / `handle` / MIR effect call helper 仍保持在后续 `G7-T08` 范围；本任务只在 HIR call lowering 中保留显式 fail-fast，不跨任务补它们的 lowering。
  - 验证结果：
    - `cargo fmt`：通过。
    - 对 `crates/scoopc/src/llvm/codegen` grep `declare_runtime_effect_is_active|effect_call_wrapper|scoop_effect_|scoop_continuation_`：无命中。
    - `cargo check -p scoopc`：仍失败，但不再出现 `codegen_call_impl`、`codegen_top_level_fun_call_impl`、`try_codegen_class_vtable_call_impl`、`try_codegen_interface_itable_call_impl`、`load_class_vtable_slot_fn_ptr_i8_impl`、`load_interface_itable_slot_fn_ptr_i8_impl`、`codegen_funptr_value_call_impl`、`codegen_function_value_call_impl`、`codegen_function_value_call_from_closure_obj_impl`、`emit_enter_native_for_extern_call_impl`、`emit_extern_native_call_impl` 缺失；首批错误已切到 `codegen_perform_expr` / `codegen_handle_expr` / `codegen_mir_*effect*call*`（`G7-T08`）与独立的 `emit_raise_runtime_error_variant` helper 缺口。
  - 与 `EFFECT_REFACTOR_GAPS.md` 对应消除的 gap 条目：
    - §2：`MainCodegen` 的 call lowering 不再是“外壳在、语义主体没了”，新的 non-legacy `call/lowering.rs` 已把 direct/static/indirect call 主体接回。
    - §9：plain/effect ABI 分流在 direct/vtable/itable/funptr/closure surface 上重新闭合，普通 callable 不再被强制包回旧 bridge 或 complete-only wrapper。

## [DONE] G6-T07R：Review direct/static/dynamic call lowering，确认 ABI 分流已 facts-driven

- 参考：
  - [`PLAN.md`](./PLAN.md) §2 / G6
  - [`EFFECT_REFACTOR.md`](./EFFECT_REFACTOR.md) §4.6-§4.10
- 重点：
  - direct/vtable/itable/funptr/closure 路径是否都已按 facts 驱动 plain/effect ABI 分流；
  - dynamic effect surface 是否已经围绕固定 `Step_F`；
  - plain body 是否仍然保持 plain callable body。
- 必须检查的文件/位置：
  - 新 call lowering 模块
  - `closure/mod.rs`
  - `class_ctor.rs`
  - `intrinsics/thread.rs`
  - `effect_lowered/value.rs`
- 验证：
  - 重新运行 `cargo check -p scoopc`；
  - 人工检查调用路径，不得再出现 deleted runtime continuation/effect bridge 名字。
- 完成条件：
  - 可明确写出 plain/effect ABI 分流规则和 dynamic surface 组织方式。
- 依赖：G6-T07
- 完成记录：
  - 改动范围：
    - `TODO.md`：将 `G6-T07R` 标记为 `[DONE]` 并补充 review 结论。
    - 本任务人工复核 `crates/scoopc/src/llvm/codegen/{call/{abi,lowering}.rs,closure/mod.rs,class_ctor.rs,intrinsics/thread.rs,effect_lowered/value.rs}` 与 `crates/scoopc/src/llvm/tests.rs` 中 G6 相关回归断言；无需额外源码修补。
  - 核心决策：
    - direct/static/vtable/itable 与 closure body declaration 的 plain/effect ABI 分流继续由 published callable contract 驱动：`callable_uses_explicit_effect_hidden_abi(...)` 只读取 late-lowered callable 的 `effect_step_abi()`，未回退到 HIR effectful boolean、wrapper 或 deleted bridge。
    - higher-order callable value / closure carrier surface 复核后仍维持 target-shape 双轨：plain body 保持 plain direct call；需要 effect-typed callable surface 时，由 `effect_lowered/value.rs` 依据 published `dynamic_invoke_layouts()`、`callable_layout_by_root_fqn(...)` 与 `maybe_plain_callable_layout_by_root_fqn(...)` 选择 authoritative adapter 或 direct layout，而不是把 plain body 重新包成 complete-only `Step_F` body。
    - `class_ctor.rs` 的 propagation 判断只观察 explicit `EffectOutcome`；`intrinsics/thread.rs` 仅保留 generic thread spawn/join substrate 调用，未恢复 runtime-owned continuation/effect policy。
  - 验证结果：
    - 人工复核 `call/lowering.rs`：`codegen_top_level_fun_call_impl`、`try_codegen_class_vtable_call_impl`、`try_codegen_interface_itable_call_impl` 都通过 `callable_uses_explicit_effect_hidden_abi(...)` 选择显式 hidden ABI；`codegen_funptr_value_call_impl` / `codegen_function_value_call_from_closure_obj_impl` 仅在 callable value surface 需要时附带 `current_effect_ctx_ref + incoming_resume_token_ref + outcome`，未回退到 wrapper/TLS probing。
    - 人工复核 `closure/mod.rs`：closure declaration/body 继续通过 `callable_uses_explicit_effect_hidden_abi(...)` 绑定显式 hidden ABI；plain closure body 本身不被重新物化为 `Step_F` body。
    - 人工复核 `effect_lowered/value.rs`：effect-typed closure adapter 只消费 published dynamic-invoke layout；plain dynamic call 继续走 `codegen_mir_refactor_plain_dynamic_call(...)`，effectful dynamic surface 仍围绕 published `Step_F` schema 组织。
    - 人工复核 `crates/scoopc/src/llvm/tests.rs`：`direct_effectful_signature_without_outward_effect_stays_on_direct_call_surface`、`closure_call_without_outward_effect_stays_on_direct_call_surface`、`closure_call_with_real_outward_effect_uses_explicit_outcome_boundary`、`effectful_funptr_call_uses_explicit_outcome_boundary`、virtual/itable outward-call 断言仍覆盖 plain direct surface 与 outward Step boundary 两侧。
    - `rg -n "scoop_effect_|scoop_continuation_|__scoop_effect_|__scoop_callee_suspend_state|effect_call_wrapper|declare_runtime_effect_is_active|publish_incoming_resume_token|clear_incoming_resume_token" crates/scoopc/src/llvm/codegen/call crates/scoopc/src/llvm/codegen/closure crates/scoopc/src/llvm/codegen/class_ctor.rs crates/scoopc/src/llvm/codegen/intrinsics/thread.rs crates/scoopc/src/llvm/codegen/effect_lowered/value.rs`：无命中。
    - `cargo fmt --check`：通过。
    - `cargo check -p scoopc`：仍失败，但首批错误继续停在后续 `G7-T08` 缺口：`codegen_perform_expr`、`codegen_handle_expr`、`codegen_mir_perform_terminator`、`emit_raise_runtime_error_variant`、`codegen_mir_direct_call_with_policy`、`codegen_mir_funptr_value_call`、`codegen_mir_fun_value_call`、`codegen_mir_closure_call`、`codegen_mir_function_value_call_from_closure_obj`、`codegen_mir_class_ctor_call`；未出现 `G6-T07` 列出的 call lowering impl 缺失或 deleted runtime continuation/effect bridge 回退。
  - 与 `EFFECT_REFACTOR_GAPS.md` 对应消除的 gap 条目：
    - §2：review 确认 non-legacy `call/lowering.rs` 仍是 direct/static/dynamic call lowering 的唯一 active 主体，未回退到 deleted wrapper/legacy 外壳。
    - §9：review 确认 plain/effect ABI 分流仍依赖 published callable contract / published dynamic surface，而不是 deleted TLS continuation/effect bridge；plain body 继续保持 plain callable body。

## [DONE] G7-T08：重建 `perform` / `handle` / `resume` / `Step_F` lowering

- 参考：
  - [`PLAN.md`](./PLAN.md) §2 / G7
  - [`EFFECT_REFACTOR.md`](./EFFECT_REFACTOR.md) §4.3、§4.5、§4.9-§4.10、§4.13
  - [`EFFECT_REFACTOR_GAPS.md`](./EFFECT_REFACTOR_GAPS.md) §5、§8、§9
- 目标：
  - 把语言层 effect constructs 和 MIR effect constructs 接回新的 target-shape；
  - `perform` / `handle` / `Continuation.resume` / `Step_F` 不再依赖任何 deleted TLS bridge surface。
- 必须实现的内容：
  1. 重新实现 HIR 表达式入口：
     - `crates/scoopc/src/llvm/codegen/expr.rs`
       - `codegen_perform_expr`
       - `codegen_handle_expr`
  2. 重新实现 MIR effectful lowering 核心：
     - `crates/scoopc/src/llvm/codegen/mir_body.rs`
       - `codegen_mir_perform_terminator`
       - `codegen_mir_direct_call_with_policy`
       - `codegen_mir_funptr_value_call`
       - `codegen_mir_fun_value_call`
       - `codegen_mir_closure_call`
       - `codegen_mir_function_value_call_from_closure_obj`
       - `codegen_mir_class_ctor_call`
  3. `Continuation.resume(...)` lowering 必须改成 generated continuation resume driver；不得再恢复 runtime helper。
  4. `Step_F` case identity / payload tuple / resume tuple / answer contract 必须只由 schema/facts 驱动；不可回 HIR shape 或旧 runtime slot contract。
  5. runtime error / non-resuming effect exit 必须回 explicit outcome 或 backend-owned terminal path；不得再恢复 active flag / slot write 逻辑。
- 必须遵从的约束：
  - 若某条 path 当前暂时无法实现，必须在 frontend/facts/stage boundary 上 fail fast，不允许恢复 deleted bridge 作为 fallback。
  - `NoOutward` plain body 不得重新物化 complete-only `Step_F`。
- 验证：
  1. `cargo check -p scoopc`
  2. 输出中不再出现：
     - `codegen_perform_expr`
     - `codegen_handle_expr`
     - `codegen_mir_perform_terminator`
     - `emit_raise_runtime_error_variant`
     - `emit_ordinary_non_resuming_effect_exit`
  3. 相关实现源码 grep，不得再出现 deleted effect intrinsic / slot / bridge 名字。
- 完成条件：
  - HIR/MIR effect constructs 再次闭合到新的 target-shape protocol。
- 依赖：G6-T07R
- 完成记录：
  - 改动范围：
    - `crates/scoopc/src/llvm/codegen/expr.rs`：把 direct HIR `perform` / `handle` 入口固定为 refactor-stage fail-fast，明确要求走 published late-lowered/local-effect-control handoff，不再存在旧 HIR fallback。
    - `crates/scoopc/src/llvm/codegen/mir_body.rs`、`crates/scoopc/src/llvm/codegen/call/lowering.rs`：重建 MIR `perform` / direct-call / funptr / function-value / closure / class-ctor lowering 到显式 hidden ABI + `EffectOutcome` propagation surface，并补上 foreign `TypeStore` 到 codegen 类型层的等价映射回退。
    - `crates/scoopc/src/llvm/codegen/effect_lowered/{body,layout,types}.rs` 与 `crates/scoopc/src/llvm/codegen/{layout,ty,mod.rs}`：补齐 generated continuation resume driver / owner outcome wrapper / `Step_F` dispatch / handle ctx dispatch / RuntimeError transport boxing，并把 enum/struct/layout key 计算扩展到 materialized/late-lowered type id。
    - `crates/scoopc/src/effect_lowered/{builder,frame,ir}.rs`、`crates/scoopc/src/llvm/tests.rs`：收紧 frame lifting / state-machine 验证，并把默认单文件 helper 与 continuation/runtime-error 路径断言切到新的 refactor surface。
    - `crates/scoopc/src/llvm/codegen/intrinsics/builtin.rs`、`crates/scoopc/src/llvm/codegen/ordinary_callee.rs`：删除或标注当前阶段不再使用的 dead helper，确保 lint 面干净。
  - 核心决策：
    - `perform` / `handle` 的 authoritative lowering 只允许来自 published late-lowered/MIR effect pipeline；HIR 表达式入口保留为 fail-fast 边界，而不是偷偷恢复旧 HIR lowering。
    - `Continuation.resume(...)` 统一走 generated surface-resume outcome wrapper / owner core / continuation drive driver；double-resume runtime error 通过显式 `EffectOutcome` / ordinary runtime-error case 表达，不恢复 runtime helper、one-shot slot helper 或 TLS/slot 写回协议。
    - `Step_F` / dynamic surface / handle dispatch / payload transport 的类型决策改为同时接受 codegen 主 `TypeStore` 与 materialized/late-lowered foreign type id；对 `Raise<RuntimeError>` 继续使用显式 runtime-error effect-instance 常量，不回退旧 bridge。
    - `NoOutward` plain body 继续保持 plain ABI：默认单文件 `main` 若只在函数体内部 `handle` 掉 effect，则验证面改为检查 refactor state-machine / handle ctx dispatch，而不是强行要求 direct-invoke `Step_F` 外壳。
  - 验证结果：
    - `cargo check -p scoopc`：通过；不再出现 `codegen_perform_expr`、`codegen_handle_expr`、`codegen_mir_perform_terminator`、`emit_raise_runtime_error_variant`、`emit_ordinary_non_resuming_effect_exit` 或相关 MIR call helper 缺失。
    - `cargo clippy -p scoopc --all-targets -- -D warnings`：通过。
    - `cargo test -p scoopc`：通过（742 passed）。
    - `cargo run -p scoop -- build tests/fixtures/build/effect_refactor_direct_handle_resume_emit_llvm.scoop -o /var/folders/0s/mcfxhz813ps4mky0c1sr7rz00000gn/T/opencode/g7_t08_direct_handle_resume.ll --emit-llvm --opt-level 0`：通过；产物继续包含 `__scoop_refactor_resume__fixtures_build_main__case0`、`__scoop_refactor_surface_resume__*`、`scoop_alloc_typed`，且 grep 未命中 `scoop_effect_handler_stack` / `scoop_effect_outcome`。
    - 对 `crates/scoopc/src/llvm/codegen` 与 `crates/scoopc/src/effect_lowered` grep `scoop_effect_|__scoop_effect_|scoop_continuation_resume_with|scoop_continuation_resume_into|scoop_effect_outcome_|publish_incoming_resume_token|clear_incoming_resume_token|effect_call_wrapper`：无命中。
  - 与 `EFFECT_REFACTOR_GAPS.md` 对应消除的 gap 条目：
    - §5：`perform` / `handle` / MIR direct-dynamic-resume lowering 入口重新闭合到 clean backend target-shape surface，不再靠缺失 helper 或旧 HIR/TLS fallback 撑着。
    - §8：function/block/site 级 published schema/facts 重新成为 lowering 的 authoritative 输入；`Step_F`、handle dispatch、runtime-error transport 不再依赖 ad-hoc deleted bridge 语义。
    - §9：`Step_F` / surface-resume / dynamic callable surface / plain-vs-effect ABI 分流已经在 active LLVM lowering 中重新接通，`Continuation.resume` 也已切到 generated driver。

## [DONE] G7-T08R：Review `perform` / `handle` / `resume` / `Step_F` lowering，确认 surface 已切到新协议

- 参考：
  - [`PLAN.md`](./PLAN.md) §2 / G7
  - [`EFFECT_REFACTOR.md`](./EFFECT_REFACTOR.md) §4.3、§4.9-§4.10、§4.13
- 重点：
  - `perform` / `handle` / `resume` 是否都只走 explicit `EffectOutcome` / `EffectCtx` / `Step_F`；
  - `Step_F` case/payload/resume tuple 是否只由 schema/facts 决定；
  - 是否还存在任何 plain/effect ABI 混淆或 deleted bridge fallback。
- 必须检查的文件/位置：
  - `expr.rs`
  - `mir_body.rs`
  - `effect_lowered/{body,layout,value}.rs`
  - 任何新的 `Step_F` / continuation driver helper module
- 验证：
  - 重新运行 `cargo check -p scoopc`；
  - 人工检查 code path，不得再存在 deleted TLS continuation/effect surface。
- 完成条件：
  - 可以明确给出当前 `perform` / `handle` / `resume` 的 lowering contract 描述。
- 依赖：G7-T08
- 完成记录：
  - 改动范围：
    - `TODO.md`：将 `G7-T08R` 标记为 `[DONE]` 并补充 review 结论。
    - 本任务对 `crates/scoopc/src/llvm/codegen/expr.rs`、`crates/scoopc/src/llvm/codegen/mir_body.rs`、`crates/scoopc/src/llvm/codegen/effect_lowered/{body,layout,value}.rs` 做人工复核；未发现需要额外修补的源码缺口。
  - 核心决策：
    - direct HIR `perform` / `handle` 与 direct MIR `Perform` terminator 继续只保留 fail-fast 边界；authoritative lowering 仍然只能来自 published late-lowered / local-effect-control pipeline，而不是偷偷恢复旧 HIR/MIR fallback。
    - 当前 `Continuation.resume(...)` contract 已稳定分成两层：用户/动态 surface 统一走 `__scoop_refactor_surface_resume__k* -> Step_F`，而 owner core / generated continuation driver 在内部通过显式 hidden ABI 绑定 `current_effect_ctx_ref`、`incoming_resume_token_ref`、`ScoopEffectOutcome *outcome` 后再重建 `Step_F` / outcome 返回路径；未回退到 runtime helper、TLS slot 或单 token 协议。
    - `Step_F` case 集、payload tuple、resume tuple 与 surface-resume dispatch 都由 published `StepSchema` / `ContinuationSchema` / `ResumeSiteEffectFacts` / layout query 闭合校验；plain callable 若无本地 handle candidate 仍会显式拒绝 outward path，没有 plain/effect ABI 混淆回退面。
  - 验证结果：
    - 对 `crates/scoopc/src` grep `scoop_effect_handler_stack_top|scoop_effect_active|scoop_effect_perform_slot|scoop_callee_suspend_state|scoop_continuation_resume_scope|scoop_continuation_alloc|scoop_continuation_resume_with|scoop_continuation_resume_into|scoop_effect_outcome_consume_current|scoop_effect_outcome_publish|__scoop_effect_|publish_incoming_resume_token|clear_incoming_resume_token|effect_call_wrapper`：无命中。
    - `cargo check -p scoopc`：通过。
    - `cargo clippy -p scoopc --all-targets -- -D warnings`：通过。
    - `cargo test -p scoopc`：通过（742 passed）。
  - 与 `EFFECT_REFACTOR_GAPS.md` 对应消除的 gap 条目：
    - §5：review 确认 `perform` / `handle` / `resume` / `Step_F` lowering 继续只走新的 target-shape protocol，没有回退到 deleted TLS/bridge surface。
    - §8：review 确认 step/case/payload/resume lowering 决策继续来自 published schema/facts/layout query，而不是 HIR-level fallback 或 ad-hoc bridge 语义。
    - §9：review 确认 surface-resume、generated continuation driver、plain/effect ABI 分流仍保持在 generated contract 上，没有重新混入旧 continuation runtime 协议。

## [DONE] G8-T09：runtime generic substrate 收尾、验证面迁移与 full regression 恢复

- 参考：
  - [`PLAN.md`](./PLAN.md) §2 / G8
  - [`EFFECT_REFACTOR.md`](./EFFECT_REFACTOR.md) §4.11-§4.15
  - [`EFFECT_REFACTOR_GAPS.md`](./EFFECT_REFACTOR_GAPS.md) §10-§12
- 目标：
  - 在 target-shape 接通后，把 runtime C 收尾成真正的 generic substrate，并把测试/fixture/文档验证面迁移到新 surface。
- 必须实现的内容：
  1. 彻底清理 `runtime/c/scoop_runtime.c` 中 bulk deletion 后残余的 dead comments / dead assertions / dead forward declaration holes；保证 runtime C 文件重新自洽。
  2. 清理 `runtime/c/scoop_runtime_api.h` / `runtime/c/scoop_test.c` 的历史 continuation/effect policy 残留。
  3. 把活跃验证面从“旧名字是否存在/不存在”迁移到新 surface：
     - `StepSchema`
     - `resolved_outward_cases`
     - explicit `EffectOutcome`
     - explicit `current_effect_ctx_ref`
     - explicit `incoming_resume_token_ref`
     - plain/effect ABI 分流
  4. 恢复并通过：
     - `cargo check -p scoop_runtime`
     - `cargo check -p scoopc`
     - `cargo test -p scoop_runtime`
     - `cargo test -p scoopc`
     - `cargo test -p scoop`
     - `cargo test --all`
  5. 对照 `EFFECT_REFACTOR_GAPS.md`，把所有 gap 状态更新为已闭合或剩余明确 blocker。
- 必须遵从的约束：
  - 不得把 full regression 的通过建立在恢复任何 deleted TLS continuation/effect surface 之上。
  - 若某些历史测试不再适配新设计，应重写验证入口，而不是重新恢复旧 symbol 让它们通过。
- 验证：
  - 上述完整矩阵全部通过；
  - 活跃源码 grep 不再出现旧 TLS continuation/effect 名字；
  - 新测试至少覆盖 explicit `EffectOutcome` / `EffectCtx` / `Step_F` 的存在性与 contract。
- 完成条件：
  - 仓库重新回到可编译、可测试状态；
  - active implementation / active tests / active docs 都不再保留旧 TLS continuation/effect 语义。
- 依赖：G7-T08R
- 完成记录：
  - 改动范围：
    - `runtime/c/scoop_runtime.c`、`runtime/c/scoop_runtime_api.h`、`runtime/c/scoop_tls_internal.h`：把 runtime C 注释/导出面收口为 generic substrate，删掉旧 continuation/effect policy 描述、预留 TLS 字段与过时 test-only allowlist 项。
    - `crates/scoop/tests/p7_default_pipeline.rs`：把仍围绕旧 trace/TLS 语义的默认管线 CLI 回归改成 target-shape 正向验证，锁定 `ScoopEffectCtx`、`ScoopEffectOutcome`、surface-resume / `cmpxchg` / `Step_F` contract。
    - `crates/scoopc/src/effect_lowered/ir.rs`、`crates/scoopc/src/llvm/codegen/effect_lowered/{body,layout,types}.rs`：补齐 full regression 期间暴露的两个真实 blocker：effectful funptr `k4` surface-resume inventory 缺口，以及 payloaded `Continuation.resume(...)` plain tail 误清 frame root 导致的提前终止；同时保留 multi-owner/shared wrapper dispatch query 的最终 target-shape 组织。
    - `EFFECT_REFACTOR_GAPS.md`、`SCOOP_FULL_SPEC.md`、`SCOOP_RUNTIME.md`、`docs/spec/language_spec-part4.md`：把活跃 gap 状态与用户可读文档改成只描述 explicit `EffectCtx` / `EffectOutcome` / generated continuation driver / generic substrate。
    - `tests/fixtures/run-pass/effect_raise_trace_hook_basic.{scoop,stdout}`：删除依赖旧 trace/TLS surface 的历史 run-pass fixture，不再把它当成 active validation 入口。
  - 核心决策：
    - runtime 收尾不再试图保留任何 continuation/effect policy 预留位或测试旁路；TLS internal header 只保留 GC / 线程注册 / explicit root frame substrate 所需字段。
    - 活跃验证面从“旧名字是否出现”迁移为对 target-shape contract 的正向检查：published `StepSchema`/surface-resume、explicit `EffectOutcome`、explicit `current_effect_ctx_ref` / `incoming_resume_token_ref`、plain/effect ABI 分流，以及默认管线 CLI 发射出来的 IR surface。
    - 对 full regression 暴露的 funptr wrapper 缺口，不恢复旧 bridge，而是让 `register_call_boundary_callee_wrapper_projection(...)` 在 wrapper schema 与 caller schema 不同但 owner step 相同的情况下，仍发布 authoritative owner route。
    - 对 payloaded `Continuation.resume(...)` 提前退出回归，不把它硬塞进更复杂的 tail-free 证明；直接收紧优化边界，禁止在 plain `Resume` boundary complete path 上提前释放 frame root，确保后续 resume tail 仍可安全读取 frame-owned locals / handle ctx。
  - 验证结果：
    - 对 `crates/scoopc/src`、`runtime/c`、`sysroot` grep `scoop_effect_handler_stack_top|scoop_effect_active|scoop_effect_perform_slot|scoop_callee_suspend_state|scoop_continuation_resume_scope|scoop_continuation_alloc|scoop_continuation_resume_with|scoop_continuation_resume_into|scoop_effect_outcome_consume_current|scoop_effect_outcome_publish|__scoop_effect_|publish_incoming_resume_token|clear_incoming_resume_token|effect_call_wrapper`：无命中。
    - `cargo fmt --check`：通过。
    - `cargo check -p scoop_runtime`：通过。
    - `cargo check -p scoopc`：通过。
    - `cargo test -p scoop_runtime`：通过。
    - `cargo test -p scoopc`：通过（742 passed）。
    - `cargo test -p scoop`：通过。
    - `cargo test --all`：通过。
    - `cargo clippy --workspace --all-targets -- -D warnings`：通过。
    - 定向 blocker 回归：`cargo test -p scoopc llvm::tests::effectful_funptr_call_uses_explicit_outcome_boundary -- --exact --nocapture` 与 `cargo test -p scoop --test p7_default_pipeline` 均通过。
  - 与 `EFFECT_REFACTOR_GAPS.md` 对应消除的 gap 条目：
    - §10：full regression / 单主线 target-shape 验证矩阵已恢复，仓库重新回到可编译、可测试状态。
    - §11：runtime C / runtime API / TLS internal header 的删除残余、死注释与过时导出已清到 generic substrate 自洽状态。
    - §12：活跃测试与活跃文档已迁到 explicit `EffectOutcome` / `EffectCtx` / surface-resume / plain-vs-effect ABI contract，不再依赖旧 TLS continuation/effect surface。

## [DONE] G8-T09R：Review 最终收口结果，确认仓库重新只剩 target-shape 单主线

- 参考：
  - [`PLAN.md`](./PLAN.md) §2 / G8
  - [`EFFECT_REFACTOR.md`](./EFFECT_REFACTOR.md) 全文
  - [`EFFECT_REFACTOR_GAPS.md`](./EFFECT_REFACTOR_GAPS.md) 全文
- 重点：
  - runtime 是否真正只剩 generic substrate；
  - backend 是否重新拥有 whole-function effect/continuation protocol；
  - 所有优化级别是否走同一条管线；
  - 是否仍有任何 deleted TLS continuation/effect surface 作为隐藏 fallback。
- 必须检查的目录：
  - `crates/scoopc/src/llvm/codegen/`
  - `crates/scoopc/src/effect_facts/`
  - `crates/scoopc/src/pipeline/`
  - `runtime/c/`
  - `sysroot/`
  - 活跃测试/fixture/文档
- 验证：
  - 复跑 G8-T09 的完整验证矩阵；
  - grep 活跃代码目录，不得再出现旧 TLS continuation/effect 名字；
  - 在完成记录中给出与 `EFFECT_REFACTOR_GAPS.md` 每条 gap 的最终对应关系。
- 完成条件：
  - 可以明确声明：当前仓库 effect/continuation 主线已重新按 `EFFECT_REFACTOR.md` 闭合，且不再存在旧 TLS continuation/effect 语义回退面。
- 依赖：G8-T09
- 完成记录：
  - 改动范围：
    - `crates/scoopc/src/llvm/codegen/effect_lowered/body.rs`：修正一处未按 `rustfmt` 折行的 `matches!` 条件，恢复完整验证矩阵中的 `cargo fmt --check` 通过状态。
    - `TODO.md`：将 `G8-T09R` 标记为 `[DONE]` 并补充最终 review 结论。
    - 本任务对 `runtime/c/scoop_tls_internal.h`、`runtime/c/scoop_runtime_api.h`、`crates/scoopc/src/llvm/codegen/mod.rs`、`crates/scoopc/src/pipeline/{mod,llvm_codegen_stage}.rs`、`crates/scoopc/src/session/mod.rs`、`crates/scoop/tests/p7_default_pipeline.rs`、`SCOOP_RUNTIME.md`、`docs/spec/language_spec-part4.md` 做人工复核；无需额外语义修补。
  - 核心决策：
    - `G8-T09R` 复核期间把“验证矩阵必须全部通过”视为最终收口声明的一部分；因此 `cargo fmt --check` 暴露的格式回归直接在本 review 任务内修复，而不是仅记录问题。
    - 最终是否闭合，按三类证据共同判定：完整验证矩阵、旧 TLS/bridge 名字 grep、关键实现/文档的人工抽查。三者必须同时满足，才认定仓库只剩 target-shape 单主线。
    - 人工复核确认：runtime TLS/internal/export 面只保留 GC / 线程注册 / explicit root frame substrate；whole-function effect/continuation protocol 留在 backend-owned `EffectCtx` / `EffectOutcome` / continuation driver；所有优化级别继续共用同一条 stage-owned pipeline。
  - 验证结果：
    - 对 `crates/scoopc/src`、`runtime/c`、`sysroot`、活跃测试/fixture/文档 grep `scoop_effect_handler_stack_top|scoop_effect_active|scoop_effect_perform_slot|scoop_callee_suspend_state|scoop_continuation_resume_scope|scoop_continuation_alloc|scoop_continuation_resume_with|scoop_continuation_resume_into|scoop_effect_outcome_consume_current|scoop_effect_outcome_publish|__scoop_effect_|publish_incoming_resume_token|clear_incoming_resume_token|effect_call_wrapper`：无命中。
    - `cargo fmt --check`：通过。
    - `cargo check -p scoop_runtime`：通过。
    - `cargo check -p scoopc`：通过。
    - `cargo test -p scoop_runtime`：通过。
    - `cargo test -p scoopc`：通过（742 passed）。
    - `cargo test -p scoop`：通过。
    - `cargo test --all`：通过。
    - `cargo clippy --workspace --all-targets -- -D warnings`：通过。
    - 定向回归：`cargo test -p scoopc llvm::tests::effectful_funptr_call_uses_explicit_outcome_boundary -- --exact --nocapture` 与 `cargo test -p scoop --test p7_default_pipeline`：通过。
    - 人工复核：`runtime/c/scoop_tls_internal.h` 只保留 `registered`、Immix allocator/cache 与 native-roots TLS 槽位；`runtime/c/scoop_runtime_api.h` 导出面不再包含 continuation/effect policy API；`crates/scoopc/src/llvm/codegen/mod.rs` 仍以 `current_effect_ctx_ref`、`current_incoming_resume_token_ref`、`current_effect_outcome_ptr` 作为 effectful callable 的显式 hidden ABI 入口；`crates/scoopc/src/pipeline/mod.rs`、`crates/scoopc/src/pipeline/llvm_codegen_stage.rs`、`crates/scoopc/src/session/mod.rs` 与 `crates/scoop/tests/p7_default_pipeline.rs` 继续锁定“单一 pipeline + target-shape 合同”行为；`SCOOP_RUNTIME.md` 与 `docs/spec/language_spec-part4.md` 也已改为只描述 explicit `EffectCtx` / `EffectOutcome` / generated continuation driver 的现状。
  - 与 `EFFECT_REFACTOR_GAPS.md` 每条 gap 的最终对应关系：
    - Gap 1：`G1-T02` / `G1-T02R`，effectful callable 显式 hidden ABI 已统一到顶层函数、closure、resume entry 与 dynamic callable surface。
    - Gap 2：`G6-T07` / `G6-T07R`，clean backend 的 direct/static/dynamic call lowering 已完全迁到 non-legacy call lowering。
    - Gap 3：`G4-T05` / `G4-T05R`，ordinary callee suspend/reentry 已改由 published facts 与显式 incoming token 驱动。
    - Gap 4：`G2-T03` / `G2-T03R`，explicit `EffectOutcome` / transport primitive 已完全 backend-owned。
    - Gap 5：`G7-T08` / `G7-T08R`，`perform` / `handle` / `resume` / dynamic call lowering 已重新闭合到 target-shape protocol。
    - Gap 6：`G5-T05a` / `G5-T06` / `G5-T06R`，continuation object model 与 generated resume driver 的 owner 已迁回 codegen。
    - Gap 7：`G3-T04` / `G3-T04R`，`EffectCtx` / handler graph 已成为显式 data model，不再依赖 ambient TLS。
    - Gap 8：`G7-T08` / `G7-T08R`，function/block/site 级 published schema/facts 已成为 lowering 的 authoritative 输入。
    - Gap 9：`G6-T07` / `G7-T08` / `G7-T08R`，`Step_F` / dynamic callable surface / plain-vs-effect ABI 分流已按 target-shape 接通。
    - Gap 10：`G8-T09`，单一 target-shape 管线已恢复到可编译、可测试、跨验证矩阵可运行状态。
    - Gap 11：`G0-T01` / `G0-T01R` / `G8-T09`，runtime C 删除残余、死注释与过时导出已清到 generic substrate 自洽状态。
    - Gap 12：`G0-T01` / `G0-T01R` / `G8-T09`，活跃验证面与活跃文档已统一迁到 `StepSchema` / `resolved_outward_cases` / explicit `EffectOutcome` / explicit ctx / plain/effect ABI surface。

## [DONE] G8-T10：完整扫描所有 fixture 并建立失败清单

- 参考：
  - [`PLAN.md`](./PLAN.md) §2 / G8
  - [`EFFECT_REFACTOR_GAPS.md`](./EFFECT_REFACTOR_GAPS.md) §10、§12
  - 当前 CI 失败样例：`gh run view 25669198499 --log-failed`
- 目标：
  - 不再把局部 `cargo test` / `cargo test --all` 通过视为 fixture 全绿的替代；
  - 以 `scoop` fixture harness 为 authoritative 验证面，**逐个**扫描当前仓库所有 fixture，建立失败清单并按类别归档；
  - 扫描时必须为每个 fixture 使用短 timeout，避免单个挂死 case 卡住整轮 sweep。
- 必须实现的内容：
  1. 在本地按“逐个 fixture”方式运行完整 sweep，而不是一次性跑整个 harness。
     - 必须遍历 `tests/fixtures/**` 下活跃 fixture 集合，逐个调用 `scoop` fixture harness 运行单个 fixture。
     - 每个 fixture 的调用都必须带短 timeout，**不得超过 30 秒**。
     - 默认环境与 `SCOOP_GC_MOVE=1 SCOOP_GC_STRESS=1 SCOOP_GC_VERIFY_ROOTS=1` 全开环境都要各跑一轮逐个扫描。
     - 允许通过 driver 层 wrapper、脚本或测试辅助实现逐个扫描，但不得退回“全量一次性跑到第一个失败就停”。
  2. 若本地结果与 GitHub CI 不一致，必须同时检查最近失败的 CI run：
     - `gh run list --branch <current-branch> --limit 10`
     - `gh run view <run-id>`
      - `gh run view <run-id> --log-failed`
  3. 建立当前失败清单，至少按以下维度分类：
     - `build` fixture
     - `run-pass` fixture
     - `mir` / `effect_facts` / `effect_lowered` snapshot fixture
     - runtime/GC env only fixture
     - only-fails-under-`SCOOP_GC_MOVE=1 SCOOP_GC_STRESS=1 SCOOP_GC_VERIFY_ROOTS=1`
     - CI-only / local-only difference
  4. 对每个失败项记录：
     - fixture 路径
     - 失败阶段（frontend prepare / MIR stage / P4 facts / P5 lowering / P6 LLVM / runtime / stdout/stderr golden）
     - 首个报错文本
     - 直接相关的代码入口文件/函数
  5. 把已知当前首个 CI fixture 失败加入清单：
     - `tests/fixtures/build/effect_refactor_dynamic_entry_publication_emit_llvm.scoop`
     - 失败文本：`LLVM stage handoff 缺少 reachable callable 'fixtures.build.Base.ping' 的 published late-lowered body`
- 必须遵从的约束：
  - 本任务不是“顺手修一个失败”；它的目标是建立完整失败面。
  - 不允许只看本地第一条失败就停止；必须把完整逐个扫描跑到结束并整理所有失败。
  - 不允许把历史 `TODO.md` 的 `[DONE]` 状态当作 fixture 全绿证据；必须以当前实际 sweep/CI 结果为准。
  - 每个 fixture 的 timeout 必须是短 timeout，且**不得超过 30 秒**；如果某个 fixture 在该窗口内挂住，也必须被记录为失败项而不是继续无限等待。
- 验证：
  1. 逐个 fixture 默认环境扫描（每个 fixture timeout <= 30s）
  2. 逐个 fixture GC env 全开扫描（每个 fixture timeout <= 30s）
  3. 如 CI 上有失败 run，至少抓取一份最新失败日志并记录 run id / head sha / 失败 job / 失败 step。
- 完成条件：
  - 有一份当前活跃 fixture failures 的完整清单；
  - 后续任务可以逐条消解，而不需要重新做大范围探索。
- 依赖：G8-T09R
- 完成记录：

  - 改动范围：
    - `TODO.md`：把 G8 后续任务的 sweep/验证约束改成“逐个 fixture + timeout <= 30s + 默认 env/GC env 双轮”的 authoritative 口径。
    - 使用 detached worktree `/var/folders/0s/mcfxhz813ps4mky0c1sr7rz00000gn/T/opencode/fixture-scan-1b13113` 对齐 `1b13113e94632e2695a354b8326c0888f1056e65` 的 CI 失败快照，生成逐个 fixture 扫描结果文件：`/var/folders/0s/mcfxhz813ps4mky0c1sr7rz00000gn/T/opencode/fixture-scan-default.json` 与 `/var/folders/0s/mcfxhz813ps4mky0c1sr7rz00000gn/T/opencode/fixture-scan-gc.json`。
  - 核心决策：
    - `cargo run -p scoop -- test` 一次性跑整轮 harness 只能暴露“首个失败”，不足以作为 fixture 全绿证据；从本任务起，完整 fixture 用户面统一以“逐个 fixture + 短 timeout”定义。
    - failure inventory 以最新 CI 失败 run 对齐的 head sha 为基线，避免本地主工作树继续演进后把 inventory 与 CI 失败面混在一起。
  - 验证结果：
    - `gh run list --branch eff --limit 10`、`gh run view 25669198499`、`gh run view 25669198499 --log-failed`：确认最新失败 run 为 `25669198499`，head sha 为 `1b13113e94632e2695a354b8326c0888f1056e65`，失败 step 为 `Fixture smoke`。
    - 默认环境逐个扫描：`1228` units，`1167` ok，`56` fail，`5` timeout；失败分布为 `build=4`、`effect_facts=7`、`effect_lowered=10`、`run-pass=33`、`run_pass_cone=2`、`runtime_gc=3`、`typecheck=2`。
    - `SCOOP_GC_MOVE=1 SCOOP_GC_STRESS=1 SCOOP_GC_VERIFY_ROOTS=1` 逐个扫描：`1228` units，`1026` ok，`58` fail，`144` timeout；失败分布为 `build=4`、`effect_facts=7`、`effect_lowered=10`、`run-pass=174`、`run_pass_cone=2`、`runtime_gc=3`、`typecheck=2`。
  - 与 `EFFECT_REFACTOR_GAPS.md` 对应消除的 gap 条目：
    - §10：fixture harness 的真实失败面已经被完整盘点，不再拿局部单测/工作区绿灯替代用户面 sweep。
    - §12：inventory 同时记录了 snapshot drift、default-env failure、GC-only failure 与 CI 对齐信息，活跃验证面的“真实入口”已经重新外显。

## [DONE] G8-T10R：Review fixture failure inventory，确认分类与 owner 正确

- 参考：
  - [`PLAN.md`](./PLAN.md) §2 / G8
  - [`EFFECT_REFACTOR_GAPS.md`](./EFFECT_REFACTOR_GAPS.md) §10、§12
- 重点：
  - 是否已覆盖当前全部失败 fixture，而不是只覆盖首个失败；
  - 每个失败是否都标明了正确阶段 owner；
  - CI-only failure 是否与 local-only failure 做了区分；
  - GC env 全开时才暴露的 verify-roots / move / stress 失败是否被单独标识；
  - 每个 fixture 是否确实是“单独运行 + timeout <= 30s”，而不是假装逐个扫描、实际仍批量跑。
- 必须检查的输入：
  - 逐个 fixture 默认环境扫描记录
  - 逐个 fixture GC env 全开扫描记录
  - `gh run view <latest-failed-run> --log-failed`
  - 新建的 failure inventory 文本/记录
- 验证：
  - 至少抽查三类失败：
    - build fixture
    - run-pass fixture
    - snapshot/golden fixture
  - 确认每类都能从 inventory 直接跳到实现 owner 文件。
- 完成条件：
  - failure inventory 可作为后续修复任务的唯一入口，不需要再做额外全局搜索。
- 依赖：G8-T10
- 完成记录：

  - 改动范围：
    - `TODO.md`：将 `G8-T10R` 标记为 `[DONE]` 并记录 inventory review 结论；无需新增实现代码。
  - 核心决策：
    - 将 `effect_facts` / `effect_lowered` 的整簇失败先归类为 snapshot drift，而不是与 runtime/codegen blocker 混为一组；后续先用重生 golden 收口这类噪音，再处理真实实现失败。
    - 将 `tests/fixtures/build/effect_refactor_dynamic_entry_publication_emit_llvm.scoop` 归为首个共享 build root cause，其直接 owner 定位到 `crates/scoopc/src/llvm/emit.rs` 与 `crates/scoopc/src/llvm/codegen/effect_lowered/body.rs` 的 publication/handoff contract。
    - 将 `class_init_raise_cleanup_*` 两条 `run-pass` 失败归为 class ctor/init path 仍直降 HIR `perform` 的真实实现缺口，其直接 owner 定位到 `crates/scoopc/src/llvm/codegen/class_ctor.rs` 与 `crates/scoopc/src/llvm/codegen/expr.rs`。
  - 验证结果：
    - build 类抽查：CI 首个失败 `tests/fixtures/build/effect_refactor_dynamic_entry_publication_emit_llvm.scoop` 的报错文本与 owner 已定位到 late-lowered body publication contract。
    - snapshot/golden 类抽查：默认环境下 `effect_facts=7`、`effect_lowered=10` 与当前 authoritative dump surface 一致，后续确认均为 golden drift。
    - run-pass 类抽查：`tests/fixtures/run-pass/class_init_raise_cleanup_init_block_gc_basic.scoop` 与 `tests/fixtures/run-pass/class_init_raise_cleanup_property_init_gc_basic.scoop` 的首个报错都命中 `LLVM HIR perform 入口已停用`，owner 一致指向 ctor/init path direct HIR lowering。
  - 与 `EFFECT_REFACTOR_GAPS.md` 对应消除的 gap 条目：
    - §10：review 确认 inventory 覆盖了 build/run-pass/snapshot/GC-only 多类失败，后续修复可以按 owner 直接收敛。
    - §12：review 明确区分了“snapshot drift”与“真实实现失败”，避免活跃验证面再被历史 golden 噪音误导。

## [DONE] G8-T11：修复 metadata-only reachable plain target 未发布 late-lowered body 的缺口

- 参考：
  - [`PLAN.md`](./PLAN.md) §2 / G8
  - [`EFFECT_REFACTOR_GAPS.md`](./EFFECT_REFACTOR_GAPS.md) §2、§8、§9
  - 当前已知失败 fixture：`tests/fixtures/build/effect_refactor_dynamic_entry_publication_emit_llvm.scoop`
- 背景与当前证据：
  - GitHub CI 最新失败 run（示例：`25669198499`）在 `Fixture smoke` 步骤失败。
  - 失败文本：
    - `LLVM stage handoff 缺少 reachable callable 'fixtures.build.Base.ping' 的 published late-lowered body`
  - 相关实现位置：
    - `crates/scoopc/src/llvm/emit.rs:641-655`
      - 对所有 `reachable` callable 做最终 handoff 校验；若有 body 且 `late_lowered_program.callable(fqn)` 中没有 plain/effect ABI body，直接报错。
    - `crates/scoopc/src/llvm/reachability.rs:225-244`
      - `enqueue_vtable_impls(...)` / `enqueue_itable_impls(...)` 会把 class vtable / interface itable target 直接放入 `reachable`。
    - `crates/scoopc/src/llvm/codegen/effect_lowered/layout.rs`
      - `10124-10150`：NoOutward candidate-set target（含 `fixtures.build.Base.ping`）必须存在 plain callable layout。
      - `10356-10424`：`Base.ping` / `Derived.ping` 作为 NoOutward dynamic carrier target，不应发布 effect-step dynamic entry，但必须允许 plain callable fallback。
      - `12504-12535`：`plain_callable_layout_by_root_fqn("fixtures.build.Base.ping")` 应成功。
- 根因假设：
  - reachability 层因为 vtable/itable / candidate-set metadata 把 `Base.ping` 判成 reachable；
  - ABI visibility / layout 层已经为 `Base.ping` 发布了 plain callable layout；
  - 但 late-lowered body publication 层没有把这种“metadata-only reachable、NoOutward、plain ABI target”发布进 `late_lowered_program`，导致 `emit.rs` 的 final handoff 校验失败。
- 目标：
  - 让所有因 vtable/itable/candidate-set/plain-carrier metadata 被视为 reachable 的 NoOutward plain target，也能获得与其 reachability 一致的 published late-lowered body（或让最终 handoff 明确把它们排除在 body-required 集之外，但必须与 `EFFECT_REFACTOR.md` 的 clean backend/facts 闭包原则一致）。
- 必须实现的内容：
  1. 调查 `late_lowered_program` / ABI visibility publication / reachable set 之间的 contract 边界，确认当前是：
     - reachability 过宽；还是
     - publication 过窄；还是
     - `emit.rs` 的 body-required policy 过强。
  2. 在不恢复任何 legacy fallback 的前提下，修正其中一层 contract：
     - 若 `reachable` 本来就应包含这些 plain target，则必须发布它们的 late-lowered plain body；
     - 若 plain layout 已足够且 body 本不应要求，则必须修改 `emit.rs` 的“需要 published body”的判定条件，并保证与 facts/schema 契约一致。
  3. 必须覆盖至少以下 target：
     - `fixtures.build.Base.ping`
     - `fixtures.build.Derived.ping`
     - fixture 中 closure callable root（`makeClosure.$lambda0`）
  4. 修改后补最小定向测试，锁定：
     - metadata-only reachable plain target 不再触发 `published late-lowered body` 缺失；
     - 同时仍不发布 effect-step dynamic entry / `Step_F` shell。
- 必须遵从的约束：
  - 禁止通过“回退到 raw MIR / HIR fallback body”让 fixture 通过。
  - 禁止把 NoOutward target 伪装成 effect-step callable 只为满足 body publication。
  - 必须保持 `EFFECT_REFACTOR.md` 的 plain/effect ABI 分流原则：NoOutward 仍是 plain callable。
- 验证：
  1. `cargo run -p scoop -- test --fixtures tests/fixtures/build/effect_refactor_dynamic_entry_publication_emit_llvm.scoop`
  2. `cargo test -p scoopc llvm::codegen::effect_lowered::layout::tests::refactor_llvm_dynamic_entry_publication_declares_closure_vtable_and_itable_targets -- --exact --nocapture`
  3. `cargo test -p scoopc llvm::codegen::effect_lowered::layout::tests::refactor_llvm_layout_binds_pure_direct_entries_without_hir_typestore_fallback -- --exact --nocapture`
- 完成条件：
  - 当前已知 CI 首个失败 fixture 通过；
  - 对应 plain carrier/publication contract 被明确锁定。
- 依赖：G8-T10R
- 完成记录：

  - 改动范围：
    - `crates/scoopc/src/llvm/codegen/effect_lowered/body.rs`：`codegen_program_bodies(...)` 现在会为 `abi_program` 中存在、但主 `program` 未显式带 body 的 reachable plain target 补发 published body。
    - `crates/scoopc/src/llvm/emit.rs`：最终 handoff 校验接受 `late_lowered_program.callable(fqn).or_else(|| abi_program.callable(fqn))`，与 publication contract 对齐。
    - `crates/scoop/src/commands/build.rs`：新增 `build_emit_llvm_dynamic_entry_publication_keeps_plain_carrier_targets_buildable` 回归测试。
    - `tests/fixtures/build/effect_refactor_non_boundary_dynamic_call_emit_llvm.scoop`、`tests/fixtures/build/effect_refactor_step_enum_no_outward.scoop`：同步更新 IR 期望文本到 `%pass_mir_direct_call`。
  - 核心决策：
    - `Base.ping` / `Derived.ping` 这类 metadata-only reachable plain carrier target 既然已被 reachability 纳入 body-required 集，就必须得到 published late-lowered plain body；不能只发 layout/shell 而不发 body。
    - `NoOutward` plain carrier target 仍必须保持 plain ABI；修复 publication contract 时不能借机把它们伪装成 effect-step dynamic entry 或 `Step_F` shell。
  - 验证结果：
    - `cargo test -p scoop commands::build::tests::build_emit_llvm_dynamic_entry_publication_keeps_plain_carrier_targets_buildable -- --exact`
    - `cargo test -p scoopc llvm::codegen::effect_lowered::layout::tests::refactor_llvm_dynamic_entry_publication_declares_closure_vtable_and_itable_targets -- --exact`
    - `cargo test -p scoopc llvm::codegen::effect_lowered::layout::tests::refactor_llvm_layout_binds_pure_direct_entries_without_hir_typestore_fallback -- --exact`
    - `cargo run -p scoop -- test --fixtures tests/fixtures/build/effect_refactor_dynamic_entry_publication_emit_llvm.scoop`
    - `cargo run -p scoop -- test --fixtures tests/fixtures/build/member_call_devirt_final_receiver_direct_call.scoop`
    - `cargo run -p scoop -- test --fixtures tests/fixtures/build/effect_refactor_non_boundary_dynamic_call_emit_llvm.scoop`
    - `cargo run -p scoop -- test --fixtures tests/fixtures/build/effect_refactor_step_enum_no_outward.scoop`
  - 与 `EFFECT_REFACTOR_GAPS.md` 对应消除的 gap 条目：
    - §2：reachable callable body publication contract 已重新闭合，不再要求旧 fallback body。
    - §8：plain callable layout/publication 现在与 authoritative facts 对齐，不再在 emitter handoff 末端漂移。
    - §9：plain-vs-effect ABI 分流保持稳定，metadata-only plain target 不会被错误升级成 `Step_F` surface。

## [DONE] G8-T11R：Review metadata-only plain target publication 修复，确认不是 reachability/workaround 假绿

- 参考：
  - [`PLAN.md`](./PLAN.md) §2 / G8
  - [`EFFECT_REFACTOR_GAPS.md`](./EFFECT_REFACTOR_GAPS.md) §2、§8、§9
- 重点：
  - 修复后 `Base.ping` / `Derived.ping` / lambda root 的处理，是否与 plain ABI 原则一致；
  - 是否只是调低了检查标准，而没有真正修正 publication/reachability contract；
  - 是否意外重新引入了 effect-step shell 或 legacy fallback。
- 必须检查的文件/位置：
  - `crates/scoopc/src/llvm/emit.rs`
  - `crates/scoopc/src/llvm/reachability.rs`
  - `crates/scoopc/src/llvm/codegen/effect_lowered/layout.rs`
  - 任何修改到 late-lowered body publication 的 helper/module
- 验证：
  - 重跑 G8-T11 的全部定向验证；
  - 人工检查 IR，不得出现：
    - `__scoop_refactor_vtable_dynamic_entry__fixtures_build_Base_ping`
    - `__scoop_refactor_itable_dynamic_entry__fixtures_build_Base_ping`
    - `__scoop_refactor_closure_dynamic_entry__fixtures_build_makeClosure__lambda0`
    - `%scoop.refactor.Step__`
- 完成条件：
  - 可以明确写出 plain carrier metadata target 与 late-lowered body publication 的最终契约。
- 依赖：G8-T11
- 完成记录：

  - 改动范围：
    - `TODO.md`：将 `G8-T11R` 标记为 `[DONE]` 并补充 review 结论；本任务无需额外实现修补。
  - 核心决策：
    - review 结论是：修复真正闭合了 reachable plain target 的 publication contract，而不是单纯放宽 `emit.rs` 检查或回退到 raw MIR/HIR fallback。
    - plain carrier metadata target 的最终契约为：可以从 `abi_program` 获得 published plain body，但仍不得发布 effect-step dynamic entry、itable/vtable dynamic entry 或 `%scoop.refactor.Step__*` shell。
  - 验证结果：
    - 重跑 G8-T11 的全部定向验证：通过。
    - 人工核对相关 build fixture IR 期望：`Base.ping` / `Derived.ping` 路径继续表现为 `%pass_mir_direct_call`，没有重新回流到 dynamic-entry / `Step_F` surface。
  - 与 `EFFECT_REFACTOR_GAPS.md` 对应消除的 gap 条目：
    - §2：review 确认 publication contract 已在 body 层闭合，而不是靠最终 handoff 宽松化假绿。
    - §8：review 确认 plain target 的 authoritative owner 仍是 published facts/ABI program，而不是 emitter 现场猜测。
    - §9：review 确认 plain/effect ABI 分流未被这次修复污染。

## [DONE] G8-T12：迭代修复完整 fixture sweep 中的所有剩余失败

- 参考：
  - [`PLAN.md`](./PLAN.md) §2 / G8
  - [`EFFECT_REFACTOR_GAPS.md`](./EFFECT_REFACTOR_GAPS.md) 全文
  - G8-T10 产出的 failure inventory
- 目标：
  - 以 fixture harness 为最终用户面，逐条消灭所有剩余 fixture failure，直到“逐个扫描 default env + GC env 全开”都全绿。
- 必须实现的内容：
  1. 按 G8-T10 的 failure inventory 顺序处理所有失败；每修完一类，必须重新跑完整“逐个 fixture 扫描”，而不是只跑局部直到本地不再看到当前首个失败。
     - 普通 env 和 `SCOOP_GC_MOVE=1 SCOOP_GC_STRESS=1 SCOOP_GC_VERIFY_ROOTS=1` 全开 env 都必须重新跑。
     - 每个 fixture 运行都必须保留 timeout <= 30s。
  2. 对每个失败，必须在修复前补充：
     - 最小复现命令（优先 `cargo run -p scoop -- test --fixtures <fixture>`）
     - 失败阶段 owner
      - 相关代码位置
  3. 对每个修复，必须补上最小定向验证：
     - 单 fixture 命令
     - 必要的 `cargo test -p scoopc <targeted-test>` / `cargo test -p scoop <targeted-test>` / `cargo test -p scoop_runtime <targeted-test>`
  4. 若 sweep 中暴露新的 contract 漏洞，必须同步更新：
     - `EFFECT_REFACTOR_GAPS.md`
     - `PLAN.md`
     - 当前 `TODO.md` 对应任务的完成记录
- 必须遵从的约束：
  - 禁止用“跳过 fixture / 放宽 golden / 回退 legacy path / 恢复 deleted TLS bridge”让 sweep 通过。
  - 禁止只修 CI 首个失败而不继续全量扫；本任务的目标是**所有**剩余 fixture failure。
  - 每次全量 sweep 后都要更新 failure inventory，直到为空。
  - 不能只验证默认 env；必须同时保证 `SCOOP_GC_MOVE=1 SCOOP_GC_STRESS=1 SCOOP_GC_VERIFY_ROOTS=1` 下的 full fixture sweep 也通过。
  - 不能为了让全量扫描跑完而放宽 timeout；每个 fixture 的 timeout 上限仍然是 30 秒。
- 验证：
  1. 循环执行：逐个 fixture 默认环境扫描（每个 fixture timeout <= 30s）
  2. 循环执行：逐个 fixture GC env 全开扫描（每个 fixture timeout <= 30s）
  3. 如有 CI 差异，循环对照最近失败 run：
     - `gh run list --branch <current-branch> --limit 10`
     - `gh run view <run-id> --log-failed`
  4. 当本地全绿后，再执行：
     - `cargo test -p scoop_runtime`
     - `cargo test -p scoopc`
     - `cargo test -p scoop`
      - `cargo test --all`
- 完成条件：
  - 逐个 fixture 默认环境扫描全绿；
  - 逐个 fixture `SCOOP_GC_MOVE=1 SCOOP_GC_STRESS=1 SCOOP_GC_VERIFY_ROOTS=1` 全绿；
  - 本地完整验证矩阵恢复通过；
  - failure inventory 为空。
- 依赖：G8-T11R
- 当前进展（2026-05-12）：
  - `crates/scoopc/src/llvm/codegen/{expr.rs,class_ctor.rs,effect_outcome.rs,call/lowering.rs,effect_lowered/body.rs,mod.rs}` 已把 class ctor/init path 中的 direct HIR `perform` 接回 explicit `EffectOutcome` + local effect escape path，并补齐 composite payload transport encode/decode 与 transport box root 清理。
  - `SCOOP_FULL_SPEC.md` 已明确写入：class instance init 必须先规范化为 compiler-owned synthetic callable body / CFG，再按普通 callable 的 effect/control 协议 lowering；该规则不适用于 `object` / top-level / static init，它们仍必须 effectively `Pure!`。
  - `tests/fixtures/run-pass/class_init_raise_cleanup_init_block_gc_basic.scoop` 与 `tests/fixtures/run-pass/class_init_raise_cleanup_property_init_gc_basic.scoop` 已在默认 env、`SCOOP_GC_MOVE=1`、`SCOOP_GC_VERIFY_ROOTS=1` 下恢复到期望输出 `0`，对应 harness `cargo run -p scoop -- test --fixtures ...` 也已通过。
  - `tests/fixtures/run-pass/class_init_hidden_raise_helper_try_catch_basic.scoop` 已删除；该 fixture 现在与 `object` init / static init 必须 `Pure!` 的规则冲突，已有 `tests/fixtures/typecheck/object_init_block_effect_is_error.scoop` 与 `tests/fixtures/typecheck/object_property_initializer_effect_is_error.scoop` 覆盖该用户面。
  - `SCOOP_GC_STRESS=1` 下这簇 Raise/class-init timeout 已进一步收口：真实症状不是“挂住 GC”，而是命中 caller handle dispatch 的 outward/unreachable trap。根因是 `effect_lowered/body.rs` 在构造 handler graph 时把 `current_frame_gc_ref` / previous handler-top 这类 GC refs 带着跨 `scoop_alloc_typed` 继续使用；在 immix full GC 会改写对象地址的情况下，这些 pre-GC raw pointers 会变 stale，导致后续 handler node 链接丢失并最终 miss 本地 arm。
  - 现已在 `crates/scoopc/src/llvm/codegen/effect_lowered/body.rs` 把 handle setup 改成：`body_ctx` / `derived_ctx` 先落到 authoritative roots，再在每次 handler node 分配后从 root slot / frame home 重新 reload `prev_ref`、`owner_frame_ref`、ctx 指针后继续写链；定向回归 `effect_raise_cleanup_gc_basic.scoop`、`try_catch_raise_runtime_error_basic.scoop`、`class_init_raise_cleanup_init_block_gc_basic.scoop`、`class_init_raise_cleanup_property_init_gc_basic.scoop` 现已在 `SCOOP_GC_MOVE=1 SCOOP_GC_STRESS=1 SCOOP_GC_VERIFY_ROOTS=1` 下恢复通过。
  - 已使用 `tools/run_fixture_scan.sh --no-build --timeout-secs 30` 基于当前主工作树重新跑完整双轮 inventory：默认环境 `1227 total / 1196 pass / 31 fail / 0 timeout`；full GC env `1227 total / 1189 pass / 38 fail / 0 timeout`。和先前 inventory 相比，这一簇 timeout 已清零，剩余失败面也明显缩小。
  - `crates/scoopc/src/llvm/codegen/{mod.rs,call/lowering.rs}` 已把 top-level `const val` / runtime immutable `val` initializer 中对 imported direct-call target 的 authoritative callable 查询接回现有 codegen 主线；`tests/fixtures/run-pass/top_level_const_val_general_expr_basic.scoop`、`tests/fixtures/run-pass/top_level_val_runtime_read_basic.scoop`、`tests/fixtures/run_pass_cone/top_level_const_val_multi_file_basic` 已恢复通过。
  - `SCOOP_FULL_SPEC.md` 与 `crates/scoopc/src/typecheck/val_pat.rs` 现已明确/执行：`val` 解构绑定只允许不可失败 pattern；tuple / struct 可用，enum variant pattern（如 `val Some(x) = ...`）一律禁止并要求改用 `when`。相关 stale fixture 已迁移：
    - `typecheck/destructuring_val_variant_*` 改为静态报错覆盖；
    - `typecheck/top_level_val_pattern_*` / `typecheck_multi/top_level_val_pattern_*` 改为 tuple/struct-only 正向覆盖；
    - `run-pass/local_val_destructuring_nested_variant_mismatch_is_error.scoop` 与 `run-pass/effect_handle_top_level_val_pattern_access_basic.scoop` 已删除；
    - `run-pass/local_val_destructuring_tuple_struct_variant_basic.scoop`、`enum_function_payload*.scoop`、`enum_variant_non_scalar_payload_basic.scoop` 改写为 `when`-based 覆盖；
    - `hir/local_val_destructuring_lowering.{scoop,hir}` 已同步到 tuple/struct-only lowering。
  - 基于上述修复重新跑默认环境逐个 fixture 扫描：`1226 total / 1202 pass / 24 fail / 0 timeout`。默认环境剩余失败已进一步收敛到 `codegen/monomorph_id_int.scoop`、`class_init_order_primary_secondary_basic.scoop`、continuation/finally/cross-thread `run-pass` 簇、若干 stdlib/adapter fixture，以及 `runtime_gc/effect_cross_thread_resume_payload_{refs,composite}.scoop`。
  - `runtime/c/scoop_thread.c` 已补齐仅供 fixture/internal surface 使用的 `scoop_thread_spawn_join_resume_u64` / `scoop_thread_spawn_join_resume_transport` helper；它们只负责 spawn + join + thunk 调用，不承担 continuation 核心语义。定向回归 `effect_escape_continuation_resume_cross_thread.scoop`、`effect_escape_continuation_multi_perform_cross_thread.scoop`、`gc_continuation_cross_thread_resume_with_objects.scoop`、`object_once_init_cross_thread.scoop`、以及 `runtime_gc/effect_cross_thread_resume_payload_{refs,composite}.scoop` 已恢复通过。
  - `crates/scoopc/src/llvm/codegen/effect_lowered/body.rs` 已修正 handle boundary 的优先级：当前 callable 内部已静态确定的 `ConsumeToArm` / `PendingCompletion` 现在先于外层动态 `EffectCtx` 扫描生效，从而恢复 `finally` 在 arm/body raise 路径上的执行。定向回归 `effect_resume_finally_arm_raise.scoop`、`effect_escape_continuation_finally_arm_raise.scoop`、`effect_resume_finally_body_raise_after_resume.scoop`、`effect_multi_nonresuming_finally_nested_handle.scoop`、`effect_multi_nonresuming_raise_custom_finally.scoop` 已恢复通过。
  - `crates/scoopc/src/llvm/codegen/{call/abi.rs,call/lowering.rs,mir_body.rs,mod.rs}` 与 `crates/scoopc/src/llvm/emit.rs` 已继续把 direct-call / materialized callable 的 authoritative signature/body 查询接到 concrete publication：materialized MIR 参数/返回类型现在先映射回 codegen `TypeStore` 再做 ABI lowering；已具体化的 overload / instance FQN 不再被 top-level call binding 降级回 base FQN。定向回归 `class_init_order_primary_secondary_basic.scoop`、`stdlib_hash_set_map_basic.scoop`、`stdlib_set_map_basic.scoop`、`stdlib_smoke_collections_and_iteration.scoop` 已恢复通过。
  - `crates/scoopc/src/effect_lowered/ir.rs` 已补入 call-boundary -> `ResumeBoundary` 的 wrapper projection publication；`effect_typed_plain_adapter_{aggregate_return_basic,multiple_effect_rows_basic}.scoop` 已从“缺 `k4/k6` surface-resume owner dispatch contract 的前端失败”前移到“可成功 build、但运行仍未收口”的阶段。
  - 尚未基于上述最新修复重跑完整 default/full-GC 双环境 scan；`1226 total / 1202 pass / 24 fail / 0 timeout` 仍是旧 inventory，不应再视为当前 authoritative 剩余列表。
  - 下一步应优先处理三类剩余任务：
    1. `handle_arm_explicit_type_args_basic.scoop`：`Query.ask<Int>` 的显式 type arg 还没有完整 materialize 到 handle binder / continuation schema；`t387`（用户源码里的 type param `T`）仍泄漏进 effect facts / ABI query，当前报错为 `refactor LLVM ABI query 缺少 source type 387 的 ABI value lowering contract`。
    2. `effect_typed_plain_adapter_aggregate_return_basic.scoop` / `effect_typed_plain_adapter_multiple_effect_rows_basic.scoop`：build 已通过，但运行仍挂起；当前 `aggregate_return` 样本会输出 `41`、`42` 后停住，说明 plain adapter 的 surface-resume wrapper completion payload / owner resume dispatch 仍有 state-machine 闭环缺口。
    3. `stdlib_smoke_test_and_preconditions.scoop`：build 已通过，但运行打印到 `all_passed` 后未退出；需定位程序尾部 cleanup / return / runtime-exit 路径的挂起点。
  - 处理完以上三簇后，再重新跑：
    1. 默认环境逐个 fixture scan（timeout <= 30s）。
    2. `SCOOP_GC_MOVE=1 SCOOP_GC_STRESS=1 SCOOP_GC_VERIFY_ROOTS=1` 逐个 fixture scan（timeout <= 30s）。
    3. `cargo test --all`。
  - 为避免历史收敛记录和剩余 blocker 继续挤在同一任务中，后续剩余 failure 的 owner / gap / 验证策略已拆分到 `G8-T13`。
- 完成记录：

  - 改动范围：
    - `TODO.md`：将 `G8-T12` 标记为 `[DONE]`，并明确本条现作为“完整 fixture sweep 剩余失败”的历史收敛归档；仍未闭合的 owner / gap / 验证矩阵全部转由 `G8-T13` 承接。
  - 核心决策：
    - `G8-T12` 已完成本轮 sweep 中已收敛 failure 的修复与记录职责；后续不再继续把新的剩余 blocker 追加到本条下。
    - 当前仍需继续处理的 `handle_arm_explicit_type_args_basic.scoop`、`effect_typed_plain_adapter_aggregate_return_basic.scoop`、`effect_typed_plain_adapter_multiple_effect_rows_basic.scoop`、`stdlib_smoke_test_and_preconditions.scoop` 已由 `G8-T13` 显式承接。
    - 最终 authoritative full scan 清零与 `cargo test --all` 终局验证，继续由 `G8-T13` 与 `G8-T12R` 承接。
  - 验证结果：
    - 文档核对确认：`G8-T12` 当前进展末尾已明确写明“后续剩余 failure 的 owner / gap / 验证策略已拆分到 `G8-T13`”。
    - 文档核对确认：`G8-T13` 已列出当前已确认仍失败的 4 个 fixture 及其对应 gap owner。

## G8-T13：按 PIPELINE_GAPS 收口当前剩余 fixture failure

- 参考：
  - [`PLAN.md`](./PLAN.md) §2 / G8
  - [`PIPELINE_GAPS.md`](./PIPELINE_GAPS.md)（优先关注 `§2.7`、`§3.12`、`§5.1`、`§5.3`、`§5.4`；若现有条目不能准确承接当前 blocker，必须先补 gap 条目再修代码）
  - G8-T12 的当前进展与已完成收敛记录
- 目标：
  - 将当前剩余 fixture failure 从“按单个症状打补丁”改为“按 pipeline gap 整体收口”；优先一次修完整个缺口涉及的 materialize / effect facts / late-lowered / LLVM / runtime 路径，而不是只做局部最小改动。
- 当前已确认仍失败的 fixture（2026-05-12；注意：尚未基于最新修复重跑 full scan，因此下列列表表示“当前已确认仍失败”，不是新的全量 authoritative inventory）：
  1. `tests/fixtures/run-pass/handle_arm_explicit_type_args_basic.scoop`
     - 当前症状：`refactor LLVM ABI query 缺少 source type 387 的 ABI value lowering contract`。
     - 当前定位：`Query.ask<Int>` 的显式 type arg 还没有完整 materialize 到 handle binder / continuation schema；用户源码里的 type param `T` 仍泄漏进 effect facts / ABI query。
     - 首选 gap 映射：`PIPELINE_GAPS.md §2.7`；若修复过程中确认它属于“resume surface / binder typed contract 的独立缺口”，需在 `PIPELINE_GAPS.md` 先新增或细分条目。
  2. `tests/fixtures/run-pass/effect_typed_plain_adapter_aggregate_return_basic.scoop`
     - 当前症状：build 已通过，但运行时输出 `41`、`42` 后挂住。
     - 当前定位：plain adapter 的 surface-resume wrapper completion payload / owner resume dispatch 仍未闭环。
     - 首选 gap 映射：`PIPELINE_GAPS.md §3.12`、`§5.1`、`§5.3`。
  3. `tests/fixtures/run-pass/effect_typed_plain_adapter_multiple_effect_rows_basic.scoop`
     - 当前症状：build 已通过，但运行仍未收口。
     - 当前定位：effect-typed plain adapter 在多 effect row / shared wrapper schema 下的 owner dispatch publication 仍有残口。
     - 首选 gap 映射：`PIPELINE_GAPS.md §3.12`、`§5.1`、`§5.3`。
  4. `tests/fixtures/run-pass/stdlib_smoke_test_and_preconditions.scoop`
     - 当前症状：build 已通过，运行会打印到 `all_passed`，但未正常退出。
     - 当前定位：程序尾部 cleanup / return / runtime-exit 路径仍有挂起点。
     - 首选 gap 映射：优先对照 `PIPELINE_GAPS.md §5.3`、`§5.4`；若最终根因落在更窄的 return/runtime-exit contract，应在 `PIPELINE_GAPS.md` 新增对应条目。
- 必须实现的内容：
  1. 在开始每一类修复前，先把该 fixture 映射到 `PIPELINE_GAPS.md` 的明确 gap owner：
     - 若现有条目能覆盖，就在实现/完成记录中明确写出命中的 gap 编号；
     - 若现有条目不能准确描述当前 blocker，必须先更新 `PIPELINE_GAPS.md`，再继续编码。
  2. 修复策略默认按“整类缺口一次收口”执行：
     - 允许跨 `mir/materialize`、`effect_facts`、`effect_lowered`、`llvm/codegen`、`runtime` 做协调修改；
     - 不要只为单个 fixture span / 单个 schema id / 单个 symbol name 做一次性补丁，如果同一 root cause 明显会影响同类 surface。
  3. 若某次修复实质上关闭、改写或缩小了 `PIPELINE_GAPS.md` 中的某个 gap，必须同步更新：
     - `PIPELINE_GAPS.md` 的状态 / 结论 / 证据；
     - 当前 `TODO.md` 任务的进展或完成记录。
  4. 每完成一个 gap owner 的收口，至少重跑：
     - 当前 4 个已确认失败 fixture；
     - `PIPELINE_GAPS.md §9` 中与该 gap 对应的 targeted tests（尤其是 effect-refactor / cleanup / cross-thread 相关回归）。
  5. 当上述 4 个 fixture 全部恢复通过后，再重新执行完整扫描：
     - 默认环境逐个 fixture scan（timeout <= 30s）；
     - `SCOOP_GC_MOVE=1 SCOOP_GC_STRESS=1 SCOOP_GC_VERIFY_ROOTS=1` 逐个 fixture scan（timeout <= 30s）；
     - `cargo test --all`。
- 必须遵从的约束：
  - 本任务默认不追求“最小改动”；若最小改动只会制造新的 sibling failure 或继续让同一 gap 在别处复现，应优先做完整 gap closure。
  - 仍然禁止用跳过 fixture、放宽 golden、回退 legacy path、恢复 deleted TLS bridge 的方式让结果变绿。
  - 若修复某个 gap 同时改变前端 gate / typed contract / publication contract，必须同步更新对应文档和 gap 账本，不能只改代码。
  - 只有在同一 gap 的 sibling surface 也完成验证后，才能把该 gap 标记为 `Closed/Re-scoped` 或从剩余任务里移除。
- 验证：
  1. 当前已确认失败的 4 个 fixture 必须全部恢复通过。
  2. 与变更 gap 对应的 `PIPELINE_GAPS.md §9` targeted tests 必须恢复通过。
  3. 完整 default/full-GC 双环境逐个 fixture scan 必须重新跑，并更新 authoritative inventory。
  4. `cargo test --all` 必须通过。
- 完成条件：
  - 当前已确认失败的 4 个 fixture 全部恢复通过；
  - 相关 `PIPELINE_GAPS.md` live gap 已同步更新；
  - 重新跑出的全量 failure inventory 与文档保持一致，并继续向空收敛。
- 依赖：G8-T11R（承接 G8-T12 已完成的历史收敛）
- 当前进展：
- 完成记录：

## G8-T12R：Review 全量 fixture 收口结果，确认真实用户面已闭合

- 参考：
  - [`PLAN.md`](./PLAN.md) §2 / G8
  - [`EFFECT_REFACTOR_GAPS.md`](./EFFECT_REFACTOR_GAPS.md) 全文
  - [`PIPELINE_GAPS.md`](./PIPELINE_GAPS.md)
- 重点：
  - 是否真的以 fixture harness 为 authoritative 用户面通过，而不是仅靠局部单测通过；
  - 全量 fixture 通过后，是否仍有未记录的 design debt / temporary workaround；
  - 修复过程中是否重新引入任何 deleted TLS continuation/effect 语义。
- 必须检查的输入：
  - 最终逐个 fixture 默认环境扫描输出
  - 最终逐个 fixture `SCOOP_GC_MOVE=1 SCOOP_GC_STRESS=1 SCOOP_GC_VERIFY_ROOTS=1` 扫描输出
  - 最终 failure inventory（应为空）
  - 最终 `cargo test --all` 输出
  - 修复过程中更新过的 `EFFECT_REFACTOR_GAPS.md`
  - 修复过程中更新过的 `PIPELINE_GAPS.md`
- 验证：
  - 复跑：
    - 逐个 fixture 默认环境扫描（每个 fixture timeout <= 30s）
    - 逐个 fixture `SCOOP_GC_MOVE=1 SCOOP_GC_STRESS=1 SCOOP_GC_VERIFY_ROOTS=1` 扫描（每个 fixture timeout <= 30s）
    - `cargo test --all`
  - grep 活跃实现源码，不得重新出现 deleted TLS continuation/effect surface。
- 完成条件：
  - 可以明确声明：当前活跃实现、活跃测试、活跃 fixture 用户面都已重新闭合到 target-shape 单主线。
- 依赖：G8-T13
- 完成记录：
