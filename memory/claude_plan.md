## Current Plan

I will not record private chain-of-thought verbatim, but this file will contain a complete, concise execution plan and progress log for this turn.

### Goal

Complete exactly the first undone task from `TODO.md`, after first checking the latest commit for any explicitly mentioned pre-existing issues that must be fixed.

### Planned Steps

1. Inspect the latest commit message and diff summary for any noted pre-existing issue.
2. Read `TODO.md` and identify the first incomplete task.
3. Read `PLAN.md` and any nearby context needed to understand that task.
4. Decide whether the task is small enough to complete in one turn.
5. If the task is too large, decompose it into smaller subtasks and update `PLAN.md` and `TODO.md` so the first new subtask becomes the current task.
6. Implement the selected task.
7. Run the relevant formatting, linting, and test commands, including `cargo fmt --check` if appropriate, `cargo clippy --all-targets -- -D warnings`, and targeted or full test coverage as needed.
8. Fix any failures or warnings found during verification.
9. Update `TODO.md` to mark the completed task done.
10. Update `PLAN.md` to reflect progress and remaining work.
11. Update this file with the results of the work performed.
12. Commit the changes with a task-specific message, then stop.

### Progress Log

- Plan initialized before repo inspection.
- Checked latest commit `[T0150c] Revert literals to SourceMap-backed parsing`; it did not explicitly mention a pre-existing issue requiring separate remediation before task work.
- Identified the first undone task in `TODO.md` as `T0150d` (`字面量解析诊断接入 SourceMap + 多文件失败回归`).
- Reviewed `PLAN.md` and current implementation. The task is manageable without decomposition.
- Implementation plan refined:
  1. Add a dedicated LLVM literal-parse diagnostic that carries source code, file name, line/column, and literal text preview.
  2. Route current string-literal parse failures through that diagnostic while preserving current SourceMap-based file selection.
  3. Fix fixture harnesses so build / run_pass_cone failure cases preserve the original diagnostic instead of wrapping it away.
  4. Make fixture location assertions consult diagnostic-attached source code when available, so non-entry-file errors can be asserted correctly.
  5. Add one single-file failure fixture and one multi-file non-entry failure fixture.
  6. Run formatting and targeted/full verification, then update TODO/PLAN and commit.
- Implemented the task:
  - Added `scoop::llvm::invalid_literal` in `crates/scoopc/src/llvm/mod.rs`, carrying diagnostic file/line/column, literal preview text, and attached `source_code` for miette rendering.
  - Routed LLVM string literal parse failures through the current `SourceMap` source context in `crates/scoopc/src/llvm/codegen/mod.rs`.
  - Updated `crates/scoop/src/fixtures/mod.rs` so expected-fail `build` / `run_pass_cone` fixtures preserve the original diagnostic and `EXPECT-ERROR-AT` can resolve locations from diagnostic-attached source code.
  - Added failure regressions for entry-file and non-entry-file literal parse errors, and cleaned the remaining T0140 comment leftovers touched by this task.
- Verification complete:
  - `cargo fmt --check` passed.
  - `cargo test --all` passed.
  - `cargo run -p scoop -- test` passed with `fixtures: ok (838)`.
  - Direct checks confirmed `scoop::llvm::invalid_literal` now reports `literal_parse_error_entry_file.scoop:12:13` for the entry-file failure and `helpers.scoop:6:12` for the non-entry-file failure.
  - `cargo clippy --workspace --all-targets -- -D warnings` still fails on the existing repository-wide baseline (`inkwell::ptr_type` deprecations and many long-standing clippy lints). The previous T0150d-local `InvalidLiteral` / `literal_text_preview` clippy issues are no longer present.
- Updated `TODO.md` / `PLAN.md` to mark `T0150d` complete and record the verification results and clippy-baseline note.
- Repository state is ready for the task commit: `[T0150d] Add SourceMap-backed literal parse diagnostics`.
