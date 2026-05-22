# Claude Execution Plan

## Current Objective

Follow `TODO.md` as the authoritative task list, complete exactly the first incomplete task, validate it, update completion records, commit the result, and stop.

## Execution Plan

1. Read `TODO.md` to identify the first task whose heading is not prefixed with `[DONE]`.
2. Check recent repository context only as needed for that task, including the latest commit if it directly mentions unfinished work relevant to the selected task.
3. Inspect the task requirements, dependencies, and validation instructions.
4. Implement the selected task completely, without narrowing scope or using workaround behavior.
5. Run focused tests first, then run broader validation required by the task and repository policy.
6. If a blocking missing feature, spec mismatch, or unscheduled failing test is found, update `TODO.md` with the minimum required prerequisite task, commit that bookkeeping, and stop.
7. If implementation and validation succeed, mark the selected task heading `[DONE]`, update its completion record in `TODO.md`, and update `PLAN.md` only if phase-level planning changed.
8. Review the final diff, commit all intended changes with a task-specific message, and stop without starting the next task.

## Progress Log

- Initialized execution plan before inspecting project task details.
- Identified first incomplete task: `P7-T03` in `TODO-6.md`, "迁移 LLVM body emission 离开 raw MIR / HIR fallback".
- Latest commit is `8166ae29 [P7-T02R] Review backend reachability cleanup`; it is directly adjacent but does not record a separate unfinished blocker.
- Focus for this invocation: inspect current LLVM body emission residuals, implement the P7-T03 boundary cleanup, validate required commands, then mark only `P7-T03` complete and commit.
- Found existing uncommitted edits already targeting P7-T03. They introduce LIR-owned source callable handoff on `LateLoweredCallable`, remove pass-view parameters from effect-lowered body emission entries, and switch entry/main selection away from HIR body fallback.
- Focused compilation/tests passed so far: `cargo test -p scoopc --no-default-features llvm::codegen::effect_lowered`; `cargo test -p scoopc llvm::codegen::effect_lowered`; `cargo run -p scoop -- test --fixtures tests/fixtures/effect_lowered`.
- Full run-pass validation passed: `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass` (421/421).
- `cargo fmt`, `cargo clippy --all-targets -- -D warnings`, and `git diff --check` passed after fixing the clippy `too_many_arguments` warning with a targeted allow on the existing helper shape.
- Updated `TODO-6.md` and `TODO.md` to mark only `P7-T03` as `[DONE]`; next task remains `P7-T03R` for the following invocation.
- After residual review, replaced effect-neutral published plain direct-call emission with a `ProgramAbiQuery`-driven path, avoiding the old pass-view-backed `mir_body` direct-call helper for that body-emission case. Re-ran effect tests, effect fixtures, full run-pass, clippy, and diff check successfully.
