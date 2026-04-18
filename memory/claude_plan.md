# 执行计划记录

说明：该文件记录可审计的高层执行计划、关键判断依据与进度更新，不包含不可审计的内部推理细节。

## 初始计划

1. 检查最新一次 Git 提交的提交信息与变更范围，确认是否明确提到需要先修复的既有问题。
2. 查看当前工作区状态，避免覆盖用户已有修改。
3. 阅读 `TODO.md`，定位第一个未完成任务。
4. 阅读 `PLAN.md`，确认该任务的上下文、依赖与当前规划是否一致。
5. 判断该任务是否过大：
   - 若可在当前轮完整实现，则直接实现。
   - 若过大或存在前置依赖缺失，则先拆分任务并更新 `TODO.md` / `PLAN.md`，本轮只执行拆分后的第一个子任务。
6. 实现本轮目标任务，并在实现过程中同步识别任何与规范不符的真实缺陷。
7. 运行相关验证：
   - 最小必要测试；
   - 相关集成/回归测试；
   - 如改动影响构建质量，则运行 `cargo clippy --all-targets -- -D warnings` 与必要的 `cargo test`。
8. 更新文档与任务跟踪：
   - 在 `TODO.md` 中将本轮任务标记完成，或在阻塞时调整任务顺序并保留为待办；
   - 在 `PLAN.md` 中更新当前状态、依赖和后续安排；
   - 在本文件记录关键进展与计划变化。
9. 使用清晰的提交信息提交本轮所有改动，然后停止，不继续下一个任务。

## 进度

- 已创建本计划文件。
- 已检查最新提交、工作区状态、`TODO.md` 与 `PLAN.md`。
- 结论：最新提交未引入需要在 `T3015` 之前单独插队处理的新增生产缺陷；其提到的 LLVM fixture suite 首个停止点仍是已由 `T3017` 跟踪的 stale expectation。
- 已定位首个未完成任务为 `T3015`，且无需再拆分子任务。
- 已验证 `T3015` 的关键验收路径：
  - `effect_escape_continuation_arm_performs_outer_effect.scoop` 通过；
  - `effect_escape_continuation_nested_arm_indirect_performs_outer.scoop` 通过；
  - `effect_escape_continuation_scheduler_round_robin.scoop` 直接运行通过；
  - `cargo test -p scoop_runtime --test continuation_cross_thread_handler_stack -- --nocapture` 通过；
  - `effect_escape_continuation_resume_cross_thread.scoop` 直接运行通过。
- 当前判断：`T3015a` 已落地的 runtime handler-stack 快照与 dispatch-loop resume 入口已经把 `T3015` 的生产语义缺口实质收口；本轮不需要额外生产代码补丁，主要工作是完成任务状态回写与全量质量复验。
- 已确认 `cargo run -p scoop --features llvm -- test` 仍只停在 `tests/fixtures/run-pass/effect_escape_continuation_async_executor_fifo.scoop` 的 stale `EXPECT: fail`，该问题属于后续 `T3017`，不在本轮提前回收。
- 已更新 `TODO.md` / `PLAN.md`，将 `T3015` 标记为完成并把下一项推进到 `T3015R`。
- 已完成全量质量复验：
  - `cargo test --all` 通过；
  - `cargo clippy --all-targets -- -D warnings` 通过。
- 下一步：检查工作区差异，提交本轮变更并停止。
