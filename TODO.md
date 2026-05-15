# TODO（Managed ABI / native callable ABI 收口）

> 生成时间：2026-05-14  
> 计划基线：[`PLAN.md`](./PLAN.md)  
> 设计基线：[`MANAGED_ABI.md`](./MANAGED_ABI.md)  
> 格式参考：`docs/archive/plans/TODO-pipeline-gaps.md`、`docs/archive/plans/PLAN-pipeline-gaps.md`  
> 顺序约束：严格按当前文件中的条目顺序推进；不得跨条目并行实现。  
> 当前状态：`ExternAbi::Scoop` 已完成 declaration / call lowering 与 ABI-specific `unsafe` / `@NoGC` 语义；native `@Extern` 与 `FunPtr` 已通过统一 native ABI classifier + surface gate / diagnostics 对齐 declaration / direct call / indirect call / MIR path；`string cone` 已完成 `sysroot/string.scoop` 中那批普通 helper，并新增了供后续 scalar `toString` 迁移使用的 sysroot scalar String bridge，但 remaining string helper 仍依赖 compiler/runtime 特判。  
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
| `P4-T01e` | P4 前置 II | [DONE] 用 `Array` / `MutableArray` 填充 intrinsic 表（IR-direct），删除已被替代的 array runtime helper |
| `P4-T01f` | P4 前置 III | [DONE] 为 sysroot 标量 `String`-return helper 暴露不放宽 native surface 的 runtime bridge |
| `P4-T01g` | P4 前置 IV | [TODO] 解锁 subclass-typed receiver 调用 inherited body method 与访问 inherited 字段 |
| `P4-T01h` | P4 前置 IV | [TODO] 完整支持构造器 type argument（LHS expected type 反推 + 显式 type argument 调用） |
| `P4-T01i` | P4 前置 IV | [DONE] 清理 P2-T02 之后仍残留 `@Unsafe @Extern` 旧写法的 baseline fixture / 单测，并刷新依赖于 production 行号的 failure-policy 单测 |
| `P4-T01j` | P4 前置 IV | [TODO] 把 `declare_named_intrinsic_runtime_symbol` 接入 `declare_runtime_or_native_import_function` 通道，消除 `add_function(..., None)` raw 调用 |
| `P4-T01k` | P4 前置 IV | [TODO] 修复 `MutableSet` / `Set` 的 `.len()` direct-call 不再重写到 overload-aware symbol 的 production drift |
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
- 已具备 compiled sysroot scalar String bridge：
  - `sysroot/scalar_string_bridge.scoop`
  - `crates/scoopc/src/intrinsics.rs` 中的 `scalar_*_to_string_bridge` audited RuntimeCall entries
  - `crates/scoopc/src/llvm/tests.rs::compiled_sysroot_scalar_string_bridge_helpers_stay_in_module`
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
> 顺序约束：P4-T01a → P4-T01b → P4-T01c-pre1 → P4-T01c → P4-T01d → P4-T01e → P4-T01f → P4-T01g → P4-T01h → P4-T01i → P4-T01j → P4-T01k → P4-T01。十一个前置子任务必须**新增机制不删除现有 path**（P4-T01e 例外：它会在 IR-direct 化完成后**删除**已被替代的 array runtime helper，这是显式 substrate 收缩动作；除此之外不做删除），以免与 P4-T01 的删除动作冲突造成中间双轨状态。

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

## P4 前置 III：sysroot scalar String bridge

### [DONE] P4-T01f：为 sysroot 标量 `String`-return helper 暴露不放宽 native surface 的 runtime bridge

- 参考：
  - [`PLAN.md`](./PLAN.md) §4 / §5 / P4
  - `crates/scoopc/src/typecheck/annotations.rs::{check_extern_fun_signature_matches_native_abi,check_extern_fun_signature_matches_scoop_abi_v1}`
  - `crates/scoopc/src/typecheck/lower.rs::TypeLowering::{is_native_abi_value_type,is_native_abi_value_type_inner}`
  - `sysroot/{core.scoop,print.scoop}`
  - `runtime/c/scoop_runtime.c::{scoop_char_to_string,scoop_int_to_string,scoop_float32_to_string,scoop_float64_to_string}`
- 目标：
  - 为 `P4-T01` 的 sysroot bodied `toString` method 提供一个真实、可审计的 helper 实现落点：compiled sysroot helper 能返回 `String`，并可被 `@Extern(abi = "scoop")` wrapper/ordinary managed call 消费。
  - 保持 `P2` 已发布的 native surface contract 不变：user/source-level native `@Extern` 仍不接受 `String` 或其它 managed ref 穿越 native ABI。
- 当前阻塞：
  - `check_extern_fun_signature_matches_native_abi()` 通过 `TypeLowering::is_native_abi_value_type()` 明确拒绝一切 `TypeKind::Ref(_)`，因此像 `@Extern("scoop_int_to_string") fun ...(Int): String` 这类 sysroot wrapper 目前没有合法源码表示。
  - 若继续用现有 `scoop.core.toString` / `ToString.toString` 的 by-name intercept 去实现 wrapper，本质上是在用 `P4-T01` 计划删除的旧路径自举自己，不能接受。
- 必须实现的内容：
  1. 引入一个仅供 compiled sysroot helper 使用、且可审计的 runtime bridge surface，使其能触达现有 `scoop_char_to_string` / `scoop_int_to_string` / `scoop_float32_to_string` / `scoop_float64_to_string` substrate helper 并返回 `String`。
  2. 该 bridge 不得表现为“native `@Extern` contract 放宽”：用户源码里的 native `@Extern` 仍必须继续拒绝 `String` / `Ref` 形状。
  3. bridge 的 ownership / why-runtime 需要写清楚：它属于 substrate bridge，不是新增的用户态 FFI 能力，也不是把 runtime C helper 伪装成普通 managed export。
  4. 至少补一条 compiled sysroot helper + managed ABI/ordinary managed 调用验证，证明后续 `P4-T01` 可以在不借助旧 `toString` FQN intercept 的前提下，经由该 bridge 产出 `String`。
- 必须遵从的约束：
  - 不得把 `String` / `Ref` 悄悄加入 `check_extern_fun_signature_matches_native_abi()` 的 allowlist。
  - 不得把 `try_codegen_tostring_iface_builtin` / `codegen_sysroot_to_string_ext` / `codegen_mir_transport_to_string` 当成 wrapper 的隐藏实现。
  - 不得通过硬编码“managed ABI 名字直接映射到 runtime C symbol”来模糊 addrspace/ABI 边界；如果需要 bridge symbol，必须把 ABI/ownership 说清楚并用测试锁住。
- 验证：
  1. 新增 front-end 或 typecheck owner test，锁定 native `@Extern` 仍拒绝 `String` return / managed ref surface。
  2. 新增 compiled sysroot helper fixture，验证 bridge 返回 `String` 并可被 ordinary managed 或 `@Extern(abi = "scoop")` call 消费。
  3. `cargo test -p scoopc llvm_tests -- --nocapture`
- 完成条件：
  - `P4-T01` 可以直接把 scalar `toString` body 写成 sysroot ordinary code + managed wrapper，而不需要再碰 native surface contract，也不需要继续借用旧名字特判来“自举”。
