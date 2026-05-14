# TODO（Managed ABI / native callable ABI 收口）

> 生成时间：2026-05-14  
> 计划基线：[`PLAN.md`](./PLAN.md)  
> 设计基线：[`MANAGED_ABI.md`](./MANAGED_ABI.md)  
> 格式参考：`docs/archive/plans/TODO-pipeline-gaps.md`、`docs/archive/plans/PLAN-pipeline-gaps.md`  
> 顺序约束：严格按当前文件中的条目顺序推进；不得跨条目并行实现。  
> 当前状态：`ExternAbi::Scoop` 尚未实现；native `@Extern` 与 `FunPtr` 已在参数/返回 lowering 上部分对齐，但 `enter_native/leave_native`、`gc-leaf`、calling convention、ABI identity、surface gate 仍分裂；`string cone` 只完成了 `sysroot/string.scoop` 中那批普通 helper，remaining string helper 仍依赖 compiler/runtime 特判。  
> 重要提醒：当前 mainline 已锁住 `unsafe_funptr_aggregate_return_tuple`、`extern_fun_effectful_funptr_is_error`、`single_file_minimal_ir_includes_compilable_sysroot_string_helpers` 等行为。除非先回写 `MANAGED_ABI.md` 与相关 fixture，否则不得把这些 current behavior 直接回退掉。

## 任务索引

| ID | 阶段 | 标题 |
| --- | --- | --- |
| `P0-T01` | P0 | [DONE] 冻结 current ABI baseline 与 regression owner map |
| `P1-T01` | P1 | [DONE] 建立 callable ABI identity，并让 `ExternFun.abi` 真正进入 lowering source of truth |
| `P1-T02` | P1 | 收紧 `FunPtr<F>` 合同：pure-only native surface |
| `P2-T01` | P2 | 建立单一 native ABI classifier，统一 direct/indirect declaration 与 call scaffolding |
| `P2-T02` | P2 | 收口 native surface gate 与诊断，统一 `@Extern` / native `FunPtr` contract |
| `P3-T01` | P3 | 扩展 `@Extern` 语法与 HIR，正式支持 `abi = "scoop"` |
| `P3-T02` | P3 | 接通 `ExternAbi::Scoop` 的 declaration / call lowering，并补 IR/run-pass 回归 |
| `P4-T01` | P4 | 以标量 `toString` 为 tracer bullet，删除第一批字符串名字特判 |
| `P4-T02` | P4 | 迁移 remaining string helper，明确 substrate 边界并收缩 runtime surface |
| `P5-T01` | P5 | 做全量稳定化、跨平台矩阵与文档收尾 |

## 全局约束

- [`PLAN.md`](./PLAN.md) 是本轮唯一计划基线；当前文件只负责把 `PLAN.md` 的 P0-P5 拆成严格顺序执行的任务。
- [`MANAGED_ABI.md`](./MANAGED_ABI.md) 是设计基线，但当前代码已经与其部分章节发生 drift。若后续任务要改变当前已通过回归的 surface，必须先回写 `MANAGED_ABI.md`，再修改代码和 fixture。
- 当前已被 mainline 锁住的 surface，未经显式设计回写不得回退：
  - `tests/fixtures/run-pass/unsafe_funptr_aggregate_return_tuple.scoop`
  - `tests/fixtures/typecheck/extern_fun_effectful_funptr_is_error.scoop`
  - `crates/scoopc/src/llvm/tests.rs::single_file_minimal_ir_includes_compilable_sysroot_string_helpers`
- `sysroot/string.scoop` 中的 `substring/indexOf/contains/startsWith/endsWith/split/trimStart/trimEnd/trim` 已视为完成基线；后续任务不得把它们搬回 runtime helper 或 compiler special-case。
- `ExternAbi::Scoop` v1 继续保持 `Pure` only / top-level only / 无 generics / 无 closure crossing / 无 outward suspend。
- `FunPtr` 重构必须同时守住两类现有 surface：
  - native function pointer surface；
  - effectful / deferred capability bridge。
- native surface 的 direct/indirect parity 不能只在单一 host 上“碰巧跑通”；至少要明确覆盖 `linux/amd64` 与 `macos/aarch64`。
- 每个任务完成后，必须回写：
  - 改动范围；
  - 核心决策；
  - 验证结果；
  - 与 `PLAN.md` / `MANAGED_ABI.md` 的对应闭合。

## 已落地基线

- C-only extern metadata 已存在，但只落到 `ExternAbi::C`：
  - `crates/scoopc/src/hir/mod.rs::ExternFun` / `ExternAbi`
  - `crates/scoopc/src/hir/lower/util.rs::extern_fun_of_decl`
  - `crates/scoopc/src/typecheck/annotations.rs`（`@Extern` 只接受 `name` / `lib`）
- direct `@Extern` native leaf contract 已存在并有 fixture：
  - `crates/scoopc/src/llvm/codegen/mod.rs::{declare_top_level_fun,declare_top_level_fun_with_signature_override}`
  - `crates/scoopc/src/llvm/codegen/call/lowering.rs::{codegen_top_level_fun_call_impl,emit_enter_native_for_extern_call_impl,emit_extern_native_call_impl}`
  - `tests/fixtures/runtime_gc/extern_enter_native_roots_gc.scoop`
  - `tests/fixtures/build/extern_enter_native_no_statepoint_writeback.scoop`
- native `FunPtr` aggregate return 已按目标 ABI 跑通一条回归：
  - `crates/scoopc/src/llvm/codegen/call/lowering.rs::codegen_funptr_value_call_impl`
  - `crates/scoopc/src/llvm/codegen/mir_body.rs::codegen_mir_funptr_value_call`
  - `tests/fixtures/run-pass/unsafe_funptr_aggregate_return_tuple.scoop`
