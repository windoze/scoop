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

## G2-T03：重建 backend-owned `EffectOutcome` / transport primitive

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

## G2-T03R：Review explicit outcome/transport primitive，确认 contract 已 backend-owned

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

## G3-T04：重建显式 `EffectCtx` / handler graph 模型

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

## G3-T04R：Review `EffectCtx` / handler graph，确认不再退回 ambient context

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

## G4-T05：重建 ordinary callee suspend/reentry 分析与 lowering

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

## G4-T05R：Review ordinary callee suspend/reentry，确认 facts 驱动且无 TLS 旁路

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

## G5-T06：重建 codegen-owned continuation object model 与 generated resume driver

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
- 依赖：G4-T05R
- 完成记录：

## G5-T06R：Review continuation object model / generated resume driver，确认 owner 已迁回 codegen

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

## G6-T07：重建 direct/static/dynamic call lowering 与 plain/effect ABI 分流

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

## G6-T07R：Review direct/static/dynamic call lowering，确认 ABI 分流已 facts-driven

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

## G7-T08：重建 `perform` / `handle` / `resume` / `Step_F` lowering

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

## G7-T08R：Review `perform` / `handle` / `resume` / `Step_F` lowering，确认 surface 已切到新协议

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

## G8-T09：runtime generic substrate 收尾、验证面迁移与 full regression 恢复

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

## G8-T09R：Review 最终收口结果，确认仓库重新只剩 target-shape 单主线

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