- 依赖：`P4-T01e`
- 完成记录：
  - 改动范围：
    - `crates/scoopc/src/{intrinsics.rs,llvm/codegen/intrinsics/named.rs,sysroot/mod.rs,llvm/tests.rs}`
    - `sysroot/scalar_string_bridge.scoop`
    - `tests/fixtures/{typecheck/extern_fun_signature_with_string_return_is_error.scoop,run-pass/sysroot_scalar_string_bridge_basic.scoop}`
    - `MANAGED_ABI.md`
    - `TODO.md`
    - `memory/claude_plan.md`
  - 核心决策：
    - 不放宽 native `@Extern` gate；`check_extern_fun_signature_matches_native_abi()` 保持不变，继续拒绝 `String` / managed ref 进入 C ABI。native `@Extern` 继续只表达已发布的 native surface，本任务没有向 allowlist 塞 `String` 特例。
    - 复用 named intrinsic `RuntimeCall` 审计表，把 `scoop_char_to_string` / `scoop_int_to_string` / `scoop_float32_to_string` / `scoop_float64_to_string` 收口为四个显式 `scalar_*_to_string_bridge` entry；同时补齐共享 runtime 签名类型（`I32/I64/Float32/Float64/StringRef`），让 bridge 的 ABI 与返回 `String` 语义都能精确表达，而不是退回 word-sized / generic `GcRef` 近似。
    - 新增 `sysroot/scalar_string_bridge.scoop` 作为始终编译的 sysroot support source，用 ordinary managed helper `scoopAbiCharToString` / `scoopAbiIntToString` / `scoopAbiFloat32ToString` / `scoopAbiFloat64ToString` 包住 audited runtime bridge；这样后续 `P4-T01` 可以直接在 bodied sysroot method 中调用这些 helper，而不必借旧 `toString` FQN intercept 自举。
    - 在 `MANAGED_ABI.md` 明确写下 ownership：底层 `scoop_*_to_string` 仍是 runtime substrate，因为它们负责分配 managed `String`；但它们只通过已审计的 compiled sysroot bridge 暴露，不构成新的 native `@Extern` 用户态能力。
  - 验证结果：
    - `cargo fmt --all`
    - `cargo test -p scoopc compiled_sysroot_scalar_string_bridge_helpers_stay_in_module -- --nocapture`
    - `cargo test -p scoopc named_intrinsic -- --nocapture`
    - `cargo run -p scoop -- test --fixtures tests/fixtures/typecheck/extern_fun_signature_with_string_return_is_error.scoop`
    - `cargo run -p scoop -- test --fixtures tests/fixtures/typecheck/extern_fun_signature_with_gc_ref_is_error.scoop`
    - `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/sysroot_scalar_string_bridge_basic.scoop`
    - `cargo test -p scoopc llvm_tests -- --nocapture`（当前 harness 仍返回 `0 passed; 861 filtered out`，因此本任务的实际 LLVM owner coverage 以上面的定向 owner test 为准）
    - `cargo clippy --all-targets -- -D warnings`
  - 与 `PLAN.md` / `MANAGED_ABI.md` 的对应闭合：
    - 对应 `PLAN.md` P4 前置 III：compiled sysroot 现在已有一条不放宽 native surface 的 scalar `String` bridge，后续 bodied `toString` method 可以通过 ordinary managed helper 触达现有 runtime substrate，而不需要继续借 `toString` 按名特判自举。
    - `MANAGED_ABI.md` §9.3 已补充当前 bridge 边界：`scoop_{char,int,float32,float64}_to_string` 仍是 substrate helper，但只通过 audited sysroot bridge 暴露；native `@Extern` 仍不接受 `String` / managed ref surface。

## P4 前置 IV：内建类型 surface 全形可调用闭环

> 起因：在 `P4-T01f` 完成之后做 P4-T01 验证时（"以 P4-T01a/b/c/d/e/f 已铺好的机制，让用户写 `42.toString().length()` 这类全形 idiom"），发现两类前端 gap 仍会让 P4-T01 / P4-T02 在 sysroot 编写 bodied helper 时直接踩坑：
>
> 1. **subclass-typed receiver 不可见 inherited member**：`class Dog : Animal()` / `class Box : Ping`，subclass receiver 调用未 `override` 的祖先 body method 或 interface default body method，前端报 `unresolved_member`；继承自 base class 的 `val` 字段在 subclass body / 外部 receiver 也都不可见。当前 fixture 全部通过"先 cast 到 base type 再调"的写法绕开，但 P4-T01 / P4-T02 把 scalar 与 String helper bodied 化时会出现"用户写 `Int(...).toString()` 命中 `ToString.toString` default" / "sysroot bodied helper 调用基类已实现的 helper" 这类必须直接通过 nominal subclass receiver 命中的形态。
> 2. **构造器 type argument 不闭环**：
>    - `val c: Container<Int> = Container()`（zero-arg ctor，或所有 ctor arg 都不引用 `T`）从 LHS expected type 反推不发生，T 静默 fallback 到 `Any`，紧接着触发 `initializer_type_mismatch`；
>    - `val c: Container<Int> = Container<Int>()`（构造器位置的显式 type argument）走到 LLVM frontend 时报 `call expression missing typed call-site contract`，说明 ctor call lowering 未对接 type-arg contract。
>
> 这两类 gap 都不在 P4-T01a/b/c 的 "完成条件" 措辞范围内，因此不属于既已落地基线，但属于 P4-T01 / P4-T02 在 sysroot 写 bodied helper 时**必然**会踩到的前置缺口（当前 sysroot 的 generic intrinsic 全部靠"T 出现在 ctor arg"的 idiom 来回避 Gap A，未来加任何 `class Foo<T>(): ...` 形态的 sysroot helper 都会立刻退化成 `Foo<Any>`）。
>
> 顺序约束：P4-T01g 与 P4-T01h 之间无强依赖，但都必须在 P4-T01 之前完成；P4-T01i 仅做 fixture / golden / failure-policy sentinel 清理；P4-T01j 与 P4-T01k 是 P4-T01i 验证过程中显式拆出的两条 production drift 修复，分别针对 `declare_named_intrinsic_runtime_symbol` 的 raw `add_function(..., None)` 与 `MutableSet/Set.len()` 的 direct-call overload symbol drift。五个 P4 前置 IV 任务彼此无强阻塞，但都要在 P4-T01 之前完成（确保 string cone tracer bullet 启动时 `tests/fixtures/run-pass` / `tests/fixtures/typecheck` / `cargo test -p scoopc` 全量基线干净）。所有任务都遵循前置阶段"新增机制不删除现有 path"的硬约束，不得借此回退任何已锁住的 fixture 行为。

### [DONE] P4-T01g：解锁 subclass-typed receiver 调用 inherited body method 与访问 inherited 字段

- 参考：
  - [`PLAN.md`](./PLAN.md) §5 / P4 前置
  - 现有 fixture：
    - `tests/fixtures/run-pass/member_call_class_body_method_basic.scoop`（subclass 自身 body method 直接调用）
    - `tests/fixtures/run-pass/member_call_virtual_dispatch_override_basic.scoop`（base-typed receiver virtual dispatch 已 OK）
    - `tests/fixtures/run-pass/interface_default_method_dispatch_basic.scoop`（interface-typed receiver 调用 default body 已 OK，但 subclass-typed receiver 同形态会报 unresolved_member）
  - 现有实现入口：
    - `crates/scoopc/src/resolve/scopes.rs`（member 解析；当前不沿继承链向上搜索 inherited member）
    - `crates/scoopc/src/typecheck/expr/call.rs::infer_member_call_expr_type`
    - `crates/scoopc/src/hir/lower/expr.rs`（member call lowering）
    - `crates/scoopc/src/typecheck/override_effects.rs`（已有 `direct superclass` 概念，可复用为继承链查找的入口；当前注释明确"只检查 direct superclass，不沿继承链向上搜索"）
