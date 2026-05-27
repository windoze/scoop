# 当前执行计划

## 约束
- 以 `TODO.md` 为唯一任务顺序和完成状态来源。
- 本次只完成第一个标题未带 `[DONE]` 的任务，然后停止。
- 如果遇到阻塞当前任务的缺失特性、规格不匹配或测试失败，先修复；若无法在本次正确修复，则在 `TODO.md` 中加入最小必要前置任务并提交后停止。
- 不使用 workaround、弱化 fixture 或改窄需求来绕过实现缺口。
- 只有阶段级计划发生变化时才更新 `PLAN.md`。

## 步骤
1. 读取 `TODO.md`，按标题 `[DONE]` 前缀识别第一个未完成任务。
2. 查看最近提交，仅判断是否明确提到与该任务直接相关的未完成事项。
3. 阅读当前任务关联的代码、测试、fixture 和文档，确定实现范围与验证要求。
4. 按任务要求做最小正确实现；编辑前更新本文件记录具体任务和执行步骤。
5. 运行格式化、lint、相关测试；若代码变更影响全局行为，再按要求运行完整 Rust 测试和 fixture 套件。
6. 若发现未安排的测试/fixture 失败，修复或在 `TODO.md` 中加入正确排序的前置/后续任务，不把当前任务标为完成。
7. 成功后更新 `TODO.md`：给任务标题加 `[DONE]`，补充完成记录与验证命令。
8. 检查 `git status`、`git diff`、最近提交，提交所有与本任务相关的变更。
9. 停止，不继续处理下一个任务。

## 当前状态
- 已读取 `TODO.md`。
- 当前任务：`P2-T06R`，复审 `PROMPT.md` 更新，确认无旧 fixture runner 入口残留。
- 任务性质：文档 review，预计只需要检查 `PROMPT.md`、相关设计要求和调用串搜索；若确认无问题，则更新 `TODO.md` 完成记录并提交。
- 已检查最近提交：`[P2-T06] Update prompt fixture command` 未说明相关未完成事项。
- 已复审 `PROMPT.md`：旧 fixture runner 入口搜索无命中；`python3 tools/run_fixtures.py` 与至少 30 分钟超时说明存在。
- 已完成验证：`cargo fmt` 通过；`cargo clippy --all-targets -- -D warnings` 通过。
- 已更新 `TODO.md`：`P2-T06R` 标为 `[DONE]` 并追加完成记录。
- 完整 Rust 测试和 fixture 套件未重跑：本 review 仅修改 markdown/task bookkeeping，且自最近完整绿色结果以来无代码变更。
- 下一步：检查 diff/status 并提交本任务变更。
