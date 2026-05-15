# TODO（Managed ABI / native callable ABI 收口）

> 生成时间：2026-05-14  
> 计划基线：[`PLAN.md`](./PLAN.md)  
> 设计基线：[`MANAGED_ABI.md`](./MANAGED_ABI.md)  
> 格式参考：`docs/archive/plans/TODO-pipeline-gaps.md`、`docs/archive/plans/PLAN-pipeline-gaps.md`  
> 顺序约束：严格按当前文件中的条目顺序推进；不得跨条目并行实现。  
> 当前状态：`ExternAbi::Scoop` 已完成 declaration / call lowering 与 ABI-specific `unsafe` / `@NoGC` 语义；native `@Extern` 与 `FunPtr` 已通过统一 native ABI classifier + surface gate / diagnostics 对齐 declaration / direct call / indirect call / MIR path；`string cone` 只完成了 `sysroot/string.scoop` 中那批普通 helper，remaining string helper 仍依赖 compiler/runtime 特判。  
> 重要提醒：当前 mainline 已锁住 `unsafe_funptr_aggregate_return_tuple`、`extern_fun_effectful_funptr_is_error`、`single_file_minimal_ir_includes_compilable_sysroot_string_helpers` 等行为。除非先回写 `MANAGED_ABI.md` 与相关 fixture，否则不得把这些 current behavior 直接回退掉。

## 任务索引

| ID | 阶段 | 标题 |
| --- | --- | --- |
| `P0-T01` | P0 | [DONE] 冻结 current ABI baseline 与 regression owner map |
| `P1-T01` | P1 | [DONE] 建立 callable ABI identity，并让 `ExternFun.abi` 真正进入 lowering source of truth |
| `P1-T02` | P1 | [DONE] 收紧 `FunPtr<F>` 合同：pure-only native surface |
| `P2-T01` | P2 | [DONE] 建立单一 native ABI classifier，统一 direct/indirect declaration 与 call scaffolding |
| `P2-T02` | P2 | [DONE] 收口 native surface gate 与诊断，统一 `@Extern` / native `FunPtr` contract |
| `P3-T01` | P3 | [DONE] 扩展 `@Extern` 语法与 HIR，正式支持 `abi = "scoop"` |
| `P3-T02` | P3 | [DONE] 接通 `ExternAbi::Scoop` 的 declaration / call lowering，并补 IR/run-pass 回归 |
| `P4-T01a` | P4 前置 I | [DONE] 解锁 struct/class（含 generic class）instance method 的常规 `receiver.method()` 调用 |
| `P4-T01b` | P4 前置 I | [DONE] vtable / itable 收集统一从 struct/class body method 抽，interface call ABI 收口为 metadata-driven marshal |
| `P4-T01c-pre1` | P4 前置 I | [DONE] 为 P4-T01c 引入 sysroot overlay fixture 能力，允许 task-specific `core.scoop` 改写 |
| `P4-T01c` | P4 前置 I | [DONE] `@Intrinsic struct/class` 含 method body 完整落地（含 generic class），并锁定 non-generic 维度零编译器后门 |
| `P4-T01d` | P4 前置 II | [DONE] 引入 method-level `@Intrinsic("name")` 与可枚举 intrinsic 表机制（IR-emission / RuntimeCall 双模，默认 IR-emission） |
| `P4-T01e` | P4 前置 II | 用 `Array` / `MutableArray` 填充 intrinsic 表（IR-direct），删除已被替代的 array runtime helper |
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
- `@Extern` 的 `@Unsafe` / `@NoGC` 语义由 ABI 决定：默认/`abi = "c"` 隐含两者，`abi = "scoop"` 不隐含两者；无论哪种 ABI，`@Extern` 都不允许显式再叠加 `@Unsafe` / `@NoGC`。
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

### [DONE] P2-T01：建立单一 native ABI classifier，统一 direct/indirect declaration 与 call scaffolding

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
- 完成记录：
  - 改动范围：
    - `crates/scoopc/src/llvm/codegen/{mod.rs,call/abi.rs,call/lowering.rs,mir_body.rs}`
    - `crates/scoopc/src/llvm/tests.rs`
    - `runtime/c/scoop_test.c`
    - `tests/fixtures/{runtime_gc/funptr_enter_native_roots_gc.*,build/funptr_enter_native_no_statepoint_writeback.scoop,run-pass/extern_native_aggregate_return_direct_indirect_parity.*}`
    - `MANAGED_ABI.md`
    - `TODO.md`
  - 核心决策：
    - 在 LLVM 共享层新增 `NativeCallableAbi` classifier，统一发布 native callable 的 param/return lowered ABI、aggregate return mode、LLVM callconv、native boundary mode、`gc-leaf-function` 决策与 effect boundary policy；direct `@Extern` declaration/call、HIR funptr call、MIR direct native call、MIR funptr call 现在都复用同一 source of truth。
    - native `FunPtr` 调用不再手写 `callconv 0` / `with_conservative_gc_local_root_spills` / `hidden_sret = None` 分支，而是改为通过 unified native boundary scaffold 插入 `enter_native/leave_native`；direct `@Extern` 与 native `FunPtr` 现在对同一 native callable family 共享 target aggregate-return ABI 与 boundary 语义。
    - 导出 `scoop_test_make_int_pair` 作为 direct/indirect aggregate parity helper，并新增 `scoop_test_get_gc_collect_in_native_funptr` 作为 indirect native boundary 的 runtime_gc 承载；用 runtime_gc/build/run-pass/LLVM tests 同时锁住 funptr roots 暴露、no-statepoint-writeback 与 aggregate-return parity。
  - 验证结果：
    - `cargo fmt --all`
    - `cargo test -p scoopc native_callable -- --nocapture`
    - `cargo run -p scoop -- test --fixtures tests/fixtures/runtime_gc/extern_enter_native_roots_gc.scoop`
    - `cargo run -p scoop -- test --fixtures tests/fixtures/build/extern_enter_native_no_statepoint_writeback.scoop`
    - `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/unsafe_funptr_extern_call_basic.scoop`
    - `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/unsafe_funptr_aggregate_return_tuple.scoop`
    - `cargo run -p scoop -- test --fixtures tests/fixtures/runtime_gc/funptr_enter_native_roots_gc.scoop`
    - `cargo run -p scoop -- test --fixtures tests/fixtures/build/funptr_enter_native_no_statepoint_writeback.scoop`
    - `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/extern_native_aggregate_return_direct_indirect_parity.scoop`
    - `cargo test -p scoopc llvm_tests -- --nocapture`（当前 harness 下该 filter 返回 `0 passed; 845 filtered out`，因此额外按 owner 测试名补跑下面两条）
    - `cargo test -p scoopc abi_baseline -- --nocapture`
    - `cargo test -p scoopc native_callable -- --nocapture`
    - `cargo clippy --all-targets -- -D warnings`
  - 与 `PLAN.md` / `MANAGED_ABI.md` 的对应闭合：
    - 对应 `PLAN.md` P2 第 1、4、5 项：single native classifier 已进入 declaration / direct call / indirect call / MIR path，native `FunPtr` 不再绕过 direct extern boundary scaffold，direct/indirect parity 现在有 runtime_gc/build/run-pass/LLVM owner 覆盖。
    - `MANAGED_ABI.md` §1.5 / §6.2a / §10.1 / §10.4 现已与实现对齐：direct `@Extern` 与 native `FunPtr` 共享同一 native callable classifier，并统一采用 `enter_native/leave_native` + target aggregate-return ABI；native surface gate / diagnostics 仍留待 `P2-T02` 收口。

### [DONE] P2-T02：收口 native surface gate 与诊断，统一 `@Extern` / native `FunPtr` contract

- 参考：
  - [`PLAN.md`](./PLAN.md) §5 / P2
  - [`MANAGED_ABI.md`](./MANAGED_ABI.md) §5.1、§10.4
- 目标：
  - 让 `@Extern` 与 native `FunPtr` 共用同一 surface gate，而不是一个看 `GC-free`、一个只看“是不是函数类型”。
  - 把当前 repo 对 native aggregate / non-scalar surface 的真实 contract 明确下来。
- 当前实现入口：
  - `crates/scoopc/src/typecheck/annotations.rs::check_extern_fun_signature_matches_native_abi`
  - `crates/scoopc/src/typecheck/lower.rs`（`FunPtr<F>` native surface contract 检查）
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
- 完成记录：
  - 改动范围：
    - `crates/scoopc/src/typecheck/{annotations.rs,lower.rs,expr/call.rs}`
    - `sysroot/unsafe.scoop`
    - `tests/fixtures/typecheck/{extern_fun_signature_with_gc_ref_is_error.scoop,extern_fun_signature_with_continuation_is_error.scoop,extern_fun_signature_with_pinned_is_error.scoop,extern_fun_signature_with_clayout_struct_ok.scoop,extern_fun_signature_with_plain_struct_is_error.scoop,uintptr_to_funptr_plain_struct_type_arg_is_error.scoop}`
    - `MANAGED_ABI.md`
    - `TODO.md`
  - 核心决策：
    - 在 `TypeLowering` 中新增 shared native value surface classifier，直接供 `@Extern` signature gate 与 `FunPtr<F>` signature contract 复用；不再让 `@Extern` 只看 `GC-free`、`FunPtr` 只看“是不是 pure 函数类型”。
    - 当前 v1 native surface 正式收口为：标量、`UIntPtr`、`Ptr<T>`、纯 `FunPtr<F>` token、tuple、以及字段递归满足同一 contract 的 `@CLayout` struct；普通非 `@CLayout` nominal aggregate、GC ref、`Pinned` / `GcHandle`、rich enum / `Option<T>` 等未固定 native value layout 的类型会在前端被直接拒绝。
    - 保留现有 tuple aggregate native surface：`unsafe_funptr_aggregate_return_tuple` 与 `extern_native_aggregate_return_direct_indirect_parity` 继续作为 direct / indirect target-ABI aggregate return owner；本任务没有回退 aggregate surface，只把它从“GC-free 近似”提升为 explicit contract。
    - 诊断文案升级为“当前 native ABI contract 允许 / 不允许哪些形状”的明确提示，并继续给出 `GcHandle.raw: UIntPtr`、`GC.pin/unpin` + `Ptr<T>` 的桥接建议。
  - 验证结果：
    - `cargo fmt --all`
    - `cargo run -p scoop -- test --fixtures tests/fixtures/typecheck/extern_fun_signature_with_gc_ref_is_error.scoop`
    - `cargo run -p scoop -- test --fixtures tests/fixtures/typecheck/extern_fun_signature_with_continuation_is_error.scoop`
    - `cargo run -p scoop -- test --fixtures tests/fixtures/typecheck/extern_fun_signature_with_pinned_is_error.scoop`
    - `cargo run -p scoop -- test --fixtures tests/fixtures/typecheck/extern_fun_signature_with_clayout_struct_ok.scoop`
    - `cargo run -p scoop -- test --fixtures tests/fixtures/typecheck/extern_fun_signature_with_plain_struct_is_error.scoop`
    - `cargo run -p scoop -- test --fixtures tests/fixtures/typecheck/extern_fun_effectful_funptr_is_error.scoop`
    - `cargo run -p scoop -- test --fixtures tests/fixtures/typecheck/uintptr_to_funptr_effectful_type_arg_is_error.scoop`
    - `cargo run -p scoop -- test --fixtures tests/fixtures/typecheck/uintptr_to_funptr_plain_struct_type_arg_is_error.scoop`
    - `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/unsafe_funptr_extern_call_basic.scoop`
    - `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/unsafe_funptr_aggregate_return_tuple.scoop`
    - `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/extern_native_aggregate_return_direct_indirect_parity.scoop`
    - `cargo test -p scoopc llvm_tests -- --nocapture`（当前 harness 下该 filter 返回 `0 passed; 845 filtered out`，因此额外按 owner 测试名补跑下面两条）
    - `cargo test -p scoopc native_callable -- --nocapture`
    - `cargo test -p scoopc abi_baseline_native_funptr_aggregate_return_uses_native_result_abi -- --nocapture`
    - `cargo clippy --all-targets -- -D warnings`
  - 与 `PLAN.md` / `MANAGED_ABI.md` 的对应闭合：
    - 对应 `PLAN.md` P2 第 2-4 项：native surface 现在已有 explicit value contract，`@Extern` 与 native `FunPtr` 共用同一 front-end gate / diagnostics，non-pure 与 non-native-shape `FunPtr<F>` 都会在前端被直接拒绝。
    - `MANAGED_ABI.md` §1.4 / §5.1 / §5.4 现已与实现对齐：tuple aggregate native surface 被显式保留，普通非 `@CLayout` nominal aggregate 不再因“GC-free”被默认放行；native token bridging 继续通过 `UIntPtr` / `Ptr<T>` / pure `FunPtr<F>` surface 明确表达。

