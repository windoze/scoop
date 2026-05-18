# Claude Execution Plan

说明：本文件记录可审查的执行计划、关键决策、进展和验证结果；不记录隐藏推理过程。

## 当前目标

- 按 `TODO.md` 的权威顺序识别第一个标题未带 `[DONE]` 的任务。
- 完整实现该任务，运行相关验证。
- 将该任务标题标记为 `[DONE]` 并更新完成记录。
- 按要求提交包含本次任务相关变更的 Git commit。
- 完成一个任务后停止，不继续下一个任务。

## 初始执行步骤

1. 读取 `TODO.md`，只为识别第一个未完成任务及其要求、依赖和验证条件。
2. 查看最新提交信息，若其中明确提到与当前任务直接相关的未完成问题，将其纳入当前任务或作为前置任务记录到 `TODO.md`。
3. 根据当前任务定位相关源码、测试或 fixture，避免开放式历史问题扫查。
4. 最小化实现必要变更，不引入 workaround 或偏离规格的替代方案。
5. 运行任务要求的验证；如发现直接阻塞当前任务的实现缺口，优先修复，或在 `TODO.md` 中插入最小前置任务并停止。
6. 更新 `TODO.md` 的任务标题和完成记录；仅当阶段级计划变化时才更新 `PLAN.md`。
7. 提交本次任务相关全部变更。

## 进展日志

- 初始化：已写入执行计划，下一步读取 `TODO.md` 识别第一个未完成任务。
- 已识别当前任务：`U6-T01：10 条 baseline test 落地`。该任务要求补齐并稳定 10 条 `crates/scoopc/src/audit/**` baseline tests，验证 U1-U5 的 inventory、bucket 文档、fixture index、spec matrix、禁词和 helper sentinel 覆盖闭环。
- 最新提交摘要为 `[U5-T03] Record execution completion`，当前未看到与 U6-T01 直接相关的未完成问题提示；下一步检查提交正文、工作区状态和现有 audit module。
- 检查结果：当前 `crates/scoopc/src/audit/` 只有 `mod.rs` 与 `umb_inventory.rs`；`umb_inventory_csv_in_sync` 已存在，另有 U1 遗留 kind/legacy 对账测试，但 U6 要求的其余稳定测试尚未落地。
- 实施计划更新：在 `umb_inventory.rs` 增加 `umb_inventory_buckets_total`、`umb_inventory_each_entry_has_spec_anchor_or_helper_marker`、`umb_inventory_class_distribution`；新增 `spec_coverage.rs` 实现 5 条 U5 fixture/matrix baseline；新增 `sentinel_tests.rs` 实现 B-01 helper sentinel coverage baseline；更新 `mod.rs` 接入；把 B-01 README sentinel 状态从 U6 planned 改为 present。
- 实施进展：新增并接入 U6 baseline tests；修正首轮定向测试发现的两个断言问题（矩阵 wildcard 不是具体 fixture、index bucket 不必等同 header BUCKETS）。`cargo test -p scoopc audit:: -- --nocapture` 已通过，匹配测试 17 passed。
- 验证完成：`cargo test -p scoopc audit:: -- --nocapture`、`cargo test -p scoopc pipeline_user_visible_failure_policy -- --nocapture`、`cargo run -p scoop -- test tests/fixtures/umb_fix/`、`cargo test --all --all-targets`、`cargo clippy --all-targets -- -D warnings` 均已通过。
- 文档进展：已将 `TODO.md` 顶部状态更新为 `U6-T01 已完成；下一项 U6-T02`，并将 U6-T01 标题标记 `[DONE]`、填写完成记录。下一步检查 diff/status 后提交本任务变更。
