# 当前执行计划

## 约束说明

- 本文件记录可公开的执行计划、关键决策和进度更新；不会记录私有逐步推理细节。
- 本轮只处理 `TODO.md` 中第一个标题未带 `[DONE]` 的任务，完成后提交并停止。

## 初始步骤

1. 读取 `TODO.md`，按文件顺序确定第一个未完成任务。
2. 查看最近提交信息，判断是否明确提到与该任务直接相关的未完成问题。
3. 阅读该任务涉及的代码、测试、文档和完成要求，只收集完成当前任务所需的上下文。

## 执行步骤

1. 如任务可直接完成，进行最小但完整的实现修改。
2. 如发现阻塞当前任务的缺失特性、规格不一致或实现边界，先将最小必要前置任务插入 `TODO.md`，更新依赖说明，提交后停止。
3. 针对实现添加或更新最小相关测试、fixture 或文档。
4. 运行当前任务要求的验证命令；必要时运行更广的相关测试。
5. 修复验证中发现的、与当前任务直接相关的问题。

## 收尾步骤

1. 在 `TODO.md` 中给完成任务标题加 `[DONE]`，并更新完成记录。
2. 仅当阶段级计划、依赖结构或完成标准变化时更新 `PLAN.md`。
3. 检查 `git status`、`git diff` 和最近提交，确保提交内容只包含本轮应提交的变更，或在恢复未完成任务时包含所有当前未提交文件。
4. 使用清晰的任务编号提交信息提交变更。
5. 停止，不继续处理下一个任务。

## 当前进度

- 已确定首个未完成任务为 `P3-T07R：Review P3 全包完成度`。
- 最近提交 `f94c94eb [P3-T07] Complete P3 cleanup audit` 是该 review 的直接前置完成提交，未在提交标题中声明未完成 blocker。
- 静态复查未发现 P3 边界 blocker：旧 root index 字段、HIR devirtualization 开关和 missing snapshot 错误在活跃 Rust 源码中无命中；剩余 `materialized_pass_view` / backend devirtualization 命中符合 P4/P5/P7 过渡归属。
- `P3-T07R` 必跑验证命令已通过：P3 相关 unit tests、dependency gate、`mir_lowered` / `mir_materialized` fixtures 和 `cargo clippy --all-targets -- -D warnings`。
- 可选 `cargo test --all --all-targets` 初次发现 `single_pipeline_runs_higher_order_function_value_handled_effect_cli` 超过 60 秒并被 SIGTERM；已定位为 `mir::escape` alias propagation 在同一 local 绑定多个 escape origin 时非单调导致 fixpoint 不收敛。
- 已修复 escape alias propagation：冲突 origin 现在收敛为 conservative `Unknown`，并新增 `conflicting_alias_origins_converge_to_unknown` 回归测试。
- 修复后已通过单独超时 fixture、完整 P3 验证和 `cargo test --all --all-targets`。
- 已在 `TODO.md` 与 `TODO-4.md` 中把 `P3-T07R` 标记为 `[DONE]` 并填写完成记录；下一步检查 diff/status 后提交。
