# Claude Execution Plan

## Scope

- Follow `TODO.md` as the authoritative task list.
- Complete exactly the first task whose heading is not prefixed with `[DONE]`.
- Do not continue to the next task after completion.

## Plan

1. Read `TODO.md` and identify the first incomplete task.
2. Inspect the latest commit only for unfinished work directly relevant to that task.
3. Read the task details, dependencies, validation requirements, and completion-record expectations.
4. Inspect the relevant implementation and test/fixture files.
5. Implement the smallest spec-correct change needed for the selected task.
6. Run targeted validation first, then any task-required broader validation.
7. If validation exposes an unscheduled failure, fix it if in scope or add the minimum prerequisite/follow-up task to `TODO.md` before marking completion.
8. Update `TODO.md` by prefixing the completed task title with `[DONE]` and filling in its completion record.
9. Update this file whenever a key step completes or the plan changes.
10. Review `git status`, `git diff`, and recent commits, then commit all intended changes with a task-specific message.
11. Stop after the commit.

## Progress

- Initial execution plan recorded.
- Identified first incomplete task: `P9-T01-a` in `TODO.md` / `TODO-7.md`.
- Latest commit is directly relevant: `[P9-T01] Schedule LLVM residual prerequisite`.
- Read `TODO-7.md` task body. Required validation includes fmt, workspace build/test, dependency gate, residual searches, and diff whitespace check.
- Implemented initial residual cleanup: `llvm/frontend.rs` no longer directly names HIR/MIR handoff types; LLVM direct HIR references are centralized through the LIR-owned `effect_lowered::source` namespace; non-`mir_body` LLVM direct MIR references were moved to the same LIR source namespace.
- Added dependency-gate recursive source boundary rules for LLVM production direct HIR and non-source-helper direct MIR residuals.
- Fixed source namespace collision by splitting HIR-shaped `source` and MIR-shaped `mir_source` payload namespaces.
- Validation completed: `cargo fmt`, `cargo build --workspace`, `cargo test --all --all-targets`, `cargo run -p scoop_tools -- dependency-gate`, residual searches, `cargo clippy --all-targets -- -D warnings`, and `git diff --check` all passed or produced only classified residual output.
- Updated `TODO.md` and `TODO-7.md`: `P9-T01-a` is now marked `[DONE]` with completion notes and validation record.
