# Claude Execution Notes

## Current Objective

Complete exactly one task for this invocation: the first undone task in `TODO.md`, which is `T0150c`.

## Commit / Issue Triage

- Checked the latest commit before task execution: `c15c4943f72921115fdf2d075f41b565d7fc4420`
- Commit subject: `[T0150b] Wire LLVM codegen through SourceMap`
- No additional pre-existing issue was called out in that commit message that needed to be fixed before `T0150c`

## Task Understanding

`T0150c` required reverting eager integer/string literal payloads in HIR back to source-backed parsing while keeping LLVM codegen able to recover the correct source file for literals and constant evaluation.

## Implementation Summary

- Reverted HIR literal storage from parsed payloads back to source-backed forms:
  - `hir::LiteralKind::Int(u128)` -> `hir::LiteralKind::Int`
  - `hir::LiteralKind::String(Vec<u8>)` -> `hir::LiteralKind::String`
  - `hir::WhenPat::IntLit { span, value }` -> `hir::WhenPat::IntLit { span }`
- Added stable source provenance fields:
  - `FunDecl.source_path`
  - `TopLevelVar.source_path`
  - `ObjectInit.source_path`
  - `ClassInit.source_path`
- Added `SourceMap::source_id_of_path(...)` so LLVM codegen can resolve a runtime `SourceId` from stable source paths.
- Updated LLVM codegen to switch source context when emitting:
  - normal functions
  - object initializers
  - class initializers / constructors
  - top-level variable initializers
- Restored source-backed parsing in:
  - literal codegen
  - integer constant evaluation
  - `when` integer pattern comparisons
- Removed duplicated integer literal parsing logic in comptime evaluation and reused the shared syntax helper.
- Updated HIR fixture goldens to match the reverted HIR shape.

## Validation Results

- `cargo check -p scoopc` passed
- `cargo test --all` passed
- `cargo run -p scoop -- test` passed (`fixtures: ok (836)`)
- `cargo clippy --workspace --all-targets -- -D warnings` does not pass repo-wide because of a pre-existing baseline unrelated to this task

Known pre-existing strict clippy failures observed:

- deprecated `inkwell::*::ptr_type` usage
- `clippy::too_many_arguments`
- `clippy::result_large_err`
- `clippy::only_used_in_recursion`

## Remaining Close-Out Steps

1. Review the final diff for `T0150c` and confirm the bookkeeping updates are correct.
2. Commit with a `T0150c` message.
3. Stop after the commit.

## Progress Log

- Reused prior implementation state instead of redoing code changes.
- Updated `TODO.md` to mark `T0150c` done and recorded the completion notes.
- Updated `PLAN.md` section `4.1` to mark `T0150c` done and keep `T0150d` as the next pending task.
- Next action: final diff review, commit `[T0150c] Revert literals to SourceMap-backed parsing`, then stop.
