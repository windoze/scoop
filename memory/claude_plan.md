# 当前执行计划

## 约束摘要

- 先以 `TODO.md` 为唯一任务顺序来源，找到第一个标题未带 `[DONE]` 的任务。
- 本次只完成第一个未完成任务；完成后更新 `TODO.md`、验证、提交，然后停止。
- 不做开放式历史问题清扫；只处理当前任务直接需要或阻塞当前任务的问题。
- 不用规避、削弱 fixture 或改变建模方式来绕过缺失功能；如遇阻塞，向 `TODO.md` 插入最小必要前置任务并提交后停止。
- 仅当阶段级计划、依赖或完成标准改变时更新 `PLAN.md`。
- 提交前检查工作区、差异和近期提交，避免回滚或覆盖他人改动。

## 初始步骤

1. 读取 `TODO.md`，按标题判断第一个未完成任务。
2. 查看最新提交是否明确提到与该任务直接相关的未完成问题。
3. 根据当前任务读取相关代码、fixture、测试与文档，确认任务要求和验证方式。
4. 如果任务可以直接完成，则做最小正确实现并补充/调整相关测试或 fixture。
5. 如果发现当前任务被具体缺失功能或 spec mismatch 阻塞，则更新 `TODO.md` 插入前置任务，记录阻塞原因，提交后停止。
6. 运行任务要求的验证命令；如失败，定位并修复当前任务相关问题后重跑。
7. 将完成任务标题加 `[DONE]`，更新 completion record，并视需要更新本计划文件的进度。
8. 提交所有与本次任务相关的变更，提交信息使用任务编号前缀。
9. 停止，不进入下一个任务。

## 当前状态

- 已读取 `TODO.md`，第一个未完成任务是 `P4-T02R：Review effect facts 只读化结果`。
- 已读取 `TODO-5.md` 中 P4-T02/P4-T02R 要求；本次任务是复查 P4-T02 是否真正移除 P4 对 MIR 的可变输入、确认 effect-owned type context 不写回 MIR、确认 two-pass solver 精度保持，并运行指定验证。
- 最新提交主题为 `[P4-T02] Record completion plan state`，无正文；未发现与 P4-T02R 直接相关的未完成说明。
- 已完成代码复查：effect facts 生产路径只读消费 `MaterializedMir`，P4-owned type additions 写入 `EffectOwnedTypeContext`，未发现需要在本 review 内修复的阻塞项。
- 已运行验证并通过：`cargo fmt`、`cargo test -p scoopc --no-default-features effect_facts_stage`、`cargo test -p scoopc --no-default-features effect_facts`、`cargo run -p scoop -- test --fixtures tests/fixtures/effect_facts`、`cargo test -p scoopc --no-default-features effect_lowered`、`cargo run -p scoop -- test --fixtures tests/fixtures/effect_lowered`、`cargo clippy --all-targets -- -D warnings`。
- 已将 `TODO.md` 与 `TODO-5.md` 中 `P4-T02R` 标记为 `[DONE]` 并填写完成记录。
- 已运行 `git diff --check` 并通过；已检查 git 状态、差异和近期提交。
- 本次任务已完成，下一步仅提交 `P4-T02R` 相关变更并停止。

## P4-T02R 执行步骤

1. 检查最新提交信息，确认是否存在直接相关未完成事项。
2. 复查 `crates/scoopc/src/pipeline/effect_facts_stage.rs`、`crates/scoopc/src/effect_facts/`、`crates/scoopc/src/effect_lowered/` 中 P4-T02 相关边界。
3. 搜索 `canonical_snapshot_mut\(|&mut MaterializedMir|from_materialized_snapshot\(`，确认活跃生产路径没有 P4 mutable MIR 输入。
4. 如发现 review 阻塞项，在本任务内修复并补充验证；如无阻塞项，记录 review 结论。
5. 运行 P4-T02/P4-T02R 指定验证命令：`cargo fmt`、相关 `cargo test`、effect facts/effect lowered fixture、`cargo clippy --all-targets -- -D warnings`、以及搜索检查。
6. 将 `TODO.md` 与 `TODO-5.md` 的 `P4-T02R` 标记为 `[DONE]` 并填写完成记录。
7. 检查 `git status`、`git diff`、`git log --oneline -10` 后提交本次变更。