- non-pure `FunPtr<F>` 签名必须在前端被拒绝：
  - `tests/fixtures/typecheck/extern_fun_effectful_funptr_is_error.scoop`
  - `tests/fixtures/typecheck/uintptr_to_funptr_effectful_type_arg_is_error.scoop`
- 已迁移到 ordinary sysroot helper 的 string 逻辑：
  - `sysroot/string.scoop`
  - `crates/scoopc/src/llvm/tests.rs::single_file_minimal_ir_includes_compilable_sysroot_string_helpers`
  - `crates/scoopc/src/llvm/codegen/runtime_abi.rs` 中移除 `substring` / `starts_with/ends_with/index_of/contains/split/trim*` runtime declarations 的注释
- remaining string special-case 主要集中在：
  - `sysroot/core.scoop`
  - `crates/scoopc/src/resolve/scopes.rs`
  - `crates/scoopc/src/typecheck/expr/call.rs`
  - `crates/scoopc/src/hir/lower/expr.rs::should_keep_member_call_as_member_access`
  - `crates/scoopc/src/llvm/codegen/intrinsics/builtin.rs`
  - `crates/scoopc/src/llvm/codegen/effect_lowered/{body.rs,value.rs}`
  - `crates/scoopc/src/llvm/codegen/runtime_abi.rs`
  - `runtime/c/scoop_runtime.c`

## P0：冻结 current baseline

### [DONE] P0-T01：冻结 current ABI baseline 与 regression owner map

- 参考：
  - [`PLAN.md`](./PLAN.md) §5 / P0
  - [`MANAGED_ABI.md`](./MANAGED_ABI.md) §1、§5、§10
- 目标：
  - 在继续做 ABI refactor 之前，先把 current mainline 已经成立的 ABI 行为锁成一份统一 baseline。
  - 让后续任务不需要重新判断“这是设计 drift 还是已经被 repo 接受的 contract”。
- 当前实现入口：
  - `crates/scoopc/src/hir/mod.rs::ExternFun` / `ExternAbi`
  - `crates/scoopc/src/typecheck/annotations.rs::{check_extern_fun_effect_contract,check_extern_fun_signature_is_gc_free}`
  - `crates/scoopc/src/llvm/codegen/mod.rs::{declare_top_level_fun,declare_top_level_fun_with_signature_override}`
  - `crates/scoopc/src/llvm/codegen/call/lowering.rs::{codegen_top_level_fun_call_impl,emit_enter_native_for_extern_call_impl,emit_extern_native_call_impl,codegen_funptr_value_call_impl}`
  - `crates/scoopc/src/llvm/codegen/mir_body.rs::{codegen_mir_direct_call_with_policy,codegen_mir_funptr_value_call,codegen_mir_funptr_invoke_call}`
  - `tests/fixtures/runtime_gc/extern_enter_native_roots_gc.scoop`
  - `tests/fixtures/build/extern_enter_native_no_statepoint_writeback.scoop`
  - `tests/fixtures/run-pass/unsafe_funptr_extern_call_basic.scoop`
  - `tests/fixtures/run-pass/unsafe_funptr_aggregate_return_tuple.scoop`
  - `tests/fixtures/typecheck/extern_fun_effectful_funptr_is_error.scoop`
  - `crates/scoopc/src/llvm/tests.rs::single_file_minimal_ir_includes_compilable_sysroot_string_helpers`
- 必须实现的内容：
  1. 把以下 current behavior 明确冻结成 regression owner map：
     - direct `@Extern` 会插 `enter_native/leave_native`，并暴露 native roots；
     - direct `@Extern` 不重新进入 statepoint rewrite；
     - native `FunPtr` aggregate return 不回 ordinary hidden sret；
     - non-pure `FunPtr<F>` 必须在前端被拒绝；
     - `sysroot/string.scoop` helper 继续编进模块而不是 runtime 映射。
  2. 推荐在 `crates/scoopc/src/llvm/tests.rs` 补一个集中 ABI baseline audit，至少覆盖：
     - native leaf direct call；
     - native funptr aggregate return；
     - non-pure `FunPtr` rejection；
     - compiled sysroot string helper。
  3. 若审计发现 current code 与 `MANAGED_ABI.md` 描述不一致，先在文档中标出 drift，不提前改变实现。
- 必须遵从的约束：
  - 本任务不实现 `ExternAbi::Scoop`，也不改变 native surface gate。
  - 不允许通过删除或降级现有 passing fixture 来“消除 drift”。
- 验证：
  1. `cargo run -p scoop -- test --fixtures tests/fixtures/runtime_gc/extern_enter_native_roots_gc.scoop`
  2. `cargo run -p scoop -- test --fixtures tests/fixtures/build/extern_enter_native_no_statepoint_writeback.scoop`
  3. `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/unsafe_funptr_extern_call_basic.scoop`
  4. `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/unsafe_funptr_aggregate_return_tuple.scoop`
  5. `cargo run -p scoop -- test --fixtures tests/fixtures/typecheck/extern_fun_effectful_funptr_is_error.scoop`
  6. `cargo test -p scoopc llvm_tests -- --nocapture`
- 完成条件：
  - 后续任务可以直接引用同一份 baseline，不需要再来回搜索 current behavior。
