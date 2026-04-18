# 当前执行计划

## 约束说明

- 按要求先记录计划，再进行任何仓库检查或命令执行。
- 这里记录的是可审阅的执行方案与决策摘要，不包含不可审计的内部推理展开。
- 本次调用只处理 `TODO.md` 中第一个未完成任务；若遇到阻塞，则按要求改写 `TODO.md` / `PLAN.md` 后提交并停止。

## 初始步骤

1. 检查最新一次 Git 提交，确认提交说明中是否提到尚未修复的问题；若有，则优先修复这些既有问题。
2. 阅读 `TODO.md`，定位第一个未完成任务。
3. 阅读 `PLAN.md` 与相关上下文，确认该任务的依赖、范围和验收标准。
4. 如果该任务过大，则将其拆分为更小的子任务，并同步更新 `PLAN.md` 与 `TODO.md`，随后执行拆分后的第一个子任务。

## 当前进展（进行中）

- 已检查最新提交 `e5eb91e [T3016a0] Repair tail-merge result transport` 的提交正文；正文未额外列出需要先插队修复的既有问题。
- 已定位当前首个未完成任务为 `T3016a0R`：复审 no-suspend tail merge result transport 是否真正回到统一 completion/result 合同。
- 已完成定向审查：`state_preserves_handle_result_on_entry()` 仅基于 unified state machine 的 state/terminator 元数据递归判断透明 relay，允许继续传递结果的 state 也仅限 marker/no-op（`StmtEmpty` / `CleanupEdgeComplete` / `ReturnToEnclosingExpression`），未重新引入源码形状分流。
- 已完成定向验证：
  - `cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/effect_handle_tail_if_result.scoop`
  - `cargo test -p scoopc tail_if_else_result_flows_through_transparent_merge_state -- --nocapture`
  - `cargo test -p scoopc cleanup_enter_ir_checks_cleanup_flag_before_reentering_finally -- --nocapture`
  - `cargo test -p scoopc dispatch_loop_ir_checks_terminal_state_before_tls_active -- --nocapture`
  - `cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/effect_nosuspend_finally_nested_handle.scoop`
  - `cargo test --all`
  - `cargo clippy --all-targets -- -D warnings`
- 审查结论：`T3016a0` 的 tail-merge result transport 修复已闭环，且未冲掉 cleanup/finally completion 的后续修复前提；`effect_nosuspend_finally_nested_handle.scoop` 仍输出 `0/0`，说明剩余缺口继续属于后续 `T3016a`。
- 下一步：把 review 结论写回 `TODO.md` / `PLAN.md`，提交本轮变更并停止。

## 执行策略

1. 先在不破坏现有未提交修改的前提下检查工作区状态。
2. 阅读与当前任务直接相关的源码、测试、规范或设计文档。
3. 实现任务所需修改，必要时补充或调整测试。
4. 运行与改动相关的验证命令；若任务涉及通用质量门禁，则补充运行格式化、测试和 `clippy` 检查。
5. 若发现任何规范不匹配、缺失特性或现存缺陷阻塞当前任务：
   - 将该问题提升为新的前置任务写入 `TODO.md`；
   - 在 `PLAN.md` 记录阻塞原因和依赖关系；
   - 提交这些计划性变更并停止。

## 收尾步骤

1. 完成后将当前任务在 `TODO.md` 中标记为已完成。
2. 更新 `PLAN.md` 反映实际完成情况和后续顺序。
3. 复查 `memory/claude_plan.md`，补充已完成的关键步骤和任何计划调整。
4. 使用清晰的 Git 提交信息提交本次变更。
5. 停止，不继续处理后续任务。
