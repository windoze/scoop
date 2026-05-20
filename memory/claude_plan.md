执行计划摘要

1. 读取 TODO.md，按文件顺序识别第一个标题未带 `[DONE]` 的任务。
2. 查看最近提交信息，仅判断是否有与该任务直接相关的未完成事项。
3. 阅读当前任务相关的说明、依赖、验证要求和必要代码区域。
4. 按任务要求实现最小但完整的变更；若发现阻塞该任务的规格或实现缺口，先把最小前置任务写入 TODO.md 并停止。
5. 运行任务要求的验证命令和必要的针对性测试，修复由本任务引入或暴露且阻塞任务完成的问题。
6. 更新 TODO.md：在完成任务标题前加 `[DONE]`，并填写完成记录；仅在阶段计划实际变化时更新 PLAN.md。
7. 检查工作区差异，提交本次任务相关的全部变更。
8. 完成一个任务后停止，不继续处理下一个任务。

当前状态：计划已初始化，下一步读取 TODO.md 并识别第一个未完成任务。

进度更新：已识别第一个未完成任务为 TODO-2-INIT。最近提交为 `[P0-T04R] Review P0 cleanup completion`，未显示当前任务的直接未完成事项。下一步读取 TODO-2.md、PLAN.md 与相关设计/审计文档，提取 P1 任务拆分依据。

进度更新：已完成 P1 资料和代码触碰面抽查。关键现状：`span`、`source`、`ty`、`stable_id`、`cone`、`frontend` 仍在 `scoopc` 内；`SourceConeGraph` 已有拓扑排序和 cone metadata，但 `run_frontend` 仍 flatten 整个 graph；`AstStageOutput` 仍是单文件 handoff；`ConeId`/`ConeInfo` 仍位于 resolver。下一步编辑 TODO-2.md 和 TODO.md，生成 P1 实现任务及 review 任务。

进度更新：已将 TODO-2.md 细化为 6 个 P1 实现任务和 6 个 review 任务，并在 TODO.md 索引中把 TODO-2-INIT 标记为完成、插入 P1-T01 到 P1-T06R。下一步复读文档变更并运行文档级校验。

进度更新：已复读 TODO-2.md / TODO.md 变更并运行 `git diff --check`，未发现空白错误。下一步执行提交前的 git 状态、diff 与最近提交检查，然后提交本次 TODO-2-INIT 文档变更。
