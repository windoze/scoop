# Claude 执行计划

## 范围

本次只完成 `TODO.md` 引用的第一个未完成详细任务，完成并提交后立即停止。

## 执行步骤

1. 先读取 `TODO.md`，只把它当作全局索引使用。
2. 按索引顺序检查对应的 `TODO-Px.md` 文件，定位第一个标题未带 `[DONE]` 的详细任务。
3. 检查最新提交信息是否明确提到与该任务直接相关的未完成问题。
4. 从权威的 `TODO-Px.md` 中读取当前任务的要求、依赖、验证命令和完成记录格式。
5. 只检查实现该任务所需的相关源码、fixture 和测试文件。
6. 用最小且规格正确的改动完整实现任务；如果遇到阻塞实现的前置缺口，则新增最小必要前置任务、同步 `TODO.md`、提交并停止。
7. 运行任务相关的定向验证，再按任务要求运行更广的验证。
8. 在详细任务标题前加 `[DONE]`，填写或刷新完成记录，并同步 `TODO.md` 中对应条目的 `[DONE]` 标记。
9. 关键步骤完成或计划变化时，更新本文件。
10. 检查工作区，使用清晰的任务标签提交本次调用所需的所有改动，然后停止，不进入下一个任务。

## 进度

- 已在执行仓库命令或代码前写入初始计划。
- 已发现 `memory/claude_plan.md` 是受跟踪文件，本次会先提交计划更新，再继续选择任务。
- 已读取 `TODO.md` 与 `TODO-P6-part2.md`，确认第一个未完成详细任务为 `P6-T03`：按 P5 state graph / boundary contract 完成 refactor LLVM body lowering。
- 最新任务相关提交未留下需要优先登记的新 blocker；接下来检查 P5 late-lowered 表达、P6 ABI/query 和现有 refactor LLVM stage 的实现边界。
- 已确认 `P6-T03` 的指定 run-pass 验证命令存在直接 blocker：fixture runner 的 `run-pass` 子进程没有继承父级 `--effect-pipeline refactor`，会导致验证不能证明 refactor LLVM body lowering。
- 计划改为新增最小前置任务 `P6-T02qf`，要求先修复 run-pass fixture 子进程的 effect-pipeline 传播，再继续 `P6-T03`；同步 `TODO.md` 后提交并停止。
- 已在 `TODO-P6-part2.md` 中把 `P6-T02qf` 插入 `P6-T03` 之前，并把 `P6-T03` 依赖更新为包含 `P6-T02qf`。
- 已同步 `TODO.md` 索引；`PLAN.md` 不涉及阶段级计划变化，保持不改。
