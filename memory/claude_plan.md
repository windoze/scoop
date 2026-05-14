## 当前执行计划

说明：这里记录可执行的步骤、关键判断与进度更新，不记录逐字内部推理。

1. 读取 `TODO.md`，按标题是否带有 `[DONE]` 确认第一个未完成任务。
2. 检查最近一次提交信息，确认是否存在与该任务直接相关且明确未完成的问题；若有，将其视为当前任务的一部分或作为前置任务写回 `TODO.md`。
3. 阅读当前任务相关代码、测试、规范与依赖说明，只聚焦该任务所需上下文，不做开放式问题排查。
4. 实现该任务要求；若遇到真实阻塞且无法按规范完成，则以最小必要方式在 `TODO.md` 中添加前置任务并调整依赖顺序。
5. 运行任务要求的验证，并补充必要测试，直到相关检查通过。
6. 更新 `TODO.md`：将当前任务标题标为 `[DONE]`，填写完成记录；仅在阶段计划真的变化时更新 `PLAN.md`。
7. 复查工作区中与本任务相关的改动，按仓库约定创建一次 git 提交，然后停止，不继续下一个任务。

## 进度记录

- 已写入初始执行计划，下一步读取 `TODO.md` 并锁定当前任务。
- 已确认首个未完成任务为 `P4-T01`：让 actual outward effect set 唯一决定 callable ABI，并补齐 effect-typed callable adapter。
- 最近一次提交信息为 `[P3-T03] Record final execution note`，未见与 `P4-T01` 直接相关的未完事项描述；当前按 `P4-T01` 原任务推进。
- 下一步：读取 `PLAN.md` 的 P4 段、`PIPELINE_GAPS.md` 的 `§3.12` / `§5.1` / `§5.4`，并检查 `effect_lowered/body.rs`、`effect_lowered/value.rs`、`llvm/codegen/mod.rs` 与现有测试入口，确定真实缺口和最小正确改动面。
- 已完成关键实现：
  - `effect_facts/builder.rs` 的 `FunValue` 调用点现在会解析 local callable provenance（`MakeClosure` / `TopLevelRef` / resolved member fun / direct-call result provenance），优先把动态调用点绑定到真实 callable facts，而不是只按 surface `declared_row` 生成 effect-step fallback。
  - `llvm/codegen/mir_body.rs` 的 plain dynamic call 现在优先查询 published late-lowered callable ABI；对于 surface effectful 但 actual outward-empty 的 closure/function-value，会允许走 plain ABI，而不是继续按声明 effect row 报 “requires adapter”。
  - 针对 closure 的两条关键回归：`closure_call_without_outward_effect_stays_on_direct_call_surface`、`closure_call_with_real_outward_effect_uses_explicit_outcome_boundary` 已通过。
- 已完成账本回写：`PIPELINE_GAPS.md`、`codegen_gap_inventory.rs`、`pipeline_gap_audit.rs`、`pipeline_user_visible_failure_policy.rs`、`TODO.md` 已同步到 `P4-T01` 完成状态。
- 已完成验证：task-level 单测、fixture、inventory audit、failure-policy audit 与 `cargo clippy --all-targets -- -D warnings` 均通过。
- 下一步：创建 `[P4-T01]` 提交并停止。
