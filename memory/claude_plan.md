# Claude Plan

## Planning Note

I will not record private chain-of-thought verbatim. This file tracks a concise reasoning summary, execution plan, and progress updates for the current invocation.

## Current Goal (P7-T01 — completed)

Execute the full regression matrix and run the legacy residual grep audit.

## Plan

1. Confirm P7-T01 is the first incomplete task. (P6-T03 is `[DONE]`; P7-T01 only depends on it.)
2. Re-read the validation matrix the task requires:
   - `cargo test --all`
   - `cargo run -p scoop -- test`
   - `SCOOP_GC_MOVE=1 SCOOP_GC_STRESS=1 SCOOP_GC_VERIFY_ROOTS=1 cargo run -p scoop -- test --fixtures tests/fixtures/runtime_gc`
   - `cargo test -p scoopc llvm_tests`
   - `cargo test -p scoopc codegen_gap_inventory`
   - `rg '<legacy reasons + UnsupportedMainBody>'` over `crates/scoopc/src crates/scoop/src tests/fixtures`
3. Execute the matrix. Investigate any failures. If full regression exposes a new blocker, either fix it now or insert a prerequisite task in `TODO.md` and stop.
4. Run the legacy residual grep audit over the active tree (and split out doc/archive/sentinel/active hits).
5. Write a categorized hit summary into the P7-T01 completion record. The summary must include:
   - command output (or representative subset) for each validation step;
   - classification of every grep hit into doc/archive vs internal bug sentinel vs active residual;
   - explicit confirmation that no active residual remains.
6. Mark P7-T01 `[DONE]` only if every audit and validation step passes; otherwise leave the task as `[TODO]` and record the blocker.
7. Commit on the `eff` branch with a `[P7-T01]`-prefixed message and stop.

## Progress Updates

1. Identified P7-T01 as the first incomplete task. P6-T03 already `[DONE]`; no historical issue blocked execution.
2. Ran the full validation matrix:
   - `cargo test --all`: 1035 / 0 / 0 (pass / fail / ignored).
   - `cargo run -p scoop -- test`: 1232 / 0 fixture rows.
   - `SCOOP_GC_MOVE=1 SCOOP_GC_STRESS=1 SCOOP_GC_VERIFY_ROOTS=1 cargo run -p scoop -- test --fixtures tests/fixtures/runtime_gc`: 25 / 25 PASS.
   - `cargo test -p scoopc --lib llvm::tests::`: 230 / 0 (607 filtered) — covered the LLVM unit set the TODO labels `llvm_tests` (no separate target with that name exists).
   - `cargo test -p scoopc codegen_gap_inventory`: 14 / 0.
   - `cargo clippy --all-targets -- -D warnings`: clean.
3. Ran the legacy residual grep audit. 0 hits for `LegacyOnly` and the eight legacy reason strings in `crates/scoopc/src`, `crates/scoop/src`, `tests/fixtures`. 1535 `UnsupportedMainBody` hits, all in `crates/scoopc/src`, all classified as canonical impossible-state sentinel + the audit/inventory infrastructure that watches them. No active residual remains.
4. Marked P7-T01 `[DONE]` in `TODO.md` and filled the four required completion-record sections (改动范围 / 核心决策 / 验证结果 / `PLAN.md` `PIPELINE_GAPS.md` 对应闭合).
5. Committing the change set on the `eff` branch with a `[P7-T01]` prefix and stopping.
