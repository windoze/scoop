# Claude Execution Plan

## Current Invocation

Goal: complete exactly the first incomplete task in `TODO.md`, then stop after validation and a git commit.

## Execution Plan

1. Read `TODO.md` first and identify the first task whose heading is not prefixed with `[DONE]`.
2. Check the latest commit only for directly relevant unfinished work tied to that selected task.
3. Inspect the task requirements, dependencies, and validation notes before editing code.
4. Implement the task as written, avoiding workaround behavior or scope narrowing.
5. If a concrete blocker or missing prerequisite prevents correct implementation, update `TODO.md` with the minimum prerequisite task, keep the original task incomplete, commit that bookkeeping change, and stop.
6. Run focused tests first, then the task-required validation commands. Fix any failures that are in scope for the selected task.
7. Mark the completed task heading in `TODO.md` with `[DONE]` and update its completion record.
8. Update this plan file when key steps complete or if the plan changes.
9. Commit all relevant changes with a clear task-tagged message.
10. Stop without starting the next task.

## Progress Log

- Initialized execution plan before running repository commands.
- Read `TODO.md`; selected first incomplete task: `C3-T01` (`RefCell<T>` / `Box<T>` in sysroot).
- Next checks are limited to this task: inspect relevant sysroot/test files and latest commit for directly related unfinished work.
- Latest commit is `[C2-T02] Fix mutable closure capture locals`; no directly relevant unfinished C3-T01 blocker found.
- `sysroot/scoop.core/core.scoop` already hosts core root/container types, so `RefCell<T>` and `Box<T>` will be added there instead of creating a new sysroot file.
- Added `RefCell<T>` and `Box<T>` as ordinary sysroot classes.
- Added targeted coverage: sysroot type-env registration, run-pass construction/read/write behavior, and typecheck rejection for assigning `Box.value`.
- Fixed class-field lookup so a sysroot `Box.value` field cannot be selected while lowering unrelated struct fields with the same member name.
- Validation completed: `cargo build`, `cargo test -p scoopc sysroot_type_env -- --nocapture`, targeted fixtures, HIR fixture directory, and `cargo clippy --all-targets -- -D warnings` pass.
- Full `cargo run -p scoop -- test` was run after fixes; remaining failures are the three pre-tracked `C4-T01A` CaptureBox MIR snapshot refresh targets (`mir/closure_capture_var`, `mir_lowered/aggregate_transport`, `mir_lowered/assignment_places`).
- Marked `C3-T01` as `[DONE]` in `TODO.md` and filled its completion record.
