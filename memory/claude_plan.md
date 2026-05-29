# 执行计划

本文件记录可共享的执行计划、关键进度和决策，不记录私有推理过程。

## 范围

- 目标：依据 `TODO.md` 的顺序完成第一个标题未带 `[DONE]` 的任务，然后停止。
- 当前任务：`P6-T03R：Review 旧 surface 与回归审计`。
- 约束：不跳过 review 任务；不拆分任务；不把 `SPEC_FIX.md` / `OVERLOAD_RESOLUTION.md` 设计基线当作 active old-surface violation。

## 步骤

1. 读取 `TODO.md`，识别第一个未完成任务及其验证要求。
2. 检查最近提交是否提到与当前任务直接相关的未完成事项。
3. 阅读 `TODO-5.md` 中 `P6-T03` / `P6-T03R` 的任务体，并对照 P0-T01 inventory。
4. 抽样复查旧 surface 命中分类，确认剩余命中属于 negative fixture、diagnostic 文本、内部术语、注释、active 非 handler `with` 语法或允许的 spec removal 说明。
5. 验证 overload/codegen baseline fixtures、overload diagnostics policy 和 `.cone` / `scoopir` public export 行为实际可运行。
6. 按要求运行格式化、clippy、targeted fixtures、完整 Rust 测试、spec check、完整 fixture suite 和 diff whitespace check。
7. 更新 `TODO.md` 与 `TODO-5.md`，将 `P6-T03R` 标记为 `[DONE]` 并填写完成记录；仅在阶段计划变化时更新 `PLAN.md`。
8. 检查 git 状态和差异，只提交本任务相关文件，然后停止。

## 当前进度

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
