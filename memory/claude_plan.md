# Claude Execution Plan

## Scope
- Follow the task queue exactly: read `TODO.md` as the index, then inspect the referenced detailed `TODO-Px.md` files in order.
- Select the first detailed task whose heading is not prefixed with `[DONE]`.
- Complete exactly that one task, or insert the minimum prerequisite task if a concrete blocker makes correct completion impossible.
- Keep `TODO.md` synchronized with the authoritative detailed TODO file.
- Commit the resulting changes and stop.

## Execution Steps
1. Inspect the current repository state and task files without changing implementation code.
2. Identify the first incomplete detailed task from the authoritative `TODO-Px.md` file.
3. Read the task requirements, constraints, dependencies, and completion record.
4. Inspect only the code, fixtures, and docs relevant to that task.
5. Implement the smallest spec-correct change needed for the task.
6. Add or update tests/fixtures required by the task.
7. Run relevant validation commands, escalating to broader tests if needed.
8. Fix any failures that are in scope for the selected task.
9. Update the detailed task heading with `[DONE]`, update its completion record, and sync `TODO.md` if the task appears there.
10. Update this plan file as key steps complete or if the plan changes.
11. Review the final diff for accidental/unrelated changes.
12. Commit all relevant changes with a task-specific commit message.
13. Stop after the commit without starting the next task.

## Current Status
- Task index read. First incomplete detailed task is `P7-T02Z` in `TODO-P7.md`.
- Task scope: close remaining default-refactor run-pass blockers before `P7-T03` full regression can resume.
- Latest commit is directly relevant: `[P7-T03] Fix default regression blockers and add run-pass prerequisite`, which introduced/recorded `P7-T02Z` as the current prerequisite.
- Working tree has only this plan file modified so far.
- Next execution plan: reproduce the current run-pass failures one fixture at a time, fix shared implementation gaps without fixture-specific workarounds, run targeted regression commands, then mark `P7-T02Z` done and commit.
- First reproduced blocker: `object_init_raise_try_catch_basic.scoop` stopped after `boom.init`.
- Diagnosis summary: a `TopLevelRef(BoomObject)` namespace receiver was lowered before the published hidden-effect member boundary for `BoomObject.x`, so object init raised outside the boundary. The implementation now skips `TopLevelRef` statements used only as static/member namespace receivers; the actual object/property init remains lowered by the published boundary.
- Second hidden-init blocker fixed: object value access and top-level immutable value access now publish `TopLevelRef` hidden init effects with stable MIR site ids, P4 `ClassCtor`-style site facts, P5 boundaries, and P6 outcome capture. Targeted fixtures `object_value_init_raise_helper_try_catch_basic.scoop` and `top_level_immutable_init_raise_helper_try_catch_basic.scoop` now pass.
- Roadblock found: virtual/interface hidden-suspend helpers expose dynamic dispatch ABI schema identity drift between the body program and ABI program. The current invocation inserted new prerequisite `P7-T02Za` before `P7-T02Z`, updated `P7-T02Z` to depend on it, and will stop after committing.
- Validation so far: `cargo check -p scoopc` passes; hidden-init targeted fixtures listed in `TODO-P7.md` pass. Remaining failing fixtures are `effect_handle_hidden_suspend_virtual_helper_basic.scoop` and `effect_handle_hidden_suspend_interface_helper_basic.scoop`, now tracked by `P7-T02Za`.
