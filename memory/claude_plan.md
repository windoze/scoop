# 本轮执行计划

1. 读取 `TODO.md`，按标题是否带 `[DONE]` 判断第一个未完成任务；不做开放式历史问题排查。
2. 查看该任务的要求、依赖、验证方式，以及必要的相邻上下文；仅在与当前任务直接相关时查看 `PLAN.md` 或最新提交。
3. 检查当前工作树状态，识别已有未提交变更，避免覆盖或回退非本轮修改。
4. 实现第一个未完成任务；如果遇到阻塞当前任务的规格缺口或缺失前置条件，则只在 `TODO.md` 中加入最小必要前置任务并停止。
5. 运行任务要求的验证命令；如失败，先修复与当前任务相关的问题并重新验证。
6. 将完成情况写回 `TODO.md`，把任务标题加上 `[DONE]` 并更新 completion record；仅当阶段计划确实变化时才更新 `PLAN.md`。
7. 提交本轮所有相关变更到 Git，提交信息使用任务编号和简短说明。
8. 提交后停止，不继续处理下一个任务。

## 进度

- 已读取 `TODO.md`，首个未完成任务为 `U0-T01：现状摸底与基线冻结`。
- 下一步读取相关基线段落、检查最新提交和工作树状态，然后复算 `UnsupportedMainBody` 基线并生成 `audit/` 文件。
- 已复算当前基线：1,284 个 constructor、61 个文件、982 个唯一 `kind:` 字面量、836 个单次出现字面量、638 条 stale frozen count、21 个 gap inventory entry。
- 已创建 `audit/.gitkeep`、`audit/_baseline_files.txt`、`audit/_baseline_sampling.md`，并同步更新 `PLAN.md` / `UnsupportedMainBody_FIX.md` 的漂移基线。
- 已完成验证：`cargo build`、`cargo test --all --all-targets`、`cargo run -p scoop -- test`、`rg -n/-c`、`cargo test -p scoopc pipeline_user_visible_failure_policy -- --nocapture`、`cargo clippy --all-targets -- -D warnings` 均通过。
- 已回写 `TODO.md`，将 `U0-T01` 标记为 `[DONE]` 并写入完成记录。下一步提交本轮变更。
