# 当前执行计划

## 约束

- `TODO.md` 是任务顺序和完成状态的权威来源。
- 只处理第一个标题未带 `[DONE]` 的任务，完成后停止。
- 若遇到阻塞当前任务的实现缺口或规格不匹配，先在 `TODO.md` 中插入最小必要前置任务并提交，不绕过问题。
- 完成任务后必须更新 `TODO.md` 的任务标题和完成记录，并按需运行相关验证。
- 仅当阶段级计划变化时更新 `PLAN.md`。
- 最终需要提交本次变更。

## 初始步骤

1. 读取 `TODO.md`，定位第一个未完成任务。
2. 检查最近一次提交信息，判断是否有直接关联当前任务的未完成事项。
3. 阅读当前任务相关源码、测试和文档，确认验收要求。
4. 实现当前任务，优先采用最小且规格正确的改动。
5. 运行当前任务要求的验证，以及必要的相关测试。
6. 根据验证结果修复问题，或在发现真实阻塞时更新 `TODO.md` 并停止。
7. 完成后更新 `TODO.md`：给任务标题加 `[DONE]`，补充完成记录。
8. 运行最终必要检查，提交所有本任务相关变更。

## 进度记录

- 已创建本执行计划。
- 已读取 `TODO.md`，第一个未完成任务为 `U3-T01：编写 audit/spec_coverage_matrix.md`。
- 下一步检查最近提交是否含有与 `U3-T01` 直接相关的未完成事项，然后读取 `PLAN.md`、`UnsupportedMainBody_FIX.md`、spec 文档、inventory 与 bucket 文档。
- 最近提交为 `[U2-T02] Complete UMB bucket analyses`，未发现需要先处理的直接未完成事项。
- 已读取 U3 相关计划与设计段落；发现 inventory 中 `docs/spec/language_spec-part2.md#10-function-type` 已随 spec 编号漂移，当前正确锚点为 `#11-function-type`。该问题会阻塞 U3 的“无 inventory entry 找不到 spec 锚”验收，因此先作为本任务内数据修正处理。
- 已修正 audit inventory 生成规则并重新生成 `audit/UMB_inventory.csv`；同时更新 `audit/UMB_categories/B-03.md` 与 `B-04.md` 的 Function Type 锚点。
- 已生成 `audit/spec_coverage_matrix.md`：覆盖 spec part1-6 的 165 个 `##`/`###` section、49 个 inventory spec anchor、1,213 个非 helper inventory id；part4 async 与 generator/yield 行已标 `INTENTIONALLY-EMPTY` 并关联 B-36。
- 下一步运行矩阵结构校验、`umb-audit stats/diff`、`cargo test -p scoopc audit::umb_inventory -- --nocapture`，再更新 `TODO.md` 完成记录。
- 矩阵结构校验、`umb-audit stats/diff`、`cargo test -p scoopc audit::umb_inventory -- --nocapture`、`cargo clippy --all-targets -- -D warnings` 均通过。
- `cargo test --all --all-targets` 首次 120 秒超时但测试仍在推进；已用 600 秒超时重跑并通过（scoopc lib 874 passed，umb-audit bin 3 passed）。
- 已更新 `TODO.md`：`U3-T01` 标记为 `[DONE]`，顶部状态改为下一项 `U4-T01`，完成记录已写入。
- 已提交本任务主体变更：`2b48a5ed [U3-T01] Add UMB spec coverage matrix`。
- 下一步仅提交本执行记录更新，然后停止，不继续处理 `U4-T01`。
