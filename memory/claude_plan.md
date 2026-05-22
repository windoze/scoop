# Claude Execution Plan

## Current Invocation

- Goal: complete exactly the first incomplete task in `TODO.md`, then stop.
- Source of truth: `TODO.md` for task order, requirements, dependencies, validation, and completion records.
- Project plan file: `PLAN.md` is only updated if the phase/stage plan or dependency structure changes.

## Execution Plan

1. Read `TODO.md` and identify the first task whose title is not prefixed with `[DONE]`.
2. Check the latest commit message only for unfinished work directly relevant to that selected task.
3. Inspect the task requirements, dependencies, and relevant code/tests before editing.
4. Implement the task as written, avoiding workarounds or scope narrowing.
5. If a concrete blocker or missing prerequisite prevents spec-correct implementation, update `TODO.md` with the minimum prerequisite task, keep the current task incomplete, commit the bookkeeping change, and stop.
6. Run the task-relevant tests and any required validation from `TODO.md`; fix regressions caused by this work.
7. Update `TODO.md` by prefixing the completed task title with `[DONE]` and filling its completion record.
8. Update this progress file after key milestones or plan changes.
9. Review the git diff, then commit all changes required for this task with a task-specific commit message.
10. Stop without starting the next task.

## Progress Log

- Initialized execution plan before reading project task files.
- Identified first incomplete task: `P6-T02R` in `TODO-6.md`, a review of latest commit `P6-T02` (`12635e0a`).
- Review scope: verify eager top-level init covers `val`, `@Global var`, and entry-thread `@ThreadLocal var`; confirm final entry order is driven by source-cone DAG / LIR facts; search for residual top-level `val` first-access once initialization.
- Review findings under investigation: final entry order may not include explicit source-cone DAG edges, and initializer dependency facts may miss top-level root reads hidden behind plain function calls. These are in scope for `P6-T02R` if confirmed.
- Confirmed in-scope fixes for `P6-T02R`: publish source-cone topo order through MIR/LIR global init facts and verify final entry routine order; extend top-level initializer dependency collection to include dependencies reached through direct top-level function calls; add entry-thread `@ThreadLocal` eager-init coverage.
- Implemented `P6-T02R` review fixes: source-cone order now flows through HIR/MIR/LIR global init facts, LIR verifier rejects final entry source-order drift, indirect top-level initializer dependencies through direct function calls are collected, and new run-pass fixtures cover entry-thread TLS eager init plus indirect dependency ordering.
- Validation completed: `cargo fmt`; `cargo test -p scoopc_lir_facts`; `cargo test -p scoopc_mir_facts`; `cargo test -p scoopc --no-default-features global_init`; `cargo test -p scoopc --no-default-features hir_top_level_init_publishes_storage_and_extern_roots`; `cargo test -p scoopc --no-default-features lir_facts_builder`; `cargo test -p scoopc global_init`; `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/global_init`; `cargo clippy --all-targets -- -D warnings`; `git diff --check`.
- Full `run-pass` subset was rerun and still has 7 known non-global-init failures; all new and P6-T02R-related fixtures passed.
- Updated `TODO.md` and `TODO-6.md` to mark `P6-T02R` as `[DONE]`; next step is diff review and commit only this task's changes.