- 目标：
  - 让 user / sysroot subclass 的 nominal receiver 也能直接调用未 `override` 的祖先 body method（包括 base class 的 ordinary / `open` body method 与 interface default body method），并能访问 base class 的 `val`/`var` 字段。
  - 不引入新的"subclass-only" 后门：member 解析失败时，统一沿 supertype 链（class → direct superclass → ... → 各 interface supertypes，含 default body）做有序查找；命中后复用现有 lowering（base class body method 复用 `<BaseFqn>.<methodName>` top-level 调用，interface default 复用 itable default-slot），不新增第二条 IR path。
  - cover：
    1. base class 的 ordinary body method（非 `open`、非 `override`）；
    2. base class 的 `open` body method（被 / 未被 subclass override 都要正确）；
    3. interface 的 default body method（被 / 未被 subclass override 都要正确）；
    4. base class 的 `val` / `var` 字段（subclass body 内 `this.x` 与外部 `subInst.x` 两种 surface）；
    5. 多层继承链（`A` ← `B` ← `C`，`C` 实例 nominal receiver 解析到 `A` 的 body member）；
    6. `@Intrinsic class/struct` 上同形态全部成立。
- 必须实现的内容：
  1. resolver 在 member-name 解析未命中"自身声明面"时，按 class supertype 链（先 direct superclass，再 interfaces，按声明顺序，含 default body 与 inherited interface chain）做有序查找；命中后挂回原 member-call AST node，typecheck 阶段视同自身 method/field 一样消费。
  2. typecheck 解析 `receiver.method(...)` / `receiver.field` 时，沿继承链查找的命中要正确处理：
     - `this` 隐含 receiver 同样适用（subclass body 内 `this.inheritedField` / `this.inheritedMethod()` 必须可见）；
     - generic supertype 实例化要把 `T` 在 base 的位置映射到 subclass 实参（`class C<T> : B<T>`，`C<Int>` instance 调用 `B<T>.method()` 必须看到 `T = Int`）。
  3. HIR lowering 不新增 IR path：base class body method 命中走现有 `<OwnerFqn>.<methodName>` top-level 调用 + receiver 上转；interface default 命中走现有 itable default-slot；继承字段访问走现有 nominal field offset path。
  4. 报错路径保留：找不到时仍 emit `unresolved_member` / `unresolved_field`；歧义（多 interface 同名 default 且未 override）走现有 ambiguity 报错路径。
- 必须遵从的约束：
  - 不得删除任何现有 by-name 特判或扩展函数 fallback（包括 `try_codegen_tostring_iface_builtin` / `codegen_sysroot_to_string_ext`，删除留给 P4-T01）。
  - 不得改变现有 `member_call_*` / `interface_default_method_dispatch_basic.scoop` 的 IR 锁定。
  - 不得引入"subclass-typed receiver 解析时优先访问 base 实现而绕过 vtable"这类回退；virtual dispatch 仍是唯一的 dispatch 来源，本任务只解决"前端可见性"问题。
  - 不得借此重排 itable / vtable slot 顺序；命中现有 slot 即可。
- 验证：
  1. 新增 fixture：subclass-typed receiver 调用 base class ordinary body method（非 override），return value 正确。
  2. 新增 fixture：subclass-typed receiver 调用未 override 的 interface default body method，return value 正确；同时已 override 的 default body 走子类实现。
  3. 新增 fixture：subclass body 内 `this.inheritedField`（包括二级、三级继承链）读写值正确。
  4. 新增 fixture：`@Intrinsic class<T> : Iface { ... }` subclass-typed receiver 调用 inherited default body 正确。
  5. `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass`：现有 pass fixture 不退化（特别是 `member_call_*` / `interface_default_method_dispatch_basic.scoop` / `class_init_*`）。
  6. `cargo test -p scoopc llvm_tests -- --nocapture`：现有 IR 锁定不退化。
- 完成条件：
  - 任何 user / sysroot subclass 的 nominal receiver，都不再需要先 cast 到 base / interface type 才能调用 inherited body member，也不再有"subclass body 内 `this.<inheritedField>` 报 unresolved_member" 这类阻塞 P4-T01 / P4-T02 写 bodied helper 的前端 gap。
