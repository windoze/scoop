## 2026-04-26 当前执行计划

本文件记录本轮执行计划与进度。按系统安全要求，这里提供可审计的执行摘要与步骤，不记录逐字内部推理。

### 目标

完成 `TODO.md` 中首个未完成任务 `T5000d2` 的收尾工作，并在确认测试与 lint 通过后提交；本轮不继续处理后续任务。

### 已知上下文

- 前一轮实现已基本完成：MIR 已显式表达 `Virtual` / `Interface` / `Resume` 调用，相关 lowering、typed HIR dump、monomorph 路径和 fixtures 已更新。
- 已成功跑过：
  - `cargo fmt --all`
  - `cargo run -p scoop -- test --fixtures tests/fixtures/mir`
  - `cargo test -p scoopc monomorph::lower`
  - `cargo test -p scoop --test t1124_incremental_cone_run -- --nocapture`
  - `cargo test --all`
- 尚未完成：
  - 顺序重跑 `cargo clippy --all-targets -- -D warnings`
  - 将最终状态回写到 `TODO.md`、`PLAN.md`、本文件
  - 提交 git commit

### 执行步骤

1. 查看最新提交信息，确认是否提到需要先修复的既有问题。
2. 检查当前工作区状态，确认前一轮修改是否仍在。
3. 顺序运行 `cargo clippy --all-targets -- -D warnings`。
4. 如果 `clippy` 报错：
   - 修复问题；
   - 重新运行 `cargo fmt --all`、`cargo test --all`、`cargo clippy --all-targets -- -D warnings`。
5. 如果 `clippy` 通过：
   - 检查 `TODO.md` 首个未完成任务仍为 `T5000d2`；
   - 更新 `TODO.md` 为已完成；
   - 更新 `PLAN.md` 记录完成情况与下一个任务；
   - 更新本文件记录完成状态。
6. 使用与任务一致的提交信息提交本轮改动，然后停止。

### 风险与约束

- 所有 `cargo` 命令必须顺序执行，避免并行污染 `t1124_incremental_cone_run` 的缓存命中断言。
- 若在验证过程中发现既有 bug / 规格不匹配，必须先修复或把前置任务插入 `TODO.md` 后停止，不能绕过。

### 进度

- [x] 初始计划写入
- [x] 查看最新提交
- [x] 顺序运行 clippy
- [x] 回写 TODO / PLAN / memory
- [ ] 提交并停止

### 执行记录

- 已检查最新提交：`[T5000d1R] Review explicit MIR call kinds`，提交标题与正文均未提到需要优先处理的既有缺陷。
- 已确认 `TODO.md` 首个未完成任务仍为 `T5000d2`，因此本轮继续对该任务做收尾验证与回写。
- 已顺序运行 `cargo clippy --all-targets -- -D warnings`，通过。
- 已将 `T5000d2` 的完成记录写回 `TODO.md` 与 `PLAN.md`，并记录了本轮中途修复的两个真实阻塞点：
  - dump 路径缺少 typed facts，导致 `Virtual` / `Interface` / `Resume` 无法稳定进入 MIR dump / monomorph 路径；
  - `HirLowerError` 过大触发 `clippy::result_large_err`。

### 当前状态

- 剩余动作只有一次提交：
  - 计划提交信息：`[T5000d2] Lower Virtual/Interface/Resume into MIR`
  - 提交后立即停止，不继续处理 `T5000d2R`
