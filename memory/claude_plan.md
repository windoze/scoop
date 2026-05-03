## 执行计划

说明：这里记录可审阅的执行计划、关键决策与进度更新，不记录逐字内部推理。

1. 读取 `TODO.md`，确认详细任务文件映射与顺序。
2. 按索引顺序读取对应 `TODO-Px.md`，定位第一个标题未标记 `[DONE]` 的详细任务。
3. 检查最新提交是否存在与该任务直接相关且未完成的问题；如果是当前任务的直接前置阻塞，则先按规则处理。
4. 阅读当前任务的详细要求、约束、验收标准与完成记录，并检查相关代码与测试。
5. 以最小正确改动实现该任务；若发现无法按规范完成的前置缺口，则在相应 `TODO-Px.md` 中新增最小前置任务并同步 `TODO.md`。
6. 运行相关验证，包括任务要求的测试，以及必要的格式化/静态检查/编译验证。
7. 更新 `memory/claude_plan.md` 记录关键进展；更新对应 `TODO-Px.md` 完成记录，并在任务真正完成时将标题改为 `[DONE]`；如有必要同步 `TODO.md`，仅在阶段计划变化时更新 `PLAN.md`。
8. 按要求创建一次 git 提交，然后停止，不继续下一个任务。

## 当前状态

- 当前任务：`P6-T02m` 发布 continuation surface-resume -> owner dispatch contract
- 进度：
  - 已按 `TODO.md -> TODO-P6.md` 确认首个未完成详细任务为 `P6-T02m`。
  - 已核对最新提交 `[P6-T02m] Track surface-resume dispatch prerequisite`，确认它正是当前任务的直接上下文。
  - 已对 `P6-T02m` 做过一次实现尝试，并用定向测试/`dump-effect-lowered` 验证了真实 blocker：
    - `effect_refactor_step_enum_single_case.scoop` 中，同一 `ContinuationSchemaId k0` 会复用到同一个 continuation object 的多个 surface case，但当前 authoritative internal method shell 只保留了单一 reachable case；说明 schema -> method identity 不是现成已发布事实。
    - `effect_resume_if_else_branch_single_perform.scoop` 中，resume site 直接需要 `k3` 的 surface-resume symbol，但 `k3` 不存在于任何 continuation object surface/method shell；同时 handle continuation binder 仍要求 `k0` 拥有 published surface-resume source。
  - 结论：`P6-T02m` 的真正前置缺口是“surface-resume dispatch-source inventory 尚未 authoritative 发布”，不能仅在 LLVM query 层靠 continuation object/method 列表补推。
  - 已撤回实验性代码改动，恢复代码到原始通过状态。
  - 下一步：按规则把 blocker 写成新的前置任务 `P6-T02ma`，同步 `TODO.md`/`TODO-P6.md`，提交后停止，等待下一次调用先完成该 prerequisite。
