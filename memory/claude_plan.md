# Claude Execution Plan

This file records the auditable execution plan and progress for the current invocation. It intentionally contains a concise rationale and step-by-step plan, not private chain-of-thought.

## Current Objective

Complete exactly the first incomplete task in `TODO.md`, then stop after verification, documentation updates, and a Git commit.

## Step-by-Step Plan

1. Read `TODO.md` first and identify the first task whose title is not prefixed with `[DONE]`.
2. Inspect the task body, dependencies, validation requirements, and completion-record expectations.
3. Check the latest commit only for directly relevant unfinished issue references, without doing broad historical triage.
4. Examine the smallest relevant code, fixture, and test areas needed for the selected task.
5. Implement the selected task as specified, avoiding workarounds or weakened fixture shapes.
6. If a concrete blocker or missing prerequisite prevents spec-correct implementation, update `TODO.md` with the minimum prerequisite task, update this plan, commit that bookkeeping, and stop.
7. Run targeted validation first, then broader validation required by the task or affected code.
8. Fix any task-relevant failures discovered during validation.
9. Mark the completed task title in `TODO.md` with `[DONE]` and update its completion record.
10. Update `PLAN.md` only if phase-level sequencing, dependencies, assumptions, or completion criteria changed.
11. Review the final diff and Git status, then commit all intended changes with a task-specific message.
12. Stop without starting the next task.

## Progress Log

- Initial plan recorded before reading task details or running commands.
- Read `TODO.md`; first incomplete task is `P2-T07` in `TODO-3.md`.
- Read `TODO-3.md`; `P2-T07` requires P2 cleanup, documentation sync, dependency audit, full validation, and TODO completion updates. Latest commit is `P2-T06R` and does not introduce a directly relevant unfinished prerequisite.
- Completed initial cleanup audit: old `TypedHirEffectContracts` / `ProgramFacts` / `FallbackSideTables` names are absent from production Rust code; remaining `materialized_mir` / `materialized_pass_view` hits belong to MIR/P4+ handoff paths or tests, not HIR output APIs. Began documentation/comment cleanup for P2 closeout and P3 entry wording.
- Updated P2 closeout documentation in `README.md`, `PIPELINE_REFACTOR.md`, `PIPELINE-CLEANUP.md`, `PLAN.md`, `TODO-4.md`, and comments in the HIR/HIR-facts code so current docs no longer describe deleted HIR typed/fallback bridges as active P3 input.
- Full fixture validation exposed a task-blocking materialized MIR failure for array literal synthetic `mutableArrayNew<T>` calls. Fixed HIR lowering side-table publication so synthetic array helper calls (`mutableArrayNew`, `push`, `freeze`) carry inferred type arguments into HIR facts / materialization bindings; targeted array fixtures now pass.
- Fixed two additional validation blockers found by the full suite: de-duplicated enum variant / layout facts in `HirFacts`, and made materialization drop no-op value-erasure transports after generic substitution. Updated affected HIR/MIR goldens.
- Final validation passed: `cargo fmt`, `cargo test -p scoopc_hir_facts`, `cargo test --all --all-targets --no-default-features`, `cargo run -p scoop -- test`, `cargo run -p scoop_tools -- spec-fixtures check`, `cargo run -p scoop_tools -- dependency-gate`, `cargo clippy --all-targets -- -D warnings`, `cargo tree -p scoopc_hir_facts`, `git diff --check`, and P2 cleanup keyword searches.
- Marked `P2-T07` as `[DONE]` in `TODO.md` and `TODO-3.md`; completion record now documents cleanup scope, validation, search classification, dependency audit, and residual P3+ risks.
## 当前执行计划

说明：我不能写入逐字内部思维过程，但会记录可审计的执行计划、关键判断、进度和验证结果。

1. 读取 `TODO.md`，严格按标题是否带 `[DONE]` 识别第一个未完成任务。
2. 必要时查看最新提交，判断是否存在与该任务直接相关的未完成事项。
3. 读取该任务涉及的代码、测试和文档，确认要求、依赖、验证命令和完成条件。
4. 若发现阻塞当前任务的实现缺口或规格不匹配，先在 `TODO.md` 增加最小必要前置任务并提交，然后停止。
5. 若无阻塞，按最小正确变更实现当前任务，避免规避规格或夹带无关改动。
6. 运行任务要求的相关测试，并根据失败结果修复同一根因类别的问题。
7. 更新 `TODO.md`，将完成任务标题加 `[DONE]`，填写完成记录；仅当阶段计划变化时更新 `PLAN.md`。
8. 检查工作区差异，提交本次任务涉及的所有未提交文件，然后停止，不继续下一个任务。

状态：初始计划已写入，下一步读取 `TODO.md` 识别当前任务。

## 当前任务

- 第一个未完成任务：`P2-T07R`，Review P2 全包完成度。
- 最新提交：`64c16412 [P2-T07] Complete P2 cleanup audit`，标题未提示未完成事项。
- 初始工作区状态：仅 `memory/claude_plan.md` 因本计划更新而修改。

## P2-T07R 执行步骤

1. 复查 `P2-T07` 完成记录与 P2 全包任务记录，确认 `AST -> HIR` semantic barrier、独立 `hir_facts`、legality gate、error boundary 均有实现与验证支撑。
2. 搜索旧 P2 迁移关键词，确认 `ProgramFacts`、`TypedHirEffectContracts`、fallback side tables、HIR 携带 MIR artifacts 等旧路径没有在当前代码中形成阻塞。
3. 复查 `scoopc_hir_facts` 依赖、模型、dump/verifier，以及 `HIR -> MIR` handoff 是否只依赖 HIR 本体与 `hir_facts`。
4. 运行 P2-T07R 要求的验证命令和抽查 fixture。
5. 若 review 发现阻塞项，直接修复并重新验证；若无阻塞，更新 `TODO.md` / `TODO-3.md` 将 `P2-T07R` 标记为 `[DONE]` 并填写 review 记录。
6. 检查差异并提交本任务改动后停止。

状态：已识别任务并准备开始复审。

## P2-T07R 进度

- 已复查任务要求、最新提交和初始工作区状态。
- 旧 P2 迁移关键词在 `crates/**/*.rs` 中无命中；`materialized_mir` / `materialized_pass_view` / pass view 关键词在 HIR 源码目录中无命中。
- 文档旧名称命中已初步分类为历史任务记录、P2 已解决说明或 P3+ 后续 handoff 范围。
- 下一步运行 P2-T07R 要求的完整验证和抽查 fixture。
- 完整验证通过：`cargo fmt`、`cargo test -p scoopc_hir_facts`、`cargo test --all --all-targets --no-default-features`、`cargo run -p scoop -- test`、`cargo run -p scoop_tools -- spec-fixtures check`、`cargo run -p scoop_tools -- dependency-gate`、`cargo clippy --all-targets -- -D warnings`、`cargo tree -p scoopc_hir_facts`、三类 fixture 抽查、`git diff --check` 和 P2 review 关键词搜索。
- 已将 `P2-T07R` 在 `TODO.md` 与 `TODO-3.md` 标记为 `[DONE]`，并填写 review 完成记录。
- 下一步检查最终差异并提交本任务变更。
