# Current Task: T0115 — String 补齐: trim/replace/charAt/isEmpty 等缺失方法

## Status: IN PROGRESS

## Task Description
Add missing String methods. Currently String has ~11+7 resolver whitelist methods. Need to add:
- P0: `trim(): String`, `isEmpty(): Bool`
- P1: `replace(old: String, new: String): String`, `charAt(index: Int): Int`
- P2: `trimStart(): String`, `trimEnd(): String`, `repeat(n: Int): String`, `compareTo(other: String): Int`

## Implementation Path
runtime/c → scoop_runtime_api.h → resolver whitelist → typecheck → codegen → fixtures

## Step-by-step Plan

### Step 1: Runtime/C Implementation
Add C functions in `runtime/c/scoop_runtime.c`:
- `scoop_string_trim(s) -> ScoopString*`
- `scoop_string_is_empty(s) -> i64`
- `scoop_string_replace(s, old, new) -> ScoopString*`
- `scoop_string_char_at(s, index) -> i64`
- `scoop_string_trim_start(s) -> ScoopString*`
- `scoop_string_trim_end(s) -> ScoopString*`
- `scoop_string_repeat(s, n) -> ScoopString*`
- `scoop_string_compare_to(a, b) -> i64`

Register in `scoop_runtime_api.h`.

### Step 2: Resolver Whitelist
Add method names to String whitelist in `resolve/scopes.rs`.

### Step 3: Typecheck
Add parameter/return type validation in `typecheck/expr/call.rs`.

### Step 4: Codegen (LLVM)
- Add runtime symbol constants in `runtime_symbols.rs`
- Add LLVM declarations in `runtime_abi.rs`
- Add dispatch in `codegen/mod.rs` `codegen_string_method`

### Step 5: Fixtures
Create run-pass fixtures covering each method.

### Step 6: Test & Verify
- `cargo test --all`
- `cargo run -p scoop -- test`

## Progress
- [ ] Step 1: Runtime/C
- [ ] Step 2: Resolver
- [ ] Step 3: Typecheck
- [ ] Step 4: Codegen
- [ ] Step 5: Fixtures
- [ ] Step 6: Test & Verify
