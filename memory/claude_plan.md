# Current Task: T0120 — String 字节访问器: getByte + byteLength

## Status: COMPLETE

## Task Description
为 `String` 提供 O(1) 的只读字节级访问能力：
- `String.byteLength(): UInt` — 返回底层 UTF-8 字节数组长度
- `String.getByte(index: Int): Byte`（`Byte = UInt8`）— 返回指定偏移处的字节值

两个方法均为编译器 intrinsic（resolver 白名单 + codegen 内联 LLVM IR），不需要 runtime/c 函数。

## Plan

### Step 1: 了解现有 String 方法实现管线
- resolver 白名单（`resolve/scopes.rs`）
- typecheck（`typecheck/expr/call.rs`）
- codegen（`codegen/mod.rs`）
- 了解 ScoopString 的内存布局（len + data）

### Step 2: 实现 byteLength()
- resolver 白名单新增 `byteLength`
- typecheck：0 参数 → 返回 `UInt` 或 `Int`
- codegen：GEP 到 ScoopString.len，load 返回

### Step 3: 实现 getByte(index: Int)
- resolver 白名单新增 `getByte`
- typecheck：1 参数 (Int) → 返回适当类型
- codegen：越界检查 + GEP 到 data[index] + load i8 + zext

### Step 4: 新增 run-pass fixture
- ASCII 字符串 byteLength + 逐字节 getByte
- 多字节 UTF-8 字节序列

### Step 5: 验证全部测试通过 + commit

## Progress
- [x] 步骤1：了解现有管线
- [x] 步骤2：实现 byteLength()
- [x] 步骤3：实现 getByte()
- [x] 步骤4：新增 fixture
- [x] 步骤5：验证 + commit
