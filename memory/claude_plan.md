## Current Invocation Plan

Status: selected task U1-T02.

Scope: Complete exactly the first incomplete task from `TODO.md`, then stop after committing the result.

Execution plan:

1. Read `TODO.md` first and identify the first task whose title is not prefixed with `[DONE]`.
2. Check the latest commit message only for unfinished work directly relevant to that selected task.
3. Read the selected task details, dependencies, validation requirements, and any nearby context needed to implement it correctly.
4. Inspect the minimal relevant code and tests for the selected task.
5. Implement the task completely, avoiding workarounds or spec deviations.
6. Run targeted validation first, then broader required validation from the task entry.
7. If a concrete blocker prevents correct implementation, update `TODO.md` with the minimum prerequisite task, commit that bookkeeping, and stop.
8. If the task is completed, mark its title `[DONE]` in `TODO.md` and update its completion record.
9. Run final verification appropriate to the changed files.
10. Commit all relevant changes with a descriptive task-tagged message.
11. Stop without starting the next task.

Progress log:

- Created this plan before reading project task files or running commands.
- Read `TODO.md` and selected the first incomplete task: `U1-T02：inventory schema 文档 + 索引子命令`.
- Next checks are limited to the latest commit message and files directly needed for U1-T02.
- Latest commit was `[U1-T01] Generate UnsupportedMainBody inventory`; no explicit unfinished issue was found.
- Implemented the planned U1-T02 artifacts in draft: schema document, `umb-audit` bin target, and shared access to the U1 inventory scanner.
- Validation passed for `umb-audit list --bucket B-02`, `umb-audit diff`, `umb-audit stats`, `cargo test -p scoopc audit::umb_inventory -- --nocapture`, and `cargo clippy --all-targets -- -D warnings`.
- Updated `TODO.md` to mark U1-T02 `[DONE]` and record the completion details.