## P3：落地 `ExternAbi::Scoop`

- 已确认语义：`@Extern` 的 `unsafe` / `@NoGC` 不再由独立注解决定，而是由 ABI 决定。
- 默认/`abi = "c"` 继续隐含 `@Unsafe + @NoGC`；`abi = "scoop"` 不隐含这两者。
- `@Extern` 无论哪种 ABI 都不得显式再写 `@Unsafe` / `@NoGC`。

### [DONE] P3-T01：扩展 `@Extern` 语法与 HIR，正式支持 `abi = "scoop"`

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
- 完成记录：
  - 改动范围：
    - `crates/scoopc/src/hir/{mod.rs,lower/util.rs,lower/mod.rs}`
    - `crates/scoopc/src/typecheck/annotations.rs`
    - `tests/fixtures/parse/extern_fun_scoop_abi_basic.{scoop,ast}`
    - `tests/fixtures/typecheck/extern_fun_scoop_abi_*`
    - `memory/claude_plan.md`
    - `TODO.md`
  - 核心决策：
    - 在共享 HIR 层把 `ExternAbi` 扩展为 `C` / `Scoop`，并让 `CallableAbiIdentity::from_extern_abi(...)` 正式区分 `NativeExtern` 与 `ManagedExtern`；`hir::lower::util` 现在会从 `@Extern(..., abi = "...")` 直接产出 `ExternFun.abi`，省略时默认仍是 `C`。
    - `@Extern` 参数校验已正式接受 `abi = "scoop"`，并补上无效 ABI 名、重复 `abi`、以及 `abi` 与位置/命名参数异常混用的前端诊断；`abi` 目前只允许用于函数声明，避免把尚未定义的 surface 扩展到 extern val/object。
    - `abi = "scoop"` 的 v1 前端门禁先在本任务收紧为：Pure-only、无 receiver 的顶层函数、无泛型、无 direct function-value / continuation surface；同时明确 `@CallingConvention` 对 Managed ABI 当前无效，会在前端直接拒绝，而不会默默忽略。
    - 本任务只打通语法、typecheck 与 HIR side table；没有提前修改 `ExternAbi::Scoop` 的 LLVM declaration / call lowering，这部分仍由 `P3-T02` 继续完成。
  - 验证结果：
    - `cargo test -p scoopc refactor_hir_collects_scoop_extern_abi_metadata -- --nocapture`
    - `cargo run -p scoop -- test --fixtures tests/fixtures/parse/extern_fun_scoop_abi_basic.scoop`
    - `cargo run -p scoop -- test --fixtures tests/fixtures/typecheck/extern_fun_scoop_abi_invalid_is_error.scoop`
    - `cargo run -p scoop -- test --fixtures tests/fixtures/typecheck/extern_fun_scoop_abi_duplicate_is_error.scoop`
    - `cargo run -p scoop -- test --fixtures tests/fixtures/typecheck/extern_fun_scoop_abi_positional_after_named_is_error.scoop`
    - `cargo run -p scoop -- test --fixtures tests/fixtures/typecheck/extern_fun_scoop_abi_calling_convention_is_error.scoop`
    - `cargo run -p scoop -- test --fixtures tests/fixtures/typecheck/extern_fun_scoop_abi_receiver_is_error.scoop`
    - `cargo run -p scoop -- test --fixtures tests/fixtures/typecheck/extern_fun_scoop_abi_generics_not_supported_is_error.scoop`
    - `cargo run -p scoop -- test --fixtures tests/fixtures/typecheck/extern_fun_scoop_abi_callable_surface_not_supported_is_error.scoop`
    - `cargo fmt --all`
    - `cargo clippy --all-targets -- -D warnings`
  - 与 `PLAN.md` / `MANAGED_ABI.md` 的对应闭合：
    - 对应 `PLAN.md` P3 第 1 项：`@Extern` 现已具备显式 ABI 字段，前端/HIR 可以稳定区分 `C` 与 `Scoop`，并把 ABI 身份写进 `ExternFun.abi` side table。
    - `MANAGED_ABI.md` §4.2 / §4.3 / §5.2 的 v1 边界已与实现对齐：`abi = "scoop"` 当前只是 managed extern metadata + front-end gate，不是 machine calling convention 扩展点，也尚未在本任务提前接入 native leaf lowering。

### [DONE] P3-T02：接通 `ExternAbi::Scoop` 的 declaration / call lowering，并补 IR/run-pass 回归

- 参考：
  - [`PLAN.md`](./PLAN.md) §5 / P3
  - [`MANAGED_ABI.md`](./MANAGED_ABI.md) §3、§6、§10.1、§10.2
- 目标：
  - 让 imported `ExternAbi::Scoop` 顶层函数能走 ordinary managed ABI，而不是 native leaf。
  - 把 `@Extern` 的 ABI-specific `unsafe` / `@NoGC` 语义正式固定下来。
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
  3. annotation / typecheck contract：
     - `abi = "c"`（含省略时默认）继续隐含 `@Unsafe + @NoGC`；
     - `abi = "scoop"` 不隐含 `@Unsafe`、也不隐含 `@NoGC`；
     - `@Extern` 无论哪种 ABI 都不允许显式再叠加 `@Unsafe` / `@NoGC`。
  4. 用一个最小 external managed helper 建立第一条 IR/run-pass 回归。
     - 推荐从简单的 `Int -> Int` 或 `Int -> String` helper 开始；
     - 若选择 `String` 返回值，应同时验证 live GC roots 在 helper 内部分配后仍正确。
  5. 为 hidden sret case 再补一条 IR regression，证明 imported managed external aggregate return 走 ordinary sret，而不是 native ABI。
- 必须遵从的约束：
  - 不得把 `ExternAbi::Scoop` 视为“无 enter_native 的 extern C ABI”。
  - 不得让 `ExternAbi::Scoop` 复用 `@Extern` 的 GC-free gate。
  - `ExternAbi::Scoop` 调用不需要 `unsafe context`；`@NoGC` 语义只对 `ExternAbi::C` 放行。
  - `@Extern` 无论 ABI 为何都必须拒绝显式 `@Unsafe` / `@NoGC`。
- 验证：
  1. `cargo test -p scoopc managed_extern_ -- --nocapture`
  2. `cargo run -p scoop -- test --fixtures tests/fixtures/build/managed_abi_aggregate_return_hidden_sret.scoop`
  3. `cargo run -p scoop -- test --fixtures tests/fixtures/build/managed_abi_direct_call_ordinary_contract.scoop`
  4. `cargo run -p scoop -- test --fixtures tests/fixtures/run_pass_cone/managed_abi_string_gc`
  5. `cargo run -p scoop -- test --fixtures tests/fixtures/typecheck/extern_fun_scoop_abi_nogc_call_is_error.scoop`
  6. `cargo run -p scoop -- test --fixtures tests/fixtures/typecheck/extern_fun_default_abi_unsafe_redundant_is_error.scoop`
  7. `cargo run -p scoop -- test --fixtures tests/fixtures/typecheck/extern_fun_c_abi_nogc_redundant_is_error.scoop`
  8. `cargo run -p scoop -- test --fixtures tests/fixtures/typecheck/extern_fun_scoop_abi_unsafe_not_supported_is_error.scoop`
  9. `cargo run -p scoop -- test --fixtures tests/fixtures/typecheck/extern_fun_scoop_abi_nogc_not_supported_is_error.scoop`
  10. `cargo clippy --all-targets -- -D warnings`
- 完成条件：
  - imported managed external helper 已具备正式 ABI surface，可作为 P4 tracer bullet 的承载路径。
