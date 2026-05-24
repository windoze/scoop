# Autonomous execution plan

## Selected task
**P10-T04: per-cone fingerprint cache + 增量 build** (TODO-7.md)

## Step-by-step execution plan
1. Read TODO.md and identify the first incomplete task ([TODO] heading without [DONE]).
2. Inspect existing uncommitted progress on P10-T04 (incremental.rs, frontend.rs, cone.rs, artifact.rs) and the prior commit `[P10-T04-a]` for handoff context.
3. Audit per-cone fingerprint chain semantics: inputs.fingerprint must depend on each direct dependency cone's outputs.fingerprint and the artifact-level outputs.fingerprint must be a stable function of artifact files.
4. Identify the divergence bug: cache-miss deps use the placeholder `inputs_fingerprint` for `dep_outputs`, but on the next run the real `outputs.fingerprint` from disk replaces it, making the consumer-cone fingerprint unstable across runs and breaking the user-cone short-circuit cache hit.
5. Fix the divergence by recomputing the build fingerprint AFTER the build runs (when all cache-miss dep artifacts have been written and their real outputs.fingerprint is on disk) and storing that in `build.json`.
6. Add a unit test that proves the round-trip stability: fingerprint computed on a fresh artifact set matches the fingerprint computed after a no-op rerun.
7. Run formatting, clippy with warnings denied, and the full Rust test suite plus the cone-pass fixture suite, fixing failures immediately.
8. Capture fresh-build vs cache-hit timing data using `scoop build` against a representative cone fixture.
9. Update TODO.md and TODO-7.md to mark P10-T04 [DONE] with full completion record (timing data included).
10. Commit all task-related changes (PLUGIN_ABI.md and run_agent.sh adjustments included) with a `[P10-T04]` tagged message and stop.

## Progress log
- 2026-05-25: Identified P10-T04 as first incomplete task; previous invocation already implemented the per-cone fingerprint chain in incremental.rs, an outputs-fingerprint computation API in artifact.rs, and the frontend cache write hook in frontend.rs.
- 2026-05-25: Reviewed the current implementation — it computes per-cone inputs/outputs fingerprints and propagates dependency outputs into the consumer cone's input fingerprint, but has a bug: cache-miss deps use a placeholder dep_outputs (= dep_inputs_fingerprint) when computing the consumer fingerprint, while future runs use the real on-disk outputs_fingerprint. This breaks user-cone cache hits on subsequent runs.
- 2026-05-25: Plan adjusted to fix the divergence by recomputing the fingerprint after the build (when all dep artifacts are on disk) and to add a regression test that asserts the round-trip stability before marking P10-T04 done.