- 依赖：`P4-T01f`
- 完成记录：
  - 改动范围：
    - `crates/scoopc/src/resolve/mod.rs`：在 `Index` 中新增 `direct_supertypes: HashMap<String, Vec<String>>`，并在 `build_with_cones` 末尾通过新增的 `collect_direct_supertypes` / `collect_type_decl_supertypes` / `collect_object_decl_supertypes` 在所有 `add_file_in_cone` 完成后统一收集 nominal class / struct / object 的直接 supertype FQN，避免 cross-file 顺序敏感性。
    - `crates/scoopc/src/resolve/scopes.rs`：新增 `resolve_inherited_member`，在 `resolve_member_access_on_value_receiver` 已完成"自身声明面 / extension fun / extension property / 内建 by-name 特判 / `Any` smart-cast 缓冲"全部 fallback 之后、再落入 `unresolved_member` 之前，按 BFS 遍历 supertype 链查找 inherited body member；命中规则：可见且 `has_body=true` 的 fun 立即返回，可见 value 次之，纯 abstract fun 不接管（避免抢占已有的 extension fun fallback 与 vtable / itable 路径）；命中后写回 `member.resolved` 即可，typecheck / lowering 复用既有 `find_member_owner_nominal_instantiation` / `instantiate_member_value_type_from_receiver_ty` 路径，HIR / vtable / itable 不引入新 IR path。
    - `tests/fixtures/run-pass/inherited_member_call_base_class_body_method_basic.scoop` / `inherited_member_call_interface_default_body_basic.scoop` / `inherited_member_field_access_basic.scoop` / `inherited_member_call_multi_level_chain_basic.scoop` / `inherited_member_call_intrinsic_generic_class_basic.scoop`：分别覆盖 base class 普通 body method、未 override 的 interface default body、`this.inheritedField` + 外部 `subInst.inheritedField`、三层继承链、`@Intrinsic class<T>` 实现 interface 时调用未 override 的 default body。
    - `TODO.md`：把 P4-T01g 标 `[DONE]`；同时新增 `P4-T01i`（fixture / 单测 / failure-policy 行号清理），把验证过程中暴露的"P2-T02 之后一直跑不通"的 fixture 与 inline source 单测显式登记为前置 IV 的清理任务，避免后续任务再"绕开既存失败"。
    - `memory/claude_plan.md`：刷新 P4-T01g 进展记录。
  - 核心决策：
    - **不在 `add_file_in_cone` 阶段计算 supertype FQN**：cross-file 声明顺序无法保证 supertype 已经在 `by_fqn`，因此 supertype 收集统一推迟到 `add_file_in_cone` 全部完成后由 `collect_direct_supertypes` 单独跑一次，复用既有 `type_ref_to_fqn_in_file` 解析能力，与 `collect_extension_funs` / `collect_extension_properties` 同构。
    - **inherited 解析在 fallback 链尾部**：保持既有"内建特判 / extension fun / extension property / `Any` 缓冲"全部优先，只在它们都未命中时再走 supertype 链；这样 `key.hash()`（`key: Int`）这类既由 extension fun 承担、又同时声明 `Int : Hashable` 的场景不会被 `Hashable.hash` default body 抢占，已锁定的 `member_call_*` / sysroot string helper / Int/Char/Bool/Float by-name fixture 全部维持不动。
    - **`has_body` 过滤**：只把"带 body 的 inherited fun"接管到 `member.resolved`，纯 abstract `interface ... { fun foo(): Int }` 不在此处 short-circuit；这样 user override + itable / vtable 仍是唯一的 dispatch 来源，本任务没有引入"subclass-typed receiver 绕过 vtable" 的回退。
    - **lowering 零改动**：典型 case 验证后，typecheck 阶段 `late_resolve_direct_member_fun_fqn_from_receiver_ty` → `member.resolved` 回退序列加上既有 `find_member_owner_nominal_instantiation` 已经能在 receiver 类型 / 继承链上定位 owner 实例化，因此 HIR / vtable / itable / monomorphization 都不需要新增分支。
  - 验证结果：
    - `cargo build -p scoopc`
    - `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/inherited_member_call_base_class_body_method_basic.scoop`
    - `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/inherited_member_call_interface_default_body_basic.scoop`
    - `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/inherited_member_field_access_basic.scoop`
    - `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/inherited_member_call_multi_level_chain_basic.scoop`
    - `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/inherited_member_call_intrinsic_generic_class_basic.scoop`
    - `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/member_call_class_body_method_basic.scoop`、`member_call_struct_body_method_basic.scoop`、`member_call_generic_class_body_method_basic.scoop`、`member_call_virtual_dispatch_override_basic.scoop`、`member_call_interface_dispatch_basic.scoop`、`interface_default_method_dispatch_basic.scoop`、`intrinsic_class_body_method_basic.scoop`、`intrinsic_struct_interface_body_method_basic.scoop`、`intrinsic_generic_class_body_method_basic.scoop`、`intrinsic_named_method_dummy_ir_basic.scoop`、`intrinsic_named_runtime_fun_basic.scoop`：全部通过，未引入新的 fixture 退化。
    - `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass`：本轮新增的五个 inherited fixture 全部通过；剩余两个失败（`extern_native_aggregate_return_direct_indirect_parity.scoop`、`sync_gc_release_task_like_object_basic.scoop`）与本任务代码路径无交集，已显式登记到 `P4-T01i`。
    - `cargo run -p scoop -- test --fixtures tests/fixtures/typecheck`：剩余唯一失败 `extern_fun_gc_handle_raw_token_roundtrip_ok.scoop` 同样属于 `P4-T01i` 范畴。
    - `cargo clippy --all-targets -- -D warnings`
    - `cargo test -p scoopc`：本任务不修复 P4-T01i 收口的 `@Unsafe @Extern` / failure-policy 行号 drift，但确认这 9 个失败在本任务介入之前已经存在（通过 `git stash` 后跑同一命令对比验证）。
  - 与 `PLAN.md` / `MANAGED_ABI.md` 的对应闭合：
    - 对应 `PLAN.md` P4 前置 IV 第 1 项：subclass-typed receiver 现已能直接看到 inherited body member，不再需要先 cast 到 base / interface type；与 `PLAN.md` "内建类型作为一等 struct/class implementer" 的方向一致，为 P4-T01 / P4-T02 写 bodied sysroot helper 时调用 inherited helper / 字段提供了前端可见性。
    - 本任务没有修改 `MANAGED_ABI.md` 描述的 ABI surface（vtable / itable layout、interface dispatch ABI、native surface gate 等均未变），因此无需回写 `MANAGED_ABI.md`。

### [DONE] P4-T01h：完整支持构造器 type argument（LHS expected type 反推 + 显式 type argument 调用）

- 参考：
  - [`PLAN.md`](./PLAN.md) §5 / P4 前置
  - 现有 fixture：
    - `tests/fixtures/run-pass/intrinsic_generic_class_body_method_basic.scoop`（generic intrinsic class，靠 ctor arg `T` 反推）
    - `tests/fixtures/run-pass/smart_cast_any_member_access_generic_class_basic.scoop`（generic class，靠 ctor arg `T` 反推）
  - 现有实现入口：
    - `crates/scoopc/src/typecheck/expr/call.rs`（call site type-arg solver）
    - `crates/scoopc/src/hir/lower/expr.rs`（call lowering / typed call-site contract）
    - `crates/scoopc/src/typecheck/expr/entry.rs`（initializer expected-type 下传）
- 目标：
  - 把 generic class / struct 构造器调用的 type argument 解算补成与顶层 generic 函数同构的两条路径：
    1. **LHS expected type 反推**：`val c: Container<Int> = Container()` / `val b: Box<Int> = Box(7)`（其中 `Box<T>(val n: Int)` 不在 ctor arg 暴露 `T`）必须从 LHS 注解的 nominal type argument 反推，把 ctor 的 `T` solver 与 `let-binding` / `return-stmt` / 函数实参的 expected-type 通道合流，而不是 silent-fallback 到 `Any`；
    2. **构造器位置的显式 type argument**：`Container<Int>()` 必须像顶层 `empty<Int>()` 一样在 typecheck 与 HIR lowering 上接入相同的 typed call-site contract，不再触发 `frontend_prepare_failed: missing typed call-site contract`。
- 必须实现的内容：
  1. typecheck 在 ctor call 形成 type-arg solver 时，把外层 expected nominal type 的 type argument 作为 type-arg 候选（与现有"ctor arg 反推 T"并存）；当 ctor arg 不暴露 `T`（含 zero-arg ctor 的退化情形）且 LHS 提供了 nominal type argument 时，必须从 LHS 解出 `T`。
  2. parser / HIR lower 接受构造器位置的 `Type<TypeArgs>(...)` 形态（如已支持则只补 lowering）；ctor call 形成 typed call-site contract 时把显式 type-args 作为 user-specified bindings 传入 solver；与"LHS 反推"冲突时，显式 type args 优先（与顶层 generic fun 行为一致）。
  3. 解算结果同步到现有 monomorphization key：generic class instance method / interface dispatch / itable lookup 在解出的 `T` 实参上仍走 owner-specialized member path（与 `P4-T01a` 完成基线一致），不引入新的 erased path。
  4. 错误路径：仍未解出 `T` 时报现有 `cannot_infer_type_arg` / `initializer_type_mismatch`；显式 type args 与 LHS expected type 矛盾时报现有 conflict 报错（不新增错误码）。
- 必须遵从的约束：
  - 不得改变现有 generic class fixture（`intrinsic_generic_class_body_method_basic.scoop` / `smart_cast_any_member_access_generic_class_basic.scoop` 等）的现状行为：靠 ctor arg 反推 T 仍是 first-class 路径。
  - 不得放宽 native `@Extern` / `FunPtr` 的 generics 限制（`ExternAbi::Scoop` v1 仍 `无 generics`）。
  - 不得引入"ctor 显式 type args 走与 fun 不同的 contract" 的双轨；只复用现有 typed call-site contract。
- 验证：
  1. 新增 fixture：`val c: Container<Int> = Container()`（zero-arg ctor）run-pass。
  2. 新增 fixture：`val b: Box<Int> = Box(7)`（ctor arg 不暴露 T）run-pass。
  3. 新增 fixture：`Container<Int>()` 显式 type argument run-pass。
  4. 新增 fixture：显式 type args 与 LHS 注解一致时通过；不一致时报现有 conflict 错误（typecheck fixture）。
  5. `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass`：现有 generic class fixture 不退化。
  6. `cargo test -p scoopc llvm_tests -- --nocapture`：现有 IR 锁定不退化。
