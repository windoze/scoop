# Current Task: T0141 — Block-level Control Flow (return/break/continue)

## Status: COMPLETED

## Problem
Current LLVM codegen uses recursive expression-based codegen without pre-allocated basic blocks. `return`/`break`/`continue` inside nested blocks or loop bodies need to jump to target basic blocks (function exit / loop-after / loop-head) that don't exist yet at codegen time. They are rejected with `UnsupportedMainBody`.

## Solution
Incremental block-based control flow (no MIR required):

1. **LoopContext stack** on MainCodegen: `codegen_while_stmt` pushes `{ break_bb: after_bb, continue_bb: cond_bb }`, pops after body codegen.
2. **ReturnContext** on MainCodegen: `codegen_top_level_fun` creates `return_bb` + `return_alloca`, sets `return_context`. Both normal path and early returns go through return_bb.
3. **codegen_early_return** helper: stores value to return_alloca, branches to return_bb.
4. **codegen_if_expr fix**: After codegen'ing then/else branches, check for terminator before emitting coercion/store/merge-branch (avoids "terminator in middle of basic block" LLVM error).
5. All 4 block codegen functions updated: `codegen_block_stmt`, `codegen_block_as_return_value`, `codegen_block_value_in_expected_context`, `codegen_block_as_exit_code`.

## Files Modified
- `crates/scoopc/src/llvm/codegen/mod.rs` — LoopContext/ReturnContext structs, fields, codegen_early_return, codegen_top_level_fun
- `crates/scoopc/src/llvm/codegen/stmt.rs` — break/continue/return in codegen_block_stmt, loop context push/pop in codegen_while_stmt
- `crates/scoopc/src/llvm/codegen/control_flow.rs` — All 4 block codegen functions + codegen_if_expr terminator fix

## Fixtures Added
- `block_early_return.scoop` — early return from if block in function
- `while_break_basic.scoop` — break from while loop
- `while_continue_basic.scoop` — continue in while loop
- `nested_loop_break.scoop` — nested loops with break/continue
- `return_from_loop.scoop` — return from inside while loop

## Test Results
- 139 unit tests pass
- 807 fixtures pass (802 original + 5 new)
