# Claude Execution Plan

## Scope

- Follow `TODO.md` as the authoritative task list.
- Identify the first task whose heading is not prefixed with `[DONE]`.
- Complete exactly that one task, update its completion record, commit the result, then stop.

## Execution Plan

1. Read `TODO.md` first and identify the first incomplete task by heading prefix.
2. Inspect recent git history only as needed to detect an explicitly unfinished issue directly relevant to that task.
3. Read the files and tests relevant to the selected task.
4. Implement the task without narrowing scope or adding workaround behavior.
5. Run targeted validation first, then broader required validation from the task entry where feasible.
6. If a concrete blocker prevents spec-correct completion, update `TODO.md` with the minimum prerequisite task, commit that bookkeeping, and stop.
7. If the task is completed, mark its `TODO.md` heading with `[DONE]`, update its completion record, and leave `PLAN.md` unchanged unless phase-level sequencing changed.
8. Inspect git status and diffs, then commit all intended changes with a task-specific message.

## Progress Log

- Initialized execution plan before running repository commands.
- Read `TODO.md` and selected first incomplete task: `P7-B2.3` for B-06/B-07/B-21 aggregate, pattern, and field schema contract retirement.
- Next steps: inspect latest commit for directly relevant unfinished issues, inspect current worktree, then gather active IDs, strategy docs, fixtures, verifier code, and LLVM fallback sites for this task only.
- Confirmed latest commit is completed P7-B2.2 and worktree only had this plan file.
- Implemented verifier-backed internal contracts for materialized aggregate schema, pattern schema, pattern extraction, and value member metadata; replaced the 80 target `UnsupportedMainBody` sites with internal invariant panics/expectations.
- `umb-audit diff` now reports exactly 80 deletions for B-06/B-07/B-21 target IDs plus expected line drift for remaining active rows.
- Synchronized inventory, retired ledger, bucket docs, strategies, spec matrix, fixture index, fixture headers, stale baseline, and `TODO.md` completion record.
- Validation completed: `umb-audit list` for B-06/B-07/B-21, `umb-audit diff`, `umb-audit stats`, materialize tests, audit tests, failure-policy tests, B-06/B-07/B-21 fixture directories, `cargo fmt`, and `cargo clippy --all-targets -- -D warnings` all pass.
