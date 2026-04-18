# 本轮执行计划

## 约束说明

- 按要求先记录执行计划，再进行仓库检查与实现。
- 这里记录的是可审计的执行计划、判断依据摘要和进度日志，不包含不可审计的内部推理细节。
- 本轮目标：先检查最新提交中提到的既有问题，再定位 `TODO.md` 中第一个未完成任务，只完成该一个任务并停止。

## 步骤计划

1. 检查最新一次提交信息，确认是否提到需要先修复的既有问题。
2. 阅读 `TODO.md`、`PLAN.md`，识别第一个未完成任务及其上下文依赖。
3. 评估该任务是否足够小且可在本轮完整完成。
4. 若任务过大，则把任务拆分为更小子任务，并更新 `TODO.md` 与 `PLAN.md`，本轮只执行拆分后的第一个子任务。
5. 实现当前目标任务所需修改。
6. 运行相关格式化、lint 与测试，修复出现的问题，直到相关检查通过。
7. 更新 `TODO.md`、`PLAN.md`、`memory/claude_plan.md` 记录完成情况。
8. 以清晰的提交信息提交本轮改动，然后停止。

## 进度日志

- 已创建本计划文件。
- 已检查最新提交 `820d347 [T3016a] Fix cleanup completion result transport`；提交正文未额外记录必须先修的既有 issue。
- 已读取 `TODO.md` / `PLAN.md`，确认第一个未完成任务为 `T3016aR`：复审 cleanup/finally completion 恢复合同。
- 当前执行策略：
  - 先复审 `crates/scoopc/src/llvm/codegen/effect/state_machine_emitter.rs` 中 `dispatch_check`、`CleanupEnter`、`completion_tag`、result slot relay 的生产路径。
  - 再运行 `T3016a` 相关 fixtures、定向 IR/结构测试、`cargo test --all` 与 `cargo clippy --all-targets -- -D warnings`。
  - 若未发现生产缺口，则把 `T3016aR` 标记完成并提交；若发现问题，则在本任务内修复并复验。
- 已完成生产代码复审：
  - `dispatch_check` 继续先检查 terminal `state_tag`，未把 stale TLS active 当成完成态的更高优先级信号。
  - `CleanupEnter` 继续通过 persisted `cleanup_flag` 决定是否重入 cleanup，cleanup done 路径仅通过 `completion_tag` 暂存并恢复 terminal tag。
  - `should_relay_last_value_through_goto()` 继续阻止 cleanup context 经由 transparent `Goto` 覆盖真实 handle result。
- 已完成验证：
  - fixtures：`effect_escape_continuation_finally_multi_perform.scoop`、`effect_resume_mixed_escape_direct_finally.scoop`、`effect_resume_mixed_source_path_matrix.scoop`、`effect_nosuspend_finally_nested_handle.scoop`、`effect_handle_tail_if_result.scoop`
  - 定向测试：`cleanup_enter_ir_checks_cleanup_flag_before_reentering_finally`、`dispatch_loop_ir_checks_terminal_state_before_tls_active`、`tail_if_else_result_flows_through_transparent_merge_state`
  - 全量检查：`cargo test --all`、`cargo clippy --all-targets -- -D warnings`
- 结论：本轮未发现新的生产缺口，`T3016aR` 可标记完成；下一项将推进到 `T3016b`。
- 已更新 `TODO.md`、`PLAN.md`，将 `T3016aR` 标记为完成，并把当前执行顺序推进到 `T3016b`。