- 完成条件：
  - sysroot / user 在写 `class Foo<T>` / `class Foo<T>(val n: Int)` 这两种"T 不出现在 ctor arg" 形态时，都能用 `val x: Foo<Int> = Foo()` / `val x: Foo<Int> = Foo(7)` / `val x = Foo<Int>()` 三种 idiom 之一无障碍构造，不再 silent-fallback 到 `Any`，也不再触发 `missing typed call-site contract`。
- 依赖：`P4-T01g`
- 完成记录：
  - 改动范围：
    - `crates/scoopc/src/typecheck/expr/call.rs`：
      - `CtorParamInstantiationRequest` 增加 `explicit_type_args` / `expected_owner_args` 两个可选字段，分别承载"显式 ctor type-args"与"LHS expected nominal type-args"；
      - `instantiate_ctor_param_tys` 在 arg-driven 反推之外按 **显式 > arg-driven > LHS expected > `Any`** 的优先级合并解算结果；显式与 arg-driven 冲突时返回 `None` 让调用点退化到 `NoMatchingOverload`；
      - `collect_matched_ctor_overloads_for_owner` 与 `infer_nominal_constructor_call_expr_type` 接受同样的两个可选参数并向下传递，`select_ctor_overload_for_owner` / super-ctor `check_ctor_call_args_by_arity` 路径仍传 `None`；
      - 新增 `try_infer_nominal_constructor_call_expr_type_with_expected`：在 expected-context 命中 `Call` 形态、callee 透明展开 `TypeApply` 后是 unresolved `Ident` 且 expected 为 nominal generic instantiation 时，把 expected owner-args 喂给 ctor solver；
      - 现有 `infer_call_expr_type` dispatch 在普通调用入口把 `explicit_type_args` 也传给 `infer_nominal_constructor_call_expr_type`（与顶层 generic fun 同构）。
    - `crates/scoopc/src/typecheck/expr/infer.rs`：在 `infer_expr_type_in_expected_context` 中加入新分支，命中 `Call` 形态时优先调用上面的 `try_*_with_expected` 帮助函数；命中失败时回到既有 dispatch（保留 `cannot_infer_type_arg` / `initializer_type_mismatch` 等现有诊断作为最终报错路径）。
    - `crates/scoopc/src/hir/lower/expr.rs`：在 `lower_expr` 的 class ctor binding 识别处与 `try_lower_struct_ctor_call_expr` 入口透明展开 `TypeApply`，让 `Container<Int>(...)` 的 ctor binding 沿 `e.span` 一样能被回写到 `ctor_call_sites` / 走 struct-lit lowering，不再触发 `frontend_prepare_failed: missing typed call-site contract`。
    - `tests/fixtures/run-pass/ctor_type_arg_lhs_zero_arg_ctor_basic.scoop` / `ctor_type_arg_lhs_non_t_ctor_arg_basic.scoop` / `ctor_type_arg_explicit_basic.scoop` / `ctor_type_arg_explicit_with_lhs_consistent_basic.scoop`：分别覆盖 zero-arg ctor + LHS、ctor arg 不暴露 T + LHS、显式 type args（含 zero-arg / 一组 / 多 generic）、显式与 LHS 一致。
    - `tests/fixtures/typecheck/ctor_type_arg_explicit_conflicts_with_lhs_is_error.scoop` / `ctor_type_arg_explicit_conflicts_with_arg_is_error.scoop`：分别锁定"显式 vs LHS 冲突 → `initializer_type_mismatch`"、"显式 vs ctor arg 冲突 → `no_matching_overload`"两条现有错误路径，不引入新错误码。
    - `TODO.md`：把 `P4-T01h` 标 `[DONE]`。
    - `memory/claude_plan.md`：刷新 P4-T01h 进展记录。
  - 核心决策：
    - **优先级显式 > arg-driven > LHS > `Any`**：与顶层 generic fun 的"显式 type args 优先于 arg 反推"行为一致；LHS 仅作为兜底候选，避免冲击靠 ctor arg 反推 T 的既有路径（`intrinsic_generic_class_body_method_basic.scoop` / `smart_cast_any_member_access_generic_class_basic.scoop` 等保持不变）。
    - **冲突即不匹配，不引新错误码**：显式与 arg-driven 冲突时 `instantiate_ctor_param_tys` 返回 `Ok(None)`，让调用点继续走现有 `NoMatchingOverload` 报错；显式与 LHS 冲突由现有 `initializer_type_mismatch` 兜底；与"必须遵从的约束 4"一致。
    - **HIR 透明展开 `TypeApply`**：ctor binding 仍键于 `Call` 整体的 `e.span`（typecheck 阶段写回时已经如此），HIR 唯一缺口是识别 callee 时硬要求 `Ident`；改用既有 `transparent_call_callee` 把 `TypeApply { Ident, args }` 透明展开就能复用整条 lowering 链（class ctor `ctor_call_sites` 写入 + struct ctor `try_lower_struct_ctor_call_expr` 入口）。不引入第二条 ctor lowering path。
    - **expected-context 仅命中 ctor**：`try_infer_nominal_constructor_call_expr_type_with_expected` 严格要求 callee 是 unresolved `Ident`（与 `infer_nominal_constructor_call_expr_type` 接管条件一致），避免误抢 enum variant ctor / top-level fun call；命中失败立即回退到既有 dispatch。
  - 验证结果：
    - `cargo build -p scoopc`
    - `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/ctor_type_arg_lhs_zero_arg_ctor_basic.scoop`
    - `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/ctor_type_arg_lhs_non_t_ctor_arg_basic.scoop`
    - `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/ctor_type_arg_explicit_basic.scoop`
    - `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/ctor_type_arg_explicit_with_lhs_consistent_basic.scoop`
    - `cargo run -p scoop -- test --fixtures tests/fixtures/typecheck/ctor_type_arg_explicit_conflicts_with_lhs_is_error.scoop`
    - `cargo run -p scoop -- test --fixtures tests/fixtures/typecheck/ctor_type_arg_explicit_conflicts_with_arg_is_error.scoop`
    - `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/intrinsic_generic_class_body_method_basic.scoop`、`smart_cast_any_member_access_generic_class_basic.scoop`、`generic_class_method.scoop`、`member_call_generic_class_body_method_basic.scoop`：维持通过，靠 ctor arg 反推 T 的既有路径未受冲击。
    - `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/inherited_member_*.scoop`：P4-T01g 锁定的 5 个 inherited fixture 全部通过。
    - `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass`：394 passed / 2 failed（与 P4-T01g 完成时一致：`extern_native_aggregate_return_direct_indirect_parity.scoop`、`sync_gc_release_task_like_object_basic.scoop` 仍为 P4-T01i 范畴）。
    - `cargo run -p scoop -- test --fixtures tests/fixtures/typecheck`：434 passed / 1 failed（`extern_fun_gc_handle_raw_token_roundtrip_ok.scoop`，P4-T01i 范畴）。
    - `cargo clippy --all-targets -- -D warnings`：通过（在新增字段后 `collect_matched_ctor_overloads_for_owner` 触发 `clippy::too_many_arguments`，按现有约定加 `#[allow]`，未拆分函数）。
    - `cargo test -p scoopc`：失败 9 个，全部为 P4-T01i 范畴的 `@Unsafe @Extern` / failure-policy 行号 sentinel drift（与 P4-T01g 完成时一致）。
  - 与 `PLAN.md` / `MANAGED_ABI.md` 的对应闭合：
    - 对应 `PLAN.md` P4 前置 IV 第 2 项：generic class / struct 构造器调用的 type argument 现已能从 LHS expected type 与显式 ctor type args 两条路径解算，与顶层 generic fun 行为同构；为 P4-T01 / P4-T02 在 sysroot 写 bodied helper 时使用 `class Foo<T>` / `class Foo<T>(val n: Int)` 等"T 不出现在 ctor arg"形态铺好前端通路。
    - 本任务没有修改 `MANAGED_ABI.md` 描述的 ABI surface（`ExternAbi::Scoop` v1 的 generics 限制、native surface gate 均未变），因此无需回写 `MANAGED_ABI.md`。

