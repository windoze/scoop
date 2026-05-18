# Claude Execution Plan

## Scope

- Follow `TODO.md` as the authoritative task list.
- Identify the first task whose heading is not prefixed with `[DONE]`.
- Complete exactly that one task, then stop.

## Execution Plan

1. Read `TODO.md` to identify the first incomplete task and its validation requirements.
2. Check the latest commit message only for directly relevant unfinished context.
3. Inspect the relevant code, fixtures, and documentation for the selected task.
4. Implement the smallest spec-correct change that completes the selected task.
5. Run targeted validation first, then broader required validation from the task.
6. Update `TODO.md` by prefixing the selected task heading with `[DONE]` and recording completion details.
7. Update this file when key steps complete or if the plan changes.
8. Inspect git status and diffs, then commit all intended changes for this task.
9. Stop without starting the next task.

## Progress Log

- Initial plan recorded before task inspection.
- Selected first incomplete task from `TODO.md`: `P7-B2.4` for B-03/B-09/B-14 call ABI, TypeStore, and cast contract retirement.
- Required scope: retire 96 active UMB entries across B-03, B-09, and B-14; add/confirm verifier coverage; remove codegen fallbacks; update inventory, retired ledger, docs, fixtures, stale counts, and completion record.
- Latest commit checked: `2e24c7a3 [P7-B2.3] Retire aggregate pattern schema UMB rows`; no directly relevant unfinished issue was indicated.
- Initial worktree status checked: only `memory/claude_plan.md` was modified by this invocation.
- Active rows locked with `umb-audit list`: B-03 has 56, B-09 has 13, B-14 has 27; total target retirement is 96.
- Strategy docs confirm upstream contract path: MIR strict/materialized verifier for call ABI, TypeStore codegen equivalence, and cast/type-test metadata/result shape before LLVM.
- Implemented initial materialized MIR verifier coverage for direct/function-value/FunPtr call binding/return contracts and runtime type-test/cast metadata/result contracts.
- Added synthetic materialized MIR regression tests for direct call arity drift, non-Bool typecheck result drift, and `as?` Option payload drift.
- Validation checkpoint passed: `cargo test -p scoopc mir::materialize -- --nocapture`.
- Completed LLVM fallback retirement for B-03/B-09/B-14 and regenerated active inventory; B-03/B-09/B-14 now list 0 active entries.
- Updated retired ledger, bucket/category/strategy docs, fixture headers/index, stale count baseline, and `TODO.md` completion record.
- Final validation completed: audit tests, failure policy tests, B-03/B-09/B-14 fixture directories, `umb-audit diff/stats/list`, `cargo fmt`, and `cargo clippy --all-targets -- -D warnings` all passed.
