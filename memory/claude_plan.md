# Claude Execution Plan

## Scope
- Follow `TODO.md` as the source of truth.
- Identify the first task whose heading is not prefixed with `[DONE]`.
- Complete exactly that task in this invocation, then stop.

## Steps
1. Read `TODO.md` to identify the first incomplete task and its validation requirements.
2. Inspect only the relevant project files needed for that task.
3. Implement the smallest spec-correct change required by the task.
4. Run the relevant focused tests first, then broader checks if appropriate for the changed area.
5. Update `TODO.md` by prefixing the completed task heading with `[DONE]` and filling in its completion record.
6. Update this plan file when key execution milestones are completed or if the plan changes.
7. Review the resulting git diff, include all required files, and commit with a task-specific message.
8. Stop without starting the next task.

## Current Status
- Identified first incomplete task: `HIR-T09` (`with` copy-update aggregate metadata).
- Latest commit has no directly relevant unfinished note.
- Implementation direction: publish a file-level typechecked `with` copy-update contract, make refactor HIR lowering consume that contract instead of emitting `ExprKind::Todo("with_update")`, and expose the contract in typed HIR stable dumps.
- Implementation completed and formatted.
- Validation completed so far: `cargo test -p scoopc --no-default-features with_update`, `cargo test -p scoopc --no-default-features refactor_hir_placeholder_inventory`, `cargo test -p scoopc --no-default-features refactor_hir_no_todo`, `cargo test -p scoopc --no-default-features refactor_typed_hir`, `cargo run -p scoop --no-default-features -- --effect-pipeline refactor test --fixtures tests/fixtures/typecheck`, `cargo run -p scoop --no-default-features -- --effect-pipeline refactor test --fixtures tests/fixtures/parse/with_update_expr.scoop`, `cargo test -p scoop --no-default-features dump_hir`, and `cargo clippy -p scoopc -p scoop --no-default-features --all-targets -- -D warnings`.
- `TODO.md` has been updated with `[DONE] HIR-T09` and its completion record.
- Next step: review git diff/status and commit the task.
