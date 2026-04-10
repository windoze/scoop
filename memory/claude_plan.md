# 执行记录与计划

更新时间：2026-04-10

## 说明

按要求先记录执行计划，再开始任何检查或实现工作。这里记录的是可公开的高层分析、执行步骤、决策依据和进度，不包含内部私有推理细节。

## 初始目标

本轮只完成 `TODO.md` 中第一个未完成任务，并在完成后停止。

## 初始执行步骤

1. 检查最新一次 Git 提交，确认提交说明中是否提到任何已知遗留问题。
2. 若最新提交提到需要先修复的问题，则优先修复这些问题，并验证。
3. 阅读 `TODO.md`，定位第一个未完成任务。
4. 阅读 `PLAN.md`，核对该任务的上下文、依赖和预期范围。
5. 判断该任务是否可以在本轮完整完成。
6. 如果任务过大，则把任务拆分为更小的可执行子任务，并同步更新 `PLAN.md` 与 `TODO.md`，然后执行拆分后的第一个子任务。
7. 实现该任务，必要时补充或调整测试、文档和注释。
8. 运行相关校验，至少包括与改动相关的测试；若范围允许，补充运行 `cargo fmt`、`cargo test --all`、`cargo clippy --all-targets -- -D warnings`。
9. 更新 `TODO.md`、`PLAN.md` 与本文件，记录完成状态或依赖调整。
10. 用清晰的提交信息提交本轮改动，然后停止。

## 执行原则

- 优先修复最新提交明确提到的遗留问题。
- 一次只推进一个最前面的未完成任务。
- 若遇到阻塞，不把任务标成 blocked，而是调整任务顺序并记录依赖。
- 不回退用户已有改动；若发现冲突性未预期修改，先评估影响再决定如何继续。

## 进度

- [x] 已创建本计划文件。
- [x] 检查最新提交。
- [x] 读取 `TODO.md` 与 `PLAN.md`。
- [x] 确认第一个未完成任务为 `T0146c`，并判断其需要拆分。
- [x] 更新 `TODO.md` / `PLAN.md`，把 `T0146c` 拆成可单轮完成的子任务。
- [x] 执行拆分后的第一个子任务 `T0146c1`。
- [x] 运行验证。
- [x] 更新文档与任务状态。
- [ ] 提交改动并停止。

## 上下文结论

- 最新提交为 `[T0146b] Add char static semantics`，提交说明中没有额外点名需先修复的遗留问题。
- `TODO.md` 中第一个未完成任务是 `T0146c`，范围覆盖 `sysroot / runtime / LLVM codegen / run-pass`，单轮实现与回归面过大，不适合直接整体落地。

## 拆分决策

将原 `T0146c` 拆为两个连续子任务：

1. `T0146c1`：先补齐 LLVM 标量链路，让 `Char` 作为运行期 `i32` 值类型在单文件 run-pass 中可用。
   - 范围：`cg_ty_of` / `cg_ty_of_type_fqn` 的 Char 映射、Char 字面量 emission、比较、`when` Char pattern、`Char.toInt()` codegen。
   - 验收：新增 run-pass fixture，覆盖赋值、转义/Unicode escape、比较、`toInt()`、`when` pattern。
2. `T0146c2`：再补 sysroot/runtime 文本化与剩余 API。
   - 范围：`sysroot/core.scoop` 的 `Char` 声明、`toString()` / `hash()`、runtime `scoop_char_to_string`、相关 codegen、多文件与打印回归。

本轮执行目标改为：完成 `T0146c1`，然后停止。

## 本轮实现结果

- `crates/scoopc/src/llvm/codegen/ty.rs`
  - `ValueTypeKind::Char` / `scoop.core.Char` 已映射到 `IntTy { bits: 32, signed: false }`。
- `crates/scoopc/src/llvm/codegen/mod.rs`
  - `LiteralKind::Char(char)` 现在直接发射为 LLVM `i32` 常量。
  - `Char.toInt()` 已接线为 zero-extend 到目标平台 `Int`。
- `crates/scoopc/src/llvm/codegen/control_flow.rs`
  - top-level `when (char)` 的 `CharLit` 条件生成已补齐。
  - tuple element 的 `CharLit` codegen 分支也已补齐。
- `tests/fixtures/run-pass/char_runtime_scalar_basic.*`
  - 新增单文件 run-pass 回归，覆盖赋值、转义、Unicode escape、比较、`toInt()`、`when` pattern、返回 `Char` 的函数。

## 验证结果

- `cargo test --all`：通过
- `cargo run -p scoop -- test`：通过（`fixtures: ok (849)`）
- `cargo run -p scoop -- build tests/fixtures/run-pass/char_runtime_scalar_basic.scoop -o /tmp/char_runtime_scalar_basic.out`：通过
- `/tmp/char_runtime_scalar_basic.out`：stdout 与 golden 一致
- `cargo clippy --workspace --all-targets -- -D warnings`：**未通过**
  - 原因是仓库既有 baseline：大量 `inkwell` deprecated `ptr_type` / `ptr_sized_int_type_in_context`，以及长期存在的 `too_many_arguments` / `result_large_err`。
  - 本轮一度引入的 `cg_ty_of` 不可达分支 warning 已清理；当前 clippy 失败项不来自本次 Char 改动。
