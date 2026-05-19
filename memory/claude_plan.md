# Claude Execution Plan

## Scope

- Source of truth: `TODO.md` for task ordering, task body, dependencies, validation requirements, and completion records.
- Phase context: `PLAN.md` only if the selected task changes phase-level sequencing, dependencies, assumptions, or completion criteria.
- Current invocation goal: complete exactly the first incomplete task in `TODO.md`, commit it, then stop.

## Execution Plan

1. Read `TODO.md` first and identify the first task whose heading is not prefixed with `[DONE]`.
2. Check recent git context only as needed, especially whether the latest commit explicitly mentions an unfinished issue directly relevant to that selected task.
3. Read the selected task body, dependencies, validation requirements, and any relevant completion notes.
4. Inspect only the code, fixtures, docs, and tests needed to implement that task without broad historical triage.
5. If the selected task has a concrete blocker or missing prerequisite, update `TODO.md` with the minimum required prerequisite task in dependency order, record the blocker here, commit, and stop.
6. Otherwise, implement the selected task with small targeted patches.
7. Add or update the smallest relevant tests/fixtures required by the task.
8. Run task-specific validation first, then broader validation required by the task or affected code paths.
9. Fix any failures that are in scope for the selected task and repeat validation until passing.
10. Mark the task heading in `TODO.md` with `[DONE]` and update its completion record with implementation and validation details.
11. Update `PLAN.md` only if the task changed phase-level plan details.
12. Inspect git status and diff, then commit all intended changes with a task-specific commit message.
13. Stop after the commit without starting the next task.

## Progress Log

- New invocation started on 2026-05-19. Plan: re-read `TODO.md` as the task source of truth, select the first heading without `[DONE]`, inspect only directly relevant context, implement or record a concrete prerequisite blocker, validate, update task records, commit, and stop.
- Read `TODO.md`; first incomplete task is `P7-C2: B-25 Platform / RTTI intrinsic 实现`.
- Immediate execution plan for P7-C2: check latest commit for directly relevant unfinished notes, list active B-25 rows, inspect B-25 strategy/category docs and fixtures, inspect RTTI/type-test/as? lowering sites, implement real RTTI metadata/runtime type lowering or add a concrete prerequisite if blocked, validate B-25/audit/policy/clippy, mark TODO complete, commit, and stop.
- Latest commit is `[P7-C1] Retire reflection comptime UMB rows`; no directly relevant unfinished B-25 note was found.
- B-25 active IDs are `UMB-1044`, `UMB-1057`, `UMB-1060`, `UMB-1061`, `UMB-1062`, `UMB-1063`, `UMB-1086`, `UMB-1089`, `UMB-1215`, `UMB-1216`, `UMB-1217`, `UMB-1218`, `UMB-1219`, and `UMB-1220`.
- Implementation focus: retire B-25 `UnsupportedMainBody` fallbacks in MIR runtime type/cast/pattern and getPlatform literal lowering by relying on materialized MIR / sysroot contracts and preserving real runtime descriptor/platform codegen; expand B-25 fixture coverage to exercise both RTTI and Platform paths.
- Implemented B-25 production changes: runtime type/cast/pattern and getPlatform UMB fallbacks now route to verifier/sysroot invariant panics; materialized MIR pattern type-test validation now checks descriptor shape and dynamic codegen support; B-25 fixtures are active and the directory passes.
- Updated B-25 audit data: removed 14 active rows from `audit/UMB_inventory.csv`, appended them to `audit/UMB_retired.csv`, updated B-25 category/strategy/overview/spec matrix/fixture index, and adjusted stale UMB baseline from 149 to 135.
- Validation completed: `umb-audit list --bucket B-25`, `umb-audit diff`, `umb-audit stats`, `cargo test -p scoopc audit:: -- --nocapture`, `cargo test -p scoopc pipeline_user_visible_failure_policy -- --nocapture`, `cargo test -p scoopc mir::materialize -- --nocapture`, B-25 fixture directory, B-14 RTTI smoke fixture, `cargo fmt`, and `cargo clippy --all-targets -- -D warnings` all pass.
- Updated `TODO.md` to mark P7-C2 `[DONE]` with completion record and current active=183/retired=1101 status.
- Initialized execution plan before reading project task files.
- Read `TODO.md`; first incomplete task is `P7-C1: B-24 Reflection / comptime intrinsic 实现`.
- Next focus: inspect recent git context, B-24 inventory/category/strategy docs, B-24 fixtures, and the reflection/comptime intrinsic code paths before editing.
- Latest commit is `[P7-B3.5] Retire named unsafe FunPtr UMB rows`; no explicit unfinished B-24 issue was found.
- B-24 active IDs are `UMB-0568`, `UMB-0569`, `UMB-0571`, `UMB-0954`, `UMB-0956`, and `UMB-0957`.
- Implementation focus: replace B-24 `UnsupportedMainBody` fallbacks in HIR and MIR reflection/comptime intrinsic lowering with verified contract handling and real size/kind/desc lowering.
- Implemented the codegen change: HIR `sizeOf` now consumes type arguments for `sizeOf<T>()` and legacy value overloads use static value type; MIR `sizeOf`/`kindOf`/`descOf` unsupported type drift now routes to verified intrinsic contract panic boundaries instead of UMB diagnostics.
- `umb-audit diff` now reports exactly the six B-24 rows as deleted; next step is to retire those rows in audit data and activate B-24 fixtures.
- Added `umb_fix` comptime routing for fixtures that need const-eval diagnostics, activated B-24 fixtures, and added a B-24 runtime codegen smoke for `sizeOf`/`kindOf`/`descOf`.
- Updated inventory and retired ledger; `umb-audit diff` is in sync with active=197 and retired=1087.
- Validation completed: B-24 list is empty; B-24 fixture directory passes; `umb-audit diff`/`stats`, `cargo test -p scoopc audit:: -- --nocapture`, `cargo test -p scoopc pipeline_user_visible_failure_policy -- --nocapture`, `cargo fmt`, and `cargo clippy --all-targets -- -D warnings` pass.
- Updated `TODO.md` to mark P7-C1 `[DONE]` with completion record.