- 依赖：`P3-T01`
- 完成记录：
  - 改动范围：
    - `crates/scoopc/src/typecheck/{annotations.rs,builtin_annotations.rs,expr/{call.rs,error.rs,mod.rs},lower.rs}`
    - `crates/scoopc/src/resolve/mod.rs`
    - `crates/scoopc/src/llvm/codegen/effect_lowered/value.rs`
    - `crates/scoopc/src/llvm/tests.rs`
    - `tests/fixtures/{build/managed_abi_aggregate_return_hidden_sret.scoop,build/managed_abi_direct_call_ordinary_contract.scoop,run_pass_cone/managed_abi_string_gc/**,typecheck/extern_fun_*}`
    - `MANAGED_ABI.md`、`PLAN.md`、`TODO.md`、`docs/spec/language_spec-part6.md`、`SCOOP_FULL_SPEC.md`、`memory/claude_plan.md`
  - 核心决策：
    - `ExternAbi::Scoop` 的 direct/materialized direct call 路径现已在 effect-lowered 调用入口按 ABI family 分流到 ordinary managed `codegen_mir_direct_call`，而不是继续复用 native leaf 分支。
    - managed extern declaration/direct call/aggregate return 现已由 LLVM 单测、build fixture 与 run-pass cone fixture 共同锁定：返回值走 ordinary managed surface 或 hidden sret，不插 `enter_native/leave_native`，也不再打 `gc-leaf-function`。
    - `@Extern` 的 `@Unsafe/@NoGC` 语义现已完全 ABI 化：`abi = "c"`（含省略时默认）隐含两者且拒绝重复标注，`abi = "scoop"` 不隐含两者且显式叠加会在前端报错；调用门禁与 side table 只把 native `@Extern` 当作 unsafe / `@NoGC` leaf。
  - 验证结果：
    - `cargo fmt --all`
    - `cargo run -p scoop -- test --fixtures tests/fixtures/typecheck/extern_fun_default_abi_unsafe_redundant_is_error.scoop`
    - `cargo run -p scoop -- test --fixtures tests/fixtures/typecheck/extern_fun_c_abi_nogc_redundant_is_error.scoop`
    - `cargo run -p scoop -- test --fixtures tests/fixtures/typecheck/extern_fun_scoop_abi_unsafe_not_supported_is_error.scoop`
    - `cargo run -p scoop -- test --fixtures tests/fixtures/typecheck/extern_fun_scoop_abi_nogc_not_supported_is_error.scoop`
    - `cargo run -p scoop -- test --fixtures tests/fixtures/typecheck/extern_fun_scoop_abi_nogc_call_is_error.scoop`
    - `cargo run -p scoop -- test --fixtures tests/fixtures/build/managed_abi_aggregate_return_hidden_sret.scoop`
    - `cargo run -p scoop -- test --fixtures tests/fixtures/build/managed_abi_direct_call_ordinary_contract.scoop`
    - `cargo run -p scoop -- test --fixtures tests/fixtures/run_pass_cone/managed_abi_string_gc`
    - `cargo test -p scoopc managed_extern_ -- --nocapture`
    - `cargo clippy --all-targets -- -D warnings`
  - 与 `PLAN.md` / `MANAGED_ABI.md` 的对应闭合：
    - 对应 `PLAN.md` P3 第 3-5 项：`ExternAbi::Scoop` 现已具备 external linkage 下的 ordinary managed declaration / call lowering，aggregate return 走 hidden sret，且 ABI-specific `unsafe` / `@NoGC` 语义已在前端与调用门禁完成收口。
    - `MANAGED_ABI.md` §3 / §5.2 / §5.4 / §6 / §10.1 / §10.2 已与实现对齐：managed extern 不再复用 native leaf scaffold，`FunPtr` 继续保持 native-only，binary-boundary 上的 `Pure` / no outward suspend 约束维持不变。

## P4 前置 I：内建类型 interface 实现机制

> 起因与设计基线：[`PLAN.md`](./PLAN.md) §5 / "P4 前置 I. 内建类型 interface 实现机制"。
>
> 目标是把内建类型从"扩展函数 + codegen by-name 拦截"升级为**一等的 struct/class implementer**：唯一的特殊性是 layout 由编译器内置，其它（method body / interface impl / vtable / itable）和用户自定义类型完全同构。
>
> 硬约束（"零编译器后门"，non-generic 维度）：本前置阶段必须保证，未来再加任何新的非 generic 内建 method（含 interface override 与独立 method）都不会再要求改编译器；只动 sysroot 即可。generic-by-T 维度的后门（不可避免）由 P4 前置 II 收敛为可枚举 intrinsic 表。
>
> 顺序约束：P4-T01a → P4-T01b → P4-T01c-pre1 → P4-T01c → P4-T01d → P4-T01e → P4-T01。六个前置子任务必须**新增机制不删除现有 path**（P4-T01e 例外：它会在 IR-direct 化完成后**删除**已被替代的 array runtime helper，这是显式 substrate 收缩动作；除此之外不做删除），以免与 P4-T01 的删除动作冲突造成中间双轨状态。

### [DONE] P4-T01a：解锁 struct/class（含 generic class）instance method 的常规 `receiver.method()` 调用

- 参考：
  - [`PLAN.md`](./PLAN.md) §5 / "P4 前置"
  - `sysroot/core.scoop` 中关于"当前阶段 typecheck 尚未支持普通成员函数调用（class/interface methods）"的注释
- 目标：
  - 让 user 与 sysroot 的 declarer 共享同一前端路径调用 instance method，不再依赖扩展函数承接 instance method 语义。
  - 必须 cover generic class（这是 P4-T01b/c 在 Array/MutableArray 这类 `@Intrinsic class` 上验证机制的前提）。
- 当前实现入口：
  - `crates/scoopc/src/typecheck/expr/call.rs::infer_member_call_expr_type`
  - `crates/scoopc/src/resolve/scopes.rs`（builtin allowlist / member call 解析）
  - `crates/scoopc/src/hir/lower/expr.rs::should_keep_member_call_as_member_access`
  - `crates/scoopc/src/hir/lower/mod.rs`（instance method → top-level fun lowering）
  - `crates/scoopc/src/itable.rs`、`crates/scoopc/src/vtable.rs`（method 收集）
- 必须实现的内容：
  1. typecheck 接受 struct body 内 `fun` 声明并视作 instance method（含 `override`）；class body 内同理。
  2. typecheck 接受 generic class（如 `class Box<T>`）body 内 `fun` 声明，receiver type 含 type parameter 的 instance method 解析正确。
  3. typecheck 解析 `receiver.method()` 时优先解析到 struct/class body 内的 method，再 fall back 到扩展函数（保留扩展函数路径，与本任务并存，不删除）。
  4. HIR lower 把 struct/class body method lower 为 top-level fun，FQN 为 `<TypeFqn>.<methodName>`，与现有扩展函数 lowering 一致；这样 `fun_index` 自然消费、`itable.rs` 自然按 FQN 命中。
  5. monomorphization 对 generic class instance method 正确生成特化版本（与现有 `fun <T> Box<T>.method()` 扩展函数 lowering 路径对齐）。
- 必须遵从的约束：
  - 不得删除任何现有扩展函数路径或既有 by-name 特判。本任务只**新增** instance method 路径。
  - 不得修改 `sysroot/core.scoop` / `sysroot/scalar.scoop` 中关于内建类型的现有声明。
  - 不得修改 `sysroot/core.scoop` 中 Array / MutableArray 的现有 declaration-only 形式。
- 验证：
  1. 新增 fixture：用户自定义 `struct Foo { fun bar(): Int { ... } }` + `f.bar()` 通过编译并 run-pass。
  2. 新增 fixture：用户自定义 `class Bar { fun baz(): Int { ... } }` + `b.baz()` 通过编译并 run-pass。
  3. 新增 fixture：用户自定义 `class Box<T> { fun get(): T { ... } }` + `box.get()` 通过编译并 run-pass。
  4. `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass`：所有现有 pass fixture 不退化。
  5. `cargo test -p scoopc llvm_tests -- --nocapture`：现有 IR 锁定测试不退化。
- 完成条件：
  - 用户自定义 struct/class（含 generic class）能用常规 `receiver.method()` 调用 instance method；既有扩展函数路径与现有内建类型 lowering 不变。
- 依赖：`P3-T02`
- 完成记录：
  - 改动范围：
    - `crates/scoopc/src/typecheck/expr/entry.rs`
    - `crates/scoopc/src/hir/lower/expr.rs`
    - `tests/fixtures/run-pass/member_call_{struct,class,generic_class}_body_method_basic.*`
    - `TODO.md`
    - `memory/claude_plan.md`
  - 核心决策：
    - `struct` 成员函数体现在复用既有的 member-fun body typecheck 主线：`this` 与主构造参数的可见性、返回类型推断、默认参数检查与 effect gate 和 class member 保持同构；class 专属的 property/init/super-ctor 路径保持不变。
    - ordinary `receiver.method()` / `receiver?.method()` 的 HIR 降糖现在统一覆盖 `struct/class/interface/object`：除 builtin keep-list 与 GC/handle intrinsics 外，都改写为 `<Owner>.method(receiver, ...)` 顶层调用形状；既有扩展函数 fallback 与 by-name 特判不删除。
    - generic class instance method 沿用现有 owner-specialized member monomorphization 路径，不新增新的 generic special-case；新增的 struct fixture 额外锁定“member method 优先于同名 extension”这一解析顺序。
  - 验证结果：
    - `cargo fmt --all`
    - `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/member_call_struct_body_method_basic.scoop`
    - `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/member_call_class_body_method_basic.scoop`
    - `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/member_call_generic_class_body_method_basic.scoop`
    - `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass`（新增/既有 member-call 相关 fixture 均通过；全量当前仍暴露两个与本任务代码路径无直接交集的现存失败：`extern_native_aggregate_return_direct_indirect_parity.scoop`、`sync_gc_release_task_like_object_basic.scoop`）
    - `cargo test -p scoopc llvm_tests -- --nocapture`（当前 filter 仍返回 `0 passed; 848 filtered out`，因此额外按 owner test 名补跑下面三条）
    - `cargo test -p scoopc object_member_call_uses_gc_managed_singleton_receiver -- --nocapture`
    - `cargo test -p scoopc builtin_string_member_calls_lower_to_direct_calls -- --nocapture`
    - `cargo test -p scoopc builtin_string_trim_indent_member_calls_lower_to_direct_calls -- --nocapture`
    - `cargo clippy --all-targets -- -D warnings`
  - 与 `PLAN.md` / `MANAGED_ABI.md` 的对应闭合：
    - 对应 `PLAN.md` P4 前置 I 第 1 项：user struct/class（含 generic class）的 instance method 现已走与 sysroot declarer 同构的 ordinary member-call 前端路径，不再只依赖扩展函数承接实例方法语义。
    - 本任务没有改动 `sysroot/core.scoop` 的既有内建类型声明，也没有提前改写 vtable/itable、intrinsic 表或 ABI substrate，因此无需回写 `MANAGED_ABI.md`。

### [DONE] P4-T01b：vtable / itable 收集统一从 struct/class body method 抽，interface call ABI 收口为 metadata-driven marshal

- 参考：
  - [`PLAN.md`](./PLAN.md) §5 / "P4 前置"
  - `crates/scoopc/src/itable.rs::{collect_classes_in_type_decl,collect_runtime_interfaces_and_class_itables_with_env,build_precise_class_itable_entries}`
  - `crates/scoopc/src/vtable.rs`
  - `crates/scoopc/src/llvm/codegen/call/lowering.rs::{try_codegen_class_vtable_call,try_codegen_interface_itable_call_impl,load_interface_itable_slot_fn_ptr_i8_impl}`
  - `crates/scoopc/src/llvm/codegen/mir_body.rs::{codegen_mir_composite_value_box,mir_value_box_itable_entries,get_or_create_mir_value_box_itable_global}`
- 目标：
  - 让 vtable / itable 在 P4-T01a 的 instance method 路径上正确收集 method（含 `override`）。
  - 让 interface call 的 caller side 完全按 itable slot 元数据 marshal 参数（class 类型按 ptr、value 类型按 by-value 与 method signature 一致），不为 value type 合成 box thunk。
  - 让"用户自定义 struct/class（含 generic class）实现 interface" 走完整 itable dispatch 路径。
- 当前实现入口：
  - 同上"参考"
  - `crates/scoopc/src/llvm/codegen/effect_lowered/{body.rs,value.rs}` 中 `ToString.toString` / scalar `toString` ext 的 FQN intercept（**不在本任务删除**，留给 P4-T01）
