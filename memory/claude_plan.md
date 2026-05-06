执行计划

约束说明
- 不记录私密逐步推理；本文件记录可审计的执行计划、决策依据摘要和进度。
- 以 TODO.md 为任务顺序和完成状态的唯一来源；只完成第一个标题未带 [DONE] 的任务。
- 若遇到阻塞当前任务的规格不符、缺失语言特性、实现缺口或相关历史回归，优先修复；若无法在本次完成，则在 TODO.md 中插入最小必要前置任务并提交后停止。
- 不使用绕过、弱化测试、改变目标表示或 fixture-only hack。

步骤
1. 读取 TODO.md，定位第一个未完成任务及其依赖、验收要求和完成记录。
2. 查看最新提交摘要，只判断是否明确提到与当前任务直接相关的未完成问题；不做开放式历史问题排查。
3. 阅读当前任务相关代码、测试和规格资料，确认最小正确实现范围。
4. 实现当前任务；如果发现必须先修复的阻塞问题，更新 TODO.md 记录前置依赖并停止当前实现路径。
5. 添加或更新最小相关测试、fixture 或文档，确保行为符合任务要求和规格。
6. 运行相关验证；必要时运行更广泛的 cargo 测试或 clippy，修复验证中暴露的当前任务相关问题。
7. 将已完成任务标题加上 [DONE]，更新 TODO.md completion record；仅当阶段级计划变化时更新 PLAN.md。
8. 检查 git 状态和 diff，提交本次所有相关变更，提交信息引用任务编号。
9. 停止，不处理下一个任务。

进度
- 已创建初始执行计划。
- 已读取 TODO.md；第一个未完成任务为 MIR-T08R：Review MIR-T08 dispatch/resume/perform/handle contract。
- 最新提交为 `[MIR-T08] Close effect site MIR contracts`，与当前 review 直接相关，但提交摘要未声明未完成问题。
- 已审查 MIR-T08 相关 typed handoff、MIR lowering、strict verifier、placeholder inventory 和 fixtures 的主要实现面。
- MIR-T08R 指定验证、dump 抽查、placeholder 搜索审计、MIR-T08 完成记录中的定向回归和 clippy 均已通过。
- 已更新 TODO.md，将 MIR-T08R 标记为 [DONE] 并记录 review 结论；PLAN.md 无阶段级变化，不更新。
- 下一步提交 TODO.md 与本计划文件，然后停止。
