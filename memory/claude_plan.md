# 本轮执行计划

说明：按安全约束，这里不记录逐字内部思维链，但会完整记录可执行计划、关键判断、检查点、阻塞原因与后续变更，便于随时审阅进度。

## 初始目标

本轮只完成 `TODO.md` 中“第一个未完成任务”，完成后测试、更新文档、提交 git commit，然后停止。

## 执行步骤

1. 检查最新一次 git commit，确认是否提到任何已知遗留问题。
2. 如果最新 commit 提到遗留问题，先定位并修复这些问题，再继续后续任务流。
3. 读取 `TODO.md`，确定当前排在最前面的未完成任务。
4. 读取 `PLAN.md`、必要的规范/相关源码/测试，确认该任务的边界、依赖和当前实现状态。
5. 判断该任务是否过大或存在前置缺口：
   - 如果任务可直接完成，则进入实现。
   - 如果任务过大，则拆分为更小的子任务，更新 `PLAN.md` 与 `TODO.md`，并执行拆分后的第一个子任务。
   - 如果遇到规范缺口、实现边界、语言特性缺失或其它真实阻塞，则把阻塞修复项前置写入 `TODO.md`，更新 `PLAN.md`，提交后停止。
6. 实现当前目标任务，必要时同时补充/整理代码注释、模块组织和 README 相关缺失。
7. 运行相关验证：
   - 优先运行最小相关测试；
   - 再运行任务相关的更完整测试；
   - 按要求检查无警告构建/静态检查（包括 `cargo clippy --all-targets -- -D warnings`，若该检查与任务影响范围明显无关且仓库当前已有外部问题，会在此文件记录）。
8. 修复测试或检查中发现的问题，直到任务满足要求，或确认出现必须前置的新任务。
9. 更新文档状态：
   - 在 `TODO.md` 中标记任务完成，或在阻塞场景下按依赖顺序重排未完成任务；
   - 在 `PLAN.md` 中记录当前状态与计划调整；
   - 继续同步本文件中的进度说明。
10. 查看工作区变更，确认没有误改；然后创建一次清晰的 git commit。
11. 停止，不继续处理下一个任务。

## 当前状态

- 已检查最新 commit：最近一次提交为 `[T4016b4a0] Register module-level GC roots for object and top-level globals`，提交说明中未额外列出待先修复的遗留 issue。
- 已读取 `TODO.md` / `PLAN.md` / `README.md`。
- 已确认当前首个可执行的未完成任务是 `T4016b4b0`：
  - 父条目 `T4016`、`T4016b`、`T4016b4` 虽仍标为 `[TODO]`，但都明确标注为“拆分执行”的总括条目；
  - 在其已完成子任务之后，顺序上最前的未完成具体子任务是 `T4016b4b0`：修复/核查 GC stress 下 cross-thread escaped continuation resume 的 runtime 崩溃，并恢复 `T4016b4b` 的有效验收前提。
- 已完成 `T4016b4b0` 核查与收口：
  - 手动构建并执行 `tests/fixtures/run-pass/gc_continuation_multi_thread_concurrent_alloc_resume.scoop` 后，确认 `SCOOP_GC_STRESS=1` 下程序按 golden stdout 正常结束；
  - 说明原先记录的 “`workerA_resuming` 后异常退出” blocker 已随上一轮 `T4016b4a0` 的 GC roots 修复一并消失；
  - 本轮已把该 fixture 从历史失败占位恢复为真实 `run-pass` 回归：改为 `EXPECT: pass`，并在 fixture 内加入 `ENV: SCOOP_GC_STRESS=1`；
  - 已通过隔离的 `scoop test` fixture runner 子集、`cargo test --all` 与 `cargo clippy --all-targets -- -D warnings`。
- 下一步：整理工作区、提交本轮变更；本轮不会继续执行 `T4016b4b`。

## 进度更新

- 2026-04-21：创建本文件并写入初始执行计划。
- 2026-04-21：完成初始仓库检查；确认本轮目标任务为 `T4016b4b0`。
- 2026-04-21：确认 `gc_continuation_multi_thread_concurrent_alloc_resume.scoop` 已在 `SCOOP_GC_STRESS=1` 下恢复正确行为，并已把该用例重新纳入 stress-mode `run-pass` 回归。
