# Execution Plan

I will follow TODO.md as the authoritative task list, complete exactly the first task whose title is not prefixed with [DONE], update TODO.md and this progress file, commit the result, and stop without starting the next task.

## Selected task

P4-T06: `tools/run_fixture_scan.sh` / `tools/run_run_pass_gc_scan.sh` 用新入口跑通.

## Steps

1. Confirm the selected task and relevant validation requirements from TODO.md, PLAN.md, and TEST_INFRA_CLEANUP.md.
2. Inspect the two shell scripts to confirm they already invoke `python3 tools/run_fixtures.py` rather than removed fixture-runner entries.
3. Run formatting and linting first as required: `cargo fmt`, then `cargo clippy --all-targets -- -D warnings`.
4. Run the P4-T06 validation scripts end-to-end:
   - `tools/run_fixture_scan.sh --out-dir target/fixture-scan/p4-t06`
   - `tools/run_run_pass_gc_scan.sh target/run-pass-gc-scan/p4-t06`
5. If validation exposes failures, fix the root cause or add the minimum prerequisite task to TODO.md if a concrete blocker prevents completion.
6. If validation passes, mark P4-T06 `[DONE]` in TODO.md, append its completion record with the commands and summaries, commit the changes, and stop.

## Progress

- Initialized progress tracking.
- Identified P4-T06 as the first incomplete task.
- Confirmed the scripts' command shape uses `tools/run_fixtures.py`.
- Completed formatting and clippy gate: `cargo fmt` and `cargo clippy --all-targets -- -D warnings` passed.
- Full `tools/run_fixture_scan.sh` initially failed because it recursively scheduled 46 `.sysroot` overlay source files as standalone fixtures. This violated the documented discovery contract that overlay directories are skipped.
- Updated `tools/run_fixture_scan.sh` discovery to skip directories whose names end in `.sysroot` during recursive file collection and direct case-directory enumeration.
- Reran validation successfully: `tools/run_fixture_scan.sh --out-dir target/fixture-scan/p4-t06-fixed` passed with total=1504, pass=1504, fail=0, timeout=0 and no `.sysroot/` targets in `all.txt`.
- Ran `tools/run_run_pass_gc_scan.sh target/run-pass-gc-scan/p4-t06`; it passed with total=412, pass=412, fail=0, timeout=0.
- Reran `cargo fmt` and `cargo clippy --all-targets -- -D warnings` after the shell-script edit; both passed.
- Marked P4-T06 `[DONE]` in TODO.md and added the completion record.