- 依赖：无
- 完成记录：
  - 改动范围：
    - `crates/scoopc/src/llvm/tests.rs`
    - `MANAGED_ABI.md`
    - `TODO.md`
  - 核心决策：
    - 在 `crates/scoopc/src/llvm/tests.rs` 新增 `abi_baseline_*` 集中审计入口，冻结四类 current behavior：
      - direct `@Extern` native leaf 的 `enter_native/leave_native` + `gc-leaf-function` + explicit-frame reload；
      - native `FunPtr` aggregate return 继续走目标 ABI 直接返回，不回 ordinary hidden sret；
      - non-pure `FunPtr<F>` 继续在前端被拒绝；
      - compiled `sysroot/string.scoop` helper 继续编进当前模块。
    - 保留 `single_file_minimal_ir_includes_compilable_sysroot_string_helpers` 等既有 owner 测试，并新增 `abi_baseline_*` 前缀测试作为 P0 的集中 audit 入口。
    - 在 `MANAGED_ABI.md` 修正了已经与现状不一致的 native `FunPtr` aggregate-return 描述，并新增 `§1.5 P0-T01` owner map，明确记录当前 design drift：`GC-free` gate 仍在、native classifier 仍分裂、`ExternAbi::Scoop` 尚未实现。
    - 本任务只冻结 baseline 与文档，不改变当前用户可见 ABI surface。
  - 验证结果：
    - `cargo fmt --all`
    - `cargo test -p scoopc abi_baseline -- --nocapture`
    - `cargo run -p scoop -- test --fixtures tests/fixtures/runtime_gc/extern_enter_native_roots_gc.scoop`
    - `cargo run -p scoop -- test --fixtures tests/fixtures/build/extern_enter_native_no_statepoint_writeback.scoop`
    - `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/unsafe_funptr_extern_call_basic.scoop`
    - `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/unsafe_funptr_aggregate_return_tuple.scoop`
    - `cargo run -p scoop -- test --fixtures tests/fixtures/typecheck/extern_fun_effectful_funptr_is_error.scoop`
    - `cargo test -p scoopc llvm_tests -- --nocapture`（当前 harness 下该 filter 返回 `0 passed; 842 filtered out`，因此额外按 owner 测试名补跑下面两条）
    - `cargo run -p scoop -- test --fixtures tests/fixtures/typecheck/uintptr_to_funptr_effectful_type_arg_is_error.scoop`
    - `cargo test -p scoopc single_file_minimal_ir_includes_compilable_sysroot_string_helpers -- --nocapture`
    - `cargo clippy --all-targets -- -D warnings`
  - 与 `PLAN.md` / `MANAGED_ABI.md` 的对应闭合：
     - 对应 `PLAN.md` P0 第 1-4 项：direct extern / native funptr baseline、non-pure `FunPtr` rejection 与 compiled sysroot string helper baseline 已冻结为可执行 regression owner map，不再需要后续任务重复搜索 current behavior。
    - `MANAGED_ABI.md` 现已把 native `FunPtr` aggregate-return 的 current reality 回写到 §1.4，并在 §1.5 明确标出仍待后续阶段收口的 drift；本任务没有提前实现 `ExternAbi::Scoop`，也没有改变 native surface gate。

## P1：建立 ABI identity

### [DONE] P1-T01：建立 callable ABI identity，并让 `ExternFun.abi` 真正进入 lowering source of truth

- 参考：
  - [`PLAN.md`](./PLAN.md) §5 / P1
  - [`MANAGED_ABI.md`](./MANAGED_ABI.md) §4.3、§4.4、§6
- 目标：
  - 把源码级函数签名与 ABI 身份分开建模。
  - 让 `ExternFun.abi` 不再只是 HIR side table 的占位字段，而是真正驱动 lowering 的 source of truth。
- 当前实现入口：
  - `crates/scoopc/src/hir/mod.rs`
  - `crates/scoopc/src/hir/lower/util.rs::extern_fun_of_decl`
  - `crates/scoopc/src/pipeline/hir_stage.rs::TypedCallSiteContract`
  - `crates/scoopc/src/llvm/codegen/mod.rs`
  - `crates/scoopc/src/llvm/codegen/call/lowering.rs`
  - `crates/scoopc/src/llvm/codegen/mir_body.rs`
- 必须实现的内容：
  1. 引入显式的 callable ABI identity（命名可自定，但职责必须稳定），至少能表示：
     - ordinary managed callable；
     - native external callable；
     - managed external callable；
     - effect-step managed callable。
  2. 让 declaration path 与 call lowering 都通过该 identity 分流，而不是继续靠：
     - `extern_funs.contains_key(fqn)`
     - `nominal.fqn == "scoop.unsafe.FunPtr"`
     - `FunctionType.effects.is_pure()`
     之类局部条件拼装 ABI。
  3. 让 `ExternFun.abi` 被真正消费；即使当前 parser 还只会产出 `C`，consumer 也必须已经按 ABI family 组织。
  4. 推荐把 identity 定义放到 HIR/codegen 都能共享的位置，避免再在 LLVM 层重造一份并行 classifier 输入。
- 必须遵从的约束：
  - 不得新增新的布尔旗标链把 ABI identity 再拆回零散 `if` 分支。
  - 不得改变当前用户可见 surface，只重组内部 source-of-truth。
- 验证：
  1. `cargo run -p scoop -- test --fixtures tests/fixtures/typecheck/refactor_hir_call_contracts_surface_ok.scoop`
  2. `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/unsafe_funptr_extern_call_basic.scoop`
  3. `cargo test -p scoopc llvm_tests -- --nocapture`
- 完成条件：
  - declaration path 与 call lowering 可以查询同一 ABI identity，而不是各自从语法形状猜测。
