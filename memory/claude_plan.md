## 本次执行计划

1. 读取 `TODO.md`，仅将其作为任务索引。
2. 按 `TODO.md` 引用顺序读取对应的 `TODO-Px.md`，以任务标题是否带有 `[DONE]` 作为唯一完成判定，定位第一个未完成的详细任务。
3. 查看最近一次提交信息，确认是否存在与该任务直接相关且明确未完成的问题；若有，则将其并入当前任务范围或作为前置任务处理。
4. 阅读当前任务涉及的代码、测试、规范和相关文档，确认约束、依赖、验证方式以及现有实现状态。
5. 实施当前任务所需的最小正确修改；若发现阻塞当前任务的真实缺口或规格不匹配，则在相应 `TODO-Px.md` 中新增最小前置任务、同步 `TODO.md`，并停止继续推进后续任务。
6. 运行与当前任务直接相关的验证，并补充执行必要的回归检查；若任务完成，则继续运行要求中的质量检查（包括 `cargo clippy --all-targets -- -D warnings`，若适用）。
7. 更新 `memory/claude_plan.md` 记录关键进展与计划变化；完成后在对应 `TODO-Px.md` 中将任务标题标记为 `[DONE]` 并补全完成记录，如任务索引变化则同步 `TODO.md`；仅在阶段计划发生变化时更新 `PLAN.md`。
8. 按仓库提交风格创建一次提交，提交信息包含当前任务编号，然后停止，不继续处理下一个任务。

## 说明

- 这里记录的是可审计的执行计划与决策摘要，不包含不可审计的内部推理细节。
- 在执行过程中，如发现阻塞、范围变化、关键实现步骤完成或验证结果，需要及时追加更新本文件。

## 进展更新

- 2026-05-02：已按 `TODO.md -> TODO-P5.md` 顺序定位到首个未完成详细任务为 `P5-T02R`；`P5-T02` 已完成但 review 任务标题尚未带 `[DONE]`，因此本次执行单元确定为该 review 任务。
- 2026-05-02：已检查最近一次提交 `"[P5-T02] Define late-lowered IR shells"`。提交说明只描述固定 late-lowered IR 形状，未显式记录尚未纳入 TODO 的 unfinished issue，因此无需先插入新的前置任务。
- 2026-05-02：已完成代码复核。`crates/scoopc/src/effect_lowered/ir.rs` 中的 `LateLoweredBodyVersionKey` 明确保留 `surface_instance + allowed_row + impl_plan + needs_reentry`；`LateLoweredStepType` / `LateLoweredStepCase` 维持 canonical `StepSchema` case 集；`LateLoweredResumeInterface` / `LateLoweredContinuationObject` / `LateLoweredContinuationMethod` 已把 continuation carrier 与完整 method 集显式建模为可比较、可 dump 的中层壳层。`crates/scoopc/src/effect_lowered/builder.rs` 进一步确认 `ImplPlan::SingleCase` 只影响 continuation method reachability，不会收缩成第二套 `Step` 类型。
- 2026-05-02：已完成验证并通过：`cargo test -p scoopc --no-default-features refactor_late_lowered_ir`、`cargo test -p scoopc --no-default-features refactor_body_version_key`、`cargo test -p scoopc --no-default-features refactor_effect_lowered_stage`、`cargo clippy -p scoopc --no-default-features --all-targets -- -D warnings`。
- 2026-05-02：已执行 `rg "Signal \{|Any|Todo\(|SingleCase.*Step|CanonicalFull.*Step" crates/scoopc/src/effect_lowered crates/scoopc/src/effect_refactor_pipeline`。唯一命中为 `crates/scoopc/src/effect_refactor_pipeline/hir_stage.rs` 中既有的 `StmtKind::Todo(_)` / `ExprKind::Todo(_)` 语法遍历分支；它们属于上游 typed-HIR 节点处理，不是 P5 late-lowered representation 或 stage contract，因此不构成当前任务阻塞。
- 2026-05-02：当前未发现需要新增的前置任务；`P5-T02R` 已在 `TODO-P5.md` 中标记为 `[DONE]`，并已同步更新 `TODO.md` 索引。`PLAN.md` 保持不变；下一步是复核工作区差异并创建本任务提交，然后停止。
