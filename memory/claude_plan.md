# Claude Execution Plan

## Current Invocation

Goal: complete exactly the first incomplete task in `TODO.md`, validate it, mark it `[DONE]`, commit the resulting changes, and stop.

## Plan

1. Read `TODO.md` first and identify the first task whose title is not prefixed with `[DONE]`.
2. Check the latest commit message only for unfinished work directly relevant to that selected task.
3. Inspect the selected task's required files, dependencies, and validation criteria.
4. Implement the smallest spec-correct change needed for that task, without workarounds or scope narrowing.
5. Add or update tests/fixtures required by the task.
6. Run the task-specific validation commands and any relevant broader checks.
7. If a concrete blocker prevents completion, update `TODO.md` with the minimum prerequisite task, commit that bookkeeping, and stop.
8. If the task is completed, prefix its `TODO.md` title with `[DONE]` and update its completion record.
9. Commit all relevant current changes with a task-scoped commit message.
10. Stop without starting the next task.

## Progress

- Initial execution plan recorded before running code or shell commands.
- Identified first incomplete task: `HIR-T11` (`收口 custom iterator for-loop 与 remaining debug fallbacks`).
- Latest commit `[HIR-T10] Add assignment place contracts` does not declare unfinished work directly blocking `HIR-T11`.
- Found two relevant fallback paths: parser fallback can produce `ast::StmtKind::Missing` on successful parse, and HIR lowering can emit `StmtKind::Todo("for_custom_iterator")` when the typechecked for-loop contract is absent.

## Implementation Notes

1. Make unknown block statements produce a parse error and use `StmtKind::Missing` only as recovery state for failed parses.
2. Add typed HIR for-loop contract validation during lowering so missing `resolved_for_info` or missing custom iterator metadata becomes `HirStageError` before any successful handoff.
3. Remove `missing_stmt` and `for_custom_iterator` HIR Todo constructors; if such AST reaches lowering unexpectedly, record a stage error and return a throwaway empty statement that is not exposed on successful paths.
4. Update placeholder inventory/no-Todo tests to remove the eliminated reasons.
5. Add targeted tests and fixtures for custom iterator success/error and parser recovery failure.

- Parser fallback now reports an error for unknown statements instead of returning a successful `StmtKind::Missing`.
- HIR lowering now records a stage error for unexpected Missing statements or missing for-loop contracts instead of constructing `missing_stmt` / `for_custom_iterator` Todo nodes.
- Synthetic custom-iterator `iterator()` / `next()` calls now use distinct synthetic spans so typed call contracts do not collide.
- Added targeted custom iterator HIR tests and a parser recovery negative fixture.
- Validation passed for targeted HIR tests, placeholder inventory/no-Todo tests, parser fixtures, custom iterator fixtures, dump-hir tests, and clippy.
- `TODO.md` now marks `HIR-T11` as `[DONE]` with completion notes and validation commands.
