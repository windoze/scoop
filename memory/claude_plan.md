# 执行记录

## 说明

按要求先记录执行计划。这里记录的是可共享的推理摘要与步骤计划，不包含逐字内部思维。

## 当前目标

完成 `TODO.md` 中第一个未完成任务；如果该任务过大，则先拆分任务并更新 `PLAN.md` 与 `TODO.md`，然后只执行拆分后的第一个子任务。

## 初始执行计划

1. 查看最新一次 Git 提交，确认提交信息里是否提到已有问题；如果提到，需要先修复这些问题。
2. 阅读 `TODO.md`，定位第一个未完成任务。
3. 阅读 `PLAN.md`、相关规范文档与受影响代码，评估任务范围和依赖关系。
4. 如果任务过大或存在前置缺陷：
   - 在 `PLAN.md` 中补充细化计划；
   - 在 `TODO.md` 中拆分或重排任务，只保留正确依赖顺序；
   - 本次只执行新的第一个子任务。
5. 实现本次目标任务，确保实现符合规范，不引入临时绕过方案。
6. 运行相关测试，并尽量执行完整质量检查，至少包括与改动相关的测试；如可行则执行 `cargo test --all`、`cargo clippy --all-targets -- -D warnings` 等。
7. 更新文档状态：
   - 在 `TODO.md` 中标记任务完成，或在受阻时按依赖关系调整顺序；
   - 在 `PLAN.md` 中记录当前状态、风险与后续任务。
8. 提交 Git，提交信息对应本次任务。
9. 停止，不继续处理下一个任务。

## 进度日志

- 已创建本计划文件，下一步将检查最新提交与任务列表。
- 已确认最新提交信息仅为 `[T2999] Add zero-warning baseline prerequisite`，未额外声明新的既有缺陷；当前首个未完成任务为 `T2999`。
- 已执行 `cargo check -p scoopc --message-format=short`，确认当前有 151 条 warning，主要集中在：
  - `crates/scoopc/src/llvm/codegen/effect/state_machine_plan.rs`
  - `crates/scoopc/src/llvm/codegen/effect/state_machine_segments.rs`
  - `crates/scoopc/src/llvm/codegen/effect/state_machine_transform.rs`
  - `crates/scoopc/src/llvm/codegen/runtime_abi.rs`
  - `crates/scoopc/src/llvm/codegen/runtime_symbols.rs`
  - `crates/scoopc/src/llvm/codegen/mod.rs`
- 当前处理策略：
  1. 删除明确无读取路径的死代码，例如未被消费的字段、未调用的方法、纯占位结构。
  2. 对尚未接回生产入口、但属于后续统一 state-machine / effect lowering 主线骨架的代码，建立模块级或方法级的可审计保留边界，避免散落式 `allow`。
  3. 修改后重新跑 `cargo check -p scoopc`，再跑 `cargo clippy --all-targets -- -D warnings` 进行最终验证。
- 已完成代码调整：
  - 删除未读取的 `CalleeSuspendSaveCtx` 及相关写入，删除未使用的 `entry_source` 等死代码。
  - 把统一 state-machine plan / segment / transform 骨架收口到单一共享作用域的保留边界。
  - 把 effect runtime ABI 声明与相关 runtime 符号的保留边界显式收口。
- 在执行 `cargo test --all` 时发现一个既有失败：
  - `llvm::tests::effect_runtime_intrinsics_are_emitted_as_symbol_calls`
  - 失败原因是 `scoop.core.__scoop_effect_*` sysroot 测试辅助 intrinsic 仍是占位报错，没有直接 lowering 到现有 runtime ABI。
- 已修复上述既有失败：
  - 为 `__scoop_effect_is_active` / `set_active` / `clear` / `slot_write` / `slot_write2` / `slot_read_*` 补齐直接 runtime 符号调用。
  - 扩展对应 LLVM IR 测试，覆盖单 word 与多 word perform slot 路径。
- 最终验证结果：
  - `cargo check -p scoopc --message-format=short` 通过且无 warning。
  - `cargo clippy --all-targets -- -D warnings` 通过。
  - `cargo test -p scoopc llvm::tests::effect_runtime_intrinsics_are_emitted_as_symbol_calls -- --exact` 通过。
  - `cargo test --all` 通过。
- 下一步：
  - 更新 Git 状态并提交本轮改动。
  - 本轮到此停止；下一次调用从 `T2999R` 开始。
