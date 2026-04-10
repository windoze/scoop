# 当前执行计划

## 本轮目标

- 只完成 `TODO.md` 中拆分后的首个未完成子任务 `T0147a`，完成后立即停止。
- 在收尾前补齐 `TODO.md`、`PLAN.md` 与本文件的状态记录，并提交一个清晰的 Git commit。

## 决策摘要

- 最新提交 `d4a0495a`（`[T0146c2] Add Char runtime text API`）的提交信息未提出需要先修复的遗留问题；当前未发现“提交已注明但未处理”的 pre-existing issue。
- 原始 `T0147` 跨 builtin 类型、LLVM 标量、sysroot API、resolver/typecheck builtin 路由与 runtime ABI，单轮实现面过大，已拆分为 `T0147a`、`T0147b`、`T0147c`。
- 本轮只做 `T0147a`：让 `Float64` / `Float32` / `Double` 在编译器的共享静态基础设施里成立；不提前实现 float literal、浮点运算、runtime 方法或 LLVM 标量 lowering。

## 已完成步骤

1. 已检查最新提交与任务顺序，确认当前首个未完成任务是拆分后的 `T0147a`。
2. 已把 `T0147` 拆分写入 `TODO.md` 与 `PLAN.md`，使后续可按 `T0147a -> T0147b -> T0147c` 顺序推进。
3. 已完成 `T0147a` 代码实现，核心内容包括：
   - `ValueTypeKind` 新增 `Float64` / `Float32`，`BuiltinTypes` 新增 `float64` / `float32`，`TypeStore::intern_builtins()` 与 `fmt::Display` 补齐。
   - implicit builtin type lookup 与 type lowering 补齐：`Float64` / `Float32` / `Double` 能在 resolver、typecheck、HIR lowering、Cone pre-specialize 等路径中被识别。
   - 共享基础设施补齐：layout、RTTI、Cone 导出、branch merge、builtin-to-interface assignable、HIR layout FQN 映射等穷举分支均已纳入 Float builtin。
   - `sysroot/core.scoop` 新增 `struct Float64`、`struct Float32` 与 `typealias Double = Float64`。
   - 新增 fixture `tests/fixtures/typecheck/float_builtin_type_refs_ok.scoop`。
   - 因 builtin `TypeId` 顺序变化，已刷新受影响的 HIR/MIR goldens。
4. 已知边界：
   - `crates/scoopc/src/llvm/codegen/ty.rs` 目前对 `ValueTypeKind::Float64/Float32` 明确返回 `None`，并用注释标出“LLVM 标量映射留待 `T0147b`”。

## 已完成收尾

1. `TODO.md` 已将 `T0147a` 标记为 `[DONE]`，并补上完成记录。
2. `PLAN.md` 已将 `T0147a` 状态更新为 `DONE`，并保留 `T0147b` / `T0147c` 为后续步骤。
3. 已重新运行验证：
   - `cargo test --all`：通过。
   - `cargo run -p scoop -- test`：通过（`fixtures: ok (852)`）。
   - `cargo fmt --check`：最初在 `crates/scoopc/src/llvm/codegen/ty.rs` 报出一处换行格式差异；执行 `cargo fmt` 后复查通过。
   - `cargo clippy --workspace --all-targets -- -D warnings`：仍失败，但失败点与此前判断一致，均为既有 workspace baseline，包括大量 `inkwell` deprecated `ptr_type` / `ptr_sized_int_type_in_context`，以及长期存在的 `too_many_arguments` / `result_large_err` 等，不是 `T0147a` 新引入的问题。

## 当前最后一步

1. 复核 `git status`，确认只包含本轮任务相关改动。
2. 提交本轮改动，提交信息使用 `[T0147a] Add Float builtin type plumbing`。

## 待补记录

- 提交哈希
