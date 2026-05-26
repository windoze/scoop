# P2-T02 Execution Plan

## Current task

First incomplete task in `TODO.md`: `P2-T02` — switch internal invocation chains in:

- `tools/run_fixture_scan.sh`
- `tools/run_run_pass_gc_scan.sh`
- `tools/gc_microbench.sh`

The latest commit is `[P2-T01R] Review CI fixture command switch`; it is directly relevant only as the preceding completed task. The untracked `CALLER_LOCATION.md` and `RTTI_REFINE.md` files appear unrelated and will not be modified unless commit policy requires preserving resumed task state.

## Execution plan

1. Inspect the three shell scripts and identify old fixture-runner calls, especially `scoop test --fixtures`.
2. Replace old fixture execution with `python3 tools/run_fixtures.py` while preserving script behavior, timeout handling, output directories, logs, GC stress environment variables, and target selection.
3. Confirm `tools/gc_microbench.sh` does not call old fixture or `scoop_tools` entrypoints; leave it unchanged if no switch is required.
4. Search the edited scripts for old entrypoints (`scoop test`, `test --fixtures`, `scoop_tools`, `test-fixtures`, and `cargo run -p scoop -- test`).
5. Validate the scripts with small targeted fixture subsets and run the repository-required formatting, linting, Rust tests, and fixture suite if needed.
6. Update `TODO.md` by prefixing `P2-T02` with `[DONE]` and appending a completion record with validation results.
7. Review the final diff and commit task changes with a `[P2-T02]` commit message and the required co-author trailer.

## Progress

- Identified `P2-T02` as the first incomplete task.
- Inspected the target scripts; `run_fixture_scan.sh` and `run_run_pass_gc_scan.sh` still invoke `scoop test --fixtures`; `gc_microbench.sh` only runs the runtime microbench binary and has no old fixture/tool entrypoint.
- Updated `run_fixture_scan.sh` and `run_run_pass_gc_scan.sh` to call `python3 tools/run_fixtures.py` with explicit `SCOOP_BIN` and `SCOOPC_BIN`; preserved per-unit timeout, logs, summaries, and GC stress/verification behavior.
- Confirmed the three P2-T02 scripts no longer contain `scoop test`, `test --fixtures`, `test-fixtures`, `cargo run -p scoop -- test`, or `scoop_tools`.
- Added non-zero exits to both scan scripts when any fixture fails or times out, so the shell wrappers do not report success-shaped failures.
- Validation found `tools/run_run_pass_gc_scan.sh` still reports 5 GC verify-root failures. Running the same fixtures through the old `scoop test --fixtures` path also fails, so this is not introduced by the Python invocation switch. The exact failures are not scheduled in `TODO.md`; a prerequisite task must be inserted before P2-T02 can be completed.
