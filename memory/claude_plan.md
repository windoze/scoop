# Claude Execution Plan

## Scope

- Complete exactly one detailed task: the first task in the authoritative `TODO-Px.md` files whose heading is not prefixed with `[DONE]`.
- Treat `TODO.md` as an index only and keep it synchronized with the detailed task files.
- If a concrete blocker prevents the current task from being implemented correctly, add the minimum prerequisite task in the appropriate detailed TODO file, sync `TODO.md`, commit that bookkeeping, and stop.

## Step-by-Step Plan

1. Read `TODO.md` to understand the indexed task order and referenced detailed TODO files.
2. Inspect the referenced `TODO-Px.md` files in order and identify the first detailed task heading without `[DONE]`.
3. Check the latest commit only for an explicitly mentioned unfinished issue directly relevant to the selected task.
4. Read the selected task body, constraints, dependencies, validation requirements, and completion-record format.
5. Inspect only the code, fixtures, docs, and tests needed to implement that selected task correctly.
6. Implement the task with the smallest spec-correct change set; do not use workarounds or weaken tests/fixtures.
7. Add or update targeted tests and fixtures required by the task.
8. Run relevant verification commands, escalating to broader tests if the task or failures require it.
9. Fix any issue introduced by this work, or add a prerequisite task if a pre-existing implementation gap blocks the specified behavior.
10. Mark the completed task heading with `[DONE]` in the authoritative `TODO-Px.md` file and update its completion record.
11. Sync `TODO.md` with any changed task title/status/order.
12. Update this plan file when major steps complete or if the execution path changes.
13. Review the final diff, commit all relevant uncommitted files with a clear task-tagged message, and stop without starting the next task.

## Current Status

- `TODO.md` and `TODO-P6-part3.md` identify `P6-T02qga` as the first incomplete detailed task.
- Latest commit is `[P6-T02qga] Track call-boundary continuation prerequisite`, so its recorded blocker is directly part of the current task.
- Implemented the first pass of the `P6-T02qga` approach: late-lowered call-boundary composition contracts, dump rendering, ABI validation, continuation object storage for composed callee continuations, and body-emitter resume dispatch that calls callee resume before caller boundary dispatch.
- Verification completed for the required targeted tests and fixture run-pass. `cargo clippy --all-targets -- -D warnings` completed successfully for Rust lints, while the existing C runtime build script still emits macOS SDK deprecation warnings for `getsectbynamefromheader_64`.
- `TODO-P6-part3.md` and `TODO.md` have been updated to mark `P6-T02qga` as `[DONE]` with completion notes.
- Next step: review the final diff, commit all relevant changes with a `P6-T02qga` message, then stop.
