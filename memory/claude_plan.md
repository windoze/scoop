# 执行计划

## 范围

- 目标：依据 `TODO.md` 的顺序完成第一个标题未带 `[DONE]` 的任务，然后停止。
- 约束：不跳过 review 任务；不拆分任务，除非存在无法继续的明确前置依赖；不使用 workaround 代替符合规格的实现。
- 说明：本文件记录可共享的执行计划、关键进度和决策，不记录私有推理过程。

## 步骤

1. 读取 `TODO.md`，识别第一个未完成任务及其验证要求。
2. 检查最近提交是否提到与该任务直接相关的未完成事项；仅在其阻塞当前任务时纳入范围或写入前置任务。
3. 按任务要求阅读相关代码、文档和测试，确认最小正确实现范围。
4. 实现任务；若发现阻塞性的缺失功能或规格不匹配，则更新 `TODO.md` 添加最小前置任务并停止。
5. 按要求运行格式化、lint、测试和 fixture 验证；发现未被明确排期的失败时修复或排期。
6. 更新 `TODO.md`：完成时在任务标题前加 `[DONE]` 并填写完成记录；仅在阶段计划变化时更新 `PLAN.md`。
7. 检查 git 状态和差异，提交本次任务相关变更。
8. 停止，不继续下一个任务。

## 当前进度

- 已读取 `TODO.md` 与 `TODO-5.md`。
- 当前任务：`P6-T02：同步 spec doctests 与 handwritten fixtures 到新 surface`。
- 最新提交：`ef752a63 [P6-T01R] Review spec updates`；未发现需要在当前任务前单独处理的直接相关未完成事项。
- 已运行 `python3 tools/spec_fixtures.py sync`，结果 `spec fixtures: ok (1)`，未产生 generated doctest 差异。
- 已审计旧 surface fixture 命中；`perform`、handler `with`、tuple `._0`、`@Inline`、`AnyRef` 仅保留在 negative fixture 或注释中；新增 `AnyValue` marker negative fixture 并通过 targeted run。
- 已通过验证：`cargo fmt`；`cargo clippy --all-targets -- -D warnings`；`python3 tools/spec_fixtures.py check`；`python3 tools/run_fixtures.py tests/fixtures/typecheck/anyvalue_marker_name_is_not_type.scoop`；`python3 tools/run_fixtures.py`；`cargo test --all --all-targets`；`git diff --check`。
- 已更新 `TODO.md` / `TODO-5.md`，将 `P6-T02` 标记为 `[DONE]` 并填写完成记录。
- 下一步：检查 git 状态和 diff，提交本任务变更。

## P6-T02 执行细化

1. 运行 `python3 tools/spec_fixtures.py sync`，同步 P6-T01/P6-T01R 后的 generated spec doctest fixtures。
2. 搜索 active fixtures 中旧 surface：`perform`、handler `with`、tuple `._0`、旧 f-string `{expr}`、`AnyRef` / `AnyValue`、缺少 `public` 的导出 API fixture、缺少 `operator` 的 operator-positioned call 目标。
3. 对 handwritten fixtures 做语义保持迁移；旧语法只保留为带明确 expected diagnostics 的 negative fixture。
4. 仅在文本变化有语义理由时刷新 HIR/MIR/effect expected dumps。
5. 验证顺序：`cargo fmt`、`cargo clippy --all-targets -- -D warnings`、`python3 tools/spec_fixtures.py check`、changed-path targeted fixture runs、`python3 tools/run_fixtures.py`、`cargo test --all --all-targets`。
6. 完成后同步更新 `TODO.md` 与 `TODO-5.md`，提交本任务变更并停止。