- 必须实现的内容：
  1. 把 itable / vtable 收集扩展到 struct/class body 内的 method declarations（与 P4-T01a lowering 出的 FQN 对齐）；保持现有扩展函数命中路径不变。
  2. 在 `mir_value_box` 路径上验证：用户自定义 struct value type 实现 interface 时，box 内 itable slot 直接指向 struct body method symbol，不经过合成 thunk；如果当前实现已是 thunk-less，加 IR 锁定测试以防回归。
  3. interface call caller side：按 itable slot 元数据中的 method signature marshal 参数。如果当前 caller 假设固定 by-ptr 形态导致需要 thunk，把 caller 改造为 metadata-driven marshal（这是"零编译器后门"的核心动作）。
  4. user 自定义 generic class 实现 interface 后的 itable 在 monomorphization 后正确出现在 `fun_index`、`class_itables`、`runtime interfaces` 三处 metadata。
- 必须遵从的约束：
  - 不得动 `try_codegen_tostring_iface_builtin` / `codegen_sysroot_to_string_ext` / `codegen_mir_transport_to_string` 等 by-name intercept；它们留给 P4-T01 删除。
  - 不得修改 `sysroot/core.scoop` 中内建类型现有声明；不得引入"内建类型 only" 的新代码路径。
  - 不得为本阶段引入新的 by-name 分支；新增机制必须与用户自定义类型同构。
- 验证：
  1. 新增 fixture：用户自定义 `interface I { fun m(): Int }` + 用户自定义 struct（value type）实现 `I` + 用 `I` 类型变量持有 + `(it as I).m()` 走 itable dispatch 通过 run-pass。
  2. 新增 fixture：用户自定义 `interface I` + 用户自定义 class（GC type）实现 `I` 通过 run-pass。
  3. 新增 fixture：用户自定义 `interface I` + 用户自定义 generic `class Box<T>` 实现 `I` 通过 run-pass。
  4. 新增 IR 锁定测试：上述 user struct value type → I 的 itable global 不包含合成 box thunk 符号；slot 直接指向 user struct body method symbol。
  5. `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass`：所有现有 pass fixture 不退化。
- 完成条件：
  - vtable / itable 收集与 interface call caller side 完全 metadata-driven，不依赖任何 by-name 内建分支或合成 thunk。
- 依赖：`P4-T01a`
- 完成记录：
  - 改动范围：
    - `crates/scoopc/src/{itable.rs,vtable.rs}`
    - `crates/scoopc/src/hir/{mod.rs,lower/mod.rs,lower/util.rs}`
    - `crates/scoopc/src/mir/materialize.rs`
    - `crates/scoopc/src/llvm/codegen/{mod.rs,call/lowering.rs,effect_lowered/layout.rs,gc.rs,mir_body.rs}`
    - `crates/scoopc/src/llvm/tests.rs`
    - `tests/fixtures/run-pass/member_call_interface_dispatch_{struct_value_box,class_body_method,generic_class_body_method}_basic.*`
    - `TODO.md`
  - 核心决策：
    - `vtable` / `itable` 现在统一把 struct/class body method 当作实现来源：`vtable` 对 struct 也收集 method slot，`itable` 的 base/runtime entries 都显式发布 `method_impl_fqns + method_receiver_type_ids`，保持已有 extension/default-method 命中路径不删除。
    - interface caller side 改为完全按 itable slot metadata marshal：lookup 同时返回函数指针与 receiver type id；ref receiver 继续按对象指针调用，value-type receiver 则从 value-box payload 按 metadata 重建 concrete receiver，不再依赖 box thunk shell。
    - MIR value-box itable global 现在直接指向 user struct body method symbol；LLVM owner test 明确锁定“slot 直接命中 body method，不出现 `refactor_itable_dynamic_entry` thunk shell”。
    - generic class interface dispatch 额外补齐了 dispatch-only member instance 发布：precise class itable 会把 owner-specialized `Box.m::<T>` 写进 metadata，materializer 则从 lowered 顶层函数首参识别 receiver owner，并用 typechecked concrete nominal 生成显式 dispatch instance 索引，确保 `Box.m::<Int>` / `Box.m::<String>` 能进入初始实例请求与 published MIR bodies。
  - 验证结果：
    - `cargo fmt --all`
    - `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/member_call_interface_dispatch_struct_value_box_basic.scoop`
    - `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/member_call_interface_dispatch_class_body_method_basic.scoop`
    - `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/member_call_interface_dispatch_generic_class_body_method_basic.scoop`
    - `cargo test -p scoopc value_box_interface_itable_points_directly_to_struct_body_method_symbol -- --nocapture`
    - `cargo test -p scoopc materialize_for_dump_publishes_generic_interface_dispatch_member_instances -- --nocapture`
    - `cargo test -p scoopc llvm_tests -- --nocapture`（当前 harness 仍返回 `0 passed; 850 filtered out`，因此以上两条 owner tests 作为本任务的实际 LLVM / materialization 锁定覆盖）
    - `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass`（当前仍存在两个既存失败：`extern_native_aggregate_return_direct_indirect_parity.scoop`、`sync_gc_release_task_like_object_basic.scoop`；本任务新增的 interface-dispatch 相关 fixture 全部通过，未引入额外回归）
    - `cargo clippy --all-targets -- -D warnings`
  - 与 `PLAN.md` / `MANAGED_ABI.md` 的对应闭合：
    - 对应 `PLAN.md` P4 前置 I 第 2 项：user struct/class/generic class 的 interface impl 现在可以通过统一的 metadata-driven itable dispatch 走完整 caller-side marshal；value type 不再依赖 thunk，generic class 的 concrete method instance 也能被发布到 fun_index / class_itables / runtime dispatch metadata。
    - 本任务没有改变 `MANAGED_ABI.md` 约定的 ABI surface，也没有删除 `ToString`/scalar/string 的 by-name intercept；这些删除动作仍按顺序留给后续 `P4-T01`。

### [DONE] P4-T01c-pre1：为 P4-T01c 引入 sysroot overlay fixture 能力，允许 task-specific `core.scoop` 改写

- 参考：
  - `crates/scoopc/src/sysroot/mod.rs`
  - `crates/scoopc/src/session/mod.rs`
  - `crates/scoop/src/fixtures/mod.rs`
  - `crates/scoop/src/commands/build.rs`
- 起因 / 阻塞说明：
  - `P4-T01c` 的验收要求里明确需要一个“把 sysroot 现有 `class Array<T>` / `class MutableArray<T>` 重写为 `@Intrinsic class ... { bodied methods }` 的本任务专用 fixture”。
  - 当前 fixture / driver 路径始终加载工作区默认 sysroot，且没有 overlay / replace 机制；若在 fixture 源里再次声明 `scoop.core.Array` / `scoop.core.MutableArray`，会直接与默认 sysroot 中同名定义冲突。
  - 在没有 sysroot overlay 能力前，只能退而求其次改用别的 FQN（例如 `Container<T>`）或只做 Rust owner test；这都会直接缩窄 `P4-T01c` 已写定的 fixture 形状，违反“不得通过更容易的表示或 fixture shape 绕过问题”的约束。
- 目标：
  - 为 fixture / build 路径补一条最小的 sysroot overlay 能力，使 `P4-T01c` 可以用真实 `scoop.core.Array` / `MutableArray` FQN 做 task-specific fixture，而不复制整份 sysroot，也不污染默认 sysroot。
- 必须实现的内容：
  1. 为测试/fixture 路径提供“在默认 sysroot 之上覆写指定文件”的能力；overlay 后仍应保留其余默认 sysroot 内容。
  2. 该能力必须同时覆盖：
     - `scoopc` 前端默认 support-source 加载路径；
     - `scoop run` / `scoop build` 经 fixture harness 触发的 CLI 路径。
  3. overlay 后的 `core.scoop` 若出现 bodied `@Intrinsic struct/class`，必须能进入 compilable sysroot source 集合，而不是继续停留在 signature-only sysroot 索引里。
  4. 不得要求 fixture 复制整份 `sysroot/` 目录；overlay 只应表达“改写哪些文件”。
- 必须遵从的约束：
  - 不得通过把 `P4-T01c` 的 Array/MutableArray fixture 换成不同 FQN 的“等价样例”来规避该前置。
  - 不得引入只为单个 fixture 生效的 ad-hoc 路径；overlay 机制必须是可复用、可审计的测试基础设施。
- 验证：
  1. 新增一条 fixture / owner test，证明 overlay `core.scoop` 后可以成功覆盖默认 `scoop.core.Array` 声明，而未覆盖的 sysroot 符号仍保持可见。
  2. `cargo run -p scoop -- test --fixtures <新增的 sysroot overlay fixture>`
  3. `cargo test -p scoopc llvm_tests -- --nocapture`
- 完成条件：
  - `P4-T01c` 可以在不复制整份 sysroot、也不缩窄 fixture 形状的前提下，为 `Array<T>` / `MutableArray<T>` 编写 task-specific overlay fixture。
- 依赖：`P4-T01b`
- 完成记录：
  - 改动范围：
    - `crates/scoopc/src/{session/mod.rs,sysroot/mod.rs,frontend.rs,llvm/frontend.rs}`
    - `crates/scoop/src/{commands/build.rs,commands/test.rs,fixtures/mod.rs,fixtures/run_pass.rs,commands/dump_mir.rs}`
    - `tests/fixtures/build/sysroot_overlay_core_array_interface_bridge.scoop`
    - `tests/fixtures/build/sysroot_overlay_core_array_interface_bridge.sysroot/core.scoop`
    - `TODO.md`
    - `memory/claude_plan.md`
  - 核心决策：
    - 用 `SessionOptions` 携带可选的 sysroot overlay 路径，并让 `Session` 与 frontend support-source loader 共用同一配置，避免 fixture path 与 build path 各自维护一套注入逻辑。
    - sysroot overlay 采用“默认 sysroot + companion overlay 目录按相对路径替换”的合并规则；fixture target 通过同伴目录 `<target>.sysroot/` 或 `<stem>.sysroot/` 声明覆盖文件，runner 会自动跳过这些目录，不把它们误当成普通 fixture/case。
    - `core.scoop` 是否进入 compilable support sources 现在不再只靠文件名白名单：若 overlay 版本声明了 bodied `@Intrinsic struct/class`，它会自动转入 `compilable_source_paths`，从而满足 `P4-T01c` 对 `@Intrinsic class Array<T> { ... }` fixture 形状的前置要求。
    - `scoop run`/`scoop build` 的外部 CLI 路径通过环境变量 `SCOOP_SYSROOT_OVERLAY` 继承同一 overlay；为避免 cone 可执行产物目录命中旧 fingerprint，overlay build 会自动关闭增量缓存。
  - 验证结果：
    - `cargo fmt --all`
    - `cargo test -p scoopc overlay_core_with_bodied_intrinsic_nominal_method_becomes_compilable_support_source -- --nocapture`
    - `cargo test -p scoop sysroot_overlay -- --nocapture`
    - `cargo test -p scoop run_pass_single_pipeline_propagates_sysroot_overlay_env -- --nocapture`
    - `cargo run -p scoop -- test --fixtures tests/fixtures/build/sysroot_overlay_core_array_interface_bridge.scoop`
    - `cargo test -p scoopc llvm_tests -- --nocapture`（当前 filter 仍返回 `0 passed; 851 filtered out`，因此实际 owner coverage 以上面新增的 `overlay_core_with_bodied_intrinsic_nominal_method_becomes_compilable_support_source` 为准）
    - `cargo clippy --all-targets -- -D warnings`
  - 与 `PLAN.md` / `MANAGED_ABI.md` 的对应闭合：
    - 对应 `PLAN.md` P4 前置 I 的 sysroot overlay 前置要求：后续 `P4-T01c` 现可在不复制整份 `sysroot/` 的前提下，用真实 `scoop.core.Array` / `MutableArray` FQN 编写 task-specific overlay fixture，并同时覆盖 fixture harness、internal build path 与外部 `scoop run` CLI 路径。
    - 本任务没有改变 `MANAGED_ABI.md` 约定的 ABI surface，也没有提前实现 `@Intrinsic struct/class` method body；它只提供后续任务所需的测试基础设施与 compilable-sysroot gate。

