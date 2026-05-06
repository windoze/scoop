# Current Invocation Plan

## Reasoning Summary

- `TODO.md` is the authoritative task list, and the first heading without a `[DONE]` prefix is the only task to complete in this invocation.
- I will avoid broad triage before selecting that task. I will only treat existing issues as in scope if they directly block the selected task or invalidate its specified behavior.
- I will not use workaround implementations. If the task exposes a missing prerequisite or spec mismatch that prevents correct implementation, I will add the minimum prerequisite task in `TODO.md`, leave the current task incomplete, commit that bookkeeping, and stop.
- I will update this file after key milestones or if the plan changes.

## Step-by-Step Plan

1. Read `TODO.md` and identify the first incomplete task by checking task headings for the `[DONE]` prefix.
2. Check the latest commit message only for directly relevant unfinished issue context.
3. Inspect the code and tests needed for the selected task, keeping the scope limited to that task.
4. Implement the smallest correct change that satisfies the task requirements.
5. Add or update tests/fixtures required by the task.
6. Run the task-specified validation commands and any focused tests needed for confidence.
7. If validation fails, diagnose and fix issues that are in scope for the selected task, then rerun validation.
8. Mark the selected task as `[DONE]` in `TODO.md` and update its completion record.
9. Update this plan file with completion notes.
10. Commit all relevant changes with a task-specific commit message.
11. Stop without starting the next task.

## Selected Task

- First incomplete task: `MIR-T11：收口 generic root、effect-row args 与 materialization substitution`.
- Latest commit checked: `[MIR-T10R] Review composite transport contract`; no directly relevant unfinished blocker was indicated by the commit subject.

## Task-Specific Plan

1. Inspect existing MIR root index, typed call-site contracts, generic metadata, and materializer substitution code.
2. Identify current test coverage and fixture style for `mir_refactor` materialization tests.
3. Extend or verify root publication for top-level/member/extension/constructor/object-side callables.
4. Ensure instance keys and materialization paths include type args, effect-row args, owner/receiver identity, and callable version where represented by existing data structures.
5. Extend substitution/no-param validation across all MIR metadata surfaces listed by `MIR-T11`.
6. Add `generic_materialization.scoop` and focused Rust tests/negative cases under `refactor_mir_materialize_generics`.
7. Run the `MIR-T11` validation command and focused regressions needed by adjacent materializer/root verifier code.
8. Mark `MIR-T11` `[DONE]`, record validation results, update this file, commit, and stop.

## Progress Notes

- Selected task `MIR-T11` has been implemented and marked `[DONE]` in `TODO.md`.
- Implemented generic materialization support around merged AST/lowered-HIR/monomorph call-site bindings, materialized direct-call and top-level-ref target rewriting, duplicate overlapping binding handling, and concrete result/member type repair for materialized pass-view bodies.
- Added `tests/fixtures/mir_refactor/generic_materialization.scoop` coverage for top-level/member/extension/object generic callables, effect-row args, generic constructor concrete owner type, and closure/effect-row substitution.
- Validation completed successfully:
  - `cargo test -p scoopc --no-default-features refactor_mir_materialize_generics`
  - `cargo run -p scoop --no-default-features -- --effect-pipeline refactor dump-mir tests/fixtures/mir_refactor/generic_materialization.scoop`
  - `cargo test -p scoopc --no-default-features refactor_materialized_mir`
  - `cargo test -p scoopc --no-default-features refactor_mir_no_todo`
  - `cargo clippy -p scoopc --no-default-features --all-targets -- -D warnings`
- Next step: commit the current task changes only, then stop.
