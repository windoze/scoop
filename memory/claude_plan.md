# Claude Execution Plan

## Scope

- Follow `TODO.md` as the authoritative task list.
- Complete exactly the first task whose heading is not prefixed with `[DONE]`.
- Do not proceed to the next task in this invocation.
- If the selected task is blocked by a concrete prerequisite, update `TODO.md`, commit that bookkeeping, and stop.

## Execution Plan

1. Read `TODO.md` to identify the first incomplete task and its validation requirements.
2. Inspect only the files and code paths needed for that task.
3. Implement the task as specified, without weakening scope or adding workarounds.
4. Add or update the smallest relevant tests or fixtures needed to prove the behavior.
5. Run the task-specific validation commands, then broader checks if required by the task.
6. Update `TODO.md` by prefixing the completed task title with `[DONE]` and filling its completion record.
7. Update this file with key progress and validation results.
8. Commit all changes for the completed task with a task-tagged message.
9. Stop after the commit.

## Progress Log

- Initial plan recorded before reading task details or running commands.
- Selected first incomplete task: `P12-T05` from `TODO.md` / `TODO-5.md`.
- Task objective: constrain `SourceFile::is_sysroot()` usage so behavior only affects the standard-cone `@file:AllowIntrinsic` gate, while retaining loader identity APIs.
- Next actions: audit `is_sysroot` call sites, update `source.rs` documentation, remove any unexpected behavior checks, validate by grep/build/full fixture suite, then mark `P12-T05` done and commit.
- Audit result: found extra `is_sysroot()` behavior in parser f-string rejection and resolve auto-prelude skipping. Removed those call sites and updated tests so sysroot origin no longer changes parsing or prelude injection behavior.
- Validation completed: grep audit passed; `cargo build`, targeted parser/resolve tests, full fixture suite, `cargo test --all --all-targets`, and `cargo clippy --all-targets -- -D warnings` all passed.
- Task bookkeeping completed: `TODO.md` and `TODO-5.md` now mark `P12-T05` as `[DONE]` with completion notes.
