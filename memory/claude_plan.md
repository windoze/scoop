# 本次执行计划

## 目标

本次只处理 `TODO.md` 中第一个未完成任务；如果在检查或执行过程中发现已有缺陷、回归、规格不匹配或未完成边界，则先修复该问题，或把它作为前置任务插入 `TODO.md` 后停止。

## 决策摘要

- 先检查最新提交信息，确认是否提到任何已知问题；若有，优先处理。
- 再读取 `TODO.md`，定位第一个未完成任务。
- 若该任务过大，则先拆分任务，更新 `PLAN.md` 与 `TODO.md`，本次只执行拆出的第一个子任务。
- 执行实现时，不接受绕过实现缺陷的变通方案；若遇到规格缺口或实现边界，必须先修复或登记为前置任务。
- 完成后必须执行相关测试，并尽量覆盖格式化、lint 与相关回归验证。
- 最后更新 `TODO.md`、`PLAN.md`、本文件，并提交一次 git commit，然后停止。

## 步骤计划

1. 查看最新一次提交，检查提交说明是否提到待修复问题。
2. 读取 `TODO.md` 与 `PLAN.md`，识别首个未完成任务及其上下文。
3. 评估任务规模与依赖：
   - 若可直接完成，则进入实现。
   - 若过大，则拆分为更小子任务，并先更新计划文件。
4. 阅读相关代码、测试与规范，确认当前实现状态。
5. 实现首个目标任务或必要的前置修复。
6. 运行相关测试；若暴露既有问题，立即修复并补充验证。
7. 更新 `TODO.md`、`PLAN.md`、本文件中的进展记录。
8. 使用清晰的提交信息完成 git commit。
9. 停止，不继续处理下一个任务。

## 进展记录

- 已检查最新提交信息，提交标题未声明需要先修复的额外遗留问题。
- 已读取 `TODO.md` / `PLAN.md`，确认首个未完成任务为 `T5000d2R Review：确认动态分派与 Resume 已成为 MIR 一等节点`。
- 已完成代码审查，重点复核了 `crates/scoopc/src/mir/mod.rs`、`crates/scoopc/src/mir/lower.rs`、`crates/scoopc/src/hir/lower/mod.rs`、`crates/scoopc/src/monomorph/lower.rs` 以及相关 MIR fixture / 单测。
- review 过程中发现一个既有覆盖缺口：MIR 只回归了 Pure continuation 的 `ResumeMetadata.suspends_outward = false`，未覆盖 non-Pure continuation 的 `suspends_outward = true`。
- 已修复该缺口：扩展 `tests/fixtures/mir/dispatch_and_resume_call.scoop`，新增 `resumeBoom` 场景，并同步更新 `tests/fixtures/mir/dispatch_and_resume_call.mir` golden。
- 已完成验证：
  - `cargo run -p scoop -- test --fixtures tests/fixtures/mir`
  - `cargo test -p scoopc monomorph::lower -- --nocapture`
  - `cargo test --all`
  - `cargo clippy --all-targets -- -D warnings`
  - 以上全部通过。
- 下一步：更新 `TODO.md` / `PLAN.md` 完成记录，检查 diff，提交 git commit，然后停止。
