# Claude Execution Plan

## Scope

- Follow `TODO.md` as the source of truth.
- Identify the first task whose heading is not prefixed with `[DONE]`.
- Complete exactly that task, then stop after committing.
- Do not perform broad historical triage before selecting the current task.

## Step-By-Step Plan

1. Read `TODO.md` to identify the first incomplete task and its validation requirements.
2. Check the latest commit message only for directly relevant unfinished work tied to the selected task.
3. Inspect the files and tests relevant to the selected task.
4. Implement the smallest spec-correct change needed for that task.
5. Add or update focused tests/fixtures required by the task.
6. Run the task-required validation and any nearby regression tests.
7. Fix any failures that are directly relevant to the task; if a concrete prerequisite blocks the task, update `TODO.md`, keep the task incomplete, commit the bookkeeping, and stop.
8. Mark the task heading in `TODO.md` with `[DONE]` and update its completion record.
9. Commit all relevant changes with a descriptive task-prefixed commit message.
10. Stop without starting the next task.

## Progress Log

- Initial plan recorded before inspecting or modifying project task state.
- Selected first incomplete task from `TODO.md`: `HIR-T06` (`array literal`, named/default/spread args, call arg canonicalization).
- Next step: check the latest commit message only for directly relevant unfinished work, then inspect the HIR/typecheck call and array lowering paths.
- Latest commit is `[HIR-T04] Lower splice fields in refactor HIR`; no directly relevant unfinished `HIR-T06` item was stated.
- Initial implementation direction: publish a typed call-argument binding contract from typecheck and make HIR lowering consume it so calls are emitted with ordered positional arguments, explicit defaults, and no raw named/spread syntax.
- Implemented first pass of the side table and HIR canonical call lowering edits. Next step is compiling the targeted `scoopc` tests to catch Rust type/borrow errors before adding fixtures.
- Completed implementation and validation for `HIR-T06`; updated `TODO.md` with `[DONE]` and a completion record. Next step is to review git diff and commit all task changes.
- Ran `cargo fmt`, reran `refactor_hir_call_args`, and reran strict clippy successfully before commit.
