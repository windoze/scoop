# Execution Plan

I will keep this file updated with the actionable plan, decisions, key progress, validation, and blockers. This file intentionally records an auditable execution plan rather than private chain-of-thought.

## Current Invocation Plan

1. Read `TODO.md` and identify the first task whose heading is not prefixed with `[DONE]`.
2. Check the latest commit message only for unfinished work that is directly relevant to that selected task.
3. Read the selected task details, dependencies, validation requirements, and any relevant nearby planning context.
4. Inspect only the code and fixtures needed to implement that task correctly.
5. Implement the selected task without workarounds or scope narrowing.
6. Add or update focused tests and fixtures required by the task.
7. Run the task-specified validation plus relevant targeted tests; fix any failures caused by this work.
8. Mark the task heading `[DONE]` in `TODO.md` and update its completion record.
9. Update `PLAN.md` only if phase-level sequencing, dependencies, assumptions, or completion criteria changed.
10. Commit all changes for this invocation with a clear task-tagged message, then stop.

## Progress Log

- Started invocation and recorded the initial execution plan before running shell commands.
- Selected first incomplete task: `P4-T01q`, which requires a frontend diagnostic for ordinary functions/methods missing bodies, while preserving the three allowed exceptions: `@Intrinsic`, `@Extern`, and abstract interface methods.
- Latest commit `75d589c3 [P4-T01p] Lock intrinsic member body diagnostics` is directly preceding but does not mention unfinished work; no prerequisite was added from commit history.
- Implemented `scoop::typecheck::fun_must_have_body` for non-sysroot ordinary user functions/methods; sysroot enforcement is handled by a dedicated audit because sysroot has header stubs backed by support sources.
- Added missing-body typecheck fixtures for top-level, member, generic, extension, `@Intrinsic`, `@Extern`, and interface abstract/default cases.
- Updated legacy effect-row fixtures to use real bodies or parameter-provided nominal values instead of declaration-only ordinary helper functions.
- Audited sysroot declaration-only ordinary surfaces, annotated compiler/runtime-backed sysroot declarations with `@Intrinsic`, preserved `core.scoop` `print/println` header stubs backed by bodyful `sysroot/print.scoop`, and added a sysroot audit unit test.
- Updated sysroot overlay fixture copies to satisfy the new declaration-body rules.
- Validation completed so far: `cargo run -p scoop -- test --fixtures tests/fixtures/typecheck`, `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass`, `cargo run -p scoop -- test --fixtures tests/fixtures/build`, `cargo test -p scoopc`, and `cargo clippy --all-targets -- -D warnings` all pass.

## Task-Specific Plan: P4-T01q

1. Inspect existing function-body representation, annotation checks, diagnostic definitions, and typecheck fixture metadata conventions.
2. Add a frontend/typecheck diagnostic for body-less ordinary functions and methods outside interface abstract declarations.
3. Ensure existing `@Extern` and `@Intrinsic` no-body declarations remain accepted and still use their existing inverse diagnostics when they incorrectly include a body.
4. Add focused positive and negative typecheck fixtures for top-level, member, generic, extension, extern, intrinsic, and interface abstract/default cases.
5. Audit sysroot declarations for ordinary body-less functions/methods and repair only noncompliant residuals, without deleting P4-T01 by-name intercepts.
6. Run formatting, targeted fixture validation, full `scoopc` tests, run-pass fixtures, and clippy.
7. Mark `P4-T01q` `[DONE]` with a completion record, then commit all changes for this invocation.
