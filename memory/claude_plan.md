# P3-T07 execution plan

I cannot record private chain-of-thought, but this file captures the actionable task interpretation, execution plan, and progress for this invocation.

## Current task

- Source of truth: `TODO.md`.
- First incomplete task: `P3-T07`.
- Task title: clean dead code, orphan constants, and `SCOOP_FIXTURE_*` env name constants left after P3-T01/P3-T04.
- Scope: complete only P3-T07, update its completion record, commit, and stop before P3-T07R.

## Execution plan

1. Check the latest commit and worktree status so any directly relevant unfinished issue is included and unrelated local changes are preserved.
2. Inspect the P3-T07 cleanup targets from `TEST_INFRA_CLEANUP.md` section 2.6 and search the non-archived source tree for fixture-only hooks, dead constants, and `SCOOP_FIXTURE_*` names.
3. Remove remaining runtime/compiler fixture-only code paths and tests that exist solely to guard those old hooks, without touching external python runner fixture concepts.
4. Run `cargo fmt`, `cargo clippy --all-targets -- -D warnings`, targeted relevant tests, then full `cargo test --all --all-targets` and `python3 tools/run_fixtures.py` unless the final diff is documentation-only.
5. Update `TODO.md` by prefixing `P3-T07` with `[DONE]` and appending a completion record with the cleanup findings and validation commands.
6. Commit only files relevant to P3-T07 and this progress file, preserving unrelated local changes.
7. Stop after P3-T07.

## Progress

- Selected first incomplete task: `P3-T07`.
- Refreshed this plan before making P3-T07 implementation changes.
- Checked latest commit: `[P3-T06R] Review docs cleanup fixture matrix path`; it does not mention an unfinished issue.
- Observed unrelated pre-existing worktree changes: `.gitignore`, `CALLER_LOCATION.md`, and `RTTI_REFINE.md`; they remain outside this task scope.
- Searched the P3-T07 cleanup symbols from `TEST_INFRA_CLEANUP.md` section 2.6. The old compiler fixture hook symbols were already absent, but migrated scripts/docs still had stale references to deleted Rust audit/tool names.
- Removed stale `scoop_tools`, `pipeline_gap_audit.rs`, `spec_coverage.rs`, `pipeline_user_visible_failure_policy.rs`, `safepoint-baseline`, and `scoop test` references from the active script/doc/comment surfaces found during cleanup.
- Validation passed: residual-token `rg` checks; Python compile/audit scripts; `python3 tools/safepoint_baseline.py`; `cargo fmt`; `cargo clippy --all-targets -- -D warnings`; `cargo test --all --all-targets`; `python3 tools/run_fixtures.py` (1533 checks).
- Updated `TODO.md` to mark `P3-T07` as `[DONE]` and added the completion record.
