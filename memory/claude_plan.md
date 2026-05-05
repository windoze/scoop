# Claude Execution Plan

## Scope

- Follow the task workflow from `TODO.md` and the referenced detailed `TODO-Px.md` files.
- Complete exactly the first incomplete detailed task, then stop after committing.
- If a blocker prevents correct implementation, add the minimum prerequisite task in the appropriate detailed TODO file, sync `TODO.md`, commit, and stop.

## Execution Plan

1. Read `TODO.md` as the task index.
2. Open the referenced detailed TODO files in indexed order and identify the first heading that is not prefixed with `[DONE]`.
3. Inspect the selected task body, dependencies, validation requirements, and recent commit context only as needed for that task.
4. Implement the task without workarounds or spec deviations.
5. Run targeted tests first, then broader required validation for the changed area.
6. If validation exposes a task-blocking implementation gap, either fix it or add a prerequisite task before the blocked task and sync the index.
7. Mark the completed task title with `[DONE]`, update its completion record, and sync `TODO.md` if needed.
8. Update this plan file as key steps are completed or if the plan changes.
9. Commit all relevant uncommitted changes with a task-specific message.
10. Stop without starting the next task.

## Progress

- Initial execution plan recorded.
- Selected first incomplete detailed task: `P7-T02Zb` in `TODO-P7.md`.
- Latest commit is directly relevant because it added this higher-order prerequisite; treat the fixture blocker as the current task scope.
- Next step: reproduce `tests/fixtures/run-pass/effect_indirect_perform_nonresuming_function_value_higher_order_when_direct.scoop`, inspect effect facts / late-lowered handoff for higher-order returned function-value calls, implement the minimal spec-correct fix, then run the task validation commands.
- Reproduced failure: default refactor run-pass exits during late lowering because `main` sees `drive` outward `Ask.ask` and cannot project it into `main`'s pure output schema.
- Diagnostic plan update: add a focused solver regression to distinguish whether `Ask.ask` leaks from finalized effect facts or from late-lowering consumption.
- Implemented fix: callable facts now prefer real surface callable contracts over top-level function-value fallback, and refactor LLVM lowering now adapts plain closure values when a later `Use(Local)` coerces them to an effect-typed function value.
- Validation completed for the P7-T02Zb fixture, adjacent closure fixtures, effect-lowered tests, refactor LLVM tests, LLVM tests, formatting, and clippy. A broader `p7_default_pipeline` run still has the pre-existing async guard failure recorded in `TODO-P7.md`; the newly added higher-order guard passes.
- Marked `P7-T02Zb` as `[DONE]` in `TODO-P7.md` and synchronized `TODO.md`.
