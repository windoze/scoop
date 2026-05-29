# Claude Execution Plan

## Scope

- Work on exactly the first incomplete task in `TODO.md`: `P2-T03`, "接入 hard cap 与 OOM 返回".
- Treat `TODO.md` and `TODO-2.md` as the source of truth for ordering, validation, and completion records.
- Stop after committing `P2-T03`; do not start `P2-T03R`.

## Step-By-Step Plan

1. Confirm the latest commit and current worktree state for directly relevant unfinished work.
2. Read the `P2-T03` task body plus the related `GC_PACING.md` and `PLAN.md` hard-cap requirements.
3. Inspect the Immix block-pool growth path, `scoop_alloc`, env parsing, and existing block-pool tests.
4. Implement `SCOOP_GC_MAX_HEAP_BYTES` as an env-backed heap cap with default `0` meaning no cap.
5. Apply the cap only after the required GC retry has had a chance to reclaim memory, so reusable blocks still allow allocation near the cap.
6. Ensure `scoop_alloc` returns `NULL` cleanly when the cap prevents further growth, including direct large-object growth.
7. Add focused Immix regression coverage for near-cap reuse success and true over-cap `NULL` return.
8. Run formatting, linting, focused runtime tests, the full Rust test suite, spec fixture check, and fixture suite.
9. Mark `P2-T03` as `[DONE]` in both `TODO.md` and `TODO-2.md` with the validation record.
10. Commit all task-related changes with the required co-author trailer, then stop.

## Progress Log

- 2026-05-29 22:43 +08: Identified `P2-T03` as the first incomplete task.
- 2026-05-29 22:55 +08: Reviewed the P2-T03 requirements and the existing Immix allocation/block-pool implementation.
- 2026-05-29 23:05 +08: Implemented env parsing, heap cap storage, Immix growth checks, large-object cap handling, and focused hard-cap regression coverage.
- 2026-05-29 23:20 +08: Validation passed: `cargo fmt`, focused hard-cap and block-pool tests, `cargo clippy --all-targets -- -D warnings`, `cargo test --all --all-targets`, `python3 tools/spec_fixtures.py check`, and `python3 tools/run_fixtures.py`.
- 2026-05-29 23:35 +08: Refactored reserved-byte debug accounting to reuse the hard-cap helper and re-ran the same validation successfully.
- 2026-05-29 23:43 +08: Confirmed alternate runtime backends compile and pass their tests with `gc-baseline`, `gc-minimal`, and `gc-hosted` feature selections.
