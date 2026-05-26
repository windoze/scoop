# Execution Plan

I cannot record private chain-of-thought, but I will maintain a concrete execution plan and progress log here.

1. Read TODO.md and identify the first task whose heading is not prefixed with [DONE].
2. Review the selected task details, dependencies, validation requirements, and any directly relevant latest-commit context.
3. Inspect only the files needed to complete that task.
4. Implement the task or, if a concrete prerequisite blocks correct implementation, update TODO.md with the minimum required prerequisite and stop after committing.
5. Run formatting, linting, tests, and fixture validation required by the task and repository policy.
6. Update TODO.md completion record and mark the task title [DONE] only after validation is complete.
7. Commit all task-related changes, then stop without starting the next task.

## Current Task

- Selected first incomplete task: `P1-T00R`.
- Review focus: `scoopc check-source` must remain a generic non-fixture command and must cover resolve/typecheck/infer phase-only checks for single-file, multi-file, and cone-shaped inputs.
- Finding: `check-source --phase typecheck --source <pass-file>` on a mixed pass/fail project is currently blocked by diagnostics from an unselected sibling source during project-wide body checking.
- Updated plan: adjust project typecheck scoping so project-wide parsing/index/env construction remains intact, while selected-source phase checks do not run unselected source bodies before the selected file is checked.
- Progress: implemented selected-source typecheck scoping, added a regression test for mixed pass/fail project sources, and documented the `--source` scoping contract.
- Validation: `cargo fmt`, `cargo clippy --all-targets -- -D warnings`, targeted `check-source` mixed multi/cone smoke checks, `cargo test --all --all-targets`, and `cargo run -p scoop -- test` (1532 checks) all passed.
- Completion: marked `P1-T00R` done in `TODO.md`; next step is committing this task only.