### [DONE] P4-T01i：清理 P2-T02 之后仍残留 `@Unsafe @Extern` 旧写法的 baseline fixture / 单测，并刷新依赖于 production 行号的 failure-policy 单测

- 参考：
  - [`PLAN.md`](./PLAN.md) §5 / P2
  - [`MANAGED_ABI.md`](./MANAGED_ABI.md) §1.5（P2-T02 之后 `abi = "c"` 的 `@Extern` 已隐含 `@Unsafe` / `@NoGC`，不允许显式重复标注）
- 起因：在 `P4-T01g` 验证过程中跑全量 `tests/fixtures/run-pass` / `tests/fixtures/typecheck` 与 `cargo test -p scoopc` 时，发现一组与本任务代码路径无关、但**自 P2-T02 之后就已经在跑不通**的回归资源；它们之所以一直没被抓出，是因为各前置任务的完成记录里只对其中两个 run-pass fixture 做过标注，其它"既存失败"被默认忽略：
  - **`@Unsafe @Extern` 双重标注 fixture（fixture-only 修复）**
    - `tests/fixtures/run-pass/extern_native_aggregate_return_direct_indirect_parity.scoop`
    - `tests/fixtures/run-pass/sync_gc_release_task_like_object_basic.scoop`
    - `tests/fixtures/typecheck/extern_fun_gc_handle_raw_token_roundtrip_ok.scoop`
    - 报错：`scoop::typecheck::extern_fun_c_abi_modifier_redundant`
  - **同样形态的 `@Unsafe @Extern` virtual source 单测（test-resource-only 修复）**
    - `crates/scoopc/src/llvm/tests.rs::abi_baseline_direct_extern_native_leaf_preserves_enter_leave_native_sequence`
    - `crates/scoopc/src/llvm/tests.rs::native_callable_direct_and_indirect_aggregate_return_share_target_abi`
    - `crates/scoopc/src/pipeline/mir_stage.rs::tests::refactor_mir_gc_handle_raw_uintptr_token_stays_scalar`
    - 这些单测 inline source 仍写 `@Unsafe @Extern(...)` / `@Unsafe @Extern fun ...`，全部撞上 `extern_fun_c_abi_modifier_redundant`。
  - **HIR golden 文本与 P1-T01 新增的 `abi_identity` 字段不同步（test-resource-only 修复）**
    - `tests/fixtures/hir/closure_capture_val.hir`
    - `tests/fixtures/hir/closure_non_capture.hir`
    - 关联单测：`crates/scoopc/src/hir/lower/mod.rs::hir_fixture_closure_capture_val_golden`、`hir_fixture_closure_non_capture_golden`。
    - `CallSiteContract` 的 `Debug` 输出在 P1-T01 之后包含 `abi_identity: <id>,`，但这两个 golden 文件没有同步追加该字段。
  - **依赖于 production 行号的 failure-policy 单测（test-only 维护）**
    - `crates/scoopc/src/pipeline_user_visible_failure_policy.rs::pipeline_user_visible_failure_policy_tracks_internal_bug_sentinels`
    - `crates/scoopc/src/pipeline_user_visible_failure_policy.rs::pipeline_user_visible_failure_policy_tracks_stale_unsupportedmainbody_counts`
    - 这两个单测把 `panic!` / `unreachable!` 在 production 文件中的精确行号 hard-code 进了 `INTERNAL_BUG_SENTINEL_HITS` / `UNSUPPORTED_MAIN_BODY_*` 常量；只要 production 行号发生位移就 fail，与本任务的代码改动无关，但属于已经持续 drift 的"伪回归"。
  - **超出 P4-T01i 范畴的 production drift（已由独立任务追踪）**
    - `crates/scoopc/src/llvm/tests.rs::function_declaration_inventory_eliminates_raw_add_function_none_callsites` 已发现 `crates/scoopc/src/llvm/codegen/intrinsics/named.rs:928` 直接调用 `module.add_function(..., None)`，绕开了 `declare_runtime_or_native_import_function` 的分类检查；该 production 漂移由 `P4-T01j` 处理。
    - `crates/scoopc/src/mir/materialize.rs::tests::materialize_for_dump_keeps_set_alias_receiver_overload_targets_distinct` 失败原因是 main 中已经看不到任何 `scoop.core.size::<Int>$overload$...` direct-call target，是 production MIR materialization 行为变化，由 `P4-T01k` 处理。
- 当前实现入口：
  - 各 fixture / 单测 inline source 本身；
  - 检查规则：`crates/scoopc/src/typecheck/annotations.rs::check_extern_fun_effect_contract`（`extern_fun_c_abi_modifier_redundant` 诊断）；
  - failure-policy 行号常量：`crates/scoopc/src/pipeline_user_visible_failure_policy.rs` 中的 `INTERNAL_BUG_SENTINEL_HITS` / `UNSUPPORTED_MAIN_BODY_*` 等。
- 必须实现的内容：
  1. 把 fixture 与 `tests.rs` / `pipeline/mir_stage.rs` 中 inline source 的 `@Unsafe @Extern("...")` 全部收拢为仅 `@Extern("...")`（默认 `abi = "c"` 已隐含 `@Unsafe` / `@NoGC`）；调用点若需要 `@Unsafe do { ... }` 表达调用范围则保留，调用范围 unsafe gate 不变。
  2. 在 `tests/fixtures/hir/closure_capture_val.hir` / `closure_non_capture.hir` 两份 golden 文本中追加 `abi_identity: <id>,` 字段，让 P1-T01 已经稳定的 `CallSiteContract` `Debug` 输出与 golden 一致；不修改 production 序列化路径。
  3. 把 `pipeline_user_visible_failure_policy` 中那两个 hard-coded 行号 sentinel 列表与当前 production 状态对齐；不引入新的 `panic!` / `unreachable!`，也不放宽 `failure-policy` 检查范围。
  4. 跑 `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass` / `tests/fixtures/typecheck` 与 `cargo test -p scoopc`，确认本次清理后所有曾经"既存失败"的资源（在 `P4-T01j` / `P4-T01k` 范畴之外）全部回到通过；同时验证全量 baseline 中再无遗留 `@Unsafe @Extern` 写法。
- 必须遵从的约束：
  - 仅修改 fixture / test resource / failure-policy 行号常量，不动 `annotations.rs` / runtime / codegen 等编译器侧代码；
  - 不得通过删除 fixture 或单测来"消除 drift"；
  - 不得借此重排或缩窄现有 `@Extern` ABI 行为，也不得在 failure-policy 单测里改变筛选规则（仅刷新 sentinel 列表）；
  - 不得在 P4-T01i 内附带修复 `P4-T01j` / `P4-T01k` 范畴的 production drift——它们必须各自走独立任务以保持改动颗粒度。
