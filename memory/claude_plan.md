# 执行计划

## 当前目标

按照 `TODO.md` 的顺序完成第一个标题未带 `[DONE]` 的任务，完成后更新任务记录、运行相关验证、提交 Git commit，并停止。

## 执行步骤

1. 读取 `TODO.md`，只识别第一个未完成任务，不进行开放式历史问题排查。
2. 查看该任务的要求、依赖、验证命令和完成记录；必要时查看最近提交，判断是否有与当前任务直接相关的未完成问题。
3. 检查当前工作区状态，避免覆盖用户或其他代理已有改动。
4. 阅读当前任务涉及的代码、测试、fixture 或文档，确定最小正确修改范围。
5. 实现当前任务；若发现阻塞当前任务的规格不匹配或缺失能力，不绕过问题，而是在 `TODO.md` 中加入最小必要的前置任务并提交后停止。
6. 运行当前任务要求的验证，以及与修改范围直接相关的测试；若失败，修复后重新验证。
7. 将当前任务标题改为 `[DONE] ...`，并更新其 completion record，记录修改内容和验证结果。
8. 如阶段级计划未变化，不更新 `PLAN.md`。
9. 检查 diff，确认只包含本次任务相关修改及必要的既有未提交恢复内容。
10. 使用清晰的任务编号提交 Git commit，然后停止，不继续下一项任务。

## 进度记录

- 已创建初始执行计划，下一步读取 `TODO.md` 识别第一个未完成任务。
- 已读取 `TODO.md`，第一个标题未带 `[DONE]` 的任务是 `P1-T06R`：Review P1 全包完成度。下一步读取 `TODO-2.md` 的任务正文与验收要求，并检查最新提交是否有与该 review 直接相关的未完成事项。
- 已读取 `TODO-2.md` 中 `P1-T06R` 正文。最新提交为 `[P1-T06] Clean P1 docs and dependency audit`，未在提交标题中发现直接相关的未完成问题。当前工作区除本计划文件外无其它未提交改动。下一步按 review 范围静态复查 P1 边界、基础 crate DAG、adapter 状态、cone-level compilation unit API 与文档一致性。
- 静态复查发现 active docs 中 `PIPELINE_REFACTOR.md` / `PIPELINE-CLEANUP.md` 仍把 MIR 内部 `InstanceKey` 描述为 `scoopc_ids` 或 fact crate 可直接使用的基础身份；这与 `P1-T03` 的完成决策冲突。下一步在本 review 内修正文档，使其明确当前 P1 基础层只发布 `SiteId`、`BodyVersionKey`、stable hash/key primitives 等，MIR `TemplateKey` / `InstanceKey` 仍是 P3 处理前的 stage-owned internal key。
- 已修正 `PIPELINE_REFACTOR.md` 与 `PIPELINE-CLEANUP.md` 中关于 `scoopc_ids` / fact crate 可用 identity 的表述。下一步运行 `P1-T06R` 要求的验证，包括 P1-T06 验证命令、基础 crate `cargo tree`、multi-file same-cone 与 local dependency cone fixture 抽查。
- 已完成验证：`cargo fmt`、`cargo fmt --check`、`dependency-gate`、5 个基础 crate `cargo tree`、`cargo test --all --all-targets --no-default-features`、完整 fixture suite、`spec-fixtures check`、`cargo clippy --all-targets -- -D warnings`、multi-file same-cone fixture、local dependency cone fixture、P1 关键词搜索和 `git diff --check` 均通过。已在 `TODO.md` 与 `TODO-2.md` 标记 `P1-T06R` 为 `[DONE]` 并填写完成记录。下一步做最终 diff/status 检查并提交。
