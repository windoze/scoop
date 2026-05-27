# 当前执行计划

## 约束
- 以 `TODO.md` 为任务顺序和完成状态的唯一来源。
- 本轮只处理第一个标题未带 `[DONE]` 的任务，完成后停止。
- 若遇到阻塞当前任务的缺失功能、规格不匹配或未排期失败，优先修复或在 `TODO.md` 中插入最小前置任务并停止。
- 完成任务后更新 `TODO.md` 的 `[DONE]` 标记和完成记录，并提交 Git commit。

## 步骤
1. 读取 `TODO.md`，定位第一个未完成任务，并查看该任务的依赖、验证要求和完成记录。
2. 检查最近提交是否明确提到与该任务直接相关的未完成问题。
3. 根据任务内容读取必要代码和文档，确认实现范围。
4. 实施最小正确变更，避免规避规格或使用夹具专用 hack。
5. 按要求先运行格式化，再运行 lint，再运行相关测试；如有必要运行完整测试和 fixture 套件。
6. 更新 `TODO.md` 的任务标题和完成记录；仅当阶段计划变化时更新 `PLAN.md`。
7. 检查工作区差异，提交本轮任务相关全部改动，然后停止。

## 当前状态
- 已读取 `TODO.md`，第一个未完成任务为 `P2-T04`：更新 `README.md` 的 fixture runner 描述与命令示例。
- 已检查最近提交：`[P2-T03R] Review AGENTS fixture commands`，未发现需要作为 `P2-T04` 前置的未完成问题。
- 已更新 `README.md`：fixture suite 改为 `python3 tools/run_fixtures.py`，Safepoint 基线改为 `python3 tools/safepoint_baseline.py`，并移除“fixture runner 属于 scoop 二进制”的表述。
- 已验证：README 旧入口搜索无命中；`cargo fmt` 通过；`cargo clippy --all-targets -- -D warnings` 通过。
- 完整 Rust 测试和完整 fixture 套件未重跑，因为本轮仅修改 markdown/任务记录，且最近完成记录已有完整绿色结果。
- 已更新 `TODO.md`：`P2-T04` 标题加 `[DONE]`，完成记录已追加验证命令和跳过完整套件的原因。
- 已检查 diff/status：本轮相关改动为 `README.md`、`TODO.md`、`memory/claude_plan.md`；工作区另有既有无关改动，将不纳入本次提交。
- 下一步：暂存本轮相关文件并创建 `P2-T04` commit，然后停止。