### [DONE] P4-T01c：`@Intrinsic struct/class` 含 method body 完整落地（含 generic class），锁定零编译器后门

- 参考：
  - [`PLAN.md`](./PLAN.md) §5 / "P4 前置"
  - `sysroot/core.scoop` 中 `@Intrinsic enum Option<T>` / `@Intrinsic` 现有用法
  - `crates/scoopc/src/typecheck/annotations.rs`（`@Intrinsic` 的现有 surface）
  - `crates/scoopc/src/sysroot/mod.rs`（compilable sysroot file gate）
  - `crates/scoopc/src/llvm/codegen/ty.rs`（builtin layout cg_ty_of 识别）
- 目标：
  - 让 `@Intrinsic struct/class` 含 method body 这种 sysroot 写法走完整编译管线：parser / typecheck / HIR / MIR / codegen 全链路。
  - layout 仍由编译器内置识别（与 sysroot 现状一致），但 method 部分照常走 P4-T01a/b 打通的路径。
  - **body 内允许声明任意 instance method（含 `override` 与不含 `override`）**：未来想给内建类型加非 interface method（例如 `Int.toBinaryString(): String`）时，直接写在 sysroot 的 `@Intrinsic struct/class` body 内即可，不需要走扩展函数，也不需要改编译器。
  - 把"零编译器后门"约束锁进 fixture：未来再加任何内建 interface 或非 interface 的内建 method 都不再要求改编译器。
- 当前实现入口：
  - 同上"参考"
- 必须实现的内容：
  1. parser / typecheck 接受 `@Intrinsic struct/class { ... method body ... }` 形式：
     - body 内**不允许**声明 fields（layout 由编译器内置）；
     - body 内**允许**声明任意 instance method：含 `override`（实现 super interface 的 method）与**不含 `override`**（独立的内建类型新功能 method，例如未来的 `fun Int.toBinaryString(): String`）；
     - 现有 `@Intrinsic` 应用到 fun / enum 的语义保持不变。
  2. `@AllowIntrinsic` gate 覆盖到 struct/class：sysroot 文件天然过 gate；user 文件需显式 `@file:AllowIntrinsic`。
  3. 编译器对 `@Intrinsic struct/class` 的特殊处理只剩两件：
     - layout 由编译器内置（不读 body fields，因为禁止声明 fields）；
     - HIR/MIR/codegen 路径与用户自定义 struct/class 完全一致。
  4. generic `@Intrinsic class`（如 `Array<T>` / `MutableArray<T>`）：layout 内置识别要正确处理 type parameter；method body 走 P4-T01a 打通的 generic class instance method 路径。
  5. **不动** `Int` / `String` / `Char` / `Float32` / `Float64` 现有 sysroot 声明（仍是 declaration-only `struct Int : Hashable, ToString`）；不动 scalar `toString` 现有 codegen 路径。这些搬迁动作留给 P4-T01。
- 必须遵从的约束：
  - 不得为本阶段在 codegen 中新增任何"按 builtin FQN 决定 method body / dispatch"的路径。
  - 不得让 `@Intrinsic struct/class` 退化成"declaration-only" 的轻量版本；method body 路径必须真打通（即使本阶段没有任何 sysroot 文件实际使用 method body）。
  - 不得修改 `runtime/c/scoop_runtime.c` 添加新 helper；本阶段不涉及 runtime 改动。
- 验证：
  1. 新增 fixture：测试用 `@file:AllowIntrinsic` + `@Intrinsic struct Dummy : DummyIface { override fun m(): Int { ... } }` 的小 program，layout 由编译器内置识别（用一个不与 Int/Bool/Char/Float 冲突的测试专用 builtin name，或者临时挂在 Dummy 上做"无 fields 即视为 zero-sized" 兜底；具体策略由 P4-T01c 实现侧决定但需在 fixture 中显式锁定），通过 typecheck / lower / codegen。
  2. 新增 fixture：`@file:AllowIntrinsic` + `@Intrinsic struct/class` body 内**只声明不带 `override` 的独立 instance method**（不实现任何 interface），调用通过编译并 run-pass —— 锁定"未来给内建类型加非 interface method 不必走扩展函数"这条 surface。
  3. 新增 fixture：`@file:AllowIntrinsic` + `@Intrinsic class Container<T> : DummyIter { override fun ...(): ... { ... } }`（dummy interface，不动 sysroot Iterable，因为 Iterable 暂不做），通过 typecheck / lower / codegen 并 run-pass。
  4. 新增 fixture：把 sysroot 现有 `class Array<T>` / `class MutableArray<T>` 用 `@Intrinsic class Array<T> : DummyIter { ... }` 形式重写**为本任务专用 fixture**（不动 sysroot 真实 Array/MutableArray）；验证机制可落到已有 generic class 的 layout 上。
  5. 新增 user-side fail fixture：user 文件未加 `@file:AllowIntrinsic` 但写 `@Intrinsic struct/class`，必须前端拒绝。
  6. 新增 fail fixture：`@Intrinsic struct/class` body 内声明 fields，必须前端拒绝。
  7. 编译器源代码 grep 验证（在 fixture 注释中说明检查项，必要时通过 CI lint 锁）：本任务结束后，编译器没有为新内建 interface 加任何 by-name 分支。
  8. `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass`：现有 pass fixture 不退化。
  9. `cargo test -p scoopc llvm_tests -- --nocapture`：现有 IR 锁定测试不退化。
- 完成条件：
  - `@Intrinsic struct/class`（含 generic class）含 method body 完整走完整编译管线；
  - 上述 fixture 双锁机制覆盖到 generic class 这一最复杂形态；
  - P4 前置 II 可以在此基础上扩展 method-level `@Intrinsic("name")` surface；
  - P4-T01 在 P4 前置 I/II 都完成后只需"sysroot 改写 + 删旧 by-name 特判"，编译器侧不再需要为单个 non-generic interface 加任何配套改动。
- 依赖：`P4-T01c-pre1`
- 完成记录：
  - 改动范围：
    - `crates/scoopc/src/{source.rs,sysroot/mod.rs,frontend.rs}`
    - `crates/scoopc/src/typecheck/{annotations.rs,inheritance.rs}`
    - `crates/scoopc/src/llvm/codegen/composite_transport.rs`
    - `crates/scoopc/src/llvm/tests.rs`
    - `tests/fixtures/run-pass/{intrinsic_struct_interface_body_method_basic.scoop,intrinsic_class_body_method_basic.scoop,intrinsic_generic_class_body_method_basic.scoop}`
    - `tests/fixtures/typecheck/{intrinsic_user_bodied_type_without_allow_intrinsic_is_error.scoop,intrinsic_type_body_field_is_error.scoop,intrinsic_type_ctor_field_is_error.scoop}`
    - `tests/fixtures/build/intrinsic_sysroot_overlay_array_mutablearray_body_methods_basic.scoop`
    - `tests/fixtures/build/intrinsic_sysroot_overlay_array_mutablearray_body_methods_basic.sysroot/core.scoop`
    - `TODO.md`
    - `memory/claude_plan.md`
  - 核心决策：
    - `@Intrinsic struct/class` 的前端门禁从“成员函数必须无 body”改为“禁止声明字段，但允许 ordinary instance method body”；字段约束同时覆盖 direct body property 与 ctor property field，从而保持 layout 仍由编译器内置决定。
    - 为 `SourceFile` 引入显式 sysroot 来源标记，并让 sysroot/support-source 加载链路保留该身份；这样 task-specific overlay `core.scoop` 会天然通过 `@AllowIntrinsic` gate，而不会被误判成用户源码。
    - class `override` 校验现在把 direct interface 成员也视为合法目标：没有 superclass 的 class、或 superclass 不提供该成员的 class，仍可对 direct interface method 显式写 `override`；未显式 `override` 的 interface impl 继续保持现有允许行为。
    - zero-field intrinsic struct 继续保留 nominal value-box/type-desc/itable 身份；本任务不把它降格成 generic `BoxedUnit`，而是放宽 composite transport 对 `size_bytes == 0` 的禁令，保证 zero-sized nominal payload 能走完整 MIR/LLVM/interface-dispatch 路径。
    - 通过 fixture + LLVM owner tests 双锁：用户态 intrinsic struct/class、generic intrinsic class，以及 overlay `Array<T>` / `MutableArray<T>` 都无需新增任何 `DummyIface`/`DummyIter` 之类的按名编译器分支。
  - 验证结果：
    - `cargo fmt --all`
    - `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/intrinsic_struct_interface_body_method_basic.scoop`
    - `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/intrinsic_class_body_method_basic.scoop`
    - `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/intrinsic_generic_class_body_method_basic.scoop`
    - `cargo run -p scoop -- test --fixtures tests/fixtures/build/intrinsic_sysroot_overlay_array_mutablearray_body_methods_basic.scoop`
    - `cargo run -p scoop -- test --fixtures tests/fixtures/typecheck/intrinsic_user_bodied_type_without_allow_intrinsic_is_error.scoop`
    - `cargo run -p scoop -- test --fixtures tests/fixtures/typecheck/intrinsic_type_body_field_is_error.scoop`
    - `cargo run -p scoop -- test --fixtures tests/fixtures/typecheck/intrinsic_type_ctor_field_is_error.scoop`
    - `cargo test -p scoopc intrinsic_value_box_interface_itable_points_directly_to_struct_body_method_symbol -- --nocapture`
    - `cargo test -p scoopc overlay_core_intrinsic_array_methods_lower_through_ordinary_generic_class_path -- --nocapture`
    - `cargo test -p scoopc intrinsic_nominal_body_method_fixtures_do_not_introduce_by_name_compiler_paths -- --nocapture`
    - `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass`（本轮新增 intrinsic fixtures 全部通过；仍保留两个既存失败：`extern_native_aggregate_return_direct_indirect_parity.scoop`、`sync_gc_release_task_like_object_basic.scoop`）
    - `cargo test -p scoopc llvm_tests -- --nocapture`（当前 harness filter 仍返回 `0 passed; 854 filtered out`，因此本任务的实际 LLVM owner coverage 以上面三条新增 test 为准）
    - `cargo clippy --all-targets -- -D warnings`
  - 与 `PLAN.md` / `MANAGED_ABI.md` 的对应闭合：
    - 对应 `PLAN.md` P4 前置 I：`@Intrinsic struct/class` 的 ordinary method body 现已在 parser/typecheck/HIR/MIR/codegen 全链路落地，non-generic 与 generic 两条 nominal method path 都能在不新增 by-name 编译器后门的前提下工作。
    - sysroot overlay fixture 现在可以把真实 `scoop.core.Array<T>` / `MutableArray<T>` 改写成 bodied intrinsic generic class，并沿用现有 generic class member + interface dispatch 路径进入 codegen，为后续 `P4-T01d/P4-T01e` 的 method-level intrinsic 表机制提供稳定前置。
    - 本任务没有改变 `MANAGED_ABI.md` 约定的 ABI surface，也没有提前迁移 `Int/String/Char/Float*` 的真实 sysroot 声明；这些内建类型的 body 搬迁与 by-name 删除仍按顺序留给后续 `P4-T01`。

