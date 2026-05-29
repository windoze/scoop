# 执行计划

本文件记录可共享的执行计划、关键进度和决策，不记录私有推理过程。

## 历史记录：P6-T03R

- 已读取 `TODO.md`；第一个未完成任务是 `P6-T03R`。
- 已检查最近提交：`fdcdc47e [P6-T03] Audit old surface regressions`，它正是本 review 的输入，未发现额外未完成前置事项。
- 已抽样复核旧 surface 命中：实际 `perform` keyword 只出现在 removal diagnostic / negative fixture，handler `with` 只出现在 removal negative，tuple `._0` / with-path `_0` 只出现在旧语法 negative，f-string `{...}` 命中为 literal-brace 覆盖或 `${...}` 内部表达式，`@Inline` / `AnyRef` / `AnyValue` 不在 sysroot/compiler 中作为 active positive surface 出现。
- 已确认 sysroot operator-like declarations 未发现缺少 `operator` 的正向 API；active spec / split spec 中剩余 `perform` 为普通动词或 removal 说明，不是旧 prefix 正例。
- 已验证 overload/codegen baseline：`overload_concrete_bug.scoop`、`overload_arity_bug.scoop`、`overload_gvc_ok.scoop` 均通过。
- 已验证 overload diagnostics：no-applicable、ambiguity、conflicting overload、generic shape mismatch、vararg overlap、infer ambiguity targeted fixtures 均通过；`python3 tools/audit_user_visible_failure_policy.py` 通过。
- 已验证 `.cone` / `scoopir` export：`public_api_filter.scoop` 确认 `.scoopir` 只导出显式 `public`；`source_path_dependency_public_call`、`source_path_dependency_private_hidden`、`source_path_dependency_internal_hidden` 确认 public 可见且 private/internal 保持隐藏。
- 已通过完整验证：`cargo fmt`；`cargo clippy --all-targets -- -D warnings`；targeted overload / cone fixtures；`cargo test --all --all-targets`；`python3 tools/spec_fixtures.py check`（`spec fixtures: ok (1)`）；`python3 tools/run_fixtures.py`（`fixtures: ok (1607)`）；`git diff --check`。
- 已更新 `TODO.md` 和 `TODO-5.md`，将 `P6-T03R` 标记为 `[DONE]` 并填写完成记录；`PLAN.md` 阶段级 sequencing 未变化，无需更新。
- 提交前检查发现未跟踪文件 `REFLECTION.md`，该文件不是本任务产生的改动，不纳入本次提交。

## 当前任务：P6-T04

## 范围

- 目标：依据 `TODO.md` 的顺序完成第一个标题未带 `[DONE]` 的任务，然后停止。
- 当前任务：`P6-T04：全量格式化、测试矩阵与最终收口记录`。
- 约束：执行完整验证矩阵；不把 `SPEC_FIX.md` / `OVERLOAD_RESOLUTION.md` 范围内事项静默延期；完成后只留下 `P6-T04R` 作为下一个 review 任务。

## 步骤

1. 读取 `TODO.md`，识别第一个未完成任务及其验证要求。
2. 检查最近提交是否提到与当前任务直接相关的未完成事项。
3. 阅读 `TODO-5.md` 中 `P6-T04` 的任务体和依赖。
4. 按要求运行 `cargo fmt`、`cargo clippy --all-targets -- -D warnings`、`cargo test --all --all-targets`、`python3 tools/spec_fixtures.py check`、`python3 tools/run_fixtures.py`。
5. 额外运行 user-visible failure policy audit 和 `git diff --check`，用于最终诊断和 whitespace 收口记录。
6. 如果出现未调度失败，修复或在 `TODO.md` 中插入最小必要前置任务后停止。
7. 验证通过后，将 `P6-T04` 在 `TODO.md` 和 `TODO-5.md` 标记为 `[DONE]` 并填写完成记录。
8. 检查 git 状态、差异和最近提交，只提交本任务相关文件，然后停止。

## 当前进度

- 已读取 `TODO.md`；第一个未完成任务是 `P6-T04`。
- 已检查最近提交：`2a8410d4 [P6-T03R] Review old surface audit`，未发现与当前任务直接相关的未完成 blocker。
- 已通过最终验证矩阵：`cargo fmt`；`cargo clippy --all-targets -- -D warnings`；`cargo test --all --all-targets`；`python3 tools/spec_fixtures.py check`（`spec fixtures: ok (1)`）；`python3 tools/run_fixtures.py`（`fixtures: ok (1607)`）；`python3 tools/audit_user_visible_failure_policy.py`（`user-visible failure policy audit: ok`）；`git diff --check`。
- 已更新 `TODO.md` 和 `TODO-5.md`，将 `P6-T04` 标记为 `[DONE]` 并填写完成记录；`PLAN.md` 阶段级 sequencing 未变化，无需更新。
- 提交前检查发现未跟踪文件 `REFLECTION.md`，该文件不是本任务产生的改动，不纳入本次提交。
