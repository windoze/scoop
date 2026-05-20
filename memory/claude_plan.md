# Claude Execution Plan

## Current Invocation

Goal: complete exactly the first incomplete task in `TODO.md`, verify it, mark it `[DONE]`, commit the result, and stop.

## Execution Plan

1. Read `TODO.md` to identify the first task whose title is not prefixed with `[DONE]`.
2. Check the latest commit message only for unfinished work directly relevant to that selected task.
3. Read the selected task body, dependencies, validation requirements, and any nearby completion records.
4. Inspect only the code and tests relevant to the selected task.
5. Implement the smallest spec-correct change that fully completes the selected task.
6. Run targeted validation first, then any broader validation required by the task.
7. If validation exposes a blocking implementation/spec gap for the current task, add the minimum prerequisite task to `TODO.md`, keep the current task incomplete, commit that bookkeeping, and stop.
8. If the task is completed, update `TODO.md` by prefixing the task title with `[DONE]` and filling in the completion record.
9. Update this file after key milestones or if the plan changes.
10. Inspect git status and diff, then commit all changes relevant to this invocation with a descriptive task-tagged commit message.
11. Stop without starting the next task.

## Progress Log

- Started invocation and created this execution plan before reading project task files or running commands.
- Identified first incomplete task: `P1-T03R` review for the `types` / `ids` migration.
- Latest commit is `[P1-T03] Migrate types and identity primitives`, directly relevant to this review task.
- Review scope is limited to the P1-T03 migration boundary, required files, dependency direction, and task-specified validation.
- Manual review found no blocking code changes needed: authoritative `TypeId` / `TypeStore` / `EffectRow` definitions are in `scoopc_types`, authoritative `SiteId` is in `scoopc_ids`, and `TemplateKey` / `InstanceKey` remain MIR materialization internals.
- Dependency review shows `scoopc_types -> scoopc_span` and `scoopc_ids -> scoopc_span` only, with no stage/fact/backend/facade dependencies.
- Validation completed so far: `cargo fmt`, `cargo test -p scoopc_types`, `cargo test -p scoopc_ids`, `cargo tree -p scoopc_types`, `cargo tree -p scoopc_ids`, `cargo run -p scoop_tools -- dependency-gate`, and `cargo test --all --all-targets --no-default-features`.
- Final validation also passed: `cargo clippy --all-targets -- -D warnings`.
- Updated `TODO.md` and `TODO-2.md` to mark `P1-T03R` as `[DONE]` with review conclusion, validation commands, and residual risks.

## Previous Invocation Snapshot

The previous committed snapshot recorded completion of `P1-T03`:

- Selected first incomplete task `P1-T03`.
- Moved `ty` and layout authoritative definitions into `scoopc_types`; `scoopc::ty` became a re-export adapter.
- Added initial `scoopc_ids` primitives for stable hash/key traits, manglers, canonical text helpers, `SiteId`, and future `BodyVersionKey` extension point; `mir::SiteId` became a re-export of the base type.
- Updated `scoopc::stable_id` to consume `scoopc_types` and re-export identity primitives from `scoopc_ids`; type-aware canonical encoding remained in the facade pending project model migration.
- Validation passed for that task and `P1-T03` was marked `[DONE]` in `TODO.md` and `TODO-2.md`.
