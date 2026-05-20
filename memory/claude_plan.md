# Execution Plan

## Current Objective
- Follow `TODO.md` as the authoritative task list.
- Identify the first task whose heading is not prefixed with `[DONE]`.
- Complete exactly that task, verify it, update records, commit, then stop.

## Initial Steps
1. Read `TODO.md` to identify the first incomplete task and its validation requirements.
2. Check the latest commit message only for directly relevant unfinished work tied to that task.
3. Inspect the minimum relevant code, fixtures, and docs needed to implement the task.
4. If the task is blocked by a concrete prerequisite, update `TODO.md` with the prerequisite, commit that bookkeeping, and stop.

## Execution Steps
1. Implement the task without workarounds or fixture-only shortcuts.
2. Add or update tests/fixtures that validate the specified behavior.
3. Run the task-required validations plus focused relevant tests.
4. Fix any regressions directly introduced or blocking the task.
5. Mark the task heading `[DONE]` in `TODO.md` and update its completion record.
6. Commit all relevant changes with a descriptive task-tagged message.

## Progress Log
- Started by creating this plan file before running repository inspection commands.
- Identified first incomplete task from `TODO.md`: `P8-T01` (`建立公开 scoop_runtime.h runtime core header`).
- Next checks are limited to P8-T01 context: latest commit relevance, runtime header layout, native build include paths, runtime allowlist, and cone-local C fixture coverage.
- Implemented initial P8-T01 changes: added `runtime/c/include/scoop_runtime.h`, exposed GC thread attach/detach wrappers, auto-added the public include path to C/C++ native compilation, and added a run-pass cone fixture that includes the public header from cone-local C.
- Tightened the public header boundary after review: `String`/`Array` refs are opaque and object-header heap internals are not exposed.
- Validation completed: formatting, targeted include-path test, runtime allowlist, new fixture, `cargo build`, `cargo clippy --all-targets -- -D warnings`, `cargo test --all --all-targets`, and full fixture suite all passed after reruns noted in `TODO.md`.
- `TODO.md` now marks `P8-T01` as `[DONE]` and records completion details; next step is committing the P8-T01 changes only.
