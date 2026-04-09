# Current Task: T0118 — @CLayout(packed) store alignment fix + run-pass tests

## Status: COMPLETED

## Summary

1. **Store alignment fix** (`codegen/gc.rs` `store_local_value`): Added `store_inst.set_alignment(1)` for packed struct stores, matching the existing load alignment fix.

2. **Audit conclusion**: Current codegen has no GEP + store path for individual packed struct fields (struct values constructed via `build_insert_value`, stored as whole aggregate). Only `store_local_value` does whole-aggregate store to alloca. Class types can't be `@CLayout`, so no class field store fix needed.

3. **Fixtures** (3 run-pass):
   - `clayout_packed_basic.scoop` — packed=1, field read/write, function passing, negative values, var reassignment (18 stdout lines)
   - `clayout_aligned_basic.scoop` — aligned=16/8, field access, function passing, var reassignment (12 stdout lines)
   - `clayout_aligned_packed_combined.scoop` — aligned=8+packed=1 combination (13 stdout lines)

4. **Note**: Fixtures use `:` syntax (`@CLayout(packed: 1)`) not `=` syntax, because `=` syntax is rejected by the general annotation checker (T1019) while the CLayout-specific parser handles both.

5. **Verification**: 139 unit tests + 794 fixtures pass (including LLVM backend).
