# 执行计划与决策摘要

## 约束

- 本轮只完成 `TODO.md` 中第一个未完成任务，然后停止。
- 在开始实现任务前，先检查最新提交是否提到遗留问题；若有，则先修复这些问题。
- 执行过程中持续更新本文件，记录计划调整、关键发现、实现进度、测试结果与待确认事项。
- 这里记录的是可共享的决策摘要与执行计划，不包含不可共享的内部推理细节。

## 初始步骤

1. 检查最新一次 git 提交，确认提交信息和变更中是否提到了需要先处理的既有问题。
2. 阅读 `TODO.md`，定位第一个未完成任务。
3. 阅读 `PLAN.md`，确认该任务是否已有拆分计划、依赖关系或顺序要求。
4. 如果该任务过大，则先将其拆分为更小子任务，并同步更新 `PLAN.md` 与 `TODO.md`；本轮只执行拆分后排在最前的那个子任务。
5. 实现任务并补充必要测试。
6. 运行格式化、测试和 lint，至少覆盖与改动相关的验证，并尽量满足 `cargo clippy --all-targets -- -D warnings` 无告警。
7. 更新 `TODO.md`、`PLAN.md` 与本文件，记录结果。
8. 使用清晰的提交信息提交本轮改动，然后停止。

## 当前状态

- 已完成：初始化执行计划文件。
- 已完成：检查最新提交、`TODO.md`、`PLAN.md`。
- 进行中：核实最新提交提到的既有 `clippy` 基线问题是否仍存在，并确定修复范围。
- 已确定：`TODO.md` 中第一个未完成任务是 `T0147c`（Float sysroot API 与 builtin 方法路由）。

## 已知发现

- 最新提交 `16ef3cfb90a84933cc41635bbddca21e3c724e42` 的 commit message 本身未直接声明新的遗留功能缺陷。
- 但该提交同步更新的 `memory/claude_plan.md` 明确记录：`cargo clippy --workspace --all-targets -- -D warnings` 仍被仓库既有 baseline 问题阻塞，主要类别包括：
  - `inkwell` deprecated `ptr_type` / `ptr_sized_int_type_in_context`
  - `too_many_arguments`
  - `result_large_err`
  - 少量 `unused_variables` / `private_interfaces` / `dead_code`
- 按本轮用户要求，这些“最新提交明确提到的既有问题”需要先核实并修复，然后再继续 `T0147c`。

## 任务拆分调整

- 由于当前 `cargo clippy --workspace --all-targets -- -D warnings` 实际报出约 331 个错误，不适合与 `T0147c` 同轮硬推，因此已在 `TODO.md` / `PLAN.md` 中把 `T0147c` 前置拆分为三个可独立验收的子任务：
  1. `T0147c-1`：LLVM opaque pointer API 去弃用（`ptr_type` / `ptr_sized_int_type_in_context`）
  2. `T0147c-2`：`too_many_arguments`
  3. `T0147c-3`：`result_large_err` + 零散 warning
- 本轮只执行新的首个未完成任务：`T0147c-1`。

## 更新后的执行顺序

1. 清理 `crates/scoopc/src/llvm/**` 中所有 `ptr_type` / `ptr_sized_int_type_in_context` 的 deprecated 调用。
2. 运行 `cargo fmt --all`。
3. 运行本任务相关验证，至少包括：
   - `cargo clippy --workspace --all-targets -- -D warnings`
   - `cargo test --all`
   - `cargo run -p scoop -- test`
4. 若 clippy 仍失败，则确认仅剩 `T0147c-2` / `T0147c-3` 范围的既有问题。
5. 更新 `TODO.md` / `PLAN.md` / 本文件，将 `T0147c-1` 标记完成并提交。

## 本轮执行结果

- 已完成 `T0147c-1`。
- 代码实现：
  - 在 `crates/scoopc/src/llvm/codegen/gc.rs` 中新增 `llvm_ptr_type(...)` / `llvm_ptr_sized_int_type(...)` helper。
  - 已将 LLVM codegen / runtime ABI / LLVM 测试路径中的旧 `*.ptr_type(...)` 与 `ptr_sized_int_type_in_context(...)` 全部迁移到 opaque pointer 新接口。
  - 已顺手移除迁移后不再需要的 typed pointer 临时变量，避免本轮引入新的 `unused_variables`。
- 验证结果：
  - `cargo fmt --all` 通过。
  - `cargo check -p scoopc --features llvm` 通过。
  - `cargo clippy --workspace --all-targets -- -D warnings` 仍失败，但已确认**不再包含** `deprecated method`、`ptr_type`、`ptr_sized_int_type_in_context` 类错误。
  - 当前 strict clippy 剩余问题已收敛到后续两个子任务范围：`too_many_arguments`、`result_large_err`，以及少量 `private_interfaces` / `dead_code`。
  - `cargo test --all` 通过。
  - `cargo run -p scoop -- test` 通过（`fixtures: ok (852)`）。
- 下一步：
  - `TODO.md` 中新的首个未完成任务变为 `T0147c-2`（`too_many_arguments` 清理）。
