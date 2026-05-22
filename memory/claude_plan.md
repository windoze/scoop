# Claude Execution Plan

## Current Objective

Follow `TODO.md` as the authoritative task list, complete exactly the first incomplete task in this invocation, validate it, update completion records, commit the result, and stop.

## Execution Plan

1. Read `TODO.md` first to identify the earliest heading that is not explicitly prefixed with `[DONE]`.
2. Check the latest commit only for unfinished work directly relevant to that selected task.
3. Inspect the selected task body, dependencies, and validation requirements, then inspect only the relevant source/tests.
4. Implement the selected task completely, preserving spec behavior and avoiding workaround or narrowed fixture shapes.
5. Run focused validation for the changed area, then run every validation command required by the task and any broader checks needed by repository policy.
6. If validation exposes an unscheduled failure or a concrete prerequisite blocker, add the minimum required task to `TODO.md`, leave the current task incomplete, commit the bookkeeping, and stop.
7. If validation succeeds, mark the selected task heading `[DONE]`, update its completion record in `TODO.md`, and update `PLAN.md` only if phase-level sequencing or criteria changed.
8. Review `git status`, `git diff`, and recent commits, then commit all intended changes with a task-specific message and stop without starting the next task.

## Progress Log

- Initialized this invocation's shareable plan before running project commands or selecting the current task.
- Identified first incomplete task: `P7-T03R` in `TODO-6.md`, "Review LLVM body emission 迁移结果".
- Latest commit is `fff17378 [P7-T03] Migrate LLVM body emission to LIR contracts`, which is the direct subject of this review task.
- Review focus: verify production LLVM body emission no longer reads raw MIR/HIR bodies, remaining MIR/HIR imports in `llvm/codegen` are test-only or scheduled residuals, LIR facts are not a shallow MIR/HIR搬运 layer, then rerun P7-T03 validation plus required residual searches.
- Residual search found `llvm_residual_pass_view` only in LLVM handoff/layout-test paths and pipeline accessors already covered by later `P7-T04`; `llvm/codegen` still has MIR/HIR helper hits, so review is classifying whether each hit is dead/test/scheduled residual or LIR-owned source-slice lowering rather than a raw body fallback.
- Fixed stale `llvm/codegen/mir_body` module documentation that still described the old `MaterializedMirPassView` production body route and HIR-compatible fallback, so the review-search result matches the current LIR-owned source-slice helper role.
- Validation passed after the doc fix: `cargo fmt`; `cargo test -p scoopc --no-default-features llvm::codegen::effect_lowered`; `cargo run -p scoop -- test --fixtures tests/fixtures/effect_lowered`; `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass` (421/421); `cargo clippy --all-targets -- -D warnings`; `git diff --check`; and the required residual searches in `llvm/codegen`.
- Updated `TODO-6.md` and `TODO.md` to mark `P7-T03R` as `[DONE]`; next task remains `P7-T04` for a future invocation.
