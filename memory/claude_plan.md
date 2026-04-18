# 执行计划与进度记录

## 说明

本文件记录本轮任务的可审阅执行计划、关键判断依据摘要、实施进度与结果。
不写入原始逐字思维链，但会持续更新足够详细的步骤、结论、阻塞点与变更原因。

## 当前目标

按 `TODO.md` 的顺序处理第一个未完成任务；如果在开始前发现最新提交提到的遗留问题，则先修复这些问题，再继续任务流。

## 初始执行计划

1. 查看最新一次 Git 提交的提交信息与差异，确认是否明确提到遗留问题、待修复项或已知缺陷。
2. 打开 `TODO.md`，定位第一个未完成任务。
3. 打开 `PLAN.md`，核对该任务是否已有既定方案、依赖或拆分说明。
4. 如首个未完成任务过大或依赖缺失：
   - 将任务拆分为更小的可执行子任务；
   - 更新 `PLAN.md`；
   - 调整 `TODO.md` 顺序与依赖，让新的第一个未完成子任务成为本轮目标；
   - 若因此无法直接实施原任务，则提交这些计划性变更并停止。
5. 如任务可直接实施：
   - 阅读相关代码与测试；
   - 实施改动；
   - 补充或调整测试；
   - 运行相关验证，再运行更全面的校验；
   - 修复验证中发现的问题直到通过或确认存在更底层阻塞。
6. 更新文档状态：
   - 在 `TODO.md` 中将本轮完成的任务标记为已完成；
   - 在 `PLAN.md` 中记录当前状态、后续影响与必要调整；
   - 持续更新本文件。
7. 使用清晰的 Git 提交信息提交本轮改动。
8. 停止，不继续处理下一个任务。

## 进度日志

- 已创建本文件并写入初始计划。
- 已检查最新提交、`TODO.md` 与 `PLAN.md`。
- 最新提交 `ff6c7f96ef380b0c9316d7ccf8c7eed1797f04fe` 为 `[T3016e] Fix nested handler arm outward propagation`，提交信息本身未声明新的未修复遗留问题。
- 已确认当前排在最前的未完成任务是 `T3016eR`，属于复审任务，可直接执行，无需进一步拆分。
- 当前复审范围：
  - `crates/scoopc/src/llvm/codegen/effect/state_machine_plan.rs`
  - `crates/scoopc/src/llvm/codegen/effect/state_machine_segments.rs`
  - `crates/scoopc/src/llvm/codegen/effect/state_machine_transform.rs`
  - 相关定向测试与 fixtures
- 当前执行策略：
  1. 阅读上一提交的实际 diff 与相关代码上下文。
  2. 审查新增 outward-suspend 判定是否只依赖统一 state-machine 合同，而非源码形状特判。
  3. 复现并运行与 `T3016e` 直接相关的定向测试。
  4. 若发现真实缺口，立即修复并补测；否则更新 `TODO.md` / `PLAN.md` / 本文件并提交 `T3016eR`。
- 已完成代码复审：
  - 审查了 `state_machine_plan.rs` 中 `HandleStateMachinePlan::may_suspend_outward()`、`arm_body_may_suspend_outward()` 与 immediate-resume arm tail 分析辅助函数。
  - 审查了 `state_machine_segments.rs` 中 `body_may_suspend_outward` 的 round-trip 投影，确认它只是元数据保留，不构成新的生产 fallback。
  - 审查了 `state_machine_transform.rs` 中新增结构测试，确认 outer boundary 的有无由统一状态机合同锁定。
- 已完成验证：
  - `cargo test -p scoopc nested_handle_ -- --nocapture`
  - `cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/effect_handler_stack_nearest_and_arm_outside_scope.scoop`
  - `cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/effect_custom_nonresuming_nested_nearest_and_arm_outside_scope.scoop`
  - `cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/effect_handler_stack_nearest_three_levels_and_arm_outside_scope.scoop`
  - `cargo test --all`
  - `cargo clippy --all-targets -- -D warnings`
  - `cargo run -p scoop --features llvm -- test`
- 关键结论：
  - 未发现新的生产缺口；`T3016e` 的修复没有回流为 fixture-only workaround，也没有把旧的 shape-based emitter 路由带回主线。
  - immediate-resume arm 的特例仍然只是既有 dedicated tail-resume 语义的一部分，用于排除“被当前 arm 自己消费的 resume 尾部”对 outward-suspend 判定的干扰。
  - 全量 LLVM fixture 总入口未暴露新的更早失败点；当前首个停止点仍是已在 `T3017` 中跟踪的 stale expectation `effect_escape_continuation_async_executor_fifo.scoop`（期望失败，但执行成功）。
- 当前状态：
  - 已更新 `TODO.md` / `PLAN.md` 将 `T3016eR` 标记为完成。
  - 下一步：整理工作区差异并提交本轮变更，然后停止。
