# Claude Plan

## 初始执行计划
1. 读取 `TODO.md`，确认它只是任务索引，并按索引顺序找到对应的 `TODO-Px.md` 详细任务文件。
2. 在详细任务文件中按顺序定位第一个标题未标记 `[DONE]` 的任务；必要时对照最新提交信息，判断是否存在与该任务直接相关且未完成的问题需要一并处理或登记为前置依赖。
3. 阅读该任务的详细要求、约束、验证方式与依赖，检查当前代码和测试现状，确认是否可以直接实现。
4. 若任务可直接完成：实现任务要求，补充或调整测试，并运行相关验证命令与必要的质量检查。
5. 若任务存在无法绕过的真实阻塞：在对应 `TODO-Px.md` 中添加最小必要前置任务，保持顺序正确，并同步更新 `TODO.md`；仅在阶段计划发生变化时更新 `PLAN.md`。
6. 在执行过程中持续更新本文件，记录当前步骤、关键发现、计划变更、验证结果与完成状态。
7. 完成当前任务后：在对应 `TODO-Px.md` 中将任务标题标记为 `[DONE]` 并填写完成记录；如索引有变化则同步 `TODO.md`；然后按仓库约定创建一次 git 提交并停止，不继续下一个任务。

## 进度记录
- 已写入初始计划，下一步开始读取任务索引与详细任务文件。
- 已确认 `TODO.md` 索引中的首个未完成任务是 `P6-T03`，并在 `TODO-P6-part2.md` 中核对了完整任务体与既有 blocker 记录。
- 关键发现：`TODO-P6-part2.md` 当前未完成链路已手工插入 `P6-T02p`，但详细任务体尚未补齐；进一步检查 `crates/scoopc/src/llvm/codegen/effect_refactor/{types,layout}.rs`、`crates/scoopc/src/effect_facts/facts.rs` 后确认，当前 ABI/query 仍缺少 callable version 选择 contract：
  - late-lowered authoritative 身份是 `LateLoweredBodyVersionKey`；
  - 但 `RefactorAbiQuery` 的 callable shell 仍主要按 `StepSchemaId` / `root_fqn` 暴露；
  - `layout.rs::callable_layout_by_root_fqn(...)` 甚至在同一 `root_fqn` 出现多个 published shell 时直接 fail fast；
  - `CallSiteEffectFacts` 也还没有显式发布“运行时 callable carrier / known-instance target 应落到哪个 callable version”的 authoritative 选择合同。
- 计划已调整：本次不直接实现 `P6-T03`，先把上述 blocker 记为新的前置详细任务 `P6-T02p`，同步 `TODO.md`，并在 `P6-T03` 中显式声明依赖后停止。