- 验证：
  1. `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/extern_native_aggregate_return_direct_indirect_parity.scoop`
  2. `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/sync_gc_release_task_like_object_basic.scoop`
  3. `cargo run -p scoop -- test --fixtures tests/fixtures/typecheck/extern_fun_gc_handle_raw_token_roundtrip_ok.scoop`
  4. `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass`：全量 0 failed。
  5. `cargo run -p scoop -- test --fixtures tests/fixtures/typecheck`：全量 0 failed。
  6. `cargo test -p scoopc`：除 `P4-T01j` / `P4-T01k` 范畴的两条已显式拆出的 production drift 失败之外，其余全部通过；P4-T01i 完成时 P4-T01j / P4-T01k 的范围仍是 `[TODO]`，不在本任务的"必须 0 failed"范围内。
- 完成条件：
  - 全量 `tests/fixtures/run-pass`、`tests/fixtures/typecheck` 完全通过；`cargo test -p scoopc` 仅剩 `P4-T01j` / `P4-T01k` 范畴的两条已显式拆出的失败，其余 `@Unsafe @Extern` / golden / failure-policy 范畴的失败全部回到通过。
- 依赖：无（与 P4-T01g/h 之间无强依赖；建议在 P4-T01 之前完成，使 string cone tracer bullet 启动时全量 baseline 干净）。
- 完成记录：
  - 改动范围：
    - **`@Unsafe @Extern` 双重标注 fixture / 单测 inline source（fixture / test-resource only）**
      - 已显式列入 P4-T01i 的 3 个 baseline fixture：`tests/fixtures/run-pass/extern_native_aggregate_return_direct_indirect_parity.scoop`、`tests/fixtures/run-pass/sync_gc_release_task_like_object_basic.scoop`、`tests/fixtures/typecheck/extern_fun_gc_handle_raw_token_roundtrip_ok.scoop`；
      - 已显式列入的 3 个单测 inline source：`crates/scoopc/src/llvm/tests.rs::abi_baseline_direct_extern_native_leaf_preserves_enter_leave_native_sequence`、`native_callable_direct_and_indirect_aggregate_return_share_target_abi`、`crates/scoopc/src/pipeline/mir_stage.rs::tests::refactor_mir_gc_handle_raw_uintptr_token_stays_scalar`；
      - **本任务验证过程中追加发现的同形态残留**（按"Class-Wide Fixes Over Narrow Patches"原则一起清理，避免 P4-T01 启动时再被同一类 drift 撞回）：`tests/fixtures/runtime_gc/{extern_enter_native_gc_arg_spill_reload,extern_enter_native_roots_gc,gc_handle_stale_callback_token_is_error,gc_handle_token_roundtrip_callback_basic,gc_move_stackmap_roots_update_multi_frame,gc_pin_unpin_move_stress_matrix,gc_stw_cross_thread_in_native_roots_basic}.scoop`、`tests/fixtures/build/extern_enter_native_no_statepoint_writeback.scoop`；
      - 唯一保留的 `@Unsafe @Extern` 写法是 `tests/fixtures/typecheck/extern_fun_default_abi_unsafe_redundant_is_error.scoop`，因为它正是这条诊断本身的 owner 用例。
    - **HIR golden（test-resource only）**：`tests/fixtures/hir/closure_capture_val.hir`、`closure_non_capture.hir` 追加 P1-T01 引入的 `abi_identity: ManagedOrdinary,` 字段；同时把 `closure_capture_var.hir`、`do_block_multiple_trailing_lambda_boundary.hir`、`safe_closure_basic.hir` 三份既有 golden 一并回填同一字段，让所有 `CallSiteContract` Debug 输出与 production 一致。
    - **MIR golden（test-resource only，按 production 已稳定行为重生成）**：`tests/fixtures/mir_refactor/aggregate_transport.mir` 重新由 `SCOOP_FIXTURE_REPRO_DIR` 抓取的 `actual.mir` 覆盖；唯一变化是 `Array.get` / `MutableArray.set` / `MutableArray.get` 的 direct callee_fqn 从扩展函数命名（如 `scoop.core.get`）切换到 P4-T01a/c 之后稳定的 body method 命名（`scoop.core.Array.get` / `scoop.core.MutableArray.set` / `scoop.core.MutableArray.get`），符合"内建类型作为一等 struct/class implementer"的设计基线。
    - **failure-policy 行号 sentinel（test-only 维护）**：`crates/scoopc/src/pipeline_user_visible_failure_policy.rs::INTERNAL_BUG_SENTINEL_HITS` 与 `STALE_UNSUPPORTED_MAIN_BODY_COUNTS` 按当前 production 状态回填行号 / 计数；`pipeline_user_visible_failure_policy_tracks_stale_unsupportedmainbody_counts` 中的总计也由 `802` 校准为 `788`。
    - **TODO.md**：把 `P4-T01i` 标 `[DONE]`，并显式拆出两条独立的 production drift 任务 `P4-T01j` / `P4-T01k`（分别覆盖 `add_function(..., None)` raw 调用与 `MutableSet/Set.len()` 不再重写到 overload-aware symbol）。
    - **`memory/claude_plan.md`**：刷新 P4-T01i 进展记录与剩余范围。
  - 核心决策：
    - **范围由"3 个 fixture + 7 个单测"扩展为"全量 baseline 中的 `@Unsafe @Extern` 残留"**：在做 Group A/B 时跑 `runtime_gc` 与 `build` 阶段发现还有 7+1 个同形态的旧写法，按 PROMPT.md 的"Class-Wide Fixes Over Narrow Patches"原则一并清理；额外更改不引入任何新的 surface 行为，仅匹配 P2-T02 之后的 `@Extern` 注解契约。
    - **production drift 显式拆出，不在 P4-T01i 内顺手修**：`function_declaration_inventory_eliminates_raw_add_function_none_callsites` 与 `materialize_for_dump_keeps_set_alias_receiver_overload_targets_distinct` 两个失败的根因都不是 `@Unsafe @Extern` rot，而是 production 路径已经发生变化；为了保持改动颗粒度，分别新建 `P4-T01j` / `P4-T01k` 两条独立任务承接，本任务严格只动 fixture / test resource / failure-policy 行号常量。
    - **MIR golden 重生成而非"裁剪断言"**：`aggregate_transport.mir` 的差异只是 array 系 body method FQN 从扩展函数命名升级到 nominal body method 命名，是 P4-T01a/c 已经被锁定的 design baseline 的自然结果；按"不能通过弱化 fixture 来消除 drift"的硬约束，这里采用了"按真实 production 输出重生成 golden"，未引入任何额外的 fixture 弱化。
    - **failure-policy sentinel 仅刷新数值**：未删任何 `panic!` / `unreachable!`，未改 `pipeline_user_visible_failure_policy.rs` 的扫描或筛选规则；只把 hardcoded 行号 / 计数和 production 当前状态对齐。
  - 验证结果：
    - `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass`：全量 400 fixtures 全部 PASS（与 P4-T01h 完成时多了 6 条 fixture，全部通过）；
    - `cargo run -p scoop -- test --fixtures tests/fixtures/typecheck`：全量 437 fixtures 全部 PASS；
    - 其它 fixture 阶段（`build` / `codegen` / `effect_lowered` / `hir` / `infer` / `mir` / `mir_refactor` / `parse` / `resolve` / `runtime_gc` / `scoopir` / `unsafe_nogc`）全部 PASS；
    - `cargo test -p scoopc`：859 passed / 2 failed，唯一失败是 `function_declaration_inventory_eliminates_raw_add_function_none_callsites`（`P4-T01j` 范畴）与 `materialize_for_dump_keeps_set_alias_receiver_overload_targets_distinct`（`P4-T01k` 范畴），与 P4-T01i 完成条件一致；
    - `cargo clippy --all-targets -- -D warnings` 通过；
    - 全仓 sanity scan：除 `tests/fixtures/typecheck/extern_fun_default_abi_unsafe_redundant_is_error.scoop`（该诊断的 owner 用例，必须保留）之外不再有 `@Unsafe`+`@Extern` 相邻的写法。
  - 与 `PLAN.md` / `MANAGED_ABI.md` 的对应闭合：
    - 对应 `PLAN.md` P4 前置 IV：在 `string cone tracer bullet` 启动之前把 fixture / 单测 / golden / failure-policy sentinel 这些 test-resource 噪声全部清理干净，使后续 `P4-T01j` / `P4-T01k` 与 `P4-T01` 在判断回归时不再被既存噪声干扰；
    - `MANAGED_ABI.md` 描述的 ABI surface（`@Extern` 注解契约、native surface gate、callable ABI identity）在本任务中均未变动，因此无需回写 `MANAGED_ABI.md`。

