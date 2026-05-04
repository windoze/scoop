# 当前执行计划

说明：这里记录可审计的执行计划和进度，不记录私有逐步思维链。

1. 读取 `TODO.md` 作为任务索引。
2. 按索引顺序打开对应的 `TODO-Px.md` 详细任务文件，找到第一个标题未带 `[DONE]` 的详细任务。
3. 阅读该任务的完整要求、依赖、约束和验证要求。
4. 检查当前仓库状态，确认是否有未提交或外部变更需要避让。
5. 实现该任务要求，优先做最小且符合规范的代码变更。
6. 添加或更新必要测试/fixture，并运行该任务要求的验证命令；若失败，修复后重跑。
7. 在对应 `TODO-Px.md` 中把已完成任务标题加上 `[DONE]` 并更新完成记录；必要时同步 `TODO.md`。
8. 运行最终相关验证，确认无警告或失败。
9. 提交所有本次任务相关变更，提交信息包含任务编号和简要说明。
10. 停止，不继续处理下一个任务。

当前状态：已完成任务定位。`TODO.md` 索引与 `TODO-P6-part3.md` 详细文件显示第一个未完成详细任务为 `P6-T04R`：Review GC/runtime 集成，确认 clean refactor path 没有 legacy runtime 语义依赖。

本任务执行细化：

1. 检查最新提交和工作区状态，确认是否存在与 `P6-T04R` 直接相关的未完成事项或未提交变更。
2. 审查 P6-T04 完成记录涉及的 GC roots、stackmap、dropped continuation、runtime error、Managed ABI/extern 与 legacy runtime call 边界。
3. 运行 `P6-T04R` 指定的全部验证命令：P6-T04 四个 `cargo test -p scoopc ...`、两个 fixture 命令，以及指定 `rg` 审计。
4. 若验证暴露与本 review 直接相关的问题，修复后重跑相关验证；若需要新增前置任务，则按 TODO 规则插入并停止。
5. 若 review 通过，更新 `TODO-P6-part3.md` 的 `P6-T04R` 标题为 `[DONE]` 并填写完成记录，同步 `TODO.md` 对应索引。
6. 运行必要的最终验证/格式检查，提交本次 review 文档与任何修复。

进度更新：已完成 `P6-T04` 指定的四个 `scoopc` 定向测试、no-legacy build fixture、moving-GC runtime fixture，全部通过。`rg` 审计命中均已归类：refactor 侧仅为 extern/native ABI fail-fast 与测试守卫；legacy handler-stack/outcome 命中只在 legacy `llvm/codegen/effect` 模块自身。

进度更新：已完成 `P6-T04R` review 文档更新。`TODO-P6-part3.md` 与 `TODO.md` 已同步标记 `[DONE]`，`cargo clippy --all-targets -- -D warnings` 已通过。下一步检查 diff 并提交本次任务变更。
