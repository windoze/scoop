# Execution Plan

## Scope for this invocation

Complete exactly one TODO item: the first incomplete task in `TODO.md`, after checking for any issues called out by the latest commit.

## Initial plan

1. Inspect the latest commit message and diff to see whether it mentions any known issue that must be fixed before new task work.
2. Read `TODO.md` and `PLAN.md` to identify the first incomplete task and understand project sequencing.
3. If the first incomplete task is too large for one clean implementation pass, break it into smaller subtasks and update `PLAN.md` and `TODO.md` so the first new subtask is actionable now.
4. Implement the selected task.
5. Run focused verification first, then broader required checks, including `cargo fmt`, relevant tests, and `cargo clippy --all-targets -- -D warnings` if feasible for the changed area.
6. Update `TODO.md`, `PLAN.md`, and this file to reflect progress and completion state.
7. Commit the result with a task-tagged message and stop.

## Notes

- I will keep this file updated when the plan changes or when a key step is completed.
- This file contains a concise execution log and plan, not private internal reasoning.

## Progress log

- Checked the latest commit (`[T0150d] Add SourceMap-backed literal parse diagnostics`). It does not describe a separate known issue that must be fixed first.
- Read `TODO.md` / `PLAN.md` and identified the first incomplete task as `T0145` (hex and binary integer literal support).
- Confirmed `T0145` is small enough to complete in one invocation; no task split is required.
- Observed that `crates/scoopc/src/syntax/int_literal.rs` already contains uncommitted partial work adding a shared `parse_int_literal(...)` helper for decimal/hex/binary. The remaining work is to wire the rest of the pipeline to that helper, update the lexer to lex prefixed literals as one token, and add regression coverage.
- Implemented the remaining pipeline wiring: prefixed literal lexing, shared parser call sites (LLVM / comptime / relevant annotation helpers), unit tests, and three new regression fixtures.
- During fixture validation, a new fixture exposed a pre-existing LLVM gap: top-level `when (Int)` subjects were unsupported. I fixed that in `llvm/codegen/control_flow.rs` so `when (x) { 0xFF -> ... }` now works directly.
- Verification status:
  - `cargo test --all`: passed.
  - `cargo run -p scoop -- test`: passed (`fixtures: ok (841)`).
  - `cargo clippy --workspace --all-targets -- -D warnings`: still fails on the existing workspace baseline (notably inkwell `ptr_type` deprecations plus longstanding clippy findings such as `too_many_arguments` / `result_large_err`), and I did a focused grep to confirm no new task-specific clippy failures in the files changed for T0145.
- Remaining end-of-turn work: stage the completed task metadata changes, create the required git commit, and stop.

## Continuation

- Resumed with `T0145` implementation already complete and verified; this continuation is limited to final documentation synchronization, careful staging from a dirty worktree, and the required git commit.
- Because many unrelated files are modified in the working tree, I need to inspect the mixed-diff files involved in `T0145` before staging so the commit only captures task-relevant changes.
- Fixed one task-adjacent clippy warning in `llvm/codegen/control_flow.rs` (`only_used_in_recursion` on an enum-pattern helper parameter) and reran the focused file-path clippy grep. The remaining hits are the pre-existing workspace baseline only (LLVM `ptr_type` deprecations, `too_many_arguments`, `private_interfaces`, `dead_code`, etc.).
- Staged the `T0145` file set, patch-staged only the relevant hunks from the mixed `hir/lower/util.rs` and `typecheck/annotations.rs` files, and spot-checked the staged diff. Next step is the task commit.
