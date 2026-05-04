# 当前执行计划

## 目标

- 按 `TODO.md` 索引和对应 `TODO-Px.md` 详细任务文件，找到第一个标题未带 `[DONE]` 的详细任务。
- 只完成这一个详细任务，完成后更新任务记录、同步索引、验证并提交，然后停止。

## 步骤

1. 读取 `TODO.md`，确认任务索引顺序和引用的详细任务文件。
2. 按索引顺序读取相关 `TODO-Px.md` 文件，定位第一个未完成详细任务。
3. 检查最近提交信息，仅判断是否有与该任务直接相关的未完成事项。
4. 阅读当前任务的详细要求、依赖、约束和验证要求。
5. 检查相关代码与测试，确定最小正确修改范围。
6. 实现当前任务；如遇到阻塞当前任务的规格缺口或实现边界，则新增最少必要前置任务、同步 `TODO.md`、提交并停止。
7. 运行相关测试和必要的质量检查；若失败，修复后重测。
8. 在对应 `TODO-Px.md` 标题前加 `[DONE]`，补全完成记录，并同步 `TODO.md` 的 `[DONE]` 状态。
9. 更新本文件记录关键进展。
10. 提交所有本次任务相关改动，提交信息使用任务编号和简洁说明。
11. 停止，不继续下一个任务。

## 当前状态

- 已读取 `TODO.md` 与 `TODO-P6-part3.md`。
- 当前第一个未完成详细任务：`P6-T02qh`，标题尚未带 `[DONE]`，虽然已有阻塞记录，仍必须继续完成。
- 最新提交 `307964ba [P6-T02qga] Implement call-boundary continuation composition` 与当前任务直接相关，作为已完成前置修复纳入本任务验证背景。
- 已确认 `P6-T02qh` 相关 handoff、ABI materialization、LLVM body emitter 与单测代码已存在，并运行指定单测与 fixture 验证通过。
- `cargo clippy --all-targets -- -D warnings` 返回成功但暴露 macOS SDK C deprecation warning；已将 `runtime/c/scoop_stackmap.c` 中 deprecated `getsectbynamefromheader_64` 调用替换为 `getsectiondata()`，准备重新验证。
- 已补齐 runtime API allowlist 中缺失的 `scoop_runtime_error_fatal`，使 `cargo test -p scoop_runtime` 通过。
- 已将 `P6-T02qh` 在 `TODO-P6-part3.md` 与 `TODO.md` 中标记为 `[DONE]`，并写入完成记录。
- 下一步：检查最终 diff/status，确认没有无关改动，然后提交本轮变更并停止。
