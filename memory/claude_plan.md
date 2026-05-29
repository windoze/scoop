# 执行计划

## 范围

- 目标：依据 `TODO.md` 的顺序完成第一个标题未带 `[DONE]` 的任务，然后停止。
- 当前任务：`P6-T02R：Review fixture 同步结果`。
- 约束：不跳过 review 任务；不拆分任务；不通过删除或弱化 fixture 覆盖通过 review。
- 说明：本文件记录可共享的执行计划、关键进度和决策，不记录私有推理过程。

## 步骤

1. 读取 `TODO.md`，识别第一个未完成任务及其验证要求。
2. 检查最近提交是否提到与当前任务直接相关的未完成事项。
3. 阅读 `TODO-5.md` 中 P6-T02/P6-T02R 的任务体和完成记录，并对照 P0-T01 inventory。
4. 复核 generated spec doctest 是否 stale，检查 P6-T02 变更的 fixture，并抽样复核各类旧 surface 迁移。
5. 确认 negative fixtures 明确表达旧 surface reject，且不存在无语义理由的 dump expect churn。
6. 按要求运行格式化、lint、spec fixture check、targeted fixture 和完整 fixture suite。
7. 更新 `TODO.md` 与 `TODO-5.md`，将 P6-T02R 标记为 `[DONE]` 并填写完成记录；仅在阶段计划变化时更新 `PLAN.md`。
8. 检查 git 状态和差异，只提交本任务相关文件，然后停止。

## 当前进度

- 已读取 `TODO.md`；第一个未完成任务是 `P6-T02R`。
- 已检查最近提交：`[P6-T02] Synchronize fixtures with new surface`，未发现直接相关的未完成事项。
- 已读取 `TODO-5.md` 与 P0-T01 inventory；确认本轮是 generated doctest 与 handwritten fixture 同步结果的 review。
- 已抽样复核旧 surface 迁移：`perform`、handler `with`、tuple `._0`、`@Inline`、`AnyRef`、`AnyValue` 的实际代码命中集中在明确 negative fixture；正向 fixture 使用目标 surface，f-string `{...}` 命中用于新语义 literal braces 覆盖。
- 已确认 P6-T02 未改 HIR/MIR/effect dump expect，因此不存在无语义理由的 dump churn。
- 已运行 `python3 tools/spec_fixtures.py sync`，结果 `spec fixtures: ok (1)`，未产生新的 generated fixture 变化。
- 已通过验证：`cargo fmt`；`cargo clippy --all-targets -- -D warnings`；`python3 tools/spec_fixtures.py check`；targeted `python3 tools/run_fixtures.py tests/fixtures/typecheck/anyvalue_marker_name_is_not_type.scoop`；`python3 tools/run_fixtures.py`（`fixtures: ok (1607)`）。
- 未重跑 `cargo test --all --all-targets`，因为本 review 只修改 `TODO` / 执行记忆且未改编译输出；沿用 P6-T02 最近完整 Rust 测试绿色结果。
- 已更新 `TODO.md` 和 `TODO-5.md`，将 `P6-T02R` 标记为 `[DONE]` 并填写完成记录。
