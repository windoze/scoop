# Claude Execution Plan

## Scope

- Follow `TODO.md` as the authoritative task list.
- Identify the first task whose heading is not prefixed with `[DONE]`.
- Complete exactly that one task, then stop after committing.
- Do not perform open-ended triage beyond issues that directly block the selected task.

## Step-by-Step Plan

1. Read `TODO.md` first and identify the first incomplete task by heading prefix.
2. Inspect the selected task's requirements, dependencies, completion record, and validation instructions.
3. Check the latest commit only for unfinished work directly relevant to the selected task.
4. Inspect the minimum relevant code, fixtures, and tests needed to implement the selected task correctly.
5. If a concrete prerequisite or spec mismatch blocks the task, update `TODO.md` with the minimum prerequisite task, commit that bookkeeping, and stop.
6. Otherwise, implement the task with the smallest correct code and fixture changes.
7. Run targeted validation first, then any broader validation required by the task.
8. Fix any regressions or warnings introduced by the task.
9. Update `TODO.md` by prefixing the completed task heading with `[DONE]` and filling its completion record.
10. Update this plan file when key steps complete or if the plan changes.
11. Review `git status`, `git diff`, and recent commits before committing.
12. Commit all task-related changes with a descriptive message.
13. Stop without starting the next task.

## Progress Log

- Initialized execution plan before reading project files or running commands.
- Read `TODO.md`; selected first incomplete task `P1-T03`.
- Read `TODO-2.md`; task scope is migrating `types` to `scoopc_types`, establishing initial `scoopc_ids` primitives, keeping `TemplateKey` / `InstanceKey` MIR-internal, and validating authoritative definitions.
- Checked latest commit `f38166b5 [P1-T02R] Review span and source migration`; it does not identify an unfinished issue that changes the selected task scope.
- Moved `ty` and layout authoritative definitions into `scoopc_types`; `scoopc::ty` is now a re-export adapter.
- Added initial `scoopc_ids` primitives for stable hash/key traits, manglers, canonical text helpers, `SiteId`, and a future `BodyVersionKey` extension point; `mir::SiteId` now re-exports the base type.
- Updated `scoopc::stable_id` to consume `scoopc_types` and re-export identity primitives from `scoopc_ids`; type-aware canonical encoding remains in the facade because it combines ids with the migrated type universe and still has `StableConeKey::from_manifest` tied to the not-yet-migrated project model.
- Validation passed: `cargo fmt`, `cargo test -p scoopc_types`, `cargo test -p scoopc_ids`, `cargo test --all --all-targets --no-default-features`, `cargo run -p scoop_tools -- dependency-gate`, `cargo tree -p scoopc_types`, `cargo tree -p scoopc_ids`, `cargo clippy --all-targets -- -D warnings`, and authoritative-definition searches.
- Marked `P1-T03` as `[DONE]` in `TODO.md` and `TODO-2.md` with completion record.
