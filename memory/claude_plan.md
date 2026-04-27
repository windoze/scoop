# 执行计划

## 当前约束

- 本轮只处理 `TODO.md` 中第一个未完成任务；完成后提交并停止。
- 在开始读取仓库、运行命令或修改代码之前，先记录本计划文件。
- 需要先检查最新提交是否提到已有问题；如有，优先修复这些问题。
- 任何发现的既有 bug、回归、规格不匹配、实现边界或临时绕过都必须优先处理；不能通过缩小范围或替代表示绕过。
- 若当前任务过大，需要先拆分到 `PLAN.md` / `TODO.md`，提交拆分结果后停止。

## 初始执行步骤

1. 检查仓库状态，确认已有未提交改动，避免覆盖用户改动。
2. 查看最新提交信息和变更内容，判断是否提到或遗留必须先处理的问题。
3. 阅读 `TODO.md`，定位第一个未完成任务。
4. 阅读相关的 `PLAN.md`、规格或源码上下文，明确任务边界和验收标准。
5. 如果发现任务依赖缺失功能或已有问题：
   - 精确定义问题；
   - 将前置修复任务插入 `TODO.md` 的正确位置；
   - 更新 `PLAN.md` 说明阻塞关系；
   - 提交文档调整并停止。
6. 如果任务可直接完成：
   - 实现最小但完整的规格正确变更；
   - 添加或更新针对性测试；
   - 运行相关测试，必要时运行更广范围测试；
   - 修复测试、编译、lint 中暴露的真实问题；
   - 更新 `TODO.md` 和 `PLAN.md`；
   - 提交代码和文档变更；
   - 停止，不继续下一个任务。

## 进度记录

- 已创建本执行计划文件，下一步将检查仓库状态、最新提交和 `TODO.md`。
- 已检查工作区状态：当前只有本轮新增/更新的 `memory/claude_plan.md` 未提交。
- 已检查最新提交 `753288c0d6bc1196732e7ae53e409425e43b8284`（`[T5000h0dR] Review canonical pass view output layer`）：
  - 该提交记录中提到并已修复 `MaterializedCallableFamilies::replace_family(...)` 的既有一致性缺陷；
  - 提交记录明确说明未发现需要插入到 `T5000h0e` 之前的新前置缺陷任务。
- 已读取 `TODO.md` 标题序列，确认第一个未完成任务是：
  - `T5000h0e 让 production LLVM codegen 真正消费 pass-rewritten callable body / summary，而不是只携带 materialized_pass_view`。
- 下一步执行计划：
  1. 阅读 `T5000h0e` 的任务详情和相邻完成记录；
  2. 梳理当前 `MaterializedMirPassView`、frontend/build 产物、LLVM emit/codegen 入口之间的数据流；
  3. 找出 production codegen 当前仍读取 raw materialized MIR / summary 的位置；
  4. 改为通过 pass view 获取 rewrite 后 callable body / summary；
  5. 增加或调整回归测试，证明 pass-rewritten body / summary 会被 production codegen 消费；
  6. 运行相关测试和 clippy；
  7. 更新 `TODO.md`、`PLAN.md` 与本文件，提交后停止。
- 已将 `T5000h0e` 拆分为两个子任务：
  - `T5000h0e1`：先让 production reachability / body-presence / effect summary 查询优先消费 pass view；
  - `T5000h0e2`：后续补齐 pass-rewritten MIR callable body 的 production LLVM lowering。
- 拆分理由：当前 pass view 已能承载 rewritten MIR body / summary，但 LLVM backend 尚无完整 MIR body lowering；本轮先完成可独立验证的 pass-backed codegen 查询面，避免继续只把 `materialized_pass_view` 当作未消费的参数。
- 当前执行目标改为完成 `T5000h0e1`，然后提交并停止。
- 已实现 `T5000h0e1` 的核心接线：
  - `llvm/reachability.rs` 对 pass-visible callable 优先扫描 `MaterializedMirPassView` 中的 MIR body；
  - `llvm/emit.rs` 的 reachable body 发射会尊重 pass view 中 callable body 是否仍存在；
  - `effect_state_machine_analysis.rs` 的 known fun outward-effect / suspendability cache 会优先读取 pass summary；
  - 新增定向 LLVM 回归，锁定 pass view 移除 reachable callable body 后不再静默按 HIR body 发射。
- 已运行 `cargo fmt --all`。
- 已运行定向测试 `cargo test -p scoopc production_codegen_body_emission_observes_pass_view_body_presence -- --nocapture`，结果通过。
- 运行全工作区测试时发现 `async_task_resume_replay_ir_terminates_step_fn_on_active_effect` 回归：
  - 原因是初版把 raw materialized summary 也用于替代 known fun suspendability；
  - raw summary 与现有 HIR/effect state-machine 分析不是完全等价，不能在没有 pass 显式改写时抢占该路径。
- 已修复为：effect/suspend cache 只消费 pass 显式 `set_instance_summary(...)` 覆盖过的 summary；未改写的初始 summary 继续走原有 HIR/effect 分析。
- 已新增并通过定向测试 `cargo test -p scoopc production_codegen_suspendability_observes_overridden_pass_summary -- --nocapture`。
- 已复跑失败测试 `cargo test -p scoopc llvm::codegen::effect::state_machine_emitter::tests::async_task_resume_replay_ir_terminates_step_fn_on_active_effect -- --nocapture`，结果通过。
- 已完成完整验证：
  - `cargo test -p scoopc llvm::tests -- --nocapture`：通过；
  - `cargo test -p scoopc --no-default-features`：通过；
  - `cargo test --all`：通过；
  - `cargo clippy --all-targets -- -D warnings`：通过；
  - `cargo run -p scoop -- test`：通过，`fixtures: ok (1201)`。
- 下一步：更新 `TODO.md` / `PLAN.md` 标记 `T5000h0e1` 完成，随后检查 diff 并提交。
