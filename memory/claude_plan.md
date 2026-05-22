# Claude Execution Plan

## Scope
- Follow `TODO.md` as the authoritative task list.
- Complete exactly the first incomplete task, then stop.
- Mark the task `[DONE]`, update its completion record, run required validation, and commit the resulting changes.
- If a concrete blocker prevents completion, update `TODO.md` with the minimum prerequisite task, commit, and stop.

## Initial Steps
1. Read `TODO.md` to identify the first task whose title is not prefixed with `[DONE]`.
2. Check the latest commit message only for unfinished work directly relevant to that task.
3. Read the selected task details, dependencies, and validation requirements.
4. Inspect only the code, fixtures, and docs needed for that task.
5. Implement the task without workarounds or spec deviations.
6. Run focused validation first, then any task-required broader validation.
7. Update `TODO.md` completion status and record.
8. Commit all intended changes with a task-specific message.

## Progress Log
- Plan file initialized before repository commands or code execution.
- Identified first incomplete task from `TODO.md`: `P7-T02` in `TODO-6.md`.
- Next steps: inspect `P7-T02` details, check the latest commit for directly relevant unfinished work, then implement only this task.
- Latest commit is `[P7-T01R] Review LLVM entry global migration`; it is directly adjacent but does not mention unfinished work that changes `P7-T02` ordering.
- `P7-T02` execution plan: remove LLVM reachability reads of HIR bodies/raw MIR/pass view and backend devirtualization; make reachability seeds/edges come from LIR/LIR facts; add or update focused tests/dumps; run the task validation commands; then mark `P7-T02` done in both TODO indexes and commit.
- Implemented first edit pass: replaced `llvm/reachability.rs` with a LIR-facts-only collector, switched `emit.rs` to consume reachable FQNs from `LirFacts`, and removed the LLVM call-lowering interface dispatch devirtualization fallback.
- Next step: run formatting and focused `llvm::reachability` tests, then fix any compile/test failures without reintroducing HIR/MIR reachability scans.
- Focused reachability unit tests pass with default LLVM feature; the required `--no-default-features llvm::reachability` command also completes but has no matching LLVM tests because the module is feature-gated.
- `effect_lowered` fixtures pass. Full `run-pass` now has broad failures, so the current LIR-only reachability is incomplete for production emission; next step is to inspect representative fixture diagnostics and repair the LIR-facts reachability coverage without restoring HIR/MIR scanning.
- Diagnosed the broad `run-pass` failures as over-enqueueing unpublished conservative candidate-set targets such as `scoop.core.Bool.toString` into legacy backend reachability.
- Adjusted LIR reachability so `KnownInstance` targets remain required, while `CandidateSet` and dispatch targets only enqueue callables actually published in `LirFacts`. Added a focused regression test and confirmed the representative global-init fixture now passes.
- Final validation completed: `cargo fmt`, required no-default reachability command, default `llvm::reachability` unit tests, `effect_lowered` fixtures, full `run-pass` with only the known 7 non-task baseline failures, `cargo clippy --all-targets -- -D warnings`, residual searches, and `git diff --check`.
- Updated `TODO.md` and `TODO-6.md` to mark `P7-T02` as `[DONE]` with completion notes. Next step is commit only; do not start `P7-T02R` in this invocation.
