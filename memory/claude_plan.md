# 本次执行计划

1. 读取 `TODO.md`，确定第一个未完成任务（仅标题带 `[DONE]` 才算完成）。
2. 查看最近一次提交信息，确认是否存在与该任务直接相关且尚未完成的问题需要一并处理，或需要先写入 `TODO.md` 作为前置任务。
3. 阅读当前任务涉及的代码、测试、规格记录与相邻任务说明，确认约束、依赖和验证要求。
4. 实现当前任务；如果发现阻塞该任务的真实前置缺陷或缺失能力，则先按要求更新 `TODO.md`/必要时更新 `PLAN.md`，并停止在前置任务落地点。
5. 运行与当前任务相关的验证，并补充必要测试；若验证失败，先修复再重复验证。
6. 完成后更新 `TODO.md`：将当前任务标题标记为 `[DONE]`，并填写/更新完成记录。
7. 仅在阶段计划、依赖结构或完成标准发生变化时更新 `PLAN.md`。
8. 检查工作区改动，按要求提交当前任务相关的所有未提交文件，然后停止，不进入下一个任务。

## 执行记录

- 已创建初始计划文件；尚未读取 `TODO.md`，下一步开始锁定当前应执行任务。
- 已读取 `TODO.md` 并确认当前第一个未完成任务为 `P1-T01`：删除 `mir/lower.rs` 中 assign/call/ctor/intrinsic legacy producer。
- 最近一次提交为 `P0-T02`，内容聚焦 failure policy 审计与前端 reject 文案冻结；未发现需要先于 `P1-T01` 插入的直接相关未完事项。
- 下一步：读取 `PLAN.md` / `PIPELINE_GAPS.md` 对应段落，以及 `crates/scoopc/src/mir/lower.rs`、相关 pipeline/tests，确认六个 legacy reason 的生产路径与当前 typed contract 覆盖情况。
- 已确认 `P1-T01` 当前需处理的真实代码面：
  - `lower_assign_stmt(...)` 仍保留 `uses_refactor_typed_contracts()` 分叉与两个 assignment legacy `StatementKind::Todo(...)` producer。
  - `lower_call_expr(...)` 在 typed call contract 未命中时，仍保留 direct/closure/fun value/class ctor/reflection intrinsic 的 legacy lowering，并会发射四个 legacy `Rvalue::Todo(...)`。
  - `lower_for_dump(...)` 与 `mir/materialize.rs` 的若干 generic-MIR 构造入口仍使用 `MirLoweringFacts::from_lowered_hir(...)`，这会继续走到 legacy branch；若直接删除 producer 而不切到 typed handoff，这些路径会失去 assign/call contract。
  - `HirCompletenessVerifier` 目前会验证 assign place contract，但不会验证普通 call expression 是否具备 typed call-site contract；若删掉 fallback 后这层缺口存在，当前最可能表现为 MIR lowering impossible-state panic。
- 当前拟定执行方案：
  1. 先把 `lower_for_dump(...)` 与 generic MIR/materialize 入口接到 typed call/assign contract handoff，避免删分支后测试辅助路径失去 contract 来源。
  2. 删除 `lower_assign_stmt(...)` 与 `lower_call_expr(...)` 中本任务范围内的 legacy producer，仅保留 enum variant 等非本任务 residual 路径，以及 `P1-T02` 负责的 resume/dispatch legacy 路径。
  3. 若实现中确认 call-site completeness 仍缺早期失败出口，则补最小化的 typed contract 缺失报错/校验，使“缺 contract”不再退化为 legacy Todo。
  4. 更新 `pipeline/mir_stage.rs` 相关 smoke/forbidden 断言，必要时同步 `pipeline/hir_preflight.rs` 的 fallback 禁止词。
  5. 跑 `P1-T01` 指定验证、`cargo clippy --all-targets -- -D warnings`，再更新 `TODO.md` 与相关 gap 文档。
- 已实施的代码改动：
  - `MirLoweringFacts::from_lowered_hir(...)` 现在会同步导入 typed call-site / assign-place contract，`lower_for_dump(...)` 和 materialize 入口因此不再依赖本任务删除掉的 legacy producer。
  - `lower_assign_stmt(...)` 已改为只走 typed place contract；`lower_call_expr(...)` 已删除普通 call / ctor / reflection intrinsic 的 legacy Todo producer。
  - `TypedHirEffectContracts` 现会在缺失 typed call-site contract 时直接报 `HirStageError`；同时补上 local callable value 的 `FunValue` contract 发布。
  - 为修复 compiler-generated helper call 使用同一 `CallSite(span)` 相互覆盖，array-builder / vararg-builder 合成调用已改为使用可区分的 span。
- 执行中遇到的 blocker：
  - `refactor_hir_preflight_checks_completeness_fixtures_and_mir_smoke` 一度暴露 `hir/refactor_call_args.scoop` 中 `__scoop_array_builder_*` 合成 helper calls 共用同一 span，导致 typed intrinsic contract 被最后一条 build-array 调用覆盖，前面的 `new/push` call 在 MIR lowering 阶段缺 contract。
  - 已通过为这些 helper calls 分配唯一 span 修复；未引入新的 lowering fallback。
- 已完成验证：
  - `cargo test -p scoopc refactor_hir_call_contracts_record_callable_provenance`
  - `cargo test -p scoopc refactor_hir_class_literal_and_intrinsic_contracts`
  - `cargo test -p scoopc refactor_hir_preflight_checks_completeness_fixtures_and_mir_smoke`
  - `cargo test -p scoopc refactor_mir_place_contract`
  - `cargo test -p scoopc refactor_mir_call_contract`
  - `cargo test -p scoopc dump_mir_lowers_safe_member_access_option_result_without_ctor_todo`
  - `cargo test -p scoopc dump_mir_publishes_member_write_contract_for_escape_continuation_cell`
  - `cargo test -p scoopc materialize_for_dump_dedups_repeated_instance_requests`
  - `cargo run -p scoop -- test --fixtures tests/fixtures/mir_refactor/assignment_places.scoop`
  - `cargo run -p scoop -- test --fixtures tests/fixtures/mir_refactor/call_contracts.scoop`
  - `cargo clippy --all-targets -- -D warnings`
- 已回写文档：
  - `TODO.md`：`P1-T01` 已标记为 `[DONE]` 并补全完成记录。
  - `PIPELINE_GAPS.md`：已更新 §1.6、§1.7、§6.3，反映 active producer 已删除、剩余 legacy reason 仅在 inventory/guard/test scaffolding 中待 `P1-T02` 清理。
- 下一步：检查工作区差异，按任务要求提交当前改动，然后停止。
