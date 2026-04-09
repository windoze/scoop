# Current Task: T0119 — @CLayout(packed = N) support for N > 1

## Status: IN PROGRESS

## Task Description
Extend `@CLayout(packed = N)` to support N > 1 (`#pragma pack(N)` semantics).
Currently typecheck only accepts `packed = 1`; other values produce `CLayoutPackedValueNotSupported`.
Spec §15.5 defines `packed` as "max field alignment" — equivalent to C `#pragma pack(N)`.

## Plan

### Step 1: Typecheck — Accept packed = 1/2/4/8/16
- File: `crates/scoopc/src/typecheck/annotations.rs` (around line 1895-1903)
- Change validation to accept powers of 2 up to 16

### Step 2: Codegen — Implement #pragma pack(N) semantics
- For packed=1: keep existing behavior (LLVM packed struct)
- For packed>1: need to compute per-field alignment as `min(natural_align, N)`
  - Cannot use LLVM `set_body(fields, true)` since that's only packed=1
  - Need to either: (a) manually insert padding bytes, or (b) use LLVM's built-in struct layout with custom alignment attributes
- Also ensure store alignment is set to `min(natural_align, N)` for each field

### Step 3: HIR/data structures
- Ensure the `packed` value (not just bool) is propagated through StructLayout / CLayout info

### Step 4: Fixtures
- Typecheck fixtures: `packed = 2`/`4`/`8` pass, non-power-of-2 values fail
- Run-pass fixtures: `@CLayout(packed = 4)` struct with Int64 field, verify correct behavior

### Step 5: Testing
- `cargo test --all` + `cargo run -p scoop -- test`

## Progress
- [ ] Explore current code
- [ ] Implement typecheck changes
- [ ] Implement codegen changes
- [ ] Add fixtures
- [ ] Run tests
- [ ] Commit