## P4 前置 II：intrinsic 表 IR-emission 与 runtime 收缩（Array/MutableArray 起步）

> 起因与设计基线：[`PLAN.md`](./PLAN.md) §5 / "P4 前置 II. intrinsic 表 IR-emission 与 runtime 收缩（Array/MutableArray 起步）"。
>
> P4 前置 I 把 non-generic 内建 method 的"零编译器后门"约束做完了。但 generic class 的 instance method 没法用 Scoop 写"按 T 选哪条 lowering 路径"（Scoop 没有 typecase / reflect），这是**真正不可避免**的后门。本阶段把它收敛为**唯一可枚举的 intrinsic 表**，并把 `Array<T>` / `MutableArray<T>` 的基础访问从 runtime helper 改为 IR-direct emission，恢复 LLVM 优化路径，同时删除对应的 runtime helper。
>
> **设计约束**：intrinsic 表的 entry **默认 IR-emission**；只有真正涉及 GC heap allocation / write barrier / move-GC composite copy 等无法纯 IR 表达的 GC-adjacent 协作时，才下放为 RuntimeCall。
>
> **长期方向（仅作背景）**：runtime 最终只保留 GC 实现与 GC-adjacent 功能（GC handle / pin、thread registration / cleanup），便于移植到 WASM / JVM 或切换 GC（如 BoehmGC）。其它内容（`exit(3)` / `print/println` / scalar helper 等）作为 v2+ backlog 逐步以 IR / Scoop ABI FFI 形式迁出，本阶段只处理 Array / MutableArray。

### [DONE] P4-T01d：引入 method-level `@Intrinsic("name")` 与可枚举 intrinsic 表机制

- 参考：
  - [`PLAN.md`](./PLAN.md) §5 / "P4 前置 II"
  - `crates/scoopc/src/typecheck/annotations.rs`（现有 `@Intrinsic` 应用到 fun / enum / struct / class 的 surface）
  - `crates/scoopc/src/llvm/codegen/intrinsics/`（intrinsic lowering 现有架构）
- 目标：
  - 让 generic-by-T 分流后门有唯一、可枚举、文档化的承载（method-level `@Intrinsic("name")` declaration + 编译器内置表）。
  - 表的 lowering 默认 **IR-emission**：直接生成 GEP / load / store / branch / phi 等基础 IR，让 LLVM 完整看穿；只有真正 GC-adjacent 协作（GC heap allocation / write barrier / descriptor-driven copy 等）才下放为 RuntimeCall entry。
  - 本任务只做"机制层"：surface + 表数据结构 + 一个 dummy IrEmission entry + 一个 dummy RuntimeCall entry 验证两条通路。`Array` / `MutableArray` 的实际填充留给 P4-T01e。
- 当前实现入口：
  - `crates/scoopc/src/typecheck/annotations.rs`
  - `crates/scoopc/src/hir/lower/`（method-level annotation 处理）
  - `crates/scoopc/src/llvm/codegen/intrinsics/`（新增 intrinsic 表实现）
  - `crates/scoopc/src/sysroot/mod.rs`（`@AllowIntrinsic` gate 已覆盖 fun / type-level，需扩到 method-level）
- 必须实现的内容：
  1. parser / typecheck 接受 method-level `@Intrinsic("name")` declaration：
     - method 必须 body-less；
     - `"name"` 必须命中编译器内置 intrinsic 表，否则前端报错（明确诊断）；
     - 受 `@AllowIntrinsic` gate 约束（user 文件没 `@file:AllowIntrinsic` 则前端拒绝）。
  2. 编译器内置 intrinsic 表数据结构：
     - 每条 entry 标注 lowering 模式（`IrEmission` / `RuntimeCall`）；
     - `IrEmission` entry 持有 lowering rule 函数，直接 emit IR；
     - `RuntimeCall` entry 持有 runtime symbol + signature + "为什么不能用 IR 表达"的明确理由（写进表内作为审计依据，CI / lint 可机器读取）。
  3. 把 type-level `@Intrinsic` 的 layout 内置识别与 method-level `@Intrinsic("name")` 的 lowering 路径解耦：
     - type-level `@Intrinsic` 仍只决定 layout / fields gating；
     - method-level `@Intrinsic("name")` 单独决定 method lowering；
     - 一个 type 上可以混合"普通 method body"与"`@Intrinsic("name")` body-less method"。
  4. 监听 codegen 调用流程，让 `@Intrinsic("name")` method call 在 codegen 时按表查找并执行对应 lowering rule。
  5. 现有 type-level `@Intrinsic` 用法不变（fun / enum / struct / class）。
- 必须遵从的约束：
  - 不得让 user 文件能声明任意 `@Intrinsic("name")`：name 必须在编译器表里枚举。
  - `RuntimeCall` entry **默认禁用**：表初始版本中不能没有 IrEmission 与 RuntimeCall 各一条 dummy entry 用于 fixture，但**不得**有任何"未给出'为什么不能 IR 表达'理由"的 RuntimeCall entry。
  - 不得删除既有 `@Intrinsic` 应用到 fun / enum / struct / class 的语义。
  - 不得把任何现有 codegen by-name 分支改名换姓塞进 intrinsic 表（必须是真正的"按 T 分流"语义，不是其它内建特判的伪装）。
- 验证：
  1. 新增 fixture：`@file:AllowIntrinsic` + 测试用 `@Intrinsic class Vec<T> { @Intrinsic("dummy_ir") fun foo(): Int }`，dummy_ir 在表中标记为 IrEmission（emit 一个常量），通过 typecheck / lower / codegen 与 run-pass。
  2. 新增 fixture：测试用 `@Intrinsic("dummy_runtime") fun bar(): Int`，dummy_runtime 在表中标记为 RuntimeCall（接到一个测试专用 runtime symbol），通过 typecheck / lower / codegen 与 run-pass。
  3. 新增 fail fixture：声明 `@Intrinsic("not_in_table") fun foo()`，前端必须拒绝并给出明确诊断。
  4. 新增 fail fixture：声明 `@Intrinsic("dummy_ir") fun foo() { /* body */ }`（body 不为空），前端必须拒绝。
  5. 新增 fail fixture：user 文件没 `@file:AllowIntrinsic` 但写 `@Intrinsic("dummy_ir") fun foo()`，前端必须拒绝。
  6. `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass`：现有 pass fixture 不退化。
  7. `cargo test -p scoopc llvm_tests -- --nocapture`：现有 IR 锁定测试不退化。
- 完成条件：
  - method-level `@Intrinsic("name")` surface 可用；
  - intrinsic 表数据结构落地，IrEmission / RuntimeCall 双通路验证通过；
  - 表内每条 entry 都可被外部审计（含 lowering 模式、runtime 理由）。
- 依赖：`P4-T01c`
- 完成记录：
  - 改动范围：
    - `crates/scoopc/src/{ast/mod.rs,lib.rs,intrinsics.rs}`
    - `crates/scoopc/src/resolve/mod.rs`
    - `crates/scoopc/src/typecheck/{annotations.rs,builtin_annotations.rs,expr/{mod.rs,collect.rs,call.rs,ops.rs}}`
    - `crates/scoopc/src/hir/lower/expr.rs`
    - `crates/scoopc/src/pipeline/hir_stage.rs`
    - `crates/scoopc/src/mir/materialize.rs`
    - `crates/scoopc/src/llvm/codegen/{call/lowering.rs,mir_body.rs,effect_lowered/value.rs,intrinsics/{mod.rs,named.rs}}`
    - `crates/scoopc/src/llvm/tests.rs`
    - `runtime/c/scoop_test.c`
    - `tests/fixtures/run-pass/{intrinsic_named_method_dummy_ir_basic.scoop,intrinsic_named_runtime_fun_basic.scoop}`
    - `tests/fixtures/typecheck/{intrinsic_named_fun_body_is_error.scoop,intrinsic_named_fun_unknown_table_entry_is_error.scoop,intrinsic_named_fun_without_allow_intrinsic_is_error.scoop}`
    - `TODO.md`
  - 核心决策：
    - 新增共享模块 `crates/scoopc/src/intrinsics.rs`，集中声明 named intrinsic 表、`@Intrinsic("name")` 参数解析、lowering 模式以及 RuntimeCall entry 的 symbol/signature/reason 审计元数据；表的初始版本只发布 `dummy_ir` 与 `dummy_runtime` 两个测试条目，分别锁住 IR-emission 与 RuntimeCall 通路。
    - 前端继续保留 legacy 零参数 `@Intrinsic`，同时把 `@Intrinsic("name")` 的 entry name 透传进 `resolve` / typecheck call binding / HIR / MIR；未知表项在前端直接报错，且 method-level / top-level intrinsic 都继续受 `@file:AllowIntrinsic` gate 约束。
    - codegen 在 direct HIR call、MIR direct call 与 effect-lowered direct call 三条通路上统一按 call binding 的 `intrinsic_entry_name` 查共享表；IR-emission entry 直接在调用点发 IR，RuntimeCall entry 则按表内 runtime metadata 声明/调用测试符号，不再物化 declaration-only Scoop body。
    - type-level `@Intrinsic` 与 method-level `@Intrinsic("name")` 语义解耦：前者仍只负责 nominal layout / field gate，后者单独决定具体 method lowering；因此同一个 intrinsic type 现在可以混合 ordinary bodied method 与 body-less named intrinsic method。
    - 为满足 `cargo clippy --all-targets -- -D warnings`，顺手把 named intrinsic MIR lowering 中未使用的 `expected` / 冗余 span 参数删掉，避免把质量门豁免成 `allow`。
  - 验证结果：
    - `cargo fmt --all`
    - `cargo test -p scoopc named_intrinsic -- --nocapture`
    - `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/intrinsic_named_method_dummy_ir_basic.scoop`
    - `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/intrinsic_named_runtime_fun_basic.scoop`
    - `cargo run -p scoop -- test --fixtures tests/fixtures/typecheck/intrinsic_named_fun_body_is_error.scoop`
    - `cargo run -p scoop -- test --fixtures tests/fixtures/typecheck/intrinsic_named_fun_unknown_table_entry_is_error.scoop`
    - `cargo run -p scoop -- test --fixtures tests/fixtures/typecheck/intrinsic_named_fun_without_allow_intrinsic_is_error.scoop`
    - `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass`（当前仍有两个既存失败：`extern_native_aggregate_return_direct_indirect_parity.scoop`、`sync_gc_release_task_like_object_basic.scoop`；本任务新增 named intrinsic fixtures 未引入新的 run-pass 回退）
    - `cargo test -p scoopc llvm_tests -- --nocapture`（当前 harness 仍返回 `0 passed; 859 filtered out`，因此本任务的实际 LLVM owner coverage 以上面的 `cargo test -p scoopc named_intrinsic -- --nocapture` 为准）
    - `cargo clippy --all-targets -- -D warnings`
  - 与 `PLAN.md` / `MANAGED_ABI.md` 的对应闭合：
    - 对应 `PLAN.md` P4 前置 II 的第 1-2 项：method-level `@Intrinsic("name")` surface 与可枚举 intrinsic 表机制已落地，且默认 IR-emission、RuntimeCall 必须附带理由的约束已经通过共享表结构与单测固定下来。
    - 对应 `PLAN.md` P4 前置 II 的“先做机制层”要求：本任务只交付 dummy IR / dummy runtime 双通路与 callsite 查表机制，没有提前把 `Array` / `MutableArray` 迁进表，也没有新增其它 by-name 内建特判；`P4-T01e` 将继续在这套机制上填充真实 array entries。
    - 本任务不改变 `MANAGED_ABI.md` 中的 ABI surface，因此无需回写 `MANAGED_ABI.md`；相关变更仅限 intrinsic 表机制与编译器内部 lowering source-of-truth。

