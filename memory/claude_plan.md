# 执行计划与进度记录

说明：这里记录简明执行计划、关键决策与进度更新，不记录内部详细推理。

## 初始计划
1. 读取 `TODO.md`，仅把它当作索引使用。
2. 按索引顺序打开对应的 `TODO-Px.md`，定位第一个标题未带 `[DONE]` 的详细任务。
3. 检查最近一次提交是否有与该任务直接相关且未完成的问题；若有，将其作为当前任务的一部分或必要前置。
4. 阅读与当前任务直接相关的代码、测试、规范和任务约束，确认实现边界。
5. 实现当前任务，避免引入变通方案；若遇到真实阻塞，按要求在相应 `TODO-Px.md`/`TODO.md` 中补充最小前置任务并停止。
6. 运行与该任务直接相关的验证；如有必要，补充或修正测试，直到通过。
7. 更新任务记录：在对应 `TODO-Px.md` 中将任务标题标记为 `[DONE]` 并填写完成记录；若索引需要同步，更新 `TODO.md`。
8. 仅在阶段计划或依赖结构变化时更新 `PLAN.md`。
9. 检查工作区变更，按要求创建一次提交，然后停止，不继续下一个任务。

## 进度更新
- 已创建本文件并写入初始计划。
- 已读取 `TODO.md`，定位首个未完成详细任务为 `TODO-P6.md` 中的 `P6-T03`。
- 已检查 `P6-T03` 任务体与完成记录：此前发现的四个 blocker 已分别以前置任务 `P6-T02c`/`P6-T02d`/`P6-T02e`/`P6-T02f` 解决，因此当前可直接执行 `P6-T03`。
- 下一步：阅读 `P6-T03` 直接相关代码（refactor LLVM ABI query、late-lowered state graph、emit 入口与现有 codegen 结构），确定最小实现方案与验证矩阵。
- 进展变更：在核对 dynamic call 的实际 carrier materialization 时发现新的前置阻塞，当前 closure object / class vtable / interface itable 仍发布 legacy 普通函数指针，而不是已发布的 canonical refactor dynamic entry。
- 这会迫使 `P6-T03` 在 backend 现场做 legacy ABI -> refactor invoke 的 remap/猜测，违反 `P6-T02d`/`P6-T02f` 的 contract-first 约束。
- 已据此把计划改为：新增最小前置任务 `P6-T02g`，在 `TODO-P6.md`/`TODO.md` 中显式记录该依赖，然后提交这些任务文档更新并停止，等待下次调用先完成 `P6-T02g`。
