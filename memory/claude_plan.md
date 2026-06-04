# Claude Execution Plan

## Scope

- Follow `TODO.md` as the authoritative task source.
- Identify and complete exactly the first task whose heading is not prefixed with `[DONE]`.
- Stop after committing that one task, or after committing any required prerequisite/blocker bookkeeping if the task cannot proceed.

## Step-by-Step Plan

1. Read `TODO.md` and identify the first incomplete task by heading prefix.
2. Check the latest commit message only for unfinished work directly relevant to that task.
3. Read the task details, dependencies, validation requirements, and any relevant project files.
4. Implement the task as written, avoiding workarounds or scope narrowing.
5. If a concrete blocker or missing prerequisite prevents correct implementation, update `TODO.md` with the minimum prerequisite task, keep the current task incomplete, commit, and stop.
6. Run validation in the required order for code changes: `cargo fmt`, `cargo clippy --all-targets -- -D warnings`, then relevant/full tests and fixture suite as required.
7. Address any failing unscheduled test or fixture by fixing it or scheduling it before marking the task complete.
8. Update `TODO.md` by prefixing the completed task title with `[DONE]` and filling its completion record.
9. Update this progress file at key milestones.
10. Inspect git status/diff/log, commit all intended changes with a task-specific message, and stop.

## Progress

- Plan initialized before running project commands.
- Identified first incomplete task: `T2-07` (define total LIR instruction set with handle-based references and no placeholder variants).
- Latest commit `5794d914 [T2-06-R] Review LIR layout ownership` has no explicit unfinished issue directly relevant to `T2-07`.
- Added a new `effect_lowered::instruction` module defining LIR operands, statements, rvalues, call kinds, transport wrappers, runtime metadata, and a state-terminator alias to the existing late-lowered state graph terminator.
- Added `LirStateBody` to explicitly bind state-owned LIR statements to the existing late-lowered state terminator type without replacing state storage yet.
- Validation passed: `cargo fmt`, `cargo clippy --all-targets -- -D warnings`, `cargo test --all --all-targets`, `cargo build -p scoop -p scoopc`, `python3 tools/dependency_gate.py`, `python3 tools/spec_fixtures.py check`, and `python3 tools/run_fixtures.py`.
- Updated `TODO.md`: marked `T2-07` as `[DONE]` and added the completion record.