- 依赖：`P0-T01`
- 完成记录：
  - 改动范围：
    - `crates/scoopc/src/hir/mod.rs`
    - `crates/scoopc/src/pipeline/hir_stage.rs`
    - `crates/scoopc/src/llvm/codegen/{mod.rs,call/abi.rs,call/lowering.rs,mir_body.rs,intrinsics/sysroot.rs}`
    - `crates/scoopc/src/typecheck/{lower.rs,annotations.rs,expr/call.rs}`
    - `tests/fixtures/typecheck/refactor_hir_call_contracts_surface_ok.scoop`
    - `tests/fixtures/typecheck/{extern_fun_effectful_funptr_is_error.scoop,uintptr_to_funptr_effectful_type_arg_is_error.scoop}`
    - `PLAN.md`
    - `MANAGED_ABI.md`
    - `TODO.md`
  - 核心决策：
    - 在共享的 HIR 层新增 `hir::CallableAbiIdentity`，稳定表示 `ManagedOrdinary`、`NativeExtern`、`ManagedExtern` 与 `EffectBridge` 四类 callable ABI family；`ExternFun.abi` 通过 `ExternFun::callable_abi_identity()` 进入实际 consumer，而不再只是 side table 占位字段。
    - `pipeline::hir_stage` 的 `FunctionTargetContract` 与 `TypedCallSiteContract::{Closure, FunValue, FunPtr}` 现在都显式携带 `abi_identity`，使 direct/member/extension/fun value/funptr 调用在 HIR handoff 阶段就保留 ABI family，而不是下游再从语法形状回推。
    - LLVM 侧把 `direct_call_abi_identity`、`managed_callable_abi_identity` 与 `funptr_callable_abi_identity` 收口到 `crates/scoopc/src/llvm/codegen/call/abi.rs`，让 declaration path、direct call、MIR direct call、function-value call 与 funptr call 都从同一 identity source-of-truth 分流 native / ordinary / effect-step 路径。
    - 当前 contract 下把 `FunPtr<F>` 明确约束为 pure-only native callable token，并把 effectful `FunPtr` 改为前端拒绝；这样 `CallableAbiIdentity::funptr()` 不再需要假装兼容 effect bridge，`funPtrToUIntPtr` / `uintPtrToFunPtr` 也继续只承载地址 round-trip。`P1-T02` 仍保留，用于继续清理 remaining `FunPtr` native-only route 与 classifier/gate 收口。
  - 验证结果：
    - `cargo run -p scoop -- test --fixtures tests/fixtures/typecheck/refactor_hir_call_contracts_surface_ok.scoop`
    - `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/unsafe_funptr_extern_call_basic.scoop`
    - `cargo run -p scoop -- test --fixtures tests/fixtures/typecheck/extern_fun_effectful_funptr_is_error.scoop`
    - `cargo run -p scoop -- test --fixtures tests/fixtures/typecheck/uintptr_to_funptr_effectful_type_arg_is_error.scoop`
    - `cargo test -p scoopc llvm_tests -- --nocapture`（当前 harness 下该 filter 返回 `0 passed; 841 filtered out`，因此额外按 owner 测试名补跑下面三条）
    - `cargo test -p scoopc refactor_hir_call_contracts_record_callable_provenance -- --nocapture`
    - `cargo test -p scoopc refactor_hir_rejects_effectful_funptr_signature_before_hir -- --nocapture`
    - `cargo test -p scoopc abi_baseline_native_funptr_aggregate_return_uses_native_result_abi -- --nocapture`
    - `cargo clippy --all-targets -- -D warnings`
  - 与 `PLAN.md` / `MANAGED_ABI.md` 的对应闭合：
    - 对应 `PLAN.md` P1 第 1-4 项：callable ABI identity 已成为 HIR/LLVM 共用的内部 contract；declaration path、direct call 与 MIR direct call 不再靠 `extern_funs.contains_key(fqn)` 或分散的 effect/native 布尔链临时拼 ABI。
    - `MANAGED_ABI.md` §4.4 / §5.4 现已与实现对齐：ABI 身份是一等信息，`FunPtr<F>` 在当前阶段明确是 pure-only native callable token；`ManagedExtern` 仍只作为 `ExternAbi::Scoop` 的预留 family，并未在本任务提前落地用户可见 surface。

### [DONE] P1-T02：收紧 `FunPtr<F>` 合同：pure-only native surface

- 参考：
  - [`PLAN.md`](./PLAN.md) §5 / P1
  - [`MANAGED_ABI.md`](./MANAGED_ABI.md) §4.4、§5.4
- 目标：
  - 把 `FunPtr<F>` 收口为 pure-only native function pointer surface。
  - 让 typed call contract、effect facts、LLVM lowering 都不再为 `FunPtr` 保留 effect/state-machine 路径。
- 当前实现入口：
  - `sysroot/unsafe.scoop`
  - `crates/scoopc/src/typecheck/lower.rs`
  - `crates/scoopc/src/typecheck/expr/call.rs`
  - `crates/scoopc/src/pipeline/hir_stage.rs::TypedCallSiteContract::FunPtr`
  - `crates/scoopc/src/effect_facts/builder.rs`
  - `crates/scoopc/src/llvm/codegen/effect_lowered/{layout.rs,body.rs,value.rs}`
- 必须实现的内容：
  1. 扩展 `TypedCallSiteContract::FunPtr` 或等价 handoff，使其不再只携带 `callee_ty/return_ty/arg_binding`。
  2. 明确 `FunPtr` 的 published route 只有 native leaf family，并交给 P2 的 native classifier。
  3. 明确 `funPtrToUIntPtr` / `uintPtrToFunPtr` 只做地址 round-trip，不承载 effect/control bridge 语义。
  4. 推荐在这一阶段就补一条专门针对 `TypedCallSiteContract::FunPtr` native-only ABI family 的单测，避免后面再回退成“只有 `FunctionType` + effect path”。
- 必须遵从的约束：
  - 不得放宽 `tests/fixtures/typecheck/extern_fun_effectful_funptr_is_error.scoop` 这条 surface。
  - 不得让 `FunPtr` 调用重新落到 effect/state-machine lowering 路径。
- 验证：
  1. `cargo run -p scoop -- test --fixtures tests/fixtures/typecheck/extern_fun_effectful_funptr_is_error.scoop`
  2. `cargo run -p scoop -- test --fixtures tests/fixtures/typecheck/uintptr_to_funptr_effectful_type_arg_is_error.scoop`
  3. `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/unsafe_funptr_extern_call_basic.scoop`
- 完成条件：
  - `FunPtr` 已不再只靠 `FunctionType` 隐式承担 ABI family。
