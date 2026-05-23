# Current Invocation Plan

## Scope
- Use `TODO.md` as the authoritative task list.
- Identify the first task whose heading is not prefixed with `[DONE]`.
- Complete exactly that task, then stop after committing the result.

## Execution Steps
1. Read `TODO.md` to identify the first incomplete task and its validation requirements.
2. Check recent repository state only as needed for the selected task, including the latest commit if it directly affects the task.
3. Inspect the relevant code, fixtures, tests, and documentation for the selected task.
4. Implement the smallest spec-correct change that fully satisfies the task.
5. Run targeted tests first, then any required broader validation from the task entry.
6. If an unscheduled blocking failure appears, either fix it or add the minimum prerequisite task to `TODO.md` and stop.
7. Mark the completed task heading with `[DONE]`, update its completion record, and update this plan file with final status.
8. Review the diff, run any final required checks, and commit all task-related changes with a descriptive message.

## Progress Log
- Plan initialized before task selection.
- Selected first incomplete task: `P7-T05R` in `TODO-6.md`, review P7 full-package completion.
- Next checks are limited to the selected review scope, latest directly relevant commit context, and P7 boundary requirements.
- Task requirements: rerun P7-T05 validations, perform additional LLVM/pipeline residual searches for upstream stage outputs, HIR, and MIR coupling, then either fix blockers or mark `P7-T05R` complete with a review record.
- Working tree note: `PLUGIN_ABI.md` is currently untracked and appears unrelated to this task; it will not be modified or committed unless it becomes directly relevant.
- Static review found no production handoff blocker, but found dead legacy HIR top-level declaration/emission/ABI identity helpers. Removed those helpers and added dependency-gate source boundary checks so they cannot be reintroduced.
- Validation completed: `cargo fmt`, dependency gate, `cargo test -p scoop_tools`, `cargo test -p scoopc_lir_facts`, no-default `llvm_codegen_stage`, no-default `llvm::codegen`, default `llvm::codegen`, full run-pass fixtures, clippy with warnings denied, and `git diff --check` all passed.
- `TODO.md` and `TODO-6.md` updated to mark `P7-T05R` as `[DONE]` with the review record.
