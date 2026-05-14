# Claude Plan

## Constraints
- 不记录或暴露私有推理细节；这里只维护可审计的执行计划、关键发现、变更决策与进度状态。
- 以 `TODO.md` 为任务唯一真源，只处理首个未完成任务。
- 若遇到阻塞当前任务的真实缺陷或缺失能力，先在 `TODO.md` 中补充最小前置任务并停止，不做规避性实现。

## Initial Execution Plan
1. 读取 `TODO.md`，定位首个标题未带 `[DONE]` 的任务。
2. 检查最近一次提交信息，确认是否存在与该任务直接相关且明确未完成的问题；若存在，将其视为当前任务内容或前置约束。
3. 阅读与当前任务直接相关的代码、测试、文档与任务说明，确认依赖、验收标准和现状。
4. 实现当前任务；若发现阻塞问题，先修复该阻塞或按要求在 `TODO.md` 中新增前置任务并停止。
5. 运行当前任务要求的验证，以及必要的相关测试、`cargo fmt`、`cargo clippy --all-targets -- -D warnings`。
6. 更新 `memory/claude_plan.md` 记录关键进展与计划调整。
7. 更新 `TODO.md`：仅在任务真正完成时给任务标题加 `[DONE]` 并填写完成记录；若有新前置任务，调整顺序与依赖说明。
8. 仅在阶段级计划发生变化时更新 `PLAN.md`。
9. 按仓库约定创建一次 git 提交，然后停止，不继续下一个任务。

## Progress Log
- 已创建执行计划文件，待读取 `TODO.md` 并确定当前任务。
- 已读取 `TODO.md` / `PLAN.md` 并确认首个未完成任务为 `P5-T01`：统一 composite transport contract，关闭 enum/array boxing residual。
- 最近提交主题为 `[P4-T03] Isolate array helper call-site identity`；下一步检查完整提交说明，确认是否存在被显式标记为 `P5-T01` 直接相关的未完成问题或前置阻塞。
- 已检查最近提交完整说明：无额外未完成事项被显式记录为 `P5-T01` 的新前置阻塞。
- 已运行并通过现有关键回归：
  - `cargo test -p scoopc refactor_llvm_composite_transport_contract_emits_layout_descriptor_globals -- --nocapture`
  - `cargo test -p scoopc refactor_llvm_value_boxing_transport -- --nocapture`
  - `cargo test -p scoopc refactor_llvm_enum_payload_transport -- --nocapture`
  - `cargo test -p scoopc refactor_llvm_array_composite_transport -- --nocapture`
  - `cargo test -p scoopc refactor_llvm_cross_thread_resume_payload_transport -- --nocapture`
  - `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/enum_payload_boxing_any_basic.scoop`
  - `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/enum_oversized_variant_boxing_suppressed.scoop`
  - `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/option_nested_custom_enum_payload_basic.scoop`
  - `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/gc_array_class_elements_cross_function.scoop`
  - `cargo run -p scoop -- test --fixtures tests/fixtures/mir_refactor/aggregate_transport.scoop`
  - `SCOOP_GC_MOVE=1 SCOOP_GC_STRESS=1 SCOOP_GC_VERIFY_ROOTS=1 cargo run -p scoop -- test --fixtures tests/fixtures/runtime_gc/effect_cross_thread_resume_payload_composite.scoop`
- 关键发现：功能面已基本闭合，但 `enum_lowering.rs` / `control_flow.rs` / `mir_body.rs` / `effect_lowered/value.rs` 仍保留旧式 `UnsupportedMainBody` residual；`codegen_gap_inventory.rs`、`pipeline_gap_audit.rs`、`PIPELINE_GAPS.md` 仍把 `§4.1/§4.3/§4.4/§4.5` 记作 live blocker 或 partial scope drift。
- 当前调整计划：
  1. 将 composite transport residual 从用户可见 `UnsupportedMainBody` 改为 backend contract guard。
  2. 将 `§4.1/§4.3/§4.4/§4.5` 从旧 owner/blocker 语义更新为 `P5-T01` closed/re-scoped guard。
  3. 重新运行相关 inventory/audit/test/fixture 与 `clippy`，然后回写 `TODO.md` 和完成记录。
- 已完成代码与账本收口：
  - `mir_body.rs` 的 composite value erasure residual 已改为 `PIPELINE_GAPS §4.1` backend guard。
  - `enum_lowering.rs` / `control_flow.rs` 的 oversized / nested / non-scalar enum payload residual 已改为 `§4.3` / `§4.4` backend guard。
  - `effect_lowered/value.rs` 的 array composite metadata / u64 decode residual 已改为 `§4.5` backend guard。
  - `codegen_gap_inventory.rs`、`pipeline_gap_audit.rs`、`pipeline_user_visible_failure_policy.rs`、`PIPELINE_GAPS.md` 已同步到 `P5-T01` 完成后的 closed/re-scoped 状态。
- 最新验证已通过：
  - `cargo test -p scoopc codegen_gap_inventory -- --nocapture`
  - `cargo test -p scoopc pipeline_gap_audit -- --nocapture`
  - `cargo test -p scoopc pipeline_user_visible_failure_policy -- --nocapture`
  - `cargo test -p scoopc refactor_llvm_composite_transport_contract_emits_layout_descriptor_globals -- --nocapture`
  - `cargo test -p scoopc refactor_llvm_value_boxing_transport -- --nocapture`
  - `cargo test -p scoopc refactor_llvm_enum_payload_transport -- --nocapture`
  - `cargo test -p scoopc refactor_llvm_array_composite_transport -- --nocapture`
  - `cargo test -p scoopc refactor_llvm_cross_thread_resume_payload_transport -- --nocapture`
  - `cargo run -p scoop -- test --fixtures tests/fixtures/mir_refactor/aggregate_transport.scoop`
  - `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/enum_payload_boxing_any_basic.scoop`
  - `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/enum_oversized_variant_boxing_suppressed.scoop`
  - `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/option_nested_custom_enum_payload_basic.scoop`
  - `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/gc_array_class_elements_cross_function.scoop`
  - `SCOOP_GC_MOVE=1 SCOOP_GC_STRESS=1 SCOOP_GC_VERIFY_ROOTS=1 cargo run -p scoop -- test --fixtures tests/fixtures/runtime_gc/effect_cross_thread_resume_payload_composite.scoop`
  - `cargo clippy --all-targets -- -D warnings`
- 下一步：回写 `TODO.md` 的 `[DONE]` 标记与完成记录，检查工作树，然后按 `P5-T01` 创建 git 提交并停止。

## Current Task Focus: P5-T01
- 先检查最近提交说明与当前工作树，确认是否已有直接相关但未落账的 blocker。
- 再阅读 `P5-T01` 相关代码与测试入口：`composite_transport.rs`、`enum_lowering.rs`、`control_flow.rs`、`effect_lowered/value.rs`、`mir_body.rs` 以及对应 LLVM stage 测试。
- 基于现状选择最小正确实现，优先修复整个 composite transport contract 的根因，而不是为 enum/array/effect payload 单独加分叉补丁。
- 完成后执行任务要求的定向测试、必要 fixture、`cargo clippy --all-targets -- -D warnings`，再回写 `TODO.md` / `PIPELINE_GAPS.md` / `memory/claude_plan.md` 并提交。
