# 当前执行计划

## 约束说明

- 本文件记录可共享的执行计划、关键决策和进度更新；不会记录不可公开的逐步内部推理。
- `TODO.md` 是任务顺序、完成状态和验收要求的权威来源。
- 本次只完成 `TODO.md` 中第一个标题未带 `[DONE]` 的任务，完成后提交并停止。

## 初始计划

1. 读取 `TODO.md`，定位第一个未完成任务，并确认其依赖、验收要求和完成记录要求。
2. 查看最近提交信息，若其明确提到与当前任务直接相关的未完成事项，则纳入当前任务或作为前置任务记录到 `TODO.md`。
3. 读取当前任务涉及的代码、测试和文档，确定最小正确实现路径。
4. 实现当前任务；如发现阻塞当前任务的真实缺口，优先修复，或在 `TODO.md` 中插入最小前置任务并停止。
5. 运行格式化、lint 和相关测试；若代码变更影响范围较大，按要求运行完整测试与 fixture 套件。
6. 更新 `TODO.md`：将完成任务标题加 `[DONE]`，填写完成记录；仅在阶段计划变化时更新 `PLAN.md`。
7. 检查 git 状态和 diff，提交本次任务相关全部变更，然后停止，不继续下一个任务。

## 当前状态

- 已读取 `TODO.md`，第一个未完成任务是 `P2-T05`：`tools/README.md` 整体重写为 python 脚本列表。
- 最近提交为 `P2-T04R` README review，未发现需要在当前任务前插入的直接相关未完成事项。
- 已读取 `tools/README.md`、`TEST_INFRA_CLEANUP.md` §6 和当前 `tools/*.py` / `tools/*.sh` 列表。
- 已重写 `tools/README.md`：主清单覆盖 8 个 Python 脚本，shell helper 段落说明 3 个现有 shell 辅助脚本，并移除旧 Rust 工具箱与旧内置 runner 调用串。
- 已验证旧入口模式在 `tools/README.md` 无命中，8 个 Python 脚本入口均已列出；`cargo fmt` 与 `cargo clippy --all-targets -- -D warnings` 通过。
- 已更新 `TODO.md`，将 `P2-T05` 标记为 `[DONE]` 并追加完成记录；完整 Rust 测试与 fixture 套件因本次仅修改文档/task bookkeeping，复用最近完整绿色结果。
- 已创建提交 `85ac3a63 [P2-T05] Rewrite tools README scripts`。
- 下一步：确认工作区只剩本轮无关既有改动，然后停止；不进入 `P2-T05R`。
