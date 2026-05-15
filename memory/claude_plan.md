## 本轮执行计划（P4-T01i）

按照 `PROMPT.md` 规范完成 `TODO.md` 中第一个未完成任务 **P4-T01i**：清理 P2-T02 之后仍残留 `@Unsafe @Extern` 旧写法的 baseline fixture / 单测，并刷新依赖于 production 行号的 failure-policy 单测。

### 任务确认

- `TODO.md` 中 `P4-T01h` 已 `[DONE]`；下一条未完成任务是 `P4-T01i`。
- 最近一次提交是 `[P4-T01h] Resolve ctor type args from LHS and explicit annotations`，无遗留未完成事项需要并入。
- P4-T01i 是 fixture / test resource only 的清理任务，约束是"不动 `annotations.rs` / runtime / codegen 等编译器侧代码"，"不得通过删除 fixture / 单测来'消除 drift'"。

### 范围（必须修复的列表）

**Group A：fixture-only**
- `tests/fixtures/run-pass/extern_native_aggregate_return_direct_indirect_parity.scoop`
- `tests/fixtures/run-pass/sync_gc_release_task_like_object_basic.scoop`
- `tests/fixtures/typecheck/extern_fun_gc_handle_raw_token_roundtrip_ok.scoop`

**Group B：unit test inline source**
- `crates/scoopc/src/llvm/tests.rs`：`abi_baseline_direct_extern_native_leaf_preserves_enter_leave_native_sequence`、`function_declaration_inventory_eliminates_raw_add_function_none_callsites`、`native_callable_direct_and_indirect_aggregate_return_share_target_abi`
- `crates/scoopc/src/hir/lower/tests.rs`：`hir_fixture_closure_capture_val_golden`、`hir_fixture_closure_non_capture_golden`
- `crates/scoopc/src/mir/materialize.rs::tests`：`materialize_for_dump_keeps_set_alias_receiver_overload_targets_distinct`
- `crates/scoopc/src/pipeline/mir_stage.rs::tests`：`refactor_mir_gc_handle_raw_uintptr_token_stays_scalar`

**Group C：failure-policy line-number sentinels**
- `crates/scoopc/src/pipeline_user_visible_failure_policy.rs`：`INTERNAL_BUG_SENTINEL_HITS` 与 `UNSUPPORTED_MAIN_BODY_*` 等常量；只刷新行号，不放宽筛选规则。

### 实施顺序

1. Group A：直接修 3 个 fixture，删除多余 `@Unsafe`；保留调用点 `@Unsafe do { ... }`。
2. Group B：在每个失败的单测 inline source 里删除 `@Unsafe @Extern(...)` 的多余 `@Unsafe`；保持调用点 unsafe 范围不变。
3. Group C：把 production 当前的实际行号回填到 sentinel 常量列表（按断言失败信息中的"right"侧已经把当前真实状态报出来）；不增删 `panic!` / `unreachable!`。
4. 跑 `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass` / `tests/fixtures/typecheck` 与 `cargo test -p scoopc`，全量必须 0 failed。
5. 跑 `cargo clippy --all-targets -- -D warnings`。
6. 写 `[DONE] P4-T01i` 完成记录并提交。

### 风险点

- 修改 inline source 时必须保留所有 `@Unsafe do { ... }` 调用范围 / 现有 IR 锁定形态；只去除 `@Extern` 上的多余 `@Unsafe` 注解。
- failure-policy 列表 hardcoded 行号常量需要按当前 production 状态回填；如果发现某条 production sentinel 已经被消除，那是 production 行为的真实变更，应当从列表里删除（这是"刷新 sentinel 列表"，不属于"放宽规则"）。

### 进展更新

- **Group A**（3 个 baseline fixture）：删除 `@Unsafe @Extern` 双重标注，全部通过。
- **Group B**（单测 inline source）：`abi_baseline_direct_extern_native_leaf_preserves_enter_leave_native_sequence`、`native_callable_direct_and_indirect_aggregate_return_share_target_abi`、`refactor_mir_gc_handle_raw_uintptr_token_stays_scalar` 三处 inline source 已收拢为只剩 `@Extern(...)`；HIR golden `closure_capture_val.hir` / `closure_non_capture.hir` 追加 `abi_identity: ManagedOrdinary,`；同时回填 `closure_capture_var.hir` / `do_block_multiple_trailing_lambda_boundary.hir` / `safe_closure_basic.hir` 三份既有 golden 的同字段；MIR golden `aggregate_transport.mir` 按 production 真实输出重生成（array 系 body method FQN 从扩展函数命名升级到 nominal body method 命名）。
- **Group C**：`pipeline_user_visible_failure_policy.rs` 中 `INTERNAL_BUG_SENTINEL_HITS` 全部行号刷新；`STALE_UNSUPPORTED_MAIN_BODY_COUNTS` 中 `mir_body.rs` 由 314 → 317、`effect_lowered/value.rs` 由 210 → 193；`pipeline_user_visible_failure_policy_tracks_stale_unsupportedmainbody_counts` 总计由 802 → 788。
- **额外发现的 P4-T01i 范畴 fixture（按"Class-Wide Fixes Over Narrow Patches"原则一并清理）**：`tests/fixtures/runtime_gc/{extern_enter_native_gc_arg_spill_reload,extern_enter_native_roots_gc,gc_handle_stale_callback_token_is_error,gc_handle_token_roundtrip_callback_basic,gc_move_stackmap_roots_update_multi_frame,gc_pin_unpin_move_stress_matrix,gc_stw_cross_thread_in_native_roots_basic}.scoop`、`tests/fixtures/build/extern_enter_native_no_statepoint_writeback.scoop`。
- **production drift 显式拆出**：`function_declaration_inventory_eliminates_raw_add_function_none_callsites`（`named.rs:928` raw `add_function(..., None)`）拆为 `P4-T01j`；`materialize_for_dump_keeps_set_alias_receiver_overload_targets_distinct`（main 中 `MutableSet.len()` direct-call 不再走 overload-aware symbol）拆为 `P4-T01k`。
- **回归确认**：
  - 全部 fixture 阶段（`build` / `codegen` / `effect_lowered` / `hir` / `infer` / `mir` / `mir_refactor` / `parse` / `resolve` / `run-pass` / `runtime_gc` / `scoopir` / `typecheck` / `unsafe_nogc`）全 PASS；
  - `cargo test -p scoopc`：859 passed / 2 failed，唯一失败属 `P4-T01j` / `P4-T01k`；
  - `cargo clippy --all-targets -- -D warnings` 通过；
  - sanity scan：除 `tests/fixtures/typecheck/extern_fun_default_abi_unsafe_redundant_is_error.scoop`（该诊断的 owner 用例）之外没有 `@Unsafe`+`@Extern` 相邻写法。

### 完成状态

- 已完成：实现、回归、`[DONE]` 完成记录、新增 `P4-T01j` / `P4-T01k` 两条独立任务、`memory/claude_plan.md` 刷新；
- 待提交：本次改动包含 fixture / golden / test source / failure-policy sentinel / `TODO.md` / `memory/claude_plan.md`。
