# Claude Execution Plan

## Scope

- Follow `TODO.md` as the authoritative task list.
- Complete exactly the first task whose heading is not prefixed with `[DONE]`, then stop.
- Do not perform broad historical issue triage before selecting that task.
- Do not use workarounds for missing or incorrect behavior; if a blocking prerequisite is found, record it in `TODO.md`, commit, and stop.

## Current Plan

1. Read `TODO.md` and identify the first incomplete task by heading prefix.
2. Review only the files and context needed for that task, including recent commit information only if it is directly relevant.
3. Implement the task as written, or add the minimum prerequisite task if correct implementation is blocked.
4. Run the task's required validation plus focused relevant checks.
5. Update `TODO.md` by prefixing the completed task heading with `[DONE]` and filling in the completion record, or record the blocker/prerequisite if blocked.
6. Update this file after major milestones or if the plan changes.
7. Inspect git status/diff/log, then commit all intended changes with a task-specific message.
8. Stop without starting the next task.

## Progress

- Initial plan recorded before running repository commands.
- Read `TODO.md`; first incomplete task is `P3-T01`: parse `[cone].kind` as `bin` / `lib` / `syslib` in `Cone.toml`.
- Latest commit is `[P2-T03] Support Pure function Any casts`; it does not mention an unfinished issue relevant to `P3-T01`.
- Decision: require explicit `[cone].kind` now instead of adding a temporary `bin` default, and update existing source-cone fixtures/tests to declare `kind = "bin"`.
- Implemented `ConeKind` parsing in `crates/scoopc/src/cone/manifest.rs`, added parser coverage for all allowed variants plus invalid/missing kind, updated generated/test manifests to declare `kind = "bin"`.
- Validation passed: `cargo fmt`; manifest/package focused tests; `cargo test -p scoop --bin scoop`; `cargo build`; `cargo clippy --all-targets -- -D warnings`; `cargo test --all --all-targets`; full `cargo run -p scoop -- test`.
- Updated `TODO.md` to mark `P3-T01` as `[DONE]` with completion record and next task `P3-T02`.
- Next: inspect git status/diff/log, then commit the completed task.
