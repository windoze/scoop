# 当前执行计划

说明：按要求记录可审计的执行计划、关键决策摘要、进度与计划变更；不记录完整内部推理细节。

## 目标

本轮只完成 `TODO.md` 中第一个未完成任务，然后停止。

## 初始步骤

1. 检查最新一次 Git 提交的信息，确认是否提到了需要先处理的既有问题。
2. 阅读 `TODO.md`，识别第一个未完成任务。
3. 阅读 `PLAN.md`，核对当前计划与任务依赖。
4. 如果首个未完成任务过大，则将其拆分为更小子任务，并同步更新 `PLAN.md` 与 `TODO.md`；本轮只执行拆分后的第一个子任务。
5. 在实现前阅读相关代码、测试与规范，确认是否存在会阻塞任务的既有缺陷或规格不匹配。
6. 实现任务，补充或调整测试，运行相关验证。
7. 更新 `TODO.md` 与 `PLAN.md` 的完成状态和后续计划。
8. 提交本轮改动，提交信息采用仓库约定格式。

## 执行约束

- 若发现最新提交提到的既有问题，必须优先修复。
- 若发现规格不匹配、缺失语言特性或必须依赖前置修复，不能绕过；必须先更新 `TODO.md` / `PLAN.md` 反映依赖，再提交并停止。
- 尽量运行与改动直接相关的测试；若任务范围较大，再补充更广泛验证。
- 目标是本轮只完成一个任务，不推进到下一个任务。

## 进度记录

- 已创建本计划文件。
- 已检查最新提交 `59c79e3ed0dbaed0338dc308b457f74232f02340`（`Update plan`），提交信息未额外提到需要先修复的既有代码问题。
- 已读取 `TODO.md` 与 `PLAN.md`。
- 已确认本轮首个未完成任务为 `T2999R`：Review 零 warning 基线恢复，确认没有用允许属性掩盖真正实现缺口。
- 下一步：审查 `crates/scoopc/src/llvm/codegen/**` 中 effect/LLVM 相关生产代码，重点检查 `allow` 边界、保留骨架与是否存在应删除而未删除的死代码。
- 审查中已发现并开始修复的问题：
  - `crates/scoopc/src/llvm/codegen/runtime_symbols.rs` 中散落的大量 `#[allow(dead_code)]` 为冗余允许项，不符合“共享边界、可审计”的目标。
  - `crates/scoopc/src/llvm/codegen/effect/state_machine_plan.rs` 与 `state_machine_transform.rs` 已处于 `effect/mod.rs` 的统一 `#[allow(dead_code)]` 骨架作用域内，文件内多个局部允许项属于重复豁免。
  - `crates/scoopc/src/llvm/codegen/runtime_abi.rs` 里 `declare_runtime_alloc` / `declare_runtime_gc_collect` 没有生产调用点，也不属于当前统一 effect 合同，应直接删除而不是继续靠 `allow(dead_code)` 保留。
- 已执行修复：
  - 删除 `runtime_symbols.rs` 中散落的冗余 `#[allow(dead_code)]`。
  - 删除 `state_machine_plan.rs` / `state_machine_transform.rs` 中被统一骨架边界覆盖的重复 `dead_code` 允许项。
  - 删除 `runtime_abi.rs` 中无调用点的 `declare_runtime_alloc` / `declare_runtime_gc_collect` 及对应符号常量，并移除 effect ABI 共享 impl 内的重复局部允许项。
- 已完成验证：
  - `cargo check -p scoopc` 通过。
  - `cargo clippy --all-targets -- -D warnings` 通过。
  - `cargo test --all` 通过。
- 已完成文档同步：
  - `TODO.md` 已将 `T2999R` 标记为 `[DONE]`，并写入审查结论与修复项。
  - `PLAN.md` 已记录 `T2999R` 的完成结果，并把当前执行顺序推进到 `T3001`。
- 下一步：检查工作区差异，提交本轮改动，然后停止。
