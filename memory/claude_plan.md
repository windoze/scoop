# Claude Plan

## Planning Note

I will not record private chain-of-thought verbatim. This file tracks a concise reasoning summary, execution plan, and progress updates for the current invocation.

## Current Goal (P6-T03 — completed)

Rewrite `PIPELINE_GAPS.md`, active inventory, and fixtures to the final state.

## Progress Summary

1. Confirmed `P6-T03` is the next incomplete task in `TODO.md`. Latest commit was `[P6-T02A] Close typed call-site / effect-facts contract regression` which left 21 unit-test snapshot drifts explicitly deferred to P6-T03 / P7-T01.
2. Verified existing `PIPELINE_GAPS.md`, `crates/scoopc/src/llvm/codegen_gap_inventory.rs`, and `crates/scoopc/src/mir/placeholder_inventory.rs` against the actual source tree.
3. Fixed 21 baseline unit test failures along the four buckets the task names:
   - `effect_facts/builder.rs` (6 tests): updated embedded test fixture to use `import scoop.core.*` short-name calls instead of unsupported FQN call syntax (`scoop.core.Raise.raise(...)` → `Raise.raise(...)`).
   - `effect_lowered/materialize.rs` (6 tests): refreshed site-id expectations after P6-T02A's synthetic-span insertions (`HandleDispatch` site0 → site1; resume sites [25,30,35,40] → [26,31,36,41]); broadened the runtime-error boundary lookup to be shape-based; broadened a known-instance closure call to also accept `CallSiteKind::FunValue`; replaced the `site1`-as-call assertion with a shape-based `helper(...)` direct-call lookup.
   - `llvm/codegen/effect_lowered/layout.rs` (4 tests): mirrored the same site-id and shape-based refreshes on the LLVM layout side.
   - `llvm/tests.rs` + `mir/lower.rs` + `mir/closure_simplify.rs` + `mir/escape.rs` (5 tests): updated the managed-function explicit root descriptor expectation to `[1 x i32] [i32 16]` (single merged root); fixed two MIR lower fixtures' FQN expectations; extended escape analysis and closure simplify to recognize `CallKind::FunValue` whose callee aliases back to a local `MakeClosure` as a non-escaping local closure call (recovered the optimization that P6-T02A's typed-callee precision unintentionally regressed).
4. Updated `PIPELINE_GAPS.md` status header to declare it the P6-T03 final ledger and clarified `UnsupportedMainBody`'s reinterpretation as a contract-violation / impossible-state sentinel.
5. Verified validation matrix:
   - `cargo test -p scoopc --lib codegen_gap_inventory`: 14/14 passed.
   - `cargo test -p scoopc --lib refactor_mir_placeholder_inventory`: 1/1 passed.
   - `cargo test -p scoopc --lib llvm::tests::`: 110/110 passed.
   - `cargo test -p scoopc --lib`: 837/837 passed (all 21 baseline regressions cleared).
   - `cargo run -p scoop -- test`: 1232 fixtures PASS.
   - `cargo clippy --all-targets -- -D warnings`: clean.
   - `rg 'LegacyOnly|assign lhs lowering pending|...'`: 0 hits in active tree (only `UnsupportedMainBody` remains as the canonical impossible-state guard, consistent with `PIPELINE_GAPS §0`).
6. Marked `P6-T03` as `[DONE]` in `TODO.md` and filled the completion record.
