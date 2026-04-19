# 执行计划记录

## 说明

按用户要求，本文件在任何命令执行前创建，并持续记录本次工作的执行思路摘要、计划、关键进展和必要调整。

出于安全与协作边界，这里记录的是可审计的推理摘要与执行计划，不写入模型的逐字内部思维。

## 当前目标

完成 `TODO.md` 中第一个未完成任务；如果存在阻塞或前置缺陷，则先修复阻塞项或按要求重排任务并提交，然后停止。

## 初始约束与检查顺序

1. 查看最新提交，确认是否提到已知遗留问题；如有，先修复这些问题。
2. 阅读 `TODO.md`，定位第一个未完成任务。
3. 如该任务过大，拆分为可执行子任务，并同步更新 `PLAN.md` 与 `TODO.md`。
4. 实现当前应执行的第一个任务或子任务。
5. 运行相关测试、格式化、lint，确保无警告。
6. 更新 `TODO.md`、`PLAN.md` 与本文件，记录完成情况或阻塞原因。
7. 提交本次变更，并停止，不继续下一个任务。

## 预期执行步骤

### 第 0 阶段：仓库状态与上下文

- 检查最新提交信息，确认是否显式提到遗留问题。
- 读取 `TODO.md`、`PLAN.md`，必要时补充依赖关系。
- 查看工作区状态，避免误覆盖已有改动。

### 第 1 阶段：任务判定

- 确认首个未完成任务的边界、涉及模块和预期行为。
- 判断是否存在规格缺口、实现缺陷或缺失能力会阻塞任务。
- 若阻塞：
  - 在 `TODO.md` 前置插入修复任务；
  - 调整原任务依赖与排序；
  - 更新 `PLAN.md` 和本文件；
  - 提交并停止。

### 第 2 阶段：实现

- 在相关模块中完成最小但完整的规范实现，不引入临时绕过方案。
- 如涉及较大文件，优先进行模块化整理，避免继续堆积复杂度。
- 为关键函数和模块补充简洁注释，说明职责与行为。

### 第 3 阶段：验证

- 运行与改动直接相关的测试。
- 运行必要的全局检查，至少包括：
  - `cargo fmt --check` 或等效格式化
  - `cargo test --all`
  - `cargo clippy --all-targets -- -D warnings`
- 若任务与夹具/规范同步有关，再运行对应 fixture/spec 检查命令。

### 第 4 阶段：收尾

- 将已完成任务在 `TODO.md` 中标记完成。
- 更新 `PLAN.md` 的当前状态、后续顺序与依赖。
- 回写本文件，记录关键实现与验证结果。
- 生成单次提交，提交信息与任务编号对应。

## 当前状态

- 已完成：创建本计划文件。
- 已完成：检查最新提交信息，未发现提交说明中显式要求优先修复的遗留问题。
- 已完成：读取 `TODO.md` / `PLAN.md` / `ISSUES.md`，确认当前首个未完成任务为 `T4009a3`。
- 已完成：审查 `Task` 相关残留实现，确认 runtime 主线当前只实际保留：
  - `scoop_task_create`
  - `scoop_task_from_result`
  - `scoop_task_join`
- 已确认的本轮核心工作：
  - 删除 LLVM codegen 中对 `scoop.task.executorCreate`、`scoop.task.destroy`、`scoop.task.debugPendingCount`、`scoop.task.runNext`、`scoop.task.runUntilIdle`、`scoop.task.taskCreateManual`、`scoop.task.state`、`scoop.task.result`、`scoop.task.tryStart`、`scoop.task.complete`、`scoop.task.onComplete` 的遗留硬编码分支；
  - 删除仅服务上述旧 surface 的 runtime symbol 常量与 ABI 声明；
  - 保留当前 `Task` core 仍需要的内部 helper：`__scoop_task_create`、`__scoop_task_from_result`、`__scoop_task_join`。
- 已完成：修改 compiler 端残留 special-case。
  - `crates/scoopc/src/llvm/codegen/mod.rs` 已删除所有公开 `scoop.task.*` / `Executor` FQN 分支与对应 helper。
  - `crates/scoopc/src/llvm/codegen/runtime_symbols.rs` / `runtime_abi.rs` 已收缩到仅保留 `scoop_task_create`、`scoop_task_from_result`、`scoop_task_join`。
- 已完成：验证改动。
  - `cargo fmt --check`
  - `cargo test -q -p scoopc async_task_ir_uses_task_create_and_internal_join -- --nocapture`
  - `cargo test -q -p scoop_runtime --test task_spawn_join`
  - `cargo run -q -p scoop -- test --fixtures tests/fixtures/run-pass` -> `fixtures: ok (357)`
  - `cargo run -q -p scoop -- test` -> `fixtures: ok (1070)`
  - `cargo test --all`
  - `cargo clippy --all-targets -- -D warnings`
- 已完成：同步更新 `TODO.md`、`PLAN.md`、`ISSUES.md`，将当前下一项推进到 `T4009b`。
- 已完成：`git diff --check` 与变更复核，当前修改集仅包含：
  - `crates/scoopc/src/llvm/codegen/mod.rs`
  - `crates/scoopc/src/llvm/codegen/runtime_symbols.rs`
  - `crates/scoopc/src/llvm/codegen/runtime_abi.rs`
  - `TODO.md`
  - `PLAN.md`
  - `ISSUES.md`
  - `memory/claude_plan.md`
- 进行中：创建 `T4009a3` 提交，并在提交后停止。
