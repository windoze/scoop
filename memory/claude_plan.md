# Execution Plan

I cannot provide private chain-of-thought, but I can keep a concise, actionable plan and progress log here.

## Initial plan

1. Inspect the latest commit message and diff for any explicitly mentioned known issues that remain unresolved; fix those first if present.
2. Read `TODO.md` and identify the first incomplete task.
3. Read `PLAN.md` and relevant code to understand scope and dependencies for that task.
4. If the task is too large to complete safely in one pass, decompose it into concrete subtasks and update `PLAN.md` and `TODO.md` so the first new subtask becomes the task for this run.
5. Implement exactly the first incomplete task or subtask.
6. Run focused tests first, then broader validation as needed, aiming for clean `cargo`/`clippy` results without warnings.
7. Update `TODO.md`, `PLAN.md`, and this file to reflect completion or any dependency-driven reordering.
8. Commit all changes with a clear task-oriented message and stop.

## Progress log

- Plan recorded before shell commands.
- Inspected the latest commit, `TODO.md`, `PLAN.md`, and current repo state. No explicit unresolved issue was advertised by the latest commit beyond already-fixed warning cleanup.
- Determined that `T0150` is too large to land safely in one pass. It will be split into focused subtasks:
  1. `T0150a`: add shared `SourceMap` / source-location infrastructure and tests.
  2. `T0150b`: thread `SourceMap` through LLVM codegen while keeping current parsed literal payloads.
  3. `T0150c`: restore span-backed Int/String literal handling on top of `SourceMap`.
  4. `T0150d`: add literal parse diagnostics + multi-file failure coverage and remove T0140 leftovers.
- Current execution target: `T0150a`.
- Implemented `T0150a`:
  - `crates/scoopc/src/source.rs` now provides `SourceId`, `SourceMapSpan`, `SourceLocation`, and `SourceMap`.
  - `SourceMap` supports multi-file registration, validated local span binding, source slicing, line/column lookup, and non-overlapping global span mapping.
  - Added unit tests covering multi-file slicing, location lookup, and global span separation.
- Cleaned up four pre-existing non-LLVM warnings encountered on the focused validation path:
  - removed an unused import in `cone/consume.rs`;
  - renamed an unused bookkeeping field in `typecheck/expr/mod.rs`;
  - removed an unused duplicate helper struct in `typecheck/expr/call.rs`;
  - removed an unused helper in `typecheck/layout.rs`.
- Validation:
  - `cargo test -p scoopc source::tests --no-default-features` passed.
  - `cargo clippy -p scoopc --no-default-features --lib --tests -- -D warnings` still fails because the repo already has many broader pre-existing Clippy violations unrelated to this task (large enum variants, result-large-err, too-many-arguments, parser style lints, etc.).
