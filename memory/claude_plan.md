# Claude 执行计划

## 范围
- 以 `TODO.md` 作为唯一权威任务列表。
- 只完成第一个标题未带 `[DONE]` 的任务，然后停止。
- 若遇到当前任务的真实前置阻塞，只添加最小必要前置任务并提交，不使用 workaround。

## 当前计划
1. 读取 `TODO.md`，确认第一个未完成任务及验证要求。
2. 只检查最新提交中是否有与该任务直接相关的未完成事项。
3. 定位该任务涉及的代码路径和测试入口。
4. 做最小的 spec-correct 实现，不引入 fixture-only hack 或 runtime workaround。
5. 先运行定向验证，再运行任务要求的更广验证。
6. 将完成任务标题标记为 `[DONE]`，并写入完成记录。
7. 关键步骤完成或计划变更时更新本文件。
8. 使用任务标签提交本次相关变更。

## 状态
- 已识别第一个未完成任务：`C1-T01`（`sealed interface` frontend 语义、marker metadata 与自动登记）。
- 最新提交为 `C0-T01` baseline 审计；未发现与 C1 直接相关的未完成前置事项。
- 已检查代码路径：`typecheck/type_env.rs`、`typecheck/lower.rs`、`typecheck/interfaces.rs`、`typecheck/where_clause.rs`、`typecheck/assignable.rs`，以及 cast/type-test 入口。
- 实现策略：在 `TypeEnv` 增加 sealed marker metadata；在 env rebuild 阶段检查 sysroot-only、body-empty、supertype、cycle、互斥 marker；在 type lowering 中拒绝 marker 的非 bound 类型位置；在 interface 检查中拒绝显式实现/继承 marker；在 where-bound assignability 中接入自动 `AnyRef` / `AnyValue` 满足关系。
- `C1-T01` 实现已完成，`TODO.md` 已标记 `[DONE]` 并写入完成记录。
- 验证已完成：`cargo test -p scoopc sealed -- --nocapture`、`cargo test -p scoopc typecheck -- --nocapture`、`cargo build`、`cargo clippy --all-targets -- -D warnings` 均通过。
- 下一步：用 `C1-T01` commit message 提交本任务变更，然后停止。
