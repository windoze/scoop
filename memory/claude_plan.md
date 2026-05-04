# 执行计划

更新时间：2026-05-05

## 当前目标

完成 `TODO.md` 索引指向的第一个未完成详细任务；完成后更新任务记录、运行相关验证、提交 Git commit，并停止。

## 步骤

1. 读取 `TODO.md` 作为索引，不把它当作任务详情来源。
2. 按索引顺序读取对应的 `TODO-Px.md` 详细任务文件，找到第一个标题未以 `[DONE]` 开头的任务。
3. 检查该任务的详细要求、依赖、验证命令和完成记录；必要时查看最新提交是否提到与当前任务直接相关的未完成问题。
4. 只围绕当前任务建立代码上下文，避免开放式历史问题排查。
5. 若任务可直接完成，实施最小正确改动，并补充或更新相关测试/fixture。
6. 若发现阻塞当前任务的真实缺口或规范不匹配，不绕过问题；在对应 `TODO-Px.md` 中插入最小必要前置任务，同步 `TODO.md`，提交后停止。
7. 运行任务指定验证以及必要的相关测试；若失败，优先修复与当前任务相关的问题。
8. 将完成的任务标题标记为 `[DONE]`，更新对应完成记录；同步 `TODO.md` 中同一任务的 `[DONE]` 标记。
9. 视情况运行格式化/检查，确认工作区变化只包含本任务相关内容。
10. 使用清晰任务编号提交所有本次任务相关改动，然后停止，不进入下一个任务。

## 进度记录

- 已写入初始执行计划，下一步读取任务索引并定位第一个未完成详细任务。
- 已读取 `TODO.md`，索引中的第一个未完成项是 `P6-T03g`，详细文件为 `TODO-P6-part3.md`。
- 已读取 `TODO-P6-part3.md`，确认当前任务为 `P6-T03g：闭合 HandleDispatch protocol`；最新提交仅记录 `P6-T03f` 完成状态，没有发现直接相关的未完成说明。
- 下一步聚焦 refactor LLVM handle dispatch、pending completion carrier、late-lowered handle contract 与对应测试，不做开放式历史问题扫描。
- 已发现 `finally` 路径的真实缺口：surface `resume` 的 body completion 会提前进入 `finally`，造成 `finally` 双执行且 wrapper completion payload 从未初始化 local 读取。
- 计划在当前任务内扩展 published `HandleDispatch` contract：发布 body completion payload source，并让 surface resume 的 body/arm completion 直接投影为 wrapper `Complete`；direct entry 仍通过 completion tag 进入 `finally`/exit。
- 已实现 body completion payload source handoff 与 surface resume completion 投影，新增 `handle_finally_boundary` run-pass fixture；直接运行结果已符合预期。
- 下一步运行格式化、指定验证、clippy，然后更新 TODO 记录并提交。
- 已完成格式化、指定验证与 clippy；`TODO-P6-part3.md` / `TODO.md` 已将 `P6-T03g` 标记为 `[DONE]` 并写入完成记录。
- 下一步检查最终 diff/status，提交本次任务改动后停止。
