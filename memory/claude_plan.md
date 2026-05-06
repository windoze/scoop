# Claude Plan

## Scope

Execute exactly the first incomplete task from `TODO.md`, then stop after implementation, validation, documentation updates, and a git commit.

## Execution Plan

1. Read `TODO.md` first and identify the first heading whose title is not prefixed with `[DONE]`.
2. Check the latest commit message only for unfinished work directly relevant to that selected task.
3. Read the selected task details, dependencies, validation requirements, and any nearby context needed to implement it correctly.
4. Inspect only the relevant code, fixtures, specs, or tests needed for the selected task.
5. Implement the smallest spec-correct change for that task, without workaround behavior or fixture-only hacks.
6. If a concrete blocker or missing prerequisite prevents correct implementation, update `TODO.md` with the minimum prerequisite task in dependency order, keep the current task incomplete, commit that bookkeeping, and stop.
7. Run the task-required validation plus any focused tests needed for changed code.
8. Fix any failures directly caused by the current task and rerun validation until it passes or a real blocker is recorded.
9. Mark the completed task title in `TODO.md` with `[DONE]` and update its completion record with implementation and validation notes.
10. Update `PLAN.md` only if the phase-level sequencing, dependencies, assumptions, or completion criteria changed.
11. Review git status/diff, commit all relevant changes with a task-specific message, and stop without starting the next task.

## Progress Log

- Initial plan recorded before project inspection.
- Selected first incomplete task: `CG-T00R` (`Review CG-T00 codegen inventory 与 backend gate`).
- Review-specific plan: rerun `CG-T00` validation commands, inspect codegen gap inventory and backend gate coverage against the documented pipeline gaps, search unsupported/fallback trigger strings for owner traceability, then either record findings or mark `CG-T00R` done with validation notes.
- Context review completed: inspected `PIPELINE_GAPS.md`, `PLAN-pipeline-gaps-codegen.md`, `codegen_gap_inventory.rs`, the raw MIR gate call site, and the refactor LLVM smoke test. Next step is validation execution.
- Validation completed: `cargo test -p scoopc codegen_gap_inventory`, `cargo test -p scoopc refactor_llvm_backend_gate`, trigger-pattern searches, and `cargo clippy --all-targets -- -D warnings` passed. `TODO.md` has been updated to mark `CG-T00R` done with completion notes.
