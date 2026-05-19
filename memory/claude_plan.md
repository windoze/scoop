# Execution Plan

## Scope

- Follow `TODO.md` as the authoritative task list.
- Identify the first task whose heading is not prefixed with `[DONE]`.
- Complete exactly that one task, then stop after committing.

## Steps

1. Read `TODO.md` and identify the first incomplete task.
2. Check recent Git context only as needed for that task, including whether the latest commit mentions a directly relevant unfinished issue.
3. Inspect the relevant source, fixtures, tests, and documentation for the selected task.
4. Implement the task as specified without workarounds or scope narrowing.
5. If a concrete blocker prevents correct implementation, update `TODO.md` with the minimum required prerequisite task, leave the current task incomplete, commit that bookkeeping, and stop.
6. Run the task-specific validation required by `TODO.md`, plus any directly relevant tests.
7. Fix any regressions or warnings introduced by the work.
8. Mark the completed task heading in `TODO.md` with `[DONE]` and update its completion record.
9. Update `PLAN.md` only if the phase-level plan or dependencies actually changed.
10. Review the Git diff, commit all intended changes with a task-specific message, and stop.

## Progress Log

- Plan initialized before reading project task details.
- Read `TODO.md`; selected first incomplete task `P7-C4: B-12 Closure / lambda / capture 实现`.
- Next step: inspect latest Git context and B-12-specific audit/code/fixture surfaces only.
- Latest commit is `P7-C3`; no explicit unfinished issue found that preempts P7-C4.
- Found existing uncommitted P7-C4/B-12 changes in code, audit data, fixtures, and this progress file; continuing them as the current task state.
- `umb-audit list --bucket B-12` currently reports 0 active entries; running validation before deciding whether further edits are needed.
- Initial validation found B-12 retired ledger `old_line` drift and a B-12 fixture expected-location mismatch.
- Fixed B-12 retired ledger line numbers against `audit/UMB_inventory_initial.csv`, synchronized affected B-12 category rows, and updated the negative fixture expected location.
- Re-ran B-12 fixture set successfully; `umb-audit diff` then reported only B-10 line drift caused by P7-C4 edits in `mir_body/call.rs`.
- Synchronized the five active B-10 line numbers in `audit/UMB_inventory.csv` and `audit/UMB_categories/B-10.md`.
- Audit tests then found B-12 category active class counts and legacy-gap provenance drift; updated B-12 class distribution to active zero and preserved `PIPELINE_GAPS §3.11` in the retired ledger.
- Updated `pipeline_user_visible_failure_policy` B-12 baselines: stale `UnsupportedMainBody` total is now 97 and internal sentinel hits include the B-12 invariant replacements.
- Failure policy tests now pass.
- Full `tests/fixtures/run-pass/` initially failed broadly because P7-C4 materialized metadata exposed a dispatch metadata validation path that assumed every metadata member function published receiver ABI params.
- Kept non-generic metadata in materialized MIR because composite closure capture transport needs it, and changed dispatch validation to fall back to result transport checks when metadata does not publish a receiver parameter.
- Representative fixtures `bool_to_string_print_basic.scoop` and `closure_env_composite_capture_basic.scoop` now both pass.
- `mir::materialize` tests then exposed unresolved nested generic metadata in materialized MIR output; added recursive filtering so only non-generic metadata subtrees are preserved.
- `cargo test -p scoopc mir::materialize -- --nocapture` now passes.
- Ran `cargo fmt` after code and audit baseline edits.
- Final full `cargo run -p scoop -- test tests/fixtures/run-pass/` remains 307/416 passing, 109 failing; sampled failures reproduce on the clean `HEAD` worktree and are not P7-C4-specific.
- P7-C4-specific closure/capture run-pass fixtures are passing; the full run-pass command remains blocked by unrelated historical/future-task failures.
- `cargo clippy --all-targets -- -D warnings` passes.
- Removed the temporary clean-HEAD comparison worktree.
- Marked `P7-C4` as `[DONE]` in `TODO.md` and recorded the B-12 completion details, validation results, and the remaining historical `run-pass` failures.
- `git diff --check` is clean after removing trailing Markdown whitespace in updated B-12 docs.
