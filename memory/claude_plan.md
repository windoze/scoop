# Claude Execution Plan

I will keep this file as an execution log and concise plan. I will not include private chain-of-thought, but I will record the actionable reasoning, decisions, and progress needed to audit the work.

## Current Objective

Complete exactly `HIR-T04：收口 splice field value.[field]`, validate it, mark it `[DONE]`, commit the resulting changes, and stop.

## Step-by-Step Plan

1. Read `TODO.md` first and identify the first task whose title is not prefixed with `[DONE]`.
2. Check the latest commit message only for directly relevant unfinished work tied to that selected task.
3. Read the selected task details, dependencies, validation requirements, and nearby context in `TODO.md`.
4. Inspect only the code and tests needed to implement that task correctly.
5. If a concrete blocker or missing prerequisite prevents spec-correct implementation, update `TODO.md` with the minimum prerequisite task, commit that bookkeeping, and stop.
6. Otherwise implement the task with the smallest correct code changes.
7. Add or update focused tests/fixtures required by the task.
8. Run the task-specified validation and any relevant narrower checks first; then run broader required checks if feasible.
9. Fix any regressions introduced by this task.
10. Mark the task title `[DONE]` in `TODO.md` and update its completion record with implementation and validation notes.
11. Commit all relevant changes with a descriptive task-tagged message.
12. Stop without starting the next task.

## Progress

- Plan initialized before repository inspection.
- Read `TODO.md`; the first incomplete task is `HIR-T04`.
- Checked latest commit summary: `ac9eecc2 [HIR-T05] Record completion plan update`. It is directly adjacent because `HIR-T04` depended on `HIR-T05`, but it does not name an unfinished issue that changes the current task.
- Inspected current splice field path. `ast::ExprKind::SpliceField` is already parsed and partially typechecked, but refactor HIR still lowers it to `ExprKind::Todo("splice_field")`; non-literal fields currently degrade to `Any`, which violates the HIR-T04 requirement for a static field contract or a clear diagnostic.

## Implementation Notes

- Add a typed splice field contract side table to `ast::File`, populated by typecheck for statically known field names.
- Treat `comptime for` binders as valid static splice field sources so runtime HIR lowering can consume the already evaluated iteration value.
- Reject non-static field expressions in typecheck with a dedicated diagnostic explaining that `.[field]` requires a compile-time known field name.
- Lower accepted splice fields to normal HIR `MemberAccess` with resolved field FQN; remove the `splice_field` HIR Todo constructor and update placeholder inventory/tests accordingly.

## Progress Update

- Implemented the AST/typecheck splice field contract side table and static-name diagnostic.
- Added `comptime for` binder tracking in expression typecheck so FieldMeta iteration can reach HIR lowering.
- Replaced `ExprKind::Todo("splice_field")` lowering with resolved HIR `MemberAccess` lowering.
- Added unit coverage for static string, FieldMeta descriptor, reflection loop FieldMeta, and non-static name rejection.
- Ran `cargo test -p scoopc --no-default-features refactor_hir_splice_field`: passed.
- Ran placeholder/no-Todo regression tests: `refactor_hir_placeholder_inventory` and `refactor_hir_no_todo`: passed.
- Ran specified fixtures: string/FieldMeta ok fixture, non-static negative fixture, unknown-field negative fixture, and `dump-hir tests/fixtures/comptime/splice_field_access_v0_basic.scoop`: passed after adding the existing required top-level type annotation to `P`.
- Ran `cargo test -p scoop --no-default-features dump_hir`: passed.
- Ran `cargo clippy -p scoopc -p scoop --no-default-features --all-targets -- -D warnings`: passed.
- Ran `cargo fmt`, then reran `cargo test -p scoopc --no-default-features refactor_hir_splice_field` and strict clippy: passed.
- Updated `TODO.md`: marked `HIR-T04` as `[DONE]` and added completion record with implementation and validation details.
