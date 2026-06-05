# 执行计划

说明：此文件记录可审计的执行计划、关键决策和进度；不记录不可公开的逐字内部推理。

## 当前状态

- 已识别首个未完成任务：`TC-04-R：Review TC-04`。
- 最新提交为 `56ccf4eb [TC-04] Use LIR handles in codegen lookups`，与本 review 任务直接相关。
- Review 发现仍有具体阻塞：生产路径存在剩余 FQN/root live callable 查找；`TODO.md` 已在 `TC-04-R` 前新增前置任务 `TC-04-FIX1`。

## 执行步骤

1. 读取 `TODO.md`，定位第一个标题未带 `[DONE]` 的任务。
2. 检查最新提交摘要，只纳入与当前任务直接相关的未完成事项。
3. 阅读当前任务正文、依赖、验收要求和相邻上下文。
4. 执行 `TC-04-R` 的静态审查 grep 与抽样确认。
5. 若发现具体阻塞，保持 `TC-04-R` 未完成，在 `TODO.md` 中新增最小前置修复任务并停止。
6. 本次只修改 markdown 任务/进度记录，不改变编译产物；按政策不运行完整代码验证套件。
7. 复查 diff，提交 `TODO.md` 与 `memory/claude_plan.md`，然后停止。

## 进度记录

- 已在运行仓库命令前写入初始执行计划。
- 已读取 `TODO.md`；选择 `TC-04-R`，因为它是第一个未带 `[DONE]` 的任务标题。
- 已检查最新提交；该提交是被 review 的 `TC-04` 实现，提交标题没有额外标明未完成问题。
- 已执行 `TC-04-R` 静态审查。基础 grep 对旧 helper 名称无命中，但更严格的生产路径检查发现 `.callable(callee_fqn)`、`callable_id_by_root`、`lir_callable_ref_for_root`、`exported_abi_symbol_for_lir_root`、root-based callable layout lookup 等剩余 live 查找。
- 已更新 `TODO.md`，新增前置任务 `TC-04-FIX1`，并保持 `TC-04-R` 未完成。