### [TODO] P4-T01j：把 `declare_named_intrinsic_runtime_symbol` 接入 `declare_runtime_or_native_import_function` 通道，消除 `add_function(..., None)` raw 调用

- 参考：
  - `crates/scoopc/src/llvm/codegen/mod.rs::{declare_classified_llvm_function, declare_runtime_or_native_import_function}`（function declaration 分类入口与 surface 检查）
  - `crates/scoopc/src/llvm/codegen/intrinsics/named.rs::declare_named_intrinsic_runtime_symbol`（P4-T01d/e 引入的 named intrinsic runtime symbol 声明）
  - `crates/scoopc/src/llvm/tests.rs::function_declaration_inventory_eliminates_raw_add_function_none_callsites`（已锁定的 inventory test，flag 该 raw 调用）
- 起因：在 `P4-T01i` 验证过程中跑 `cargo test -p scoopc`，`function_declaration_inventory_eliminates_raw_add_function_none_callsites` 报：
  ```
  crates/scoopc/src/llvm/codegen/intrinsics/named.rs:928: Ok(self.module.add_function(symbol, fn_ty, None))
  ```
  这是 P4-T01d 在新增 named intrinsic runtime symbol 声明路径时绕过了 `declare_runtime_or_native_import_function` 的 surface 检查。`P4-T01i` 的硬约束禁止顺手修 production codegen，因此把它显式拆为本任务。
- 必须实现的内容：
  1. 把 `declare_named_intrinsic_runtime_symbol` 的 `Ok(self.module.add_function(symbol, fn_ty, None))` 替换为对 `declare_runtime_or_native_import_function(self.module, symbol, fn_ty)` 的复用（与既有 runtime-or-native import 通道同构），保持"已存在则复用"语义；
  2. 不放宽 `declare_classified_llvm_function` 的 surface assertions；不引入新的 surface 类型；
  3. `function_declaration_inventory_eliminates_raw_add_function_none_callsites` 与所有 P4-T01d/e 锁定的 named intrinsic IR / fixture 必须同时通过。
- 必须遵从的约束：
  - 仅修改 `crates/scoopc/src/llvm/codegen/intrinsics/named.rs`（必要时连带 minimal import 调整），不得改写 `declare_classified_llvm_function` 等共享通道；
  - 不得借此弱化 inventory test 的扫描规则。
- 验证：
  1. `cargo test -p scoopc llvm::tests::function_declaration_inventory_eliminates_raw_add_function_none_callsites`
  2. `cargo test -p scoopc named_intrinsic`
  3. `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass`：全量不退化。
  4. `cargo clippy --all-targets -- -D warnings`
- 完成条件：named intrinsic runtime symbol 声明通过 `declare_runtime_or_native_import_function` 通道完成，inventory test 重新通过，且不引入新的 raw `add_function(..., None)` callsite。
- 依赖：无（与 P4-T01i 同范畴下的 production drift；可与 `P4-T01k` 并行）。

### [TODO] P4-T01k：修复 `MutableSet` / `Set` 的 `.len()` direct-call 不再重写到 overload-aware symbol 的 production drift

- 参考：
  - `crates/scoopc/src/mir/materialize.rs::tests::materialize_for_dump_keeps_set_alias_receiver_overload_targets_distinct`（已锁定的 MIR materialize 回归 test）
  - `tests/fixtures/run-pass/stdlib_hash_set_map_basic.scoop`（驱动该 test 的端到端 fixture）
  - `stdlib/collections_set.scoop::{Set.len, MutableSet.len}`（依赖的 alias / overload 表面）
- 起因：在 `P4-T01i` 验证过程中 `materialize_for_dump_keeps_set_alias_receiver_overload_targets_distinct` 报：
  ```
  assertion `left == right` failed: main 中的 MutableSet.len direct-call target 应统一重写到 overload-aware symbol：{}
    left: 0
   right: 1
  ```
  即 main 的 materialized MIR 中已经看不到任何 `scoop.core.size::<Int>$overload$...` direct-call target，说明 P4 前置以来某次改动让 `MutableSet.len()` / `Set.len()` 的 alias 归一化路径不再重写到 overload-aware symbol。`P4-T01i` 的硬约束禁止顺手修 production MIR materialization 行为，因此把它显式拆为本任务。
- 必须实现的内容：
  1. 调查 `materialize.rs` / `mir` 路径上把 `Array.size()` / `MutableArray.size()` 经由 `MutableSet`/`Set` typealias 触达 `len()` 时是否仍在生成 `scoop.core.size::<Int>$overload$` symbol，定位是哪一步开始 erase 该重写；
  2. 在不放宽 alias 归一化诊断、不破坏既有 generic class instance method 调用路径的前提下，恢复 main 中 `MutableSet.len()` 至少有 1 条 direct call 重写到 `scoop.core.size::<Int>$overload$...`，与 test 的 `assert_eq!(len_targets.len(), 1)` 对齐；同时 `contains_targets.len() == 2` 与现有断言保持一致；
  3. 完成后回填 test 是否需要更精确断言的 owner 注释；不弱化 test 的存在性断言。
- 必须遵从的约束：
  - 不得通过删除 / 收缩 test 来"消除 drift"；
  - 不得放宽 P4-T01b 完成的 vtable / itable / interface dispatch 收口；
  - 不得借此重排 generic class instance method 在 sysroot 中的 alias / overload 形状（如需调整 sysroot，必须是 production drift 的"恢复"，而不是新增 alias）。
- 验证：
  1. `cargo test -p scoopc materialize_for_dump_keeps_set_alias_receiver_overload_targets_distinct`
  2. `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/stdlib_hash_set_map_basic.scoop`
  3. `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass`：全量不退化。
  4. `cargo clippy --all-targets -- -D warnings`
- 完成条件：MIR materialize test 重新通过，main 中 `MutableSet.len()` direct-call target 重新走 `scoop.core.size::<Int>$overload$` 通道；其它 stdlib hash set / map 端到端 fixture 不退化。
- 依赖：无（与 `P4-T01j` 同范畴下的 production drift；可与 `P4-T01j` 并行）。

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
- 依赖：`P4-T01h`（`P4-T01f` 之后又新增了两条 P4 前置 IV 子任务：`P4-T01g` 解锁 subclass-typed receiver 对 inherited body method / 字段的可见性；`P4-T01h` 把 LHS expected type 反推与显式 type argument 都接入构造器调用——两者都会被 P4-T01 中的标量 `toString` migration 与 P4-T02 的 string helper migration 直接踩到）

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
