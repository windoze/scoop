# Claude Plan

## Note

User asked for a complete thought process. I will not record private chain-of-thought, but I will keep a concise, high-level execution plan and progress log here.

## Initial Plan

1. Inspect the latest git commit to see whether it mentions a pre-existing issue that must be fixed first.
2. Read `TODO.md` and identify the first incomplete task.
3. Read `PLAN.md` to understand the intended task ordering and current project context.
4. If the first incomplete task is too large, decompose it into smaller subtasks, update `PLAN.md`, update `TODO.md`, and execute only the first resulting subtask.
5. Implement the selected task with the smallest correct code change.
6. Run the relevant tests, plus formatting/linting if required by the task or impacted code.
7. Update `TODO.md`, `PLAN.md`, and this file to reflect progress and any newly discovered prerequisite issues.
8. Commit exactly the changes for this iteration with a task-aligned commit message, then stop.

## Progress Log

- Created this plan file before running repository inspection commands.
- Inspected the latest commit, `TODO.md`, and `PLAN.md`.
- Latest commit is a review commit for `T5001c1R` and does not describe a separate pre-existing bug to fix first.
- The first incomplete task is `T5001c2`: switch explicit mode managed root enumeration to the TLS explicit frame chain and narrow `InNative` / `native_roots` responsibilities.
- Confirmed `T5001c2` was implementable without decomposition.
- Updated runtime thread records to snapshot `explicit_root_frame_top` during STW parking and `enter_native`, and to prefer explicit-frame root maps before stackmap fallback.
- Updated baseline and Immix GC mark/update/verify paths to consume the new managed-root snapshot logic.
- Added a runtime smoke helper and Rust test covering `enter_native + explicit root frame` behavior.
- During full verification, found a regression in `gc_verify_roots` for stackmap-mode `InNative` threads; fixed verify to keep the historical stackmap behavior while still validating explicit-frame snapshots when present.
- Verification completed successfully with `cargo test -p scoop_runtime --test explicit_root_frame`, `cargo test -p scoop_runtime --test gc_verify_roots`, `cargo test --all`, `cargo run -p scoop -- test --fixtures tests/fixtures/runtime_gc`, and `cargo clippy --all-targets -- -D warnings`.

## Current Execution Focus

1. Inspect runtime code paths for explicit mode root enumeration, `InNative`, `native_roots`, and stackmap lookup usage.
2. Decide whether `T5001c2` is directly implementable in one step or needs decomposition in `TODO.md` / `PLAN.md`.
3. If implementable, change the runtime so explicit mode uses the TLS explicit frame chain for managed roots, while keeping stackmap mode available.
4. Update tests or add targeted coverage for the new explicit-mode behavior.
5. Run relevant tests and lint.
6. Update `TODO.md`, `PLAN.md`, and this file.
7. Commit and stop.
