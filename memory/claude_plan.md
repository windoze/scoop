# 工作记录 / 执行计划

## 说明

按要求，我会先记录一份可审阅的执行计划，再开始实际检查仓库状态、最新提交和 `TODO.md`。这里记录的是可共享的工作思路摘要与执行步骤，不包含不可审阅的内部推理细节。后续如果计划变化，或完成了关键步骤，我会继续更新此文件。

## 初始计划

1. 检查最新一次 Git 提交，确认提交信息、改动范围，以及是否显式提到需要先修复的遗留问题。
2. 阅读 `TODO.md`，找出第一个未完成任务。
3. 阅读 `PLAN.md`，对照当前任务确认是否已经有分解计划；如果首个未完成任务过大，则先把任务拆分为更小的子任务，并同步更新 `PLAN.md` 与 `TODO.md`。
4. 阅读与当前任务直接相关的代码、测试和文档，确认现状、约束和预期行为。
5. 实现当前应执行的首个任务或子任务。
6. 运行相关测试、格式化、lint（至少包含 `cargo clippy --all-targets -- -D warnings`，以及与改动相关的测试命令）；如果发现问题，立即修复。
7. 更新 `TODO.md` 与 `PLAN.md`，记录完成状态和后续顺序。
8. 检查工作区改动，确保没有误改；然后提交一个只覆盖本轮任务的 Git commit。
9. 停止，不继续处理下一个任务。

## 当前状态

- 已检查最新 Git 提交：`70635239dbaecd5f8f0a63c0ecb6e303f0577316`（`[T0147c-1] Migrate LLVM pointer APIs to opaque pointers`）。
- 该提交信息未显式提出新的“必须先修复”的遗留缺陷；但提交说明与 TODO 一致，表明当前严格 `clippy` gate 仍被后续子任务阻塞。
- 已定位 `TODO.md` 中首个未完成任务：`T0147c-2 [TODO] Clippy 基线清理：收缩超长 helper 参数列表（too_many_arguments）`。
- 已运行 `cargo clippy --workspace --all-targets --message-format short -- -D warnings`，确认当前共有 **78 个** `too_many_arguments`，分布在 lowering / resolve / typecheck / LLVM 多个模块。
- 基于实际规模，已将原任务拆分为 4 个子任务，并已同步更新 `TODO.md` / `PLAN.md`：
  1. `T0147c-2a`：lowering / resolve / LLVM（13 个）
  2. `T0147c-2b`：typecheck 支撑模块（12 个）
  3. `T0147c-2c`：typecheck expr 中等规模模块（17 个）
  4. `T0147c-2d`：typecheck expr 主干（36 个）
- 本轮执行目标 `T0147c-2a` 已完成，并已在 `TODO.md` / `PLAN.md` 标记为 DONE。
- clippy 复核结果：`too_many_arguments` 总数已从 **78** 降到 **65**；本轮负责的 13 个告警全部消除。
- 测试结果：
  - `cargo fmt --all`：通过
  - `cargo check -p scoopc --features llvm`：通过
  - `cargo test --all`：通过
  - `cargo run -p scoop -- test`：通过（`fixtures: ok (852)`）

## 下一步

1. 检查工作区 diff，确认只包含本轮子任务与计划文件更新。
2. 提交本轮改动，提交信息使用 `T0147c-2a`。
3. 停止，等待下一轮处理 `T0147c-2b`。
