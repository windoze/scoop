# P3-T05 execution plan

I cannot record private chain-of-thought, but this file captures the actionable task interpretation, execution plan, and progress for this invocation.

## Current task

- Source of truth: `TODO.md`
- First incomplete task: `P3-T05`
- Task title: Delete the `tools/scoop_tools/` crate and remove the `"tools/scoop_tools"` workspace member from the root `Cargo.toml`.
- Scope: perform only P3-T05, then stop. Do not proceed to P3-T05R or P3-T06.
- Latest commit: `[P3-T04R] Review audit module deletion`; no unfinished issue in the latest commit message blocks P3-T05.
- Pre-existing unrelated worktree changes observed before this task: `.gitignore`, `CALLER_LOCATION.md`, and `RTTI_REFINE.md`. These must be preserved and excluded from the P3-T05 commit unless they become directly relevant.

## Execution plan

1. Inspect the root workspace manifest and the legacy `tools/scoop_tools/` tree.
2. Search for direct `tools/scoop_tools` and `scoop_tools` references to understand expected fallout. Keep already scheduled P3-T06 source-path cleanup separate unless it blocks the crate deletion itself.
3. Remove the `tools/scoop_tools/` crate and delete its workspace member entry from `Cargo.toml`.
4. Run validation in the required order where feasible: `cargo fmt`, `cargo clippy --all-targets -- -D warnings`, `cargo test --all --all-targets`, and relevant metadata checks. If validation fails only because of an exact later scheduled task, record that precisely in `TODO.md`.
5. Update `TODO.md` by prefixing `P3-T05` with `[DONE]` and appending a completion record with changed files and validation outcome.
6. Commit the P3-T05 changes with the required co-author trailer.
7. Stop after P3-T05.

## Progress

- Selected first incomplete task: `P3-T05`.
- Removed `"tools/scoop_tools"` from the workspace member list in `Cargo.toml`.
- Deleted the tracked files under `tools/scoop_tools/`; empty directories, if any, are untracked filesystem artifacts only.
- Confirmed direct source references outside the deleted crate are limited to `tools/dependency_gate.py` deny-list text and the already scheduled `P3-T06` `p8_docs_cleanup.rs` path cleanup.
- Validation so far: `cargo fmt` passed; `cargo clippy --all-targets -- -D warnings` passed; `cargo test --all --all-targets` failed only in `crates/scoop/tests/p8_docs_cleanup.rs` because it still reads `tools/scoop_tools/src/fixtures_matrix.rs`, which is the exact P3-T06 scheduled cleanup; `python3 tools/run_fixtures.py` passed with 1533 checks; `cargo metadata --format-version 1 --no-deps` confirms `scoop_tools` is absent.
- Updated `TODO.md` to mark `P3-T05` as `[DONE]` and appended the completion record.
