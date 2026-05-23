# Claude Execution Plan

## Scope

- Execute exactly the first incomplete task in `TODO.md`.
- Treat a task as complete only when its title is explicitly prefixed with `[DONE]`.
- Stop after completing, documenting, testing, and committing that one task.

## Step-by-Step Plan

1. Read `TODO.md` and identify the first task whose title does not start with `[DONE]`.
2. Check the latest commit only for unfinished work directly relevant to that selected task.
3. Read the selected task details, dependencies, validation requirements, and completion-record format.
4. Inspect the minimum relevant code, tests, fixtures, and documentation needed for that task.
5. Implement the task as specified, without narrowing scope or introducing workarounds.
6. If a concrete blocking prerequisite is discovered, update `TODO.md` with the minimum required prerequisite task in the correct order, record the blocker here, commit that bookkeeping, and stop.
7. Run targeted validation for the changed behavior, then broader required validation from the task. Address every observed unscheduled test or fixture failure before marking the task complete.
8. Update `TODO.md` by prefixing the completed task title with `[DONE]` and filling in its completion record. Update `PLAN.md` only if phase-level sequencing or criteria changed.
9. Review the worktree diff to ensure only intended changes are included.
10. Commit all changes for this task with a clear task-id-based message.
11. Stop without starting the next task.

## Progress Log

- Plan initialized before repository inspection or command execution.
- Identified first incomplete task from `TODO.md`: `P7-T04-cR` (`Review physical ABI/layout 迁移结果`) in `TODO-6.md`.
- Read `P7-T04-cR` requirements: review P7-T04-c, rerun its validation, search physical layout residuals, fix any blockers, then mark only this review task complete.
- Checked latest commit: `5ff59320 [P7-T04-c] Record completion progress`; it does not declare an unfinished blocker, so review proceeds against `P7-T04-c` implementation.
- Review search status: production `effect_lowered` physical layout code has no direct specified HIR side-table reads or `crate::hir::mangle_nominal_fqn` calls; tests pass empty HIR physical tables; TypeStore owner verifier is called from `ProgramAbiMaterializer::new`.
- Validation completed: `cargo fmt`; `cargo test -p scoopc_lir_facts`; no-default `llvm::codegen::effect_lowered::layout`; no-default `llvm::codegen::effect_lowered`; default `cargo test -p scoopc llvm::codegen::effect_lowered`; effect-lowered fixtures; full run-pass fixtures; `cargo clippy --all-targets -- -D warnings`; `git diff --check`.
- Updated `TODO.md` and `TODO-6.md`: marked `P7-T04-cR` `[DONE]`, filled its completion record, and advanced the package status to next task `P7-T04`.
