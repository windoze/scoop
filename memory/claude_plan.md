# Claude Plan

## Working Approach

I will keep this file updated as I make progress or if the plan changes.

## Initial Plan

1. Read `TODO.md` as the index and identify which detailed task file to inspect first.
2. Read the referenced `TODO-Px.md` files in order and locate the first task whose title is not prefixed with `[DONE]`.
3. Check the latest commit message to see whether it mentions unfinished work directly relevant to that task.
4. Inspect the code, tests, and any related documentation needed to implement that exact task without changing scope.
5. Implement the task completely, keeping the change as small and correct as possible.
6. Run the relevant verification commands, including targeted tests first and broader checks if the task requires them.
7. If I hit a concrete blocker that prevents a spec-correct implementation, add the minimum prerequisite task in the correct `TODO-Px.md` file, sync `TODO.md`, and stop.
8. If I complete the task, mark it `[DONE]` in the detailed task file, sync `TODO.md` if needed, and update completion notes.
9. Commit all changes for this invocation with a task-specific commit message, then stop.

## Current Task

- Active task: `P6-T02qa`
- Title: 发布 escaped continuation aggregate/member write-read provenance contract，禁止 `P6-T02q` 在 late-lowered/ABI materialization 现场从 unresolved assign-lhs TODO 或 source shape 猜 `cell.k` 回读 continuation 的底层 surface route。

## Current Understanding

- `TODO.md` shows `P6-T02qa` as the first incomplete indexed task.
- `TODO-P6-part2.md` confirms `P6-T02qa` is a prerequisite inserted ahead of `P6-T02q` because aggregate/member assignment provenance is currently missing.
- The latest commit subject is `[P6-T02qa] Track continuation write-read provenance prerequisite`, so this invocation should implement that prerequisite rather than move on.

## Immediate Execution Steps

1. Inspect the current worktree and the implementation areas mentioned by the task (`mir/lower`, `effect_lowered`, LLVM refactor query/layout code).
2. Reproduce the current missing contract using the referenced fixture and dumps if needed.
3. Implement a compiler-owned published provenance path for continuation-bearing aggregate/member write/read.
4. Thread the published provenance into the late-lowered / LLVM refactor handoff, with fail-fast behavior for missing or ambiguous routes.
5. Add targeted tests for both success and failure cases.
6. Run the required verification commands, then update the TODO/completion record and commit.

## Refined Implementation Plan

1. Extend canonical MIR with an explicit compiler-owned statement for continuation-bearing member writes, so `cell.k = Some(k)` / `cell.k = none_k` no longer collapse into `assign lhs lowering pending`.
2. Teach `lower_assign_stmt` to emit that statement for member writes while still lowering the RHS into a local, and publish the continuation route shape carried by the RHS (for example `Some(k)` -> variant path + source local, or a clear write with no continuation route).
3. Update MIR-side helpers (rewriting/materialization, validation, escape/summary bookkeeping, tests) so the new statement remains a stable published contract rather than an opaque TODO.
4. Extend `LateLoweredResumeBoundaryOperandContract` with the authoritative underlying continuation-route provenance for the continuation operand local used by a resume boundary.
5. In late lowering, resolve that provenance from canonical MIR by combining:
   - explicit member-write publications,
   - member-read / pattern-extract chains,
   - published handle continuation binder routes.
   The resolver should fail fast on missing, mismatched, or ambiguous routes.
6. Surface the new provenance in stable dumps and ensure the LLVM ABI/query layer validates and preserves the contract.
7. Add targeted tests for:
   - MIR write contract publication on the blocker fixture;
   - late-lowered resume provenance on `effect_multi_escape_indirect_direct_while.scoop`;
   - fail-fast behavior for broken provenance where practical.

## Progress Update

- Done: added a new canonical MIR `StoreMember` statement that publishes member identity, value source, and continuation-route metadata instead of falling back to `assign lhs lowering pending` for member writes.
- Done: `lower_assign_stmt` now lowers member writes, including `cell.k = Some(k)` and `cell.k = none_k`, and publishes wrapper-path metadata for continuation-bearing writes.
- Done: updated MIR-side rewriting/analysis helpers so the new statement is preserved through materialization and handled conservatively by inline/escape/summary/frame/codegen support code.
- Done: added late-lowered `underlying_continuation_route` publication on `LateLoweredResumeBoundaryOperandContract`.
- Done: built a provenance resolver that bridges handle continuation binders, member writes, member reads, and `PatternExtract(Some[0])` readbacks into a unique underlying continuation route or fails fast on ambiguity/missing provenance.
- Done: threaded the new route through effect-lowered stable dumps and LLVM ABI/query validation.

## Verification Status

- Passed: `cargo test -p scoopc dump_mir_publishes_member_write_contract_for_escape_continuation_cell`
- Passed: `cargo test -p scoopc published_continuation_provenance_rejects_ambiguous_member_routes`
- Passed: `cargo test -p scoopc refactor_boundary_lowering_publishes_member_readback_resume_route`
- Passed: `cargo test -p scoopc refactor_llvm_boundary_operand_contract_`
- Passed: `cargo test -p scoopc refactor_effect_lowered_`
- Passed: `cargo test -p scoopc refactor_llvm_`
- Passed: `cargo run -p scoop -- --effect-pipeline refactor dump-mir tests/fixtures/run-pass/effect_multi_escape_indirect_direct_while.scoop`
- Passed: `cargo run -p scoop -- --effect-pipeline refactor dump-effect-lowered tests/fixtures/run-pass/effect_multi_escape_indirect_direct_while.scoop`
- Passed: `cargo clippy -p scoopc -p scoop --all-targets -- -D warnings`

## Remaining Steps

1. Mark `P6-T02qa` as `[DONE]` in `TODO-P6-part2.md` and sync `TODO.md`.
2. Review the worktree one more time and create the requested git commit.

## Notes

- I will not use workarounds or narrow the task scope to get a partial pass.
- If the phase-level plan changes, I will update `PLAN.md`; otherwise routine progress stays in the detailed TODO file and this plan file.
