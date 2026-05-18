# Claude Execution Plan

## Scope

- Follow `TODO.md` as the authoritative task list.
- Identify the first task whose heading is not prefixed with `[DONE]`.
- Complete exactly that task, or if a concrete blocker prevents completion, add the minimum prerequisite task needed and stop.

## Execution Plan

1. Read `TODO.md` first and identify the first incomplete task.
2. Check recent Git context only for directly relevant unfinished work after the task is selected.
3. Inspect only the files needed to understand and implement that task.
4. Make the smallest correct implementation changes without workarounds or spec deviations.
5. Add or update targeted tests/fixtures required by the task.
6. Run the task-specified validation commands and any additional relevant checks.
7. Update `TODO.md` by prefixing the completed task heading with `[DONE]` and filling its completion record.
8. Update this file when key steps complete or if the plan changes.
9. Commit all relevant changes with a clear task-tagged message.
10. Stop without starting the next task.

## Progress

- Plan file initialized before reading project task files or running commands.
- Read `TODO.md` and identified the first incomplete task as `U1-T01: inventory 脚本 + CSV 主表`.
- Next key step: inspect latest commit for directly relevant unfinished work, then inspect only U1-T01-related files and baselines.
- Latest commit is `[U0-T01] Freeze UnsupportedMainBody baseline`; it is the expected prerequisite and does not introduce a separate blocker.
- Implementation approach for U1-T01: add a `#[cfg(test)]` audit module that scans `crates/scoopc/src/llvm/codegen/**/*.rs`, derives route/surface/bucket/class/spec/gate metadata, writes/checks `audit/UMB_inventory.csv`, and validates the 1,284-entry U0 baseline.
- Implemented the audit scanner/test module and confirmed it finds 1,284 entries. The formal per-entry scan observes 1,241 literal `kind` entries plus 43 dynamic/forwarded `kind` entries; this refines the U0 broad grep count while preserving the frozen constructor total.
- Generated `audit/UMB_inventory.csv`, verified no `TBD` fields, and confirmed all 36 buckets have entries.
- Validation completed: `cargo test -p scoopc audit::umb_inventory -- --nocapture`, `cargo test -p scoopc pipeline_user_visible_failure_policy -- --nocapture`, and `cargo clippy --all-targets -- -D warnings` all passed.
- Updated `TODO.md` to mark `U1-T01` as `[DONE]` with completion notes; next step is to commit all U1-T01 changes and stop.