### [DONE] P4-T01e：用 `Array` / `MutableArray` 填充 intrinsic 表（IR-direct），删除已被替代的 array runtime helper

- 参考：
  - [`PLAN.md`](./PLAN.md) §5 / "P4 前置 II"
  - `runtime/c/scoop_array.c`（`ScoopArray` layout 与 helper）
  - `crates/scoopc/src/llvm/codegen/runtime_symbols.rs`（`SCOOP_ARRAY_*` constants）
  - `crates/scoopc/src/llvm/codegen/runtime_abi.rs::{declare_runtime_array_len,declare_runtime_array_get_*,declare_runtime_array_set_*}`
  - `crates/scoopc/src/llvm/codegen/intrinsics/containers.rs`、`crates/scoopc/src/llvm/codegen/effect_lowered/value.rs`（当前 array helper callsite）
  - `crates/scoopc/src/opt/bce.rs`（与 array_get IR-direct 协同的 BCE pass）
- 目标：
  - 把 `Array<T>` / `MutableArray<T>` 改写为 `@Intrinsic class` 含 `@Intrinsic("...")` method declaration 形式，作为 P4-T01d 机制的 first user。
  - 让 `array_size` / `array_get` / `array_set` / `array_data_ptr` 等基础访问 IR-direct emission，恢复 LLVM LICM / CSE / BCE / 自动向量化 等优化路径。
  - 删除已被替代的 runtime helper，substrate 收缩。
  - 保留：`array_alloc` / `array_builder_grow` / `write_barrier` / `composite_copy` 等真正 GC-adjacent 协作的 helper（作为 RuntimeCall entry）。
- 当前实现入口：
  - 同上"参考"
  - `sysroot/core.scoop`（`class Array<T>` / `class MutableArray<T>` 现有 declaration）
- 必须实现的内容：
  1. 在编译器 intrinsic 表中加入以下 IrEmission entry：
     - `array_size`：emit `ScoopArray.len` field load。
     - `array_get`：emit bounds check（与 BCE 标 unchecked 协同）+ GEP 到 `data + idx * elem_size_bytes` + load（按 T 的实际 LLVM type，不再 widen 到 i64）。
     - `array_set`：emit bounds check + GEP + store + write barrier intrinsic（仅当 T 为 ref kind）。
     - `array_data_ptr`：emit GEP 到 `ScoopArray.data` 字段（用于 builder / iterator 内部）。
     - 其它必要的小原语视实现需要补充（例如 `array_elem_kind` / `array_elem_size` 若 codegen 内联 BCE 之外仍需读这些 self-describing 字段；尽量让 lowering 在编译期已知 T 的情况下消除这类读取）。
  2. 在编译器 intrinsic 表中加入以下 RuntimeCall entry（保留原 helper）：
     - `array_alloc`：调用 `gc_alloc_typed` + 初始化 `ScoopArray` 字段；reasons 字段写明"涉及 GC heap allocation"。
     - `array_builder_grow`：调用 `scoop_array_builder_*`；reasons 字段写明"涉及 GC heap allocation + 旧数据迁移"。
     - `write_barrier`：调用 generational GC 的 card mark；reasons 字段写明"GC-adjacent，需写 card 表"。
     - `composite_copy`：调用 `scoop_composite_copy`；reasons 字段写明"descriptor-driven 循环，含 GC slot 协调"。
  3. 把 `sysroot/core.scoop` 的 `Array<T>` / `MutableArray<T>` 改写为 `@Intrinsic class` 含 `@Intrinsic("array_size")` / `@Intrinsic("array_get")` / `@Intrinsic("array_set")` 等 method declaration 形式：
     - 如果 sysroot 现有扩展函数（`fun <T> Array<T>.size(): Int` 等）直接保留作为 user-facing API 桥接，仍可工作；
     - 也可以同步迁成 method 形式 + 删除扩展函数（取实施时较干净的形态）。
  4. 把 `crates/scoopc/src/llvm/codegen/intrinsics/containers.rs` / `effect_lowered/value.rs` 中按 receiver type 名字分流到 `scoop_array_get_*` / `scoop_array_set_*` / `scoop_array_len` 的代码迁到 intrinsic 表 entry；callsite 走 `@Intrinsic("array_*")` lowering 路径。
  5. 删除以下 runtime helper（同步 declaration、symbol、C 实现）：
     - `runtime/c/scoop_array.c::scoop_array_get_u64`
     - `runtime/c/scoop_array.c::scoop_array_get_ref`
     - `runtime/c/scoop_array.c::scoop_array_get_composite`
     - `runtime/c/scoop_array.c::scoop_array_set_u64`
     - `runtime/c/scoop_array.c::scoop_array_set_ref`
     - `runtime/c/scoop_array.c::scoop_array_set_composite`
     - `runtime/c/scoop_array.c::scoop_array_len`
     - `crates/scoopc/src/llvm/codegen/runtime_symbols.rs` 与 `runtime_abi.rs` 中对应 constant / declare。
  6. 保留并文档化 v1 substrate 边界：`scoop_array_builder_*` / `scoop_array_alloc`（如有） / `scoop_composite_copy` / write barrier 等仍属于 substrate；其它 array 路径已 IR-direct。
- 必须遵从的约束：
  - 不得为本任务在 codegen 中保留任何"按 receiver FQN 是否为 Array / MutableArray"的旧 by-name 分支；callsite lowering 必须按 method declaration 上的 `@Intrinsic("name")` 标签查表。
  - 不得把 IrEmission entry 改回 RuntimeCall（即使遇到优化或 GC 边界问题）；遇到此类问题应从 RuntimeCall 列表里增加新的细粒度 helper（写明理由），而不是把 IrEmission entry 整个倒回。
  - 不得在本任务里顺手处理其它 runtime helper（`exit`、`print/println`、scalar helper、string helper 等），它们留 v2+ 或后续阶段。
  - 删除 runtime helper 时必须验证仓库内已无 declaration / call site 残留（grep 锁）。
- 验证：
  1. 新增 fixture：`for i in 0..n { sum += arr.get(i) }` 在 `--opt-level=2` 下 LLVM IR 中能 LICM `arr.size()`、能与 BCE 协同把 bounds check 消除（fixture 锁 IR 形态）。
  2. 新增 fixture：`val a = arr.get(0); val b = arr.get(0)` 编译后 LLVM 能 CSE 成单次 load（fixture 锁 IR 形态）。
  3. 新增 fixture：simple `for i in 0..n { sum += arr.get(i) }`（T = Int）在合适机器上能被 LLVM 自动向量化（fixture 不锁机器码，仅锁"loop body 中无 opaque function call"）。
  4. 新增 fixture：`MutableArray<ClassT>.set(i, value)` 触发 write barrier（runtime call site 仍存在，fixture 锁 IR 形态包含 barrier call）。
  5. 新增 fixture：`MutableArray<StructT>.set(i, value)`（composite element）正确走 RuntimeCall composite_copy（fixture 锁 IR 形态包含 composite_copy call + run-pass 验证语义）。
  6. 现有所有 array 相关 pass fixture 不退化。
  7. 仓库 grep：`scoop_array_get_u64` / `scoop_array_get_ref` / `scoop_array_get_composite` / `scoop_array_set_u64` / `scoop_array_set_ref` / `scoop_array_set_composite` / `scoop_array_len` 在编译器源码与 runtime/c 中均无残留。
  8. `cargo test -p scoopc llvm_tests -- --nocapture`：现有 IR 锁定测试不退化（涉及 array 的需更新为 IR-direct 形态）。
  9. `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass`：所有现有 pass fixture 不退化。
  10. `SCOOP_GC_MOVE=1 SCOOP_GC_STRESS=1 cargo run -p scoop -- test --fixtures <array-fixtures>`：write barrier 与 move-GC composite copy 的 RuntimeCall 通路正确。
- 完成条件：
  - `Array` / `MutableArray` 基础访问 IR-direct，LLVM 优化路径完整可用；
  - 上述 array runtime helper 已删除，substrate 收缩动作完成；
  - intrinsic 表已有真实 first user，"零编译器后门（generic 维度）"约束以可枚举 entry 形式落实。