- 依赖：`P1-T01`
- 完成记录：
  - 改动范围：
    - `crates/scoopc/src/hir/mod.rs`
    - `crates/scoopc/src/pipeline/{hir_stage.rs,mir_stage.rs}`
    - `crates/scoopc/src/mir/{mod.rs,lower.rs,dump.rs,summary.rs,escape.rs,closure_simplify.rs,inline.rs,materialize.rs}`
    - `crates/scoopc/src/effect_facts/{facts.rs,builder.rs}`
    - `crates/scoopc/src/effect_lowered/{frame.rs,segment.rs,materialize.rs}`
    - `crates/scoopc/src/llvm/codegen/{mod.rs,mir_body.rs,intrinsics/sysroot.rs,effect_lowered/layout.rs,effect_lowered/body.rs,effect_lowered/value.rs,call/lowering.rs,call/abi.rs}`
    - `crates/scoopc/src/llvm/reachability.rs`
    - `TODO.md`
  - 核心决策：
    - 在 MIR 中新增显式 `CallKind::FunPtr`，并在 effect-facts 中新增对应的 `CallSiteKind::FunPtr`；`FunPtr` 调用不再伪装成通用 `FunValue`，从 typed contract 往后都显式发布 native-only route。
    - 新增 `FunPtrCallSpec`，把 `codegen_funptr_value_call_impl` / `codegen_mir_funptr_value_call` 上遗留的 `call_may_suspend`、explicit effect hidden ABI 参数和 outcome-slot 逻辑全部删掉，使 `FunPtr` lowering 只表达 plain native function-pointer call。
    - 现有 `TypedCallSiteContract::FunPtr` 继续发布 `CallableAbiIdentity::NativeExtern`，并通过新的 MIR `CallKind::FunPtr` 完成“ABI family 不再只靠 `FunctionType` 隐式承担”的 handoff；`hir_stage` stable dump 现在也会把 callable-value / `FunPtr` 的 `abi_identity` 打出来，便于审计。
    - `funPtrToUIntPtr` / `uintPtrToFunPtr` 仍然只做地址 round-trip；ABI family 不存进 token 本身，而是由 typed contract / MIR `CallKind::FunPtr` 在调用路径上重新显式附着。P2 继续负责把 direct `@Extern` 与 native `FunPtr` 收口到同一个 classifier / callconv / boundary scaffold。
  - 验证结果：
    - `cargo test -p scoopc funptr -- --nocapture`
    - `cargo test -p scoopc refactor_hir_call_contracts_record_callable_provenance -- --nocapture`
    - `cargo run -p scoop -- test --fixtures tests/fixtures/typecheck/extern_fun_effectful_funptr_is_error.scoop`
    - `cargo run -p scoop -- test --fixtures tests/fixtures/typecheck/uintptr_to_funptr_effectful_type_arg_is_error.scoop`
    - `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/unsafe_funptr_extern_call_basic.scoop`
    - `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/top_level_callable_value_call_basic.scoop`
    - `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/unsafe_funptr_receiver_call_basic.scoop`
    - `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/unsafe_funptr_aggregate_return_tuple.scoop`
    - `cargo clippy --all-targets -- -D warnings`
  - 与 `PLAN.md` / `MANAGED_ABI.md` 的对应闭合：
    - 对应 `PLAN.md` P1 第 3-4 项：`FunPtr` 现在不再通过 generic `FunValue` / effect-step 兼容形状向后传递，而是以显式 native-only handoff 进入 MIR、effect-facts 和 LLVM lowering；后续 P2 可以直接基于这条 route 去统一 native classifier，而不必再从 carrier type 或 effect 布尔值反推。
    - `MANAGED_ABI.md` §5.4 / §6.2a 先前已经要求 `FunPtr<F>` 只承载 native surface、effect ABI 永远不应混入 `FunPtr` 调用、`UIntPtr <-> FunPtr` round-trip 仅保留地址事实；本任务把实现补齐到该描述，因此无需额外文档回写。

## P2：统一 native surface

### [TODO] P2-T01：建立单一 native ABI classifier，统一 direct/indirect declaration 与 call scaffolding

- 参考：
  - [`PLAN.md`](./PLAN.md) §5 / P2
  - [`MANAGED_ABI.md`](./MANAGED_ABI.md) §6.2a、§10.1、§10.4
- 目标：
  - 让 direct `@Extern` 与 native `FunPtr` 共用同一个 native ABI classifier。
  - 统一 declaration / direct call / indirect call / MIR path 对参数、返回、aggregate return、callconv、`enter_native/leave_native`、`gc-leaf` 的决策。
- 当前实现入口：
  - `crates/scoopc/src/llvm/codegen/mod.rs::{declare_top_level_fun,declare_top_level_fun_with_signature_override}`
  - `crates/scoopc/src/llvm/codegen/call/lowering.rs::{codegen_top_level_fun_call_impl,emit_enter_native_for_extern_call_impl,emit_extern_native_call_impl,codegen_funptr_value_call_impl}`
  - `crates/scoopc/src/llvm/codegen/mir_body.rs::{codegen_mir_direct_call_with_policy,codegen_mir_funptr_value_call,codegen_mir_funptr_invoke_call}`
  - `crates/scoopc/src/llvm/codegen/call/abi.rs`
- 必须实现的内容：
  1. 推荐在 `crates/scoopc/src/llvm/codegen/call/abi.rs` 或相邻模块引入单一 native ABI classifier 结构，统一产出至少以下字段：
     - lowered param ABI；
     - lowered return ABI；
     - aggregate return mode；
     - LLVM callconv；
     - native boundary mode（是否 enter/leave native）；
     - 是否 `gc-leaf-function`；
     - effect boundary policy。
  2. 让 direct `@Extern` 与 native `FunPtr` 都通过该 classifier 走同一条 declaration/call 路径，而不是各自硬编码：
     - direct extern 走 `emit_extern_native_call_impl`
     - funptr 走 `with_conservative_gc_local_root_spills`
     这种长期分裂。
  3. `FunPtr` callsite 不得继续硬编码 `callconv 0`；即便当前只支持 `c/cdecl`，也必须通过 classifier 取得。
  4. 若 native `FunPtr` 最终也需要 `enter_native/leave_native`，必须一并补 direct/indirect parity fixture；若确定不需要，则必须在 `MANAGED_ABI.md` 与回归中写明理由，不能保持隐式分裂。
