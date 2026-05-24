# Claude Execution Plan

## Current objective
Complete exactly the first incomplete task in TODO.md, then stop after documenting and committing the result.

## Selected task
P10-T04: per-cone fingerprint cache + 增量 build.

## Step-by-step execution plan
1. Check the latest commit message and include any directly relevant unfinished issue in this task scope.
2. Inspect the current build incremental fingerprint code and the existing `scoopc_cone` artifact read/write APIs from P10-T02/P10-T03.
3. Design the narrow per-cone cache surface needed for P10-T04: `inputs.fingerprint` from the current cone inputs plus direct dependency `outputs.fingerprint`, and `outputs.fingerprint` from artifact files.
4. Implement fingerprint calculation and read/write helpers in `scoopc_cone`, preserving explicit errors for stale or malformed artifacts.
5. Replace or route `crates/scoop/src/commands/build/incremental.rs` away from a single whole-project SHA-256 toward the per-cone fingerprint chain.
6. Wire cache hit/miss behavior so unchanged upstream cones load artifacts from disk, changed cones rebuild and rewrite artifacts, and unchanged final user cone plus unchanged entrypoint can skip the build.
7. Add tests for the three required scenarios: user source change rebuilds only user cone; library source change rebuilds library plus user; toolchain change rebuilds all cones.
8. Run `cargo fmt`, `cargo clippy --all-targets -- -D warnings`, required Rust tests, required cone fixture tests, manual fresh-build vs cache-hit timing, and `git diff --check`.
9. If validation reveals an unscheduled failure, fix it if in scope or add the minimum ordered TODO prerequisite/follow-up before marking completion.
10. Mark P10-T04 `[DONE]` in both `TODO.md` and `TODO-7.md`, update its completion record with validation and timing data, commit all task-related changes, and stop.

## Progress log
- 2026-05-25: Identified the first incomplete task as P10-T04 in `TODO.md` / `TODO-7.md`.
- 2026-05-25: Recorded this execution plan before implementation changes.
- 2026-05-25: Inspected the latest commit (`[P10-T03R] Review per-cone frontend orchestration`) and current incremental/artifact/frontend paths.
- 2026-05-25: Found a concrete blocker for implementing P10-T04 as written: cache hits cannot yet load and consume dependency cone artifacts without relying on source/AST handoff, so fingerprint-only work would violate the task requirements.
- 2026-05-25: Added the minimum prerequisite `P10-T04-a` before P10-T04 in `TODO.md` and `TODO-7.md`; P10-T04 now explicitly depends on it.