- 依赖：`P4-T01d`
- 完成记录：
  - 改动范围：
    - `sysroot/core.scoop`
    - `crates/scoopc/src/{intrinsics.rs,resolve/scopes.rs,hir/lower/{mod.rs,stmt.rs},effect_facts/builder.rs,mir/{lower.rs,materialize.rs},llvm/tests.rs,pipeline/llvm_codegen_stage.rs}`
    - `crates/scoopc/src/llvm/codegen/{gc.rs,runtime_symbols.rs,runtime_abi.rs,call/lowering.rs,mir_body.rs,effect_lowered/value.rs,intrinsics/{named.rs,containers.rs}}`
    - `runtime/c/scoop_array.c`
    - `tests/fixtures/build/{array_intrinsic_ir_direct_int_loop_basic.scoop,array_intrinsic_redundant_get_cse_basic.scoop,array_intrinsic_write_barrier_ref_set.scoop,array_intrinsic_composite_copy_set.scoop}`
    - `tests/fixtures/runtime_gc/{array_intrinsic_ref_set_gc_move_basic.scoop,array_intrinsic_composite_copy_gc_move_basic.scoop}`
    - 删除 `crates/scoop_runtime/tests/gc_immix_composite_array_write_barrier.rs`
    - `TODO.md`
    - `memory/claude_plan.md`
  - 核心决策：
    - 把 `Array<T>` / `MutableArray<T>` 从 sysroot extension declaration 改为 `@Intrinsic class` + method-level `@Intrinsic("array_*")` 声明，并让 `List/MutableList/Set/MapView/...` 的 direct member lookup 一并归一化到真实 array carrier，避免 surface 切换后再靠 extension 特例兜底。
    - `named` intrinsic lowering 现在直接按 `ScoopArray` 布局发 IR：`array_size` 直接 load `len`，`array_get` 直接做 bounds check + `data_offset_bytes` GEP + typed load，`array_set` 直接做 typed store；ref/string 元素走 `scoop_gc_write_barrier`，composite 元素走 `scoop_composite_drop` + `scoop_composite_copy` + per-slot write barrier，而不是回旧 `scoop_array_get/set_*` helper。
    - 删除了 `scoop_array_len/get_u64/get_ref/get_composite/set_u64/set_ref/set_composite` 及其 `runtime_symbols/runtime_abi` declaration；保留并显式发布 `scoop_array_alloc`、`scoop_array_builder_grow`、`scoop_composite_copy`、`scoop_gc_write_barrier` 作为 substrate runtime entry/audit 元数据。
    - 对仍会合成 array callsite 的内部路径（`for ... in array` desugar、部分 refactor MIR direct-call）补上 synthetic call binding / FQN fallback，保证 declaration-only named intrinsic method 在没有 user-source call binding 的情况下也能稳定回到同一 intrinsic 表，而不是退回“缺少 callable signature”或旧 by-name helper 分流。
  - 验证结果：
    - `cargo fmt --all`
    - `cargo test -p scoopc named_intrinsic -- --nocapture`
    - `cargo test -p scoopc array_ -- --nocapture`
    - `cargo test -p scoopc refactor_llvm_array_composite_transport -- --nocapture`
    - `cargo run -p scoop -- test --fixtures tests/fixtures/typecheck/list_and_mutable_list_alias_ok.scoop`
    - `cargo run -p scoop -- test --fixtures tests/fixtures/typecheck/array_mutable_array_min_api_surface_ok.scoop`
    - `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/{array_mutable_array_min_primitive_basic.scoop,array_composite_transport_basic.scoop,for_in_array_int_basic.scoop,mutable_array_ops_basic.scoop,vararg_spread_basic.scoop,stdlib_set_map_basic.scoop,stdlib_hash_set_map_basic.scoop,stdlib_smoke_collections_and_iteration.scoop}`
    - `cargo run -p scoop -- test --fixtures tests/fixtures/build/{array_intrinsic_ir_direct_int_loop_basic.scoop,array_intrinsic_redundant_get_cse_basic.scoop,array_intrinsic_write_barrier_ref_set.scoop,array_intrinsic_composite_copy_set.scoop}`
    - `cargo run -p scoop -- test --fixtures tests/fixtures/runtime_gc/{array_intrinsic_ref_set_gc_move_basic.scoop,array_intrinsic_composite_copy_gc_move_basic.scoop,gc_trace_array_string_elements_basic.scoop}`
    - `SCOOP_GC_MOVE=1 SCOOP_GC_STRESS=1 cargo run -p scoop -- test --fixtures tests/fixtures/runtime_gc/array_intrinsic_ref_set_gc_move_basic.scoop`
    - `SCOOP_GC_MOVE=1 SCOOP_GC_STRESS=1 cargo run -p scoop -- test --fixtures tests/fixtures/runtime_gc/array_intrinsic_composite_copy_gc_move_basic.scoop`
    - `SCOOP_GC_MOVE=1 SCOOP_GC_STRESS=1 cargo run -p scoop -- test --fixtures tests/fixtures/runtime_gc/gc_trace_array_string_elements_basic.scoop`
    - 生产代码 grep：`runtime/c` 已无 `scoop_array_len/get/set_*` 残留；`crates/**` 中只剩测试里的负断言，不再有 production callsite/declaration。
    - `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass`（388 passed；仍保留两个既有失败：`extern_native_aggregate_return_direct_indirect_parity.scoop`、`sync_gc_release_task_like_object_basic.scoop`）
    - `cargo clippy --all-targets -- -D warnings`
  - 与 `PLAN.md` / `MANAGED_ABI.md` 的对应闭合：
    - 对应 `PLAN.md` P4 前置 II 的第 2-4 项：`Array` / `MutableArray` 已成为 named intrinsic 表的真实 first user，基础访问恢复为 IR-direct emission，旧 array runtime helper surface 已被收缩，LLVM 的 LICM/CSE/BCE/向量化窗口重新打开。
    - 本任务没有改变 `MANAGED_ABI.md` 约定的 ABI surface；改动仅限 array substrate 与 intrinsic lowering source-of-truth，因此无需回写 `MANAGED_ABI.md`。

## P4：string cone tracer bullet

### [TODO] P4-T01：以标量 `toString` 为 tracer bullet，删除第一批字符串名字特判

- 参考：
  - [`PLAN.md`](./PLAN.md) §5 / P4
  - [`MANAGED_ABI.md`](./MANAGED_ABI.md) §9、§10
- 目标：
  - 先用最小且高频的 string helper 证明“编译器已按 ABI 而不是按名字工作”。
  - 第一批目标限定为标量/简单自返 helpers，避免一开始就把所有 string 逻辑绑在一起。
  - 实现路径以 P4 前置（P4-T01a/b/c）已铺好的"内建类型作为一等 struct/class implementer"机制为基础：把 `Int` / `Bool` / `Char` / `Float32` / `Float64` / `String` 这几个内建类型从 declaration-only `struct/class : Hashable, ToString` 改为 `@Intrinsic struct/class : Hashable, ToString { override fun toString(): String { ... } }` 形式，method body 在 sysroot 内用 Scoop 写出（`@Extern(abi = "scoop")` wrapper 调 runtime helper）。
  - 本任务不动 `Hashable.hash` 这一侧；只迁 `ToString.toString`。`Hashable` 等其它内建 interface 留以后做（按"零编译器后门"约束，未来加这些 interface 不再要求改编译器）。
- 当前实现入口：
  - `sysroot/core.scoop`（内建类型 declaration-only 形式）
  - `crates/scoopc/src/resolve/scopes.rs`（`toString` 的 builtin allowlist）
  - `crates/scoopc/src/typecheck/expr/call.rs`（内建 `toString` 直接返回 String 的 synthetic contract）
  - `crates/scoopc/src/hir/lower/expr.rs::should_keep_member_call_as_member_access`（`toString` keep-list）
  - `crates/scoopc/src/llvm/codegen/intrinsics/builtin.rs::{try_codegen_tostring_iface_builtin,codegen_sysroot_to_string_ext}`
  - `crates/scoopc/src/llvm/codegen/effect_lowered/body.rs::lower_builtin_to_string_call`
  - `crates/scoopc/src/llvm/codegen/effect_lowered/value.rs::{lower_refactor_core_to_string_call,refactor_core_print_to_string}`
  - `crates/scoopc/src/llvm/codegen/mir_body.rs::codegen_mir_transport_to_string`
  - `runtime/c/scoop_runtime.c` 中 `scoop_{bool,char,int,float32,float64}_to_string`
- 必须实现的内容：
  1. 在 sysroot 中把以下内建类型改写为 `@Intrinsic struct/class with body methods`（依赖 P4-T01c 已完成的 surface），override `ToString.toString`：
     - `Int.toString`
     - `Bool.toString`
     - `Char.toString`
     - `Float32.toString`
     - `Float64.toString`
     - `String.toString`
  2. method body 用 Scoop 写出：
     - `Bool.toString` / `String.toString` 用纯 Scoop 表达（`if (this) "true" else "false"` / `return this`）；
     - `Int.toString` / `Char.toString` / `Float64.toString` / `Float32.toString` 调用 `@Extern(abi = "scoop")` wrapper（`scoopAbiIntToString` 等），wrapper 链接到 runtime 中已有的 `scoop_{int,char,float64,float32}_to_string`。
  3. 对应清理以下层的名字特判（这些都是 P4-T01a/b/c 之后已经"对内建类型不再被需要"的旁路）：
     - resolver `toString` builtin allowlist；
     - typecheck 中内建 `toString` 的 synthetic 直接返回类型；
     - HIR `should_keep_member_call_as_member_access` 中 `toString` keep-list 条目；
     - `try_codegen_tostring_iface_builtin` / `codegen_sysroot_to_string_ext`；
     - `effect_lowered/{body.rs,value.rs}` 中 `ToString.toString` / scalar `toString` ext 拦截；
     - `codegen_mir_transport_to_string` 中按 `CgTy` 派生 runtime helper 名字的分支。
  4. `ToString.toString` interface dispatch 对用户类型仍可用（这条由 P4-T01a/b/c 已经保证）；本任务删除的所有路径都必须有对应的"用户类型也走过这条路径"证据，否则不能删。
  5. 同步检查 `print/println` 链路，确保它们最终也不再依赖旧的 scalar `toString` runtime intercept。
- 必须遵从的约束：
  - 不得绕过 P4 前置的机制：不得为了图省事保留任何"内建类型 only" 的代码路径（哪怕是为了过渡）。
  - 不得留下"新机制已接通，但旧 `try_codegen_tostring_iface_builtin` 仍完整保留"的双轨。
  - 不得回退 `Float.toString` 现有 IR / run-pass coverage。
  - 不得在本任务里顺手处理 `Hashable.hash` / `Char.hash` / `Float*.hash` / `String.hash` 等其它内建 interface method；它们留以后做，且按"零编译器后门"约束届时不再要求改编译器。
- 验证：
  1. 现有 `llvm/tests.rs` 中 `Float64.toString` / `Float32.toString` / `ToString.toString` 相关单测
  2. 新增 `managed_abi` 版 scalar toString build/run-pass fixture
  3. `SCOOP_GC_MOVE=1 SCOOP_GC_STRESS=1 cargo run -p scoop -- test --fixtures <scalar-tostring-fixtures>`
- 完成条件：
  - 第一批标量 `toString` helper 已不再依赖 compiler/runtime FQN intercept。
- 依赖：`P4-T01e`（顺序约束：scalar toString 不直接依赖 P4 前置 II 的 generic intrinsic 表机制，但顺序排在其后避免 intrinsic 机制半完成时穿插对内建类型的 sysroot 改写）

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
      - dedicated managed import/export surface（例如 `import function` / `import interface`），而不是 `FunPtr` ABI 扩展；
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
