# 当前执行计划

## 范围

- 按 `TODO.md` 的顺序只处理第一个标题未标记 `[DONE]` 的任务。
- 不做开放式历史问题排查；只处理当前任务及其直接阻塞项。
- 如果发现当前任务被真实前置问题阻塞，将在 `TODO.md` 中插入最小必要前置任务并停止。

## 步骤

1. 读取 `TODO.md`，定位第一个未完成任务，并记录其要求、依赖和验证命令。
2. 检查最新提交信息是否明确提到与该任务直接相关的未完成问题。
3. 只围绕当前任务阅读相关代码、测试和文档，确认实现边界。
4. 实现当前任务；编辑前优先采用小而集中的补丁。
5. 运行任务要求的测试和必要的相关验证；若失败，优先修复当前任务范围内的根因。
6. 更新 `TODO.md`，在任务标题前添加 `[DONE]` 并补全完成记录。
7. 仅当阶段计划发生实质变化时更新 `PLAN.md`。
8. 检查工作区差异，提交本次任务涉及的全部未提交文件。
9. 停止，不继续处理下一个任务。

## 进度记录

- 已创建初始计划；下一步读取 `TODO.md` 定位当前任务。
- 已定位第一个未完成任务：`P1-T05R：Review cone-level compilation unit API`。
- 最新提交为 `7b6f6440 [P1-T05] Add cone compilation unit facade`，直接对应本 review 任务；后续审查以该实现任务的完成条件和验证清单为范围。
- Review 发现并确认一个直接相关问题：多 cone production lowering 中部分 HIR stable metadata 仍使用单一 fallback `StableConeKey`。已开始修正，使 generic template/type-param key 计算按 source 所属 `SourceConeInfo.stable_key` 取值。
- 已修正 stable metadata 归属问题，并完成 P1-T05R 验证：格式检查、focused frontend/pipeline/project model tests、两个新增 ownership tests、全 workspace no-default tests、build fixtures、clippy `-D warnings` 均通过。
- 已更新 `TODO.md` 与 `TODO-2.md`，将 `P1-T05R` 标记为 `[DONE]` 并填写完成记录；未更新 `PLAN.md`，因为阶段级计划未改变。
