# P3-T06 execution plan

I cannot record private chain-of-thought, but this file captures the actionable task interpretation, execution plan, and progress for this invocation.

## Current task

- Source of truth: `TODO.md`.
- First incomplete task: `P3-T06`.
- Task title: clean `crates/scoop/tests/p8_docs_cleanup.rs` so it no longer references the deleted `tools/scoop_tools/src/fixtures_matrix.rs` source path.
- Scope: complete only P3-T06, update its completion record, commit, and stop before P3-T06R.
- Latest commit: `[P3-T05R] Review scoop_tools deletion`; no unfinished issue is mentioned, and it directly leaves P3-T06 as the next cleanup.
- Pre-existing unrelated worktree changes observed before this task: `.gitignore`, `CALLER_LOCATION.md`, and `RTTI_REFINE.md`. Preserve them and keep them out of this task commit unless they become directly relevant.

## Execution plan

1. Inspect the failing docs-cleanup test and the replacement Python fixture matrix script to determine whether the old source-path guard should be updated or removed.
2. Make the smallest test update that preserves the test's actual responsibility while removing the deleted `tools/scoop_tools` path reference.
3. Run the focused test for `p8_docs_cleanup`.
4. Run required formatting and linting: `cargo fmt`, then `cargo clippy --all-targets -- -D warnings`.
5. Run broader validation after lint passes: `cargo test --all --all-targets`; run the fixture suite only if compiled-output-affecting changes require it.
6. Update `TODO.md` by prefixing `P3-T06` with `[DONE]` and append a completion record with validation results.
7. Commit only the P3-T06 changes and this progress file, excluding unrelated local files.
8. Stop after P3-T06.

## Progress

- Selected first incomplete task: `P3-T06`.
- Confirmed the stale reference is in `crates/scoop/tests/p8_docs_cleanup.rs`, where the async/task surface guard still reads `tools/scoop_tools/src/fixtures_matrix.rs`.
- Updated that guard to read the replacement `tools/fixtures_matrix.py`, preserving the async/task surface regression check without referencing the deleted crate path.
- Validation passed: `cargo fmt`; `cargo clippy --all-targets -- -D warnings`; `cargo test -p scoop --test p8_docs_cleanup`.
- Full validation passed: `cargo test --all --all-targets`; `python3 tools/run_fixtures.py` (`fixtures: ok (1533)`).
