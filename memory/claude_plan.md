# 当前执行计划

## 范围

- 本次只处理 `TODO.md` 中第一个未完成任务。
- 以 `TODO.md` 作为任务顺序、依赖、验证要求和完成记录的权威来源。
- 完成或阻塞当前任务后停止，不继续处理下一个任务。

## 执行计划

1. 读取 `TODO.md`，找到第一个标题未带 `[DONE]` 前缀的任务。
2. 只检查最近提交中是否存在与该任务直接相关的未完成事项。
3. 查看当前任务需要的代码、fixture 和文档上下文。
4. 按任务原文完成实现，不缩小范围、不使用 workaround。
5. 先运行格式化，再运行 lint，最后运行任务要求的相关或完整验证。
6. 如果验证暴露未排期失败，先修复，或在 `TODO.md` 中加入最小必要前置/后续任务后再决定当前任务状态。
7. 任务完成后，在 `TODO.md` 中给任务标题加 `[DONE]` 并补充完成记录。
8. 关键步骤完成或计划变化时更新本文件。
9. 用清晰的任务编号提交信息提交本任务相关变更。
10. 提交后停止。

## 进度记录

- 初始计划已在任务发现前写入。
- 已识别首个未完成任务：`P2-T07`（更新 `SCOOP_FULL_SPEC.md` 中 fixture runner 调用串）。
- 最近提交 `315d1841 [P2-T06R] Review prompt fixture command` 没有包含与 `P2-T07` 直接相关的未完成事项。
- 已观察到现有无关工作区变更：`run_agent.sh`、`CALLER_LOCATION.md`、`RTTI_REFINE.md`、`tools/__pycache__/`；本任务不会修改这些文件。
- 已更新 `SCOOP_FULL_SPEC.md` doctest fixture 说明，改用 `python3 tools/spec_fixtures.py` 与 `python3 tools/run_fixtures.py`。
- 验证已完成：`SCOOP_FULL_SPEC.md` 旧入口 grep 无命中；新入口 grep 有命中；`cargo fmt`、`cargo clippy --all-targets -- -D warnings`、`python3 tools/spec_fixtures.py check`、`python3 tools/run_fixtures.py tests/fixtures/spec_doctest` 均通过。
- 已更新 `TODO.md`：将 `P2-T07` 标记为 `[DONE]` 并追加完成记录。