- 必须遵从的约束：
  - 不得重新引入 ordinary hidden sret 假扮 native aggregate return。
  - 不得让 `FunPtr` native call 重新混入 effect/state-machine classifier 输出。
- 验证：
  1. `cargo run -p scoop -- test --fixtures tests/fixtures/runtime_gc/extern_enter_native_roots_gc.scoop`
  2. `cargo run -p scoop -- test --fixtures tests/fixtures/build/extern_enter_native_no_statepoint_writeback.scoop`
  3. `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/unsafe_funptr_extern_call_basic.scoop`
  4. `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/unsafe_funptr_aggregate_return_tuple.scoop`
  5. `cargo test -p scoopc llvm_tests -- --nocapture`
- 完成条件：
  - native callable 的 lowering 不再取决于入口形状，而只取决于 classifier 给出的 ABI family 与 policy。
- 依赖：`P1-T02`

### [TODO] P2-T02：收口 native surface gate 与诊断，统一 `@Extern` / native `FunPtr` contract

- 参考：
  - [`PLAN.md`](./PLAN.md) §5 / P2
  - [`MANAGED_ABI.md`](./MANAGED_ABI.md) §5.1、§10.4
- 目标：
  - 让 `@Extern` 与 native `FunPtr` 共用同一 surface gate，而不是一个看 `GC-free`、一个只看“是不是函数类型”。
  - 把当前 repo 对 native aggregate / non-scalar surface 的真实 contract 明确下来。
- 当前实现入口：
  - `crates/scoopc/src/typecheck/annotations.rs::check_extern_fun_signature_is_gc_free`
  - `crates/scoopc/src/typecheck/lower.rs`（`FunPtr<F>` 形状检查）
  - `crates/scoopc/src/typecheck/expr/call.rs`
  - `tests/fixtures/run-pass/unsafe_funptr_aggregate_return_tuple.scoop`
  - `runtime/c/scoop_test.c`
- 必须实现的内容：
  1. 明确 current v1 native surface contract，并把它写进 typecheck / diagnostics / docs：
     - 默认推荐：在保留现有 aggregate native surface 的前提下，用 classifier-backed contract 正式化 current behavior；
     - 若实现者认为必须回退为更窄 allowlist，必须先更新 `MANAGED_ABI.md`、同步改现有 fixture，再落代码。
  2. 不再允许以下分裂：
     - `@Extern` 被拒绝，但等价 `FunPtr<F>` 仍可调用；
     - `@Extern` 只能过标量，`FunPtr` 却能静默过 aggregate；
     - non-pure `FunPtr<F>` 没有在前端被直接拒绝。
  3. 若保留 aggregate native surface，建议在 `runtime/c/scoop_test.c` 新增一个 direct extern aggregate helper，并补 direct vs indirect parity fixture，避免当前只有 `FunPtr` 一条回归。
  4. 将诊断从“GC-free 近似”升级为“当前 native ABI contract 不接受 / 只接受哪些形状”的明确文案；不要继续让用户只能看到 GC-free 代理错误。
- 必须遵从的约束：
  - 不得在没有设计回写的前提下，静默让 `unsafe_funptr_aggregate_return_tuple` 从 pass 变 reject。
  - 不得继续把 current contract 写成模糊的“看平台能不能过”。
- 验证：
  1. 现有 `@Extern` / `FunPtr` typecheck fixture
  2. `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/unsafe_funptr_aggregate_return_tuple.scoop`
  3. 新增的 direct vs indirect aggregate parity fixture（若保留 aggregate surface）
  4. `cargo test -p scoopc llvm_tests -- --nocapture`
- 完成条件：
  - `@Extern` 与 native `FunPtr` 共用同一 native surface contract 与诊断。
- 依赖：`P2-T01`

## P3：落地 `ExternAbi::Scoop`

### [TODO] P3-T01：扩展 `@Extern` 语法与 HIR，正式支持 `abi = "scoop"`

- 参考：
  - [`PLAN.md`](./PLAN.md) §5 / P3
  - [`MANAGED_ABI.md`](./MANAGED_ABI.md) §4.2、§4.3、§5.2
- 目标：
  - 给 `@Extern` 增加显式 ABI 字段，让前端/HIR 能稳定区分 `C` 与 `Scoop`。
- 当前实现入口：
  - `crates/scoopc/src/typecheck/annotations.rs`
  - `crates/scoopc/src/hir/lower/util.rs::{parse_extern_annotation_args,extern_fun_of_decl}`
  - `crates/scoopc/src/hir/mod.rs::ExternAbi`
- 必须实现的内容：
  1. 在 `@Extern` 参数中支持 `abi = "scoop"`（以及默认 `abi = "c"` / 省略时为 `C`）。
  2. 为无效 ABI 名、重复 `abi` 参数、`abi` 与位置参数/命名参数混用异常形状补清晰诊断。
  3. 扩展 `ExternAbi` 枚举与 HIR lowering，使 `ExternFun.abi` 真正能产出 `Scoop`。
  4. 推荐在这一阶段就明确 `@CallingConvention` 与 `abi = "scoop"` 的关系：
     - 若 v1 不支持自定义 callconv，直接 reject 并给诊断；
     - 不要默默忽略。
  5. 为 parser / typecheck / HIR 三层都补 fixture，避免后面只有 LLVM 测到才发现 annotation shape 漏洞。
