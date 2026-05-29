# Claude Execution Plan

## Scope

- Work on exactly the first incomplete task in `TODO.md`: `P2-T02`, "block pool 耗尽先 full GC 再增长".
- Treat `TODO.md` as the source of truth for task order, dependencies, validation, and completion records.
- Do not proceed to later tasks after completing or blocking the selected task.

## Step-By-Step Plan

1. Confirm the latest commit has no directly relevant unfinished prerequisite for `P2-T02`.
2. Read the P2 pacing design and current Immix block-pool implementation around `scoop_gc_immix_state_take_block`.
3. Inspect runtime tests that exercise Immix allocation, collection, block reuse, and pacing to identify the smallest correct regression coverage for full-GC-before-grow.
4. Implement the block-pool fallback so that when both `reusable_blocks` and `free_blocks` are empty, allocation requests first attempt a full collection and retry reusable/free blocks before growing with `posix_memalign`.
5. Preserve collection reentrancy safety so collection-internal helper allocation cannot recursively trigger full collection and fail or loop.
6. Add or update a tight-heap runtime regression proving reclaimable blocks are reused before growth while real exhaustion can still grow.
7. Run `cargo fmt`, then `cargo clippy --all-targets -- -D warnings`, then the required Rust test suite and fixture suite because runtime code changes affect compiled behavior.
8. Address any observed unscheduled failures before marking the task complete; if a concrete prerequisite blocks correct implementation, update `TODO.md` instead and stop.
9. Mark `P2-T02` as `[DONE]` in both `TODO.md` and `TODO-2.md`, including a completion record with implementation and validation details.
10. Commit the completed task and stop.

## Progress Log

- 2026-05-29 19:25 +08: Read `TODO.md` and `TODO-2.md`; selected first incomplete task `P2-T02`.
- 2026-05-29 19:25 +08: Recorded the task-specific execution plan before running repository commands.
- 2026-05-29 19:26 +08: Checked the latest commit (`P2-T01R`) and found uncommitted changes already in the `P2-T02` runtime/test area, so this invocation is auditing and completing that resumed work.
- 2026-05-29 19:27 +08: Reviewed the current block-pool fallback implementation and related runtime tests; next step is formatting, linting, and targeted validation before any additional edits.
- 2026-05-29 19:43 +08: Full validation exposed an STW hang in multithreaded Immix tests: registered Rust test threads blocked on barriers/joins without entering `InNative` while worker allocation can now trigger full GC. Updated the affected synchronization points to enter `InNative` with no roots while blocked.
- 2026-05-29 19:48 +08: Full validation then exposed stale P1 pacing assertions that treated `SCOOP_GC_PACING=off` or high stress as disabling all automatic collection. Updated them to the P2 contract: soft pacing can be disabled/bypassed, but block-pool exhaustion remains a hard full-GC trigger.
- 2026-05-29 20:00 +08: Full validation exposed the same native-blocking issue in the C multiframe stackmap keepalive helper while waiting for a worker to publish poll state; updated that wait/join path to enter `InNative` with no roots.
- 2026-05-29 20:10 +08: Full validation exposed the same pattern in `gc_stop_the_world`: the main registered test thread blocked on a channel/join while a worker allocation could trigger hard GC. Updated those waits to enter `InNative` with no roots.
- 2026-05-29 20:31 +08: Full validation exposed a same-process runtime-global race in `mutable_array_runtime` when its tests ran in parallel under harder auto-collection; serialized that test file with a mutex.
- 2026-05-29 20:42 +08: Final validation completed successfully: `cargo fmt`, `cargo clippy --all-targets -- -D warnings`, `cargo test --all --all-targets`, `python3 tools/spec_fixtures.py check`, and `python3 tools/run_fixtures.py`. `P2-T02` remains marked `[DONE]` in `TODO.md` and `TODO-2.md`.
