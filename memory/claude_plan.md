# Current Task: T0142 — if-without-else support

## Status: COMPLETED

## Problem
`codegen_if_expr` errors with `"if without else (non-Unit)"` when `else_branch` is None and the expected output type (from caller context) is non-Unit. This forced `stdlib/string.scoop` to use `else { }` workarounds on every statement-level if.

## Solution
In `codegen_if_expr`, when `else_branch` is None, force `out_cg = CgTy::Unit` regardless of the caller's `expected` type. The missing else branch produces no value, so the entire if expression is Unit. The else path simply branches to merge_bb.

## Files Modified
- `crates/scoopc/src/llvm/codegen/control_flow.rs` — Force Unit for if-without-else, remove error
- `stdlib/string.scoop` — Remove 9 empty `else { }` branches, update comments

## Fixtures Added
- `if_without_else.scoop` + `.stdout` — run-pass: 12 output lines (basic, clamp, nested, while+if)
- `if_without_else_as_value_is_error.scoop` — typecheck fail: val x: Int = if (true) { 5 }

## Test Results
- 139 unit tests pass
- 809 fixtures pass (807 + 2 new)
