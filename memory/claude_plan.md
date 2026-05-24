# Execution Plan

## Selected task
- First incomplete task: `P10-T03` in `TODO-7.md` — refactor `run_frontend` to run in source-cone DAG/topological order and inject upstream cone artifacts instead of making downstream cones parse upstream source.
- Latest commit: `[P10-T02R] Review per-cone artifact schema`; no explicit unfinished issue was found in the commit subject/body that changes this task selection.

## Plan
1. Inspect current `run_frontend`, pipeline stage APIs, source cone graph APIs, existing ScoopIR dependency injection, and the new `ConeArtifact` IO schema.
2. Identify whether current stage outputs contain enough data to construct per-cone `ConeArtifact` values and whether HIR `Index` / `TypeEnv` can be reconstructed from facts alone.
3. If the required fact-to-frontend injection path is implementable with existing facts, add `scoopc_cone::import_upstream_artifacts`, refactor `run_frontend` to iterate compilation units, and add a regression proving the downstream cone does not parse upstream source.
4. If a concrete missing feature blocks spec-correct implementation, add the minimum prerequisite task before `P10-T03` in `TODO.md` / `TODO-7.md`, document the blocker, commit that scheduling change, and stop without marking `P10-T03` done.
5. Run validation in order: `cargo fmt`, `cargo clippy --all-targets -- -D warnings`, then the task-specific build/tests/fixtures, followed by `git diff --check`.
6. On success, update `TODO.md` and `TODO-7.md` completion records, commit all task-related changes with the required co-author trailer, then stop.

## Current status
- Task selected initially: `P10-T03`.
- Blocker found: current `ConeArtifact` does not carry the frontend import payload needed to inject upstream cones into `Index` / `TypeEnv` without reading upstream source. HIR facts also do not contain enough public declaration/typealias/visibility information to reconstruct that surface directly.
- Action taken: inserted prerequisite `P10-T03-a` before `P10-T03` in `TODO.md` and `TODO-7.md`; `P10-T03` now explicitly depends on `P10-T03-a`.
- Next step: validate the TODO scheduling change, commit it, and stop so the next invocation completes the new first incomplete task.