- 必须遵从的约束：
  - 本任务只打通语法与 HIR，不提前把 `ExternAbi::Scoop` 混进 native leaf lowering。
  - 不得把 `@Extern("symbol")` 的位置参数语义改成 ABI 名；symbol name 仍是 symbol name。
- 验证：
  1. parser fixture：`@Extern(..., abi = "scoop")`
  2. typecheck fixture：无效 ABI / `@CallingConvention` 组合 / v1 限制
  3. HIR or lowering 单测：`ExternFun.abi == ExternAbi::Scoop`
- 完成条件：
  - `ExternAbi::Scoop` 已能稳定进入 HIR side table。
- 依赖：`P2-T02`

### [TODO] P3-T02：接通 `ExternAbi::Scoop` 的 declaration / call lowering，并补 IR/run-pass 回归

- 参考：
  - [`PLAN.md`](./PLAN.md) §5 / P3
  - [`MANAGED_ABI.md`](./MANAGED_ABI.md) §3、§6、§10.1、§10.2
- 目标：
  - 让 imported `ExternAbi::Scoop` 顶层函数能走 ordinary managed ABI，而不是 native leaf。
  - 用最小 external managed helper 验证 declaration / call / GC-root contract。
- 当前实现入口：
  - `crates/scoopc/src/llvm/codegen/mod.rs`
  - `crates/scoopc/src/llvm/codegen/call/lowering.rs`
  - `crates/scoopc/src/llvm/codegen/mir_body.rs`
  - 建议新增 fixture 位置：
    - `tests/fixtures/build/managed_abi_*`
    - `tests/fixtures/run_pass_cone/managed_abi_*`
- 必须实现的内容：
  1. declaration path：`ExternAbi::Scoop` 使用 external linkage + ordinary param ABI + ordinary return/hidden sret + no `gc-leaf-function`。
  2. call lowering：`ExternAbi::Scoop` 走 ordinary managed call + conservative root spill + statepoint correctness + no `enter_native/leave_native`。
  3. 用一个最小 external managed helper 建立第一条 IR/run-pass 回归。
     - 推荐从简单的 `Int -> Int` 或 `Int -> String` helper 开始；
     - 若选择 `String` 返回值，应同时验证 live GC roots 在 helper 内部分配后仍正确。
  4. 为 hidden sret case 再补一条 IR regression，证明 imported managed external aggregate return 走 ordinary sret，而不是 native ABI。
- 必须遵从的约束：
  - 不得把 `ExternAbi::Scoop` 视为“无 enter_native 的 extern C ABI”。
  - 不得让 `ExternAbi::Scoop` 复用 `@Extern` 的 GC-free gate。
- 验证：
  1. `cargo test -p scoopc llvm_tests -- --nocapture`
  2. `cargo run -p scoop -- test --fixtures tests/fixtures/build/managed_abi_*`
  3. `cargo run -p scoop -- test --fixtures tests/fixtures/run_pass_cone/managed_abi_*`
- 完成条件：
  - imported managed external helper 已具备正式 ABI surface，可作为 P4 tracer bullet 的承载路径。
- 依赖：`P3-T01`

## P4：string cone tracer bullet

### [TODO] P4-T01：以标量 `toString` 为 tracer bullet，删除第一批字符串名字特判

- 参考：
  - [`PLAN.md`](./PLAN.md) §5 / P4
  - [`MANAGED_ABI.md`](./MANAGED_ABI.md) §9、§10
- 目标：
  - 先用最小且高频的 string helper 证明“编译器已按 ABI 而不是按名字工作”。
  - 第一批目标限定为标量/简单自返 helpers，避免一开始就把所有 string 逻辑绑在一起。
- 当前实现入口：
  - `sysroot/core.scoop`
  - `crates/scoopc/src/resolve/scopes.rs`
  - `crates/scoopc/src/typecheck/expr/call.rs`
  - `crates/scoopc/src/llvm/codegen/intrinsics/builtin.rs::{try_codegen_tostring_iface_builtin,codegen_sysroot_to_string_ext}`
  - `crates/scoopc/src/llvm/codegen/effect_lowered/body.rs::lower_builtin_to_string_call`
  - `crates/scoopc/src/llvm/codegen/effect_lowered/value.rs::{lower_refactor_core_to_string_call,refactor_core_print_to_string}`
  - `runtime/c/scoop_runtime.c` 中 `scoop_{bool,char,int,float32,float64}_to_string`
- 必须实现的内容：
  1. 先迁：
     - `Int.toString`
     - `Bool.toString`
     - `Char.toString`
     - `Float32.toString`
     - `Float64.toString`
     - `String.toString`
  2. 对应清理以下层的名字特判：
     - resolver builtin allowlist；
     - typecheck 中的内建 member 直接返回类型；
     - codegen builtin intercept；
     - effect-lowered `toString` runtime route。
  3. 保持 `ToString.toString` interface dispatch 对用户类型仍可用，但 builtin scalar 不再靠 FQN/runtime intercept 直达。
  4. 同步检查 `print/println` 链路，确保它们最终也不再依赖旧的 scalar `toString` runtime intercept。
- 必须遵从的约束：
  - 不得留下“新 `ExternAbi::Scoop` 路径已接通，但旧 `try_codegen_tostring_iface_builtin` 仍完整保留”的双轨。
  - 不得回退 `Float.toString` 现有 IR / run-pass coverage。
- 验证：
  1. 现有 `llvm/tests.rs` 中 `Float64.toString` / `Float32.toString` / `ToString.toString` 相关单测
  2. 新增 `managed_abi` 版 scalar toString build/run-pass fixture
  3. `SCOOP_GC_MOVE=1 SCOOP_GC_STRESS=1 cargo run -p scoop -- test --fixtures <scalar-tostring-fixtures>`
