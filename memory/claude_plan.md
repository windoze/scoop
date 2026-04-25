# 执行计划与进度记录

## 说明

- 按要求，本文件在开始任何仓库探查或命令执行前创建。
- 我不会记录私有逐字思维链，但会记录可审阅的执行步骤、判断依据、发现的问题、计划变更与完成状态。
- 本次调用的目标是：先处理最近一次提交中提到的既有问题（如果有），然后完成 `TODO.md` 中第一个未完成任务，完成测试、更新文档与计划、提交 git commit，随后停止。

## 初始执行计划

1. 查看最新一次 git 提交信息，确认是否显式提到需要先修复的既有问题。
2. 阅读 `TODO.md`，定位第一个未完成任务。
3. 阅读 `PLAN.md`，理解该任务上下文、依赖关系与现有分解情况。
4. 结合任务与最近提交说明，评估是否存在阻塞性既有问题、规格不匹配、未完成实现边界或回避式实现。
5. 如果当前首个未完成任务过大，则先把它拆分为更小子任务，并同步更新 `PLAN.md` 与 `TODO.md`；本次只执行拆分后排在最前面的那个子任务。
6. 实现当前应执行的任务，不采用规避性方案；若遇到真正阻塞问题，则把该问题以前置任务形式加入 `TODO.md`，更新 `PLAN.md` 说明依赖关系，然后停止。
7. 运行与改动相关的验证：
   - 至少运行最小相关测试；
   - 若改动影响面较大，运行更完整的测试；
   - 按要求检查质量门槛，包括 `cargo clippy --all-targets -- -D warnings`，除非与本次改动完全无关且执行成本不合理时再在记录中说明。
8. 更新进度文档：
   - 在 `TODO.md` 标记已完成任务；
   - 在 `PLAN.md` 反映当前状态、后续依赖和必要说明；
   - 继续更新本文件记录关键进展与变更。
9. 检查 git diff，确保只包含本次合理修改。
10. 使用清晰提交信息提交本次变更，然后停止，不继续下一个任务。

## 进度日志

- 2026-04-25：已创建本文件并写入初始计划，下一步将检查最新提交说明与任务列表。
- 2026-04-25：已检查最新提交 `e90de94a7373c1451449e4eb4374cbd1b98d756c`，提交标题为 `[T5000b1] Split llvm mod implementation modules`，提交信息中未显式提到需要先修复的既有问题。
- 2026-04-25：已读取 `TODO.md` 与 `PLAN.md`，当前首个未完成任务为 `T5000b1R Review：确认 llvm/mod.rs 已收口为根模块而非实现巨型文件`。
- 2026-04-25：当前执行策略已细化为：
  1. 审查 `crates/scoopc/src/llvm/mod.rs` 的职责是否仅剩根模块边界；
  2. 审查 `emit.rs`、`pipeline.rs`、`reachability.rs`、`tests.rs` 与调用点，确认主体实现确已迁出；
  3. 若发现边界未收口或存在回归，则先修复并补测；
  4. 若 review 通过，则更新 `TODO.md` / `PLAN.md` / 本文件并提交。
- 2026-04-25：已完成 `T5000b1R` 的结构审查。审查结论：
  - `crates/scoopc/src/llvm/mod.rs` 只剩子模块声明、对外 re-export、测试期窄桥接 re-export、LLVM GC 策略常量、一次性全局 LLVM 选项配置与统一错误诊断边界；
  - `emit.rs` 承载 emit API 与 module build；`pipeline.rs` 承载 pass pipeline；`reachability.rs` 承载 HIR 扫描；`tests.rs` 承载根模块测试主体；
  - `llvm/codegen/effect/state_machine_emitter.rs` 的测试仅通过 `#[cfg(test)]` 下的内部 helper re-export 访问构建与 pipeline 入口，没有把实现职责倒灌回根模块；
  - 未发现需要在 `T5000b2` 之前插入的新前置缺陷任务。
- 2026-04-25：已完成验证：
  - `cargo test -p scoopc llvm::`
  - `cargo clippy --all-targets -- -D warnings`
  - 结果：全部通过。
- 2026-04-25：已更新 `TODO.md` 与 `PLAN.md`，将 `T5000b1R` 标记完成，下一条任务切换为 `T5000b2 提炼 MainCodegen 共享编译单元上下文与 child-codegen 构造路径`。
