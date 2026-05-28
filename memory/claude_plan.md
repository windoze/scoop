# 执行计划

## 范围

- 以 `TODO.md` 作为权威任务列表。
- 识别第一个标题未带 `[DONE]` 前缀的任务。
- 本轮只完成该任务，提交后停止。

## 步骤

1. 阅读 `TODO.md`，确认首个未完成任务、依赖、验证要求和完成记录要求。
2. 只检查最新提交中与当前任务直接相关的未完成说明。
3. 阅读完成当前任务所需的最小代码、测试、fixture 和设计文档上下文。
4. 按 spec 实现任务，不采用 workaround 或弱化 fixture。
5. 先运行 `cargo fmt`。
6. 再运行 `cargo clippy --all-targets -- -D warnings` 并修复所有 warning。
7. 运行当前任务相关 targeted 验证；代码有变更时运行完整 Rust 测试和完整 fixture suite。
8. 如果观察到未明确排期的测试或 fixture 失败，必须修复或在 `TODO.md` 中添加最小前置/后续任务后才能完成当前任务。
9. 在 `TODO.md` 和对应任务文件中把完成任务标题标记为 `[DONE]`，并填写实现与验证记录。
10. 只有阶段级顺序、依赖、假设或完成标准变化时才更新 `PLAN.md`。
11. 用 Git status/diff 审查 worktree，只暂存本任务相关变更并提交。
12. 提交后停止，不开始下一个任务。

## 进度

- 已在读取任务列表前初始化计划。
- 已确认首个未完成任务为 `P3-T03`：enum `with` mismatched variant 应改为 panic。
- 已实现 lowering 修改：当 enum `with` 包含 variant update 且运行期 variant 未被该 update set 覆盖时，分支调用 `panic("enum with variant mismatch")`。
- 已更新 with-update run-pass fixtures：移除旧 silent-preserve 正例，新增 expected-exit panic 用例。
- `cargo fmt`、`cargo clippy --all-targets -- -D warnings`、`cargo build -p scoop -p scoopc` 和 targeted with-update fixtures 已通过。
- 完整验证已通过：`cargo test --all --all-targets`；`python3 tools/run_fixtures.py`（`fixtures: ok (1556)`）。
- 已更新 `TODO.md` 和 `TODO-3.md`：将 `P3-T03` 标记为 `[DONE]` 并记录改动范围、核心决策、验证结果和设计闭合。