- 完成条件：
  - 第一批标量 `toString` helper 已不再依赖 compiler/runtime FQN intercept。
- 依赖：`P3-T02`

### [TODO] P4-T02：迁移 remaining string helper，明确 substrate 边界并收缩 runtime surface

- 参考：
  - [`PLAN.md`](./PLAN.md) §5 / P4
  - [`MANAGED_ABI.md`](./MANAGED_ABI.md) §9.2、§9.3、§10.3
- 目标：
  - 在 scalar toString tracer bullet 成功后，把 remaining string helper 系统性迁走。
  - 同时明确哪些 string primitive 仍属于 runtime substrate，哪些已经变成普通 helper。
- 当前实现入口：
  - `sysroot/core.scoop`
  - `crates/scoopc/src/resolve/scopes.rs`
  - `crates/scoopc/src/typecheck/expr/call.rs`
  - `crates/scoopc/src/hir/lower/expr.rs::should_keep_member_call_as_member_access`
  - `crates/scoopc/src/llvm/codegen/call/lowering.rs::{codegen_string_trim_indent,codegen_string_method}`
  - `crates/scoopc/src/llvm/codegen/effect_lowered/body.rs::lower_builtin_string_concat_call`
  - `crates/scoopc/src/llvm/codegen/effect_lowered/value.rs::{lower_refactor_string_concat_call,lower_refactor_core_string_concat_call,lower_refactor_core_string_compare_to_call,lower_refactor_core_string_trim_indent_call,lower_refactor_core_string_is_empty_call,lower_refactor_core_string_replace_call,lower_refactor_core_string_char_at_call,lower_refactor_core_string_repeat_call,lower_refactor_core_string_byte_length_call}`
  - `crates/scoopc/src/llvm/codegen/runtime_abi.rs`
  - `runtime/c/scoop_runtime.c`
- 必须实现的内容：
  1. 迁移明确不是 substrate 的 string helper：
     - `String.toInt`
     - `String.concat`
     - `String.hash`
     - `String.isEmpty`
     - `String.replace`
     - `String.charAt`
     - `String.repeat`
     - `String.compareTo`
     - `String.trimIndent`
  2. 对 `String.length` / `byteLength` / `getByte` / `unsafeSliceBytes`，必须显式给出分类结论：
     - 若保留在 runtime substrate，要在文档和注释中列成 authoritative list；
     - 若迁移出去，要像其它 helper 一样同步清掉 special-case。
  3. 每迁一项都要同步删掉：
     - resolver allowlist；
     - typecheck synthetic contract；
     - HIR member-access keep-list；
     - LLVM FQN/member-name intercept；
     - runtime declare / symbol / C helper。
  4. 更新 `sysroot/string.scoop` / `sysroot/core.scoop`，让 remaining helper 的实现归属清晰可读，不再靠散落注释暗示。
- 必须遵从的约束：
  - 不得把已经迁好的 `sysroot/string.scoop` helper 再搬回 runtime。
  - 若某个 helper 仍保留在 runtime，必须给出“它为什么是 substrate，而不是暂时没迁完”的明确理由。
- 验证：
  1. `cargo test -p scoopc llvm_tests -- --nocapture`
  2. `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass`
  3. 针对 migrated string helper 的 GC stress / move GC 回归
- 完成条件：
  - remaining string helper 的归属已清晰：要么是 `ExternAbi::Scoop` / ordinary sysroot helper，要么被明确列为 runtime substrate。
- 依赖：`P4-T01`

## P5：稳定化与收尾

### [TODO] P5-T01：做全量稳定化、跨平台矩阵与文档收尾

- 参考：
  - [`PLAN.md`](./PLAN.md) §5 / P5
  - [`MANAGED_ABI.md`](./MANAGED_ABI.md) §10、§11、§12
- 目标：
  - 把 native surface 与 managed external surface 的 contract 全量锁进测试、文档和注释。
  - 让后续 agent 不再需要重新解释“当前 repo 究竟允许哪些 ABI surface”。
- 当前实现入口：
  - `MANAGED_ABI.md`
  - `PLAN.md`
  - `TODO.md`
  - `sysroot/core.scoop`
  - `sysroot/unsafe.scoop`
  - `crates/scoopc/src/llvm/codegen/runtime_abi.rs`
  - `runtime/c/scoop_runtime.c`
  - 所有 ABI 相关 fixture / `llvm_tests`
- 必须实现的内容：
  1. 全量回归：
     - `cargo test --all --all-targets`
     - `cargo run -p scoop -- test`
     - native direct/indirect parity suite
     - `ExternAbi::Scoop` build/run-pass suite
  2. 跨平台 matrix：
     - `linux/amd64`
     - `macos/aarch64`
  3. 回写文档与注释：
     - 若 P2 的 native surface contract 与 `MANAGED_ABI.md` §5.1 不同，必须更新 `MANAGED_ABI.md`；
     - 更新 sysroot/runtime 注释，去掉已失效的“body-less / runtime route”说明；
     - 在 `PLAN.md` / `TODO.md` 中把 remaining substrate list 写成最终状态。
  4. 给 v2+ 留清晰边界：
     - managed external function pointer；
     - 泛型导入/导出；
     - outward effect / continuation ABI；
     - ABI version / feature bitmap。
- 必须遵从的约束：
  - 不得把 v1 未闭环的问题统统推到 v2+。
  - 若还有 remaining native/string special-case，必须说明它是 substrate 还是 bug，而不是模糊写成“以后再收”。
- 验证：
  1. `cargo test --all --all-targets`
  2. `cargo run -p scoop -- test`
  3. `cargo test -p scoopc llvm_tests -- --nocapture`
  4. CI / 手工跨平台矩阵
- 完成条件：
  - native callable ABI 与 managed external ABI 都已经成为明确、可回归、跨平台可审计的 contract。
- 依赖：`P4-T02`
