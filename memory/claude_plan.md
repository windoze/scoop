## 当前计划

1. 读取 `TODO.md`，确认第一个未标记为 `[DONE]` 的任务，并检查其约束、依赖、验证要求与完成记录。
2. 查看最近的提交信息，判断是否存在与当前任务直接相关且明确标注未完成的问题；若存在，将其作为当前任务的一部分或按要求记为前置。
3. 读取与当前任务直接相关的代码、测试、文档与任务记录，仅收集完成该任务所需的最小上下文。
4. 实现当前任务；如果遇到阻塞当前任务的真实缺陷或缺失特性，不绕过问题，而是先修复，或在 `TODO.md` 中添加最小必要前置任务并停止。
5. 运行当前任务要求的验证与必要的相关测试；若失败则继续修复直到通过，或按阻塞流程更新 `TODO.md`/`PLAN.md`。
6. 完成后更新 `TODO.md`：将当前任务标题标记为 `[DONE]`，补全完成记录；仅在阶段计划发生变化时更新 `PLAN.md`。
7. 复查工作区中与本任务相关的改动，按要求提交 git commit，然后停止，不继续下一个任务。

## 进度记录

- 已创建初始执行计划，下一步读取 `TODO.md` 并确认当前任务。
- 已确认首个未完成任务为 `P5-T02：收口 closure env/capture transport 与 pattern is Type residual`。
- 已核对 `PLAN.md` / `PIPELINE_GAPS.md`：当前对应 live gap 为 `§3.8`（pattern runtime type test narrow residual）与 `§3.11`（closure env/capture shape 限制）。
- 已定位当前实现与测试入口：
  - `crates/scoopc/src/llvm/codegen/mir_body.rs`
  - `crates/scoopc/src/pipeline/llvm_codegen_stage.rs`
  - `tests/fixtures/mir_refactor/pattern_is_type.scoop`
  - `tests/fixtures/runtime_gc/gc_trace_closure_capture_string_basic.scoop`
  - `tests/fixtures/runtime_gc/gc_move_enum_maybe_ref_closure_capture_basic.scoop`
- 下一步先运行任务要求的定向测试，确认当前缺口究竟是实现未闭合，还是需要新增前置阻塞任务。
- 已确认定向回归本身原先全部通过，因此当前任务的真实工作是：
  - 把 `when` 的 `is Type` unsupported target 前移为明确前端诊断，并补齐 compile-time static fold；
  - 将 closure capture 的 runtime GC 回归从 `Box` workaround 改为直接捕获 aggregate；
  - 同步关闭 `PIPELINE_GAPS.md` / `llvm/codegen_gap_inventory.rs` 中的 `§3.8`、`§3.11`。
- 已完成的关键实现：
  - `typecheck/when_pat.rs` 新增 `when is T` gate：动态 value-type target 与 function-type target 不再流到 backend unsupported。
  - `mir/lower.rs` 收紧 runtime type static fold，让 value-vs-ref 等显然不可能的 pattern 在 MIR 里直接折叠为 `AlwaysFalse`。
  - 新增 3 个 typecheck fixtures，固定 dynamic value-type / pure function-type / effectful function-type `when is T` 的前端拒绝行为。
  - `pipeline/llvm_codegen_stage.rs` 新增 static-false IR 断言，确保 disjoint value/ref pattern 不再走 runtime type test。
  - `runtime_gc/gc_move_enum_maybe_ref_closure_capture_basic.scoop` 已去掉 `Box` workaround，改为 direct enum capture，并已跑通。
  - `PIPELINE_GAPS.md`、`llvm/codegen_gap_inventory.rs`、`pipeline_gap_audit.rs` 已同步回写 `§3.8` / `§3.11` 的 closed-guard 状态。
- 已完成验证：
  - `cargo test -p scoopc refactor_llvm_closure_env_transport`
  - `cargo test -p scoopc refactor_llvm_runtime_type_primitives`
  - `cargo test -p scoopc codegen_gap_inventory`
  - `cargo test -p scoopc pipeline_gap_audit`
  - `cargo run -p scoop -- test --fixtures tests/fixtures/mir_refactor/pattern_is_type.scoop`
  - `cargo run -p scoop -- test --fixtures tests/fixtures/runtime_gc/gc_trace_closure_capture_string_basic.scoop`
  - `cargo run -p scoop -- test --fixtures tests/fixtures/runtime_gc/gc_move_enum_maybe_ref_closure_capture_basic.scoop`
  - `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/type_check_cast_generic_class_instantiation_basic.scoop`
  - `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/parameterized_supertype_interface_dispatch.scoop`
  - `cargo run -p scoop -- test --fixtures tests/fixtures/typecheck/when_is_pattern_dynamic_value_runtime_test_is_error.scoop`
  - `cargo run -p scoop -- test --fixtures tests/fixtures/typecheck/when_is_pattern_function_type_is_error.scoop`
  - `cargo run -p scoop -- test --fixtures tests/fixtures/typecheck/when_is_pattern_effectful_function_type_is_error.scoop`
  - `cargo clippy --all-targets -- -D warnings`
  - `cargo fmt`
- 已更新 `TODO.md`，将 `P5-T02` 标记为 `[DONE]` 并补全完成记录。下一步只剩创建本任务 commit，然后停止。
