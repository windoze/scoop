# Claude Execution Plan

## Scope

- Follow the project task workflow exactly: identify the first incomplete detailed task from `TODO.md` and the referenced `TODO-Px.md` files, complete that one task only, update task records, verify, commit, then stop.
- Treat `TODO-Px.md` files as authoritative when they disagree with `TODO.md`.
- Do not perform broad historical bug triage before selecting the current task.
- Do not use workarounds or weaken fixtures; if a concrete blocker prevents correct implementation, add the minimum prerequisite task in the appropriate detailed TODO file, sync the index, commit, and stop.

## Step-by-Step Plan

1. Read the task index in `TODO.md`.
2. Open the referenced detailed TODO files in indexed order and identify the first task whose heading is not prefixed with `[DONE]`.
3. Check the latest commit message only for unfinished issues directly relevant to that selected task.
4. Inspect the selected task's requirements, constraints, dependencies, and validation commands.
5. Examine the relevant source, tests, and fixtures needed for that task.
6. Implement the smallest correct change that satisfies the detailed task requirements.
7. Add or update tests/fixtures required by the task.
8. Run the task-specific validation first, then broader relevant checks if feasible.
9. If validation exposes a directly blocking spec or implementation gap, fix it if it is part of the task; otherwise add a prerequisite task, sync `TODO.md`, commit, and stop.
10. When the task is complete, prefix the detailed task heading with `[DONE]`, update its completion record, and sync the matching `TODO.md` index entry.
11. Update this plan file with key progress and final validation status.
12. Review the worktree, commit all relevant changes with a descriptive task-tagged message, and stop without starting the next task.

## Progress Log

- Plan initialized before repository inspection.
- Read `TODO.md` and `TODO-P6-part2.md`; selected first incomplete detailed task: `P6-T02qf`.
- Latest commit mentions `P6-T02qf`, so any existing unfinished selector-propagation work is directly in scope for this invocation.
- Implemented selector propagation through run-pass, dump-stackmaps build, and run_pass_cone command construction paths; added focused command-construction regression tests.
- Validation completed: formatting, selector propagation unit tests, refactor minimal run-pass smoke, and `cargo clippy -p scoop --all-targets -- -D warnings` passed.
- Optional effect run-pass smoke now fails through the refactor path with `scoop::llvm::refactor_effect_lowering_unsupported`, confirming it no longer silently falls back to legacy before `P6-T03`.
- Marked `P6-T02qf` complete in `TODO-P6-part2.md` and synchronized `TODO.md`.
