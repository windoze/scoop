## Execution Plan

This file records the actionable plan and progress for the current invocation. It intentionally contains a concise, reviewable plan rather than private internal reasoning.

### Initial Plan

1. Read `TODO.md` and identify the first task whose heading is not prefixed with `[DONE]`.
2. Check the latest commit only for directly relevant unfinished notes after the current task is identified.
3. Read the selected task details, dependencies, and validation requirements.
4. Inspect the relevant source, fixtures, and tests for that task only.
5. Implement the task as specified, without narrowing scope or adding workarounds.
6. Run targeted validation first, then broader required validation from the task record.
7. If an unscheduled blocking issue or failing test is discovered, either fix it or insert the minimum prerequisite task in `TODO.md`, then stop after committing.
8. When the task is complete, update `TODO.md` by prefixing the task title with `[DONE]` and filling the completion record.
9. Update this file at key milestones or if the plan changes.
10. Inspect git status/diff/log, commit all intended changes with a task-specific message, and stop without starting the next task.

### Progress

- Plan file initialized before running repository commands.
- Selected first incomplete task: `P7-T02-a` in `TODO-6.md`, which requires fixing the seven recorded run-pass fixture baseline failures.
- Latest commit is `Update plan`; it directly added/updated the current baseline-failure task and does not contain a separate unfinished implementation note.
- Next step: reproduce the listed fixture failures, then fix the root cause without adding LLVM HIR/raw MIR fallback or backend codegen special cases.
- Reproduced the baseline failure as an unresolved generic `scoop.core.println` direct call during materialization.
- Root cause found in MIR materializer call-site binding lookup: exact outer call bindings were being invalidated by nested overlapping generic call bindings that could be remapped to the same callee shape.
- Implemented exact call-site binding preference before overlapping fallback in `lookup_site_instance_binding_for_callee_in` and added a nested-array println regression test.
- All seven previously listed individual run-pass fixtures now pass; next step is required formatting, full run-pass validation, clippy, and diff checks.
- Required validation completed: `cargo fmt`; `cargo test -p scoopc materialize_request_root_rewrites_nested_array_println_call_sites -- --nocapture`; seven individual baseline fixtures; full `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass` with 421/421 passing; `cargo clippy --all-targets -- -D warnings`; `git diff --check`.
- Updated `TODO.md` and `TODO-6.md` to mark `P7-T02-a` as `[DONE]` with completion notes. Next step is final git inspection and committing the task changes.
