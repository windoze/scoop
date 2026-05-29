# Claude Execution Plan

## Scope

- Work on exactly the first incomplete task in `TODO.md`: `P2-T02R`, "Review block pool 回退".
- Treat `TODO.md` as the source of truth for task order, dependencies, validation, and completion records.
- Do not proceed to `P2-T03` after completing or blocking this review task.

## Step-By-Step Plan

1. Confirm the latest commit only for unfinished work directly relevant to `P2-T02R`.
2. Read the `P2-T02R` task body in `TODO-2.md` and the P2-T02 completion record.
3. Inspect the P2-T02 implementation surfaces: `scoop_gc_immix_state_take_block`, the TLS block-cache refill path, full/minor collection reentrancy guards, and block-pool regression tests.
4. Verify that block-pool exhaustion triggers full GC before growth, does not recursively collect from collector-owned allocation paths, and still grows when full GC cannot reclaim a block.
5. Add the minimum missing regression coverage if the review exposes an uncovered required path.
6. Run formatting, linting, the focused block-pool regression, the full Rust test suite, and the fixture suites.
7. Mark `P2-T02R` as `[DONE]` in both `TODO.md` and `TODO-2.md`, with a completion record summarizing the review and validation.
8. Commit all task-related changes with the required co-author trailer, then stop.

## Progress Log

- 2026-05-29 21:45 +08: Identified `P2-T02R` as the first incomplete task.
- 2026-05-29 21:49 +08: Reviewed the P2-T02 block-pool fallback implementation and existing regression coverage.
- 2026-05-29 22:04 +08: Added focused coverage for the required path where full GC cannot reclaim old-space blocks and allocation must still grow after the GC retry.
- 2026-05-29 22:09 +08: Validation completed successfully: `cargo fmt`, focused `gc_immix_block_pool`, `cargo clippy --all-targets -- -D warnings`, `cargo test --all --all-targets`, `python3 tools/spec_fixtures.py check`, and `python3 tools/run_fixtures.py`.
- 2026-05-29 22:10 +08: Marked `P2-T02R` complete in `TODO.md` and `TODO-2.md`.
