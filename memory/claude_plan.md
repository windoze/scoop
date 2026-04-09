# Current Task: T0121 — @Unsafe String.unsafeSliceBytes intrinsic

## Status: COMPLETE

## Task Description
提供 `String.unsafeSliceBytes(byteOffset: Int, byteLength: Int): String` — 一个 `@Unsafe` intrinsic，从源 String 的字节范围创建新 String，不执行 UTF-8 验证。

## Progress
- [x] Step 1: Runtime/C — `scoop_string_unsafe_slice_bytes` with defensive clamping + GC pin
- [x] Step 2: Resolver whitelist — `unsafeSliceBytes` added to String method whitelist
- [x] Step 3: Typecheck — unsafe context check + 2 args → String
- [x] Step 4: Codegen — runtime symbols + ABI declaration + dispatch in codegen_string_method
- [x] Step 5: Fixtures — run-pass (15 output lines) + typecheck error (non-unsafe context)
- [x] Step 6: 139 unit tests + 801 fixtures pass → TODO/PLAN updated → committed
