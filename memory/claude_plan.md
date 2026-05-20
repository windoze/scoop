## Current Invocation Plan

Scope: Complete exactly the first incomplete task in `TODO.md`, then stop after committing the result.

Constraints:
- `TODO.md` is the authoritative task source and completion state is determined only by `[DONE]` in task headings.
- Do not skip review tasks or tasks with partial completion notes.
- Do not use workarounds for missing or broken spec behavior; add a prerequisite task if a real blocker is found.
- Update `PLAN.md` only if phase-level sequencing or dependencies change.
- Mark the completed task with `[DONE]` and update its completion record before committing.
- Run relevant validation, including broader checks when practical.

Step-by-step plan:
1. Read `TODO.md` first and identify the first heading not prefixed with `[DONE]`.
2. Inspect that task's details, dependencies, validation requirements, and relevant nearby context.
3. Check the latest commit only for directly relevant unfinished issues if the task points to or appears affected by one.
4. Examine the minimum relevant code and fixtures needed to understand and implement the task.
5. Implement the task directly and spec-correctly, using small targeted patches.
6. Add or update tests/fixtures required by the task.
7. Run task-specific validation and then broader validation as required by the task and repository guidance.
8. If a blocker prevents spec-correct completion, update `TODO.md` with the minimum prerequisite task, record the blocker here, commit, and stop.
9. If validation passes, update `TODO.md` by prefixing the task heading with `[DONE]` and filling in the completion record.
10. Review `git status`, `git diff`, and recent log, then commit all intended task changes with a task-tagged message.
11. Stop without starting the next task.

Progress:
- Plan initialized before reading task files or running repository commands.
- First incomplete task identified: `P8-T03` (`迁移 scoop.sync native implementation`).
- Current focus: migrate `scoop.sync` native C ownership out of runtime core into the sysroot cone native-build path, update allowlist/tests, and stop after committing this task only.
- Implementation in progress: moved sync native ownership to `sysroot/lib/scoop.sync/native/`, added `native-build` metadata, converted most public sync API calls to Scoop wrappers over private `@Extern(abi = "scoop")` primitives, kept only the private `Once.run` closure adapter as an intrinsic, and removed sync symbols from the runtime core allowlist.
- Validation completed: sync and run-pass fixtures passed, runtime allowlist passed, ordinary link symbol regression passed, `cargo build`, `cargo clippy --all-targets -- -D warnings`, `cargo test --all --all-targets`, and full `cargo run -p scoop -- test` passed.
- Completion bookkeeping completed: `TODO.md` now marks `P8-T03` as `[DONE]` with a completion record and next task `P8-T04`.
