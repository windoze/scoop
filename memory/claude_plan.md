# Claude Execution Plan

## Current objective
Complete exactly the first incomplete task in TODO.md, then stop after documenting and committing the result.

## Selected task
P10-T04-a: 补齐 per-cone artifact cache handoff 边界.

## Step-by-step execution plan
1. Check the latest commit message and include any directly relevant unfinished issue in this task scope.
2. Inspect current `run_frontend`, build command orchestration, incremental cache code, and `scoopc_cone::ConeArtifact` IO APIs.
3. Design an explicit per-cone artifact cache handoff surface for frontend/build inputs: artifact directory, expected inputs fingerprint, direct dependency outputs fingerprints, and load/write decisions.
4. Implement dependency cone cache-hit loading from disk with schema/compiler/fingerprint validation and artifact frontend import injection, without parsing/indexing/typechecking dependency sources.
5. Implement dependency cone cache-miss publishing so the full frontend import payload needed by the next cache hit is written to disk.
6. Fix downstream lowering/codegen handoff so cached dependency cones are not represented by placeholder ASTs or reread sources; dependency codegen payload must come from artifact or be excluded in a semantically safe way.
7. Add regression tests proving a dependency cache hit ignores later broken dependency source, while a cache miss rereads source and reports the failure.
8. Run `cargo fmt`, `cargo clippy --all-targets -- -D warnings`, `cargo test --all --all-targets`, the required run-pass-cone fixture command, and `git diff --check`.
9. If validation reveals an unscheduled failure, fix it if in scope or add the minimum ordered TODO prerequisite before marking completion.
10. Mark P10-T04-a `[DONE]` in both `TODO.md` and `TODO-7.md`, update its completion record, commit all task-related changes, and stop.

## Progress log
- 2026-05-25: Identified the first incomplete task as P10-T04 in `TODO.md` / `TODO-7.md`.
- 2026-05-25: Recorded this execution plan before implementation changes.
- 2026-05-25: Inspected the latest commit (`[P10-T03R] Review per-cone frontend orchestration`) and current incremental/artifact/frontend paths.
- 2026-05-25: Found a concrete blocker for implementing P10-T04 as written: cache hits cannot yet load and consume dependency cone artifacts without relying on source/AST handoff, so fingerprint-only work would violate the task requirements.
- 2026-05-25: Added the minimum prerequisite `P10-T04-a` before P10-T04 in `TODO.md` and `TODO-7.md`; P10-T04 now explicitly depends on it.
- 2026-05-25: Re-read `TODO.md` and selected the first incomplete task as `P10-T04-a`; updated this plan before executing commands.
- 2026-05-25: Implemented explicit frontend artifact cache handoff, cache-hit artifact loading with inputs-fingerprint validation, cache-miss artifact publishing, and active source/AST pruning for cached dependency cones.
- 2026-05-25: Added regression coverage for dependency cache hit ignoring broken source and fingerprint mismatch re-reading broken source.
- 2026-05-25: Validation passed: `cargo fmt`, `cargo clippy --all-targets -- -D warnings`, `cargo test --all --all-targets`, and `cargo run -p scoop -- test --fixtures tests/fixtures/run_pass_cone`.
- 2026-05-25: Marked `P10-T04-a` done in `TODO.md` and `TODO-7.md`; preparing final diff check and commit.
