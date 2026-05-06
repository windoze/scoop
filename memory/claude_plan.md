# Execution Plan

## Scope
- Follow `TODO.md` as the authoritative task list.
- Identify the first task whose heading is not prefixed with `[DONE]`.
- Complete exactly that one task, then stop after committing.

## Steps
1. Read `TODO.md` to identify the first incomplete task and its validation requirements.
2. Check recent Git history only for directly relevant unfinished work related to that task.
3. Inspect the smallest necessary set of source files, fixtures, and docs for the selected task.
4. Implement the task without workarounds or spec deviations.
5. Run the task-specific validation, plus broader relevant tests if needed.
6. Update `TODO.md` by prefixing the completed task heading with `[DONE]` and filling its completion record.
7. Update this file when key milestones complete or if the plan changes.
8. Commit all changes for the completed task with a descriptive message.

## Progress
- Plan file initialized.
- Identified first incomplete task: `MIR-T02` materialized MIR strict verifier and no-param gate.
- Next step: inspect only directly relevant Git history and materializer/verifier code for `MIR-T02`.
- Inspection complete: latest commit is `MIR-T01`; no unfinished note directly changes `MIR-T02` ordering.
- Implementation focus: make materializer rewrite reject MIR Todo placeholders, add strict materialized MIR validation for unresolved params/effect rows and unresolved generic call roots, then update inventory/tests.
- Core implementation and targeted tests have been added; next step is formatting and running `refactor_materialized_mir` validation.
- Validation passed: `refactor_materialized_mir`, `refactor_mir_placeholder_inventory`, `refactor_mir_no_todo`, and `cargo clippy -p scoopc --no-default-features --all-targets -- -D warnings`.
- `TODO.md` has been updated to mark `MIR-T02` as `[DONE]` with completion records.
- Final step: inspect git status/diff, commit all MIR-T02 changes, then stop.
