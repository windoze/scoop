## Execution Plan

This file records the actionable plan and progress for the current invocation. It intentionally contains a concise rationale and step-by-step execution record rather than private chain-of-thought.

### Current Objective

- Complete exactly the first incomplete task in `TODO.md`, then stop.
- Treat `TODO.md` as the authoritative ordering and completion source.
- Mark the completed task title with `[DONE]`, update its completion record, run relevant validation, and commit all intended changes.

### Initial Plan

1. Read `TODO.md` and identify the first task whose heading is not prefixed with `[DONE]`.
2. Inspect only the files and recent commit context needed for that task, including `PLAN.md` only if phase-level dependencies may be affected.
3. Implement the task directly unless a concrete prerequisite or blocking spec mismatch makes that impossible.
4. If a blocker is found, update `TODO.md` with the minimum prerequisite task in the correct order, leave the current task incomplete, commit that bookkeeping, and stop.
5. Run targeted tests for the touched behavior, then broader relevant validation as practical. Any newly observed unscheduled failing test or fixture must be fixed or explicitly scheduled before marking the task complete.
6. Update `TODO.md` completion state and this progress file. Update `PLAN.md` only if phase-level sequencing or criteria changed.
7. Review `git status`, `git diff`, and recent commits, then commit the completed task with a descriptive task-tagged message.
8. Stop without starting the next task.

### Progress Log

- Plan file initialized before running project commands.
- Identified first incomplete task as `P9-T04`: extract `scoopc_hir` crate.
- Recent commit context (`P9-T03R`) does not mention an unfinished issue that directly blocks `P9-T04`.
- `P9-T04` requires moving HIR, resolve, typecheck, infer, intrinsics, expr_facts, vtable, and itable into a new `scoopc_hir` crate while keeping umbrella façade paths stable.
- Dependency inventory showed that the HIR/typecheck public surface also owns or consumes frontend helper types from `session`, `sysroot`, `target`, `warnings`, `stable_id`, and monomorph request keys. These must move with the HIR crate or be represented as HIR-owned APIs so `scoopc_hir` does not depend back on the `scoopc` facade.
- Planned extraction shape: `scoopc_hir` will depend on base crates, `scoopc_ast`, and `scoopc_hir_facts`; `scoopc` will re-export the migrated modules under their existing paths.
- Implemented the initial extraction and façade rewrite. `cargo check --workspace` now passes after exposing the cross-crate HIR/typecheck APIs that downstream monolith modules still consume.
- Relocated pipeline-dependent HIR golden tests and via-MIR request-root tests to umbrella `scoopc::pipeline` test modules; HIR crate tests now cover frontend-owned modules without depending back on `scoopc`.
- Validation completed: `cargo fmt`, `cargo check --workspace`, `cargo build --workspace`, `cargo test --all --all-targets`, `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass`, `cargo run -p scoop_tools -- dependency-gate`, `cargo clippy --all-targets -- -D warnings`, `cargo tree -p scoopc_hir`, and `git diff --check` all passed.
- `TODO.md` and `TODO-7.md` were updated to mark `P9-T04` done, record completion details, and note that `sysroot/` is temporarily HIR-owned until P9-T07 finalizes cone/project-model ownership.
