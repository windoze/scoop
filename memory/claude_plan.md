# Execution Plan

I will not record private chain-of-thought here; this file captures the actionable plan and progress updates.

1. Read TODO.md first to identify the first task whose heading is not prefixed with [DONE].
2. Check the latest commit only for unfinished work directly relevant to that task.
3. Inspect the task requirements, dependencies, validation instructions, and relevant code paths.
4. Implement the task completely, or add the minimum prerequisite task in TODO.md if a concrete blocker prevents correct implementation.
5. Run formatting, linting, tests, and fixtures required by the task and the repository policy.
6. Update TODO.md completion status and record validation results when the task is complete.
7. Commit all relevant changes with a descriptive message and the required Co-authored-by trailer.
8. Stop after exactly one completed task or one committed prerequisite/blocker update.

## Progress - P3-T07R

- Selected first incomplete task: P3-T07R Review P3 residual cleanup result.
- Review scope: verify active source no longer contains crate::fixtures::, crate::fixture_cli::, or SCOOP_FIXTURE_* residuals; run required formatting/lint/test/fixture validation unless only bookkeeping changes remain.
- Initial residual searches found no matches for broad legacy symbols in existing source paths, but stdlib is absent in this checkout; re-running source scans over existing paths only.
- Residual-token review scans over active source paths passed with no matches; starting validation in required order.
- cargo fmt and cargo clippy --all-targets -- -D warnings passed; starting full Rust test suite and full fixture suite.
- Full validation command failed; inspecting captured output before deciding whether to fix or schedule a prerequisite.
- Full Rust tests passed, but full fixture validation failed on two runtime_gc STW cross-thread timeout fixtures. Treating this as a blocking fixture issue under policy and investigating targeted reproduction.
- Reproducing the two timeout fixtures individually with single-worker runner.
- Both timeout fixtures pass individually; reproducing the runtime_gc subset with default parallelism to isolate suite-level contention/flakiness.
- Targeted runs pass individually and for runtime_gc as a subset; the failing TIMEOUT applies to the whole scoop run command and is too tight under full-suite parallel load. Updating only the two STW cross-thread fixture timeout budgets, keeping them under 1 minute.
- Updated the two STW cross-thread fixture timeout budgets to 30000ms; rerunning validation from formatting onward.
- Revalidation passed after timeout budget fix; updating TODO completion record for P3-T07R.
- TODO.md marked P3-T07R [DONE] with validation record. Preparing final diff/status checks and commit.
