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
