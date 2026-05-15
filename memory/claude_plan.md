## 本轮执行计划（P4-T01j）

按照 `PROMPT.md` 规范完成 `TODO.md` 中第一个未完成任务 **P4-T01j**：把 `declare_named_intrinsic_runtime_symbol` 接入 `declare_runtime_or_native_import_function` 通道，消除 `add_function(..., None)` raw 调用。

### 任务确认

- `TODO.md` 中 `P4-T01i` 已 `[DONE]`；`P4-T01j` 是 P4 前置 IV 中第一个仍未完成的 production drift 任务。
- 最近一次提交 `[P4-T01i] Clean up @Unsafe @Extern fixtures, golden drift, sentinels` 已显式留出 `P4-T01j` 范畴的失败 (`function_declaration_inventory_eliminates_raw_add_function_none_callsites`)。

### 实现方案

`crates/scoopc/src/llvm/codegen/intrinsics/named.rs::declare_named_intrinsic_runtime_symbol` 当前在末尾写：

```
Ok(self.module.add_function(symbol, fn_ty, None))
```

该 raw 调用绕开了 `declare_classified_llvm_function` 的 surface 检查。修复：复用 `crates/scoopc/src/llvm/codegen/mod.rs::declare_runtime_or_native_import_function`（surface 为 `RuntimeOrNativeImport`，linkage 为 External）。它内部的 `if let Some(existing) = module.get_function(name)` 已经处理"已存在则复用"，所以 named.rs 顶部的 early-return 可以直接交给 wrapper 完成。

具体步骤：

1. 把 `Ok(self.module.add_function(symbol, fn_ty, None))` 改为 `Ok(declare_runtime_or_native_import_function(self.module, symbol, fn_ty))`，删除冗余的 early-return（保留也行，但 wrapper 已含相同语义；为避免双重检查路径，统一交给 wrapper）。
2. 添加必要 import：`use crate::llvm::codegen::declare_runtime_or_native_import_function;`（或对应模块 path）。
3. 不放宽 `declare_classified_llvm_function` 的 surface assertions；不引入新 surface 类型。

### 验证

- `cargo test -p scoopc llvm::tests::function_declaration_inventory_eliminates_raw_add_function_none_callsites`
- `cargo test -p scoopc named_intrinsic`
- `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass`：全量不退化。
- `cargo clippy --all-targets -- -D warnings`

### 风险点

- runtime_or_native_import 的 surface assertions 要求 `linkage == External`；本任务符合（默认 None 等价于 External in raw add_function）。
- wrapper 的 `if let Some(existing)` 与原代码顶部的 `if let Some(existing) = self.module.get_function(symbol)` 等价，可以保留 named.rs 顶部那段以减少 churn，也可以一起交给 wrapper；选后者以减少代码重复。

### 进展更新

- 把 `declare_named_intrinsic_runtime_symbol` 末尾的 `Ok(self.module.add_function(symbol, fn_ty, None))` 改为 `Ok(self.declare_runtime_or_native_import_function(symbol, fn_ty))`（复用 `MainCodegen` method form）。
- 删除函数顶部冗余的 `if let Some(existing) = self.module.get_function(symbol)` early-return，避免双重检查；wrapper 内已经覆盖该语义。
- 验证：
  - `cargo build -p scoopc`
  - `cargo test -p scoopc function_declaration_inventory`：1 passed（修复确认）；
  - `cargo test -p scoopc named_intrinsic`：3 passed；
  - `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass`：400 fixtures 全部通过；
  - `cargo test -p scoopc`：860 passed / 1 failed（剩余唯一失败为 `materialize_for_dump_keeps_set_alias_receiver_overload_targets_distinct`，属 `P4-T01k` 范畴）；
  - `cargo clippy --all-targets -- -D warnings`：通过。

### 完成状态

- 已完成：实现、回归、`[DONE]` 完成记录、`memory/claude_plan.md` 刷新；
- 待提交：`crates/scoopc/src/llvm/codegen/intrinsics/named.rs`、`TODO.md`、`memory/claude_plan.md`。
