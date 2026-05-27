# Execution Plan

I will follow `TODO.md` as the authoritative task list, complete exactly the first task whose title is not prefixed with `[DONE]`, update this progress file and `TODO.md`, commit the result, and stop.

## Selected task

P4-T07R: Review P4 全包完成度（`TEST_INFRA_CLEANUP.md` §8 验收清单逐项核对）.

## Steps

1. Confirm P4-T07R is the first incomplete task and gather the `TEST_INFRA_CLEANUP.md` §8 acceptance checklist.
2. Verify the P4 checklist directly:
   - Required deleted directories/files are absent.
   - `cargo metadata` and workspace files no longer include the removed Rust tool crate.
   - Removed CLI entries fail as unknown/removed commands.
   - New Python fixture runner, audit scripts, matrix scripts, safepoint baseline, dependency gate, and shell scan helpers run successfully.
   - CI/documentation/token residual searches satisfy the §7 step 5 whitelist rules.
   - `cargo build`, formatting, clippy, and full tests pass.
3. If validation exposes an unscheduled failure, fix it if it is in scope or add the minimum prerequisite task before P4-T07R in `TODO.md`, then commit and stop.
4. If validation passes, mark P4-T07R `[DONE]`, append its completion record with the concrete commands/results, update this file, commit all changes for this task, and stop.

## Progress

- Initialized P4-T07R progress tracking.
- Confirmed latest commit completed P4-T06 and the first incomplete task is P4-T07R.
- Loaded `TEST_INFRA_CLEANUP.md` §8 and TODO completion records for P4 validation scope.
- Completed the formatting and clippy gate: `cargo fmt` and `cargo clippy --all-targets -- -D warnings` passed.
- Structural acceptance checks exposed an empty untracked `crates/scoopc/src/fixtures/` directory left behind after P3; removed that empty directory so the §8 deleted-path checklist can pass.
- Found and removed two additional empty untracked deleted-path directories for the old audit module and legacy Rust tool crate, including empty nested legacy-tool subdirectories.
- Deleted-path, `cargo metadata`, `cargo build -p scoop -p scoopc --features llvm`, and removed-command checks now pass.
- Standalone Python tooling passed: `py_compile`, `spec_fixtures.py check`, `fixtures_matrix.py check/stdlib`, `safepoint_baseline.py`, `dependency_gate.py`, and all three `audit_*.py` scripts.
- Residual-token searches passed after excluding the current plan/task source files and archived history.
- Full Rust test suite passed: `cargo test --all --all-targets`.
- Full fixture suite passed: `python3 tools/run_fixtures.py` completed 1504 targets / 1533 checks.
- Shell scan helpers passed: `tools/run_fixture_scan.sh --out-dir target/fixture-scan/p4-t07r` completed total=1504/pass=1504/fail=0/timeout=0, and `tools/run_run_pass_gc_scan.sh target/run-pass-gc-scan/p4-t07r` completed total=412/pass=412/fail=0/timeout=0.
- Marked P4-T07R `[DONE]` in `TODO.md` and appended the completion record with the full acceptance evidence.
- Confirmed no incomplete task rows remain in `TODO.md`; preparing the final task commit and `v0.1.0` release tag.
