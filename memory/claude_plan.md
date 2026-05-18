# 执行计划

## 当前约束
- 以 `TODO.md` 为唯一任务顺序与完成状态来源。
- 只完成第一个标题未带 `[DONE]` 的任务，然后停止。
- 如果发现当前任务被具体缺陷或缺失能力阻塞，先把最小必要前置任务写入 `TODO.md` 并提交，不绕过实现。
- `PLAN.md` 只在阶段级计划、依赖或完成标准变化时更新。
- 完成任务后需要测试、更新 `TODO.md` 完成记录、提交 Git 变更。

## 初始执行步骤
1. 读取 `TODO.md`，定位第一个未完成任务及其验收要求、依赖和完成记录格式。
2. 检查最新提交是否显式提到与该任务直接相关的未完成问题。
3. 针对当前任务收集最小必要代码上下文，避免开放式历史问题扫描。
4. 实现当前任务，优先选择最小且符合规格的改动。
5. 运行任务要求的验证命令，并补充必要的定向测试。
6. 如果验证失败，修复与当前任务直接相关的问题后重新验证。
7. 将当前任务标题标记为 `[DONE]`，更新 `TODO.md` 完成记录。
8. 如执行计划发生关键变化或完成关键步骤，更新本文件。
9. 按仓库提交风格提交所有与本任务相关的变更。
10. 停止，不继续处理下一个任务。

## 状态
- 已读取 `TODO.md`，首个未完成任务为 `U5-T01：fixture 目录骨架 + _index.csv + runner 支持确认`。
- 已完成 U5-T01 的主要实现：新增 `umb_fix` fixture skeleton，补充 `scoop test <path>` 位置参数兼容，加入 `IGNORE-UNTIL-FIX` 解析和 `umb_fix` runner 识别/空目录通过逻辑。
- 已完成验证：`cargo run -p scoop -- test tests/fixtures/umb_fix/`、`cargo test -p scoop -- fixtures -- --nocapture`、`cargo clippy --all-targets -- -D warnings`、`cargo test --all --all-targets` 均通过。
- 已将 `TODO.md` 中 U5-T01 标记为 `[DONE]` 并填写完成记录；下一步执行 Git 提交流程后停止。

## U5-T01 执行计划
1. 检查最近提交信息，确认是否有直接关联 U5-T01 的未完成问题需要纳入本任务。
2. 读取 fixture runner 和 expectation parser 的相关实现，确认 `umb_fix` 未知 phase 与 `IGNORE-UNTIL-FIX` 当前行为。
3. 创建 `tests/fixtures/umb_fix/`、36 个 bucket 子目录、每个目录的 `_README.md`，以及严格表头的 `_index.csv`。
4. 如 runner 不支持 `umb_fix` 或 ignore 标记，添加仅限 fixture/test infrastructure 的支持，使 `cargo run -p scoop -- test tests/fixtures/umb_fix/` 可通过。
5. 运行 U5-T01 指定验证：`cargo run -p scoop -- test tests/fixtures/umb_fix/` 与 `cargo test -p scoop -- fixtures -- --nocapture`，并补充必要的格式/编译检查。
6. 更新 `TODO.md`：将 U5-T01 标记为 `[DONE]` 并填写完成记录；如不涉及阶段级变更，不更新 `PLAN.md`。
7. 提交本任务相关全部变更并停止。
