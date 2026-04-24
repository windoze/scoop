# 执行计划与决策摘要

## 约束说明

- 按要求先写入本文件，再执行其他命令或代码。
- 这里记录的是可共享的执行计划、判断依据摘要、进度与变更说明，不包含不可直接共享的内部推理细节。
- 本次调用只处理一个任务：先检查最新提交是否提到需要先修复的既有问题；若无，则读取 `TODO.md`，定位第一个未完成任务并执行；完成后测试、更新 `TODO.md`/`PLAN.md`、提交 git commit，然后停止。

## 初始步骤

1. 查看最新一次 git 提交信息，确认是否明确提到已有问题需要优先修复。
2. 读取 `TODO.md` 与 `PLAN.md`，识别第一个未完成任务，并判断是否需要拆分为更小子任务。
3. 如发现阻塞当前任务的既有缺陷、规格不匹配或实现边界缺失：
   - 先修复该问题；若本轮无法直接修复，则把它作为前置任务插入 `TODO.md` 当前任务之前；
   - 同步更新 `PLAN.md` 说明原因；
   - 提交后停止。
4. 若当前任务可直接完成：
   - 实现代码；
   - 运行相关测试、`cargo fmt`、`cargo clippy --all-targets -- -D warnings`，以及必要的专项测试；
   - 更新 `TODO.md` 和 `PLAN.md`；
   - 提交后停止。

## 进度记录

- [x] 创建本计划文件。
- [x] 检查最新提交是否提及待修复既有问题。
- [x] 读取 `TODO.md` 并定位第一个未完成任务。
- [x] 确认当前目标为 `T4016T4R`，暂不需要进一步拆分。
- [x] 实现并验证当前任务。
- [ ] 更新任务文档并提交。

## 变更日志

- 2026-04-24：初始化计划文件。
- 2026-04-24：确认最新提交未额外声明新的待修既有问题；定位首个未完成任务为 `T4016T4R`，将按“代码/文档复扫 + 定向回归 + 全量验收 + 文档状态更新 + 提交”执行。
- 2026-04-24：完成 `T4016T4R` review。结论是 core `Task` 的无锁、轻量 claim、single-driver 合同已与实现/文档/回归一致；未发现需要前插的新 blocker。已复验：
  - `cargo test -p scoopc --features llvm task_step_ir_uses_seqcst_atomic_claim_and_trap_without_mutex`
  - `cargo run -p scoop -- test --fixtures tests/fixtures/build`
  - `cargo run -p scoop -- test --fixtures tests/fixtures/runtime_gc`
  - `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass`
  - `cargo run -p scoop -- test`
  - `cargo run -p scoop_tools -- spec-fixtures check`
  - `cargo test --all`
  - `cargo clippy --all-targets -- -D warnings`
