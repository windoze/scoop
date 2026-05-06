# Claude Execution Plan

## Current Invocation

1. Read `TODO.md` first and identify the first task whose heading is not prefixed with `[DONE]`.
2. Check the latest commit message only for directly relevant unfinished work tied to that task.
3. Inspect the task requirements and the minimum related code/tests needed to implement it correctly.
4. Implement the task without workaround behavior or scope narrowing.
5. Run the task-specified validation and any directly relevant tests; fix failures that are in scope.
6. Update `TODO.md` by prefixing the completed task heading with `[DONE]` and filling its completion record.
7. Update this plan file when key milestones complete or if the plan changes.
8. Commit all relevant changes with a clear task-tagged message, then stop.

## Progress

- Initial execution plan recorded before task discovery.
- Identified first incomplete task: `MIR-T11R` review of the generic materialization contract.
- Current focus: review `MIR-T11` implementation, rerun its validation commands, inspect materialized MIR for `generic_materialization.scoop`, and verify negative cases for Todo templates, missing roots, unresolved type params, and missing effect-row args.
- Code inspection focus completed for the `MIR-T11` materializer changes; no blocking issue identified before validation.
- Running the `MIR-T11R` required validation commands next.
- Validation passed: `cargo test -p scoopc --no-default-features refactor_mir_materialize_generics`.
- Validation passed: `cargo test -p scoopc --no-default-features refactor_materialized_mir`.
- Validation passed: `cargo run -p scoop --no-default-features -- --effect-pipeline refactor dump-mir tests/fixtures/mir_refactor/generic_materialization.scoop`.
- Additional materialized MIR audit passed: `cargo run -p scoop --no-default-features -- --effect-pipeline refactor dump-ir tests/fixtures/mir_refactor/generic_materialization.scoop`.
- Additional default effect-row materialization spot-check passed: `cargo run -p scoop --no-default-features -- --effect-pipeline refactor dump-ir tests/fixtures/typecheck/eff_row_param_default_pure_ok.scoop`.
- Regression passed: `cargo test -p scoopc --no-default-features refactor_mir_no_todo`.
- Lint passed: `cargo clippy -p scoopc --no-default-features --all-targets -- -D warnings`.
- Review conclusion: `MIR-T11` satisfies the generic root, effect-row arg, and materialization substitution contract; proceed to mark `MIR-T11R` done.
