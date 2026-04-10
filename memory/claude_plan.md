# Claude Plan

## Constraints and approach

The user asked for a complete thought process log. I will keep this file as a detailed, explicit execution plan and decision log that records what I am doing, what I found, and why I chose the next step, without dumping raw internal scratchwork.

Primary workflow for this invocation:

1. Inspect the latest commit message and diff summary to check for any explicitly mentioned pre-existing issue that must be fixed before task work.
2. Read `TODO.md` and identify the first incomplete task.
3. Read `PLAN.md` to understand project sequencing and whether the task needs decomposition.
4. If the first incomplete task is too large for one safe implementation pass, refine it into smaller subtasks and update `PLAN.md` and `TODO.md` accordingly. Then execute only the first new subtask.
5. Implement the selected task fully.
6. Run focused verification first, then broader required quality gates, including formatting, tests, and linting as needed for confidence and repo policy.
7. Update `TODO.md`, `PLAN.md`, and this file to reflect completion or any dependency-driven reordering.
8. Commit exactly the changes for this completed task and stop.

## Progress log

- Started invocation.
- Created this plan file before running any repository commands, per user instruction.
- Inspected the latest commit message: `Update plan and fix warnings`. It does not mention any pre-existing functional issue that must be fixed ahead of task work.
- Read `TODO.md` and `PLAN.md`. The first incomplete task is the original T0146 (`Char` type + char literals).
- Scoped T0146 across the codebase. It touches lexer/token/parser/AST, HIR/type system, typecheck/comptime, and LLVM/runtime/sysroot. That is too large for one safe implementation + verification pass.
- Refined T0146 into three sequential subtasks and updated `TODO.md` / `PLAN.md`:
  - `T0146a`: char literal frontend syntax and diagnostics.
  - `T0146b`: `Char` type semantics in HIR/typecheck/comptime.
  - `T0146c`: sysroot/runtime/LLVM codegen end-to-end support.
- Current execution target for this invocation: `T0146a`.

## T0146a implementation plan

1. Add a shared `syntax/char_literal.rs` parser with unit tests for plain characters, escapes, Unicode escapes, and invalid forms.
2. Extend `TokenKind` and the lexer to emit `CharLiteral` and surface dedicated lexical diagnostics for invalid / unterminated char literals.
3. Extend AST and parser support for char literals in ordinary expressions and `when` patterns.
4. Make minimal downstream compatibility edits so the workspace still compiles before full Char semantics exist.
5. Add parse fixtures for successful and failing Char literal syntax cases.
6. Run formatting and verification (`cargo test --all`, `cargo run -p scoop -- test`, and clippy if feasible; if blocked by existing workspace baseline, record that explicitly).

## Completion log

- Implemented `T0146a`:
  - Added `crates/scoopc/src/syntax/char_literal.rs` with strict parsing and unit tests.
  - Added `TokenKind::CharLiteral` and lexer support for Char literals plus dedicated lexical diagnostics.
  - Added AST/parser support for `ExprKind::CharLit` and `WhenPat::CharLit`.
  - Added minimal downstream compatibility branches so the workspace still compiles while Char semantics remain pending in T0146b/T0146c.
  - Added parser unit coverage and parse fixtures for success + failure cases.
- Verification results:
  - `cargo fmt` completed successfully.
  - `cargo test --all` passed.
  - `cargo run -p scoop -- test` passed with `fixtures: ok (846)`.
  - `cargo clippy --workspace --all-targets -- -D warnings` still fails on the pre-existing workspace baseline (large existing `inkwell` deprecation set plus longstanding `too_many_arguments` / `result_large_err` lints). No task-specific clippy regression was identified.
- Repo bookkeeping completed:
  - `TODO.md` now marks `T0146a` as done and leaves `T0146b` / `T0146c` queued.
  - `PLAN.md` now records `T0146a` as completed with verification notes.
  - Final diff review (`git diff --check`) is clean.
- Remaining final action: commit exactly this task and stop.
