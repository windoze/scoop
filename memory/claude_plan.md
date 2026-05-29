# 执行计划

本文件记录可共享的执行计划、关键进度和决策，不记录私有推理过程。

## 范围

- 目标：依据 `TODO.md` 的顺序完成第一个标题未带 `[DONE]` 的任务，然后停止。
- 当前任务：`P6-T03：执行旧 surface 与 overload/codegen 回归审计`。
- 约束：不跳过 review 任务；不拆分任务；不把 `SPEC_FIX.md` / `OVERLOAD_RESOLUTION.md` 设计基线当作 active old-surface violation。

## 步骤

1. 读取 `TODO.md`，识别第一个未完成任务及其验证要求。
2. 检查最近提交是否提到与当前任务直接相关的未完成事项。
3. 阅读 `TODO-5.md` 中 `P6-T03` 的任务体，并对照 P0-T01 inventory。
4. 审计旧 surface 命中并分类为 negative fixture、diagnostic 文本、内部术语、archive/design baseline 或 active bug。
5. 验证 overload/codegen baseline fixtures、overload diagnostics policy 和 `.cone` / `scoopir` public export 行为。
6. 按要求运行格式化、clippy、spec check、targeted fixtures、完整 Rust 测试和完整 fixture suite。
7. 更新 `TODO.md` 与 `TODO-5.md`，将 `P6-T03` 标记为 `[DONE]` 并填写完成记录；仅在阶段计划变化时更新 `PLAN.md`。
8. 检查 git 状态和差异，只提交本任务相关文件，然后停止。

## 当前进度

- 已读取 `TODO.md`；第一个未完成任务是 `P6-T03`。
- 已检查最近提交：`9e3d6476 [P6-T02R] Review fixture synchronization`，未发现直接相关的未完成事项。
- 已读取 P0-T01 inventory；确认审计范围覆盖 `perform`、handler `with`、tuple `._N`、旧 f-string 插值 / brace escape、`@Inline`、`AnyRef` / `AnyValue`、隐式 public API export 和 operator-like declarations。
- 旧 surface 审计结果：实际 `perform` keyword 只出现在 `tests/fixtures/parse/perform_keyword_removed.scoop` negative fixture；旧 handler `with` 只出现在 `tests/fixtures/parse/handle_with_keyword_removed.scoop` negative fixture；tuple `._0` / with-path `_0` 只出现在 typecheck negative fixtures；active code / sysroot 未发现 `@Inline`、`AnyRef` / `AnyValue` positive 定义或 alias；f-string `{...}` 命中为 literal-brace 覆盖或 `${...}` 表达式内部 brace，不是旧插值。
- 已验证 overload/codegen baseline：`overload_concrete_bug.scoop`、`overload_arity_bug.scoop`、`overload_gvc_ok.scoop` 均通过。
- 已验证 overload diagnostics：no-applicable、ambiguity、conflicting overload、generic shape mismatch、vararg overlap、infer ambiguity targeted fixtures 均通过；`python3 tools/audit_user_visible_failure_policy.py` 通过。
- 已验证 `.cone` / `scoopir` export：`public_api_filter.scoop` 确认 `.scoopir` 只导出显式 `public`；`source_path_dependency_public_call`、`source_path_dependency_private_hidden`、`source_path_dependency_internal_hidden` 确认 public 可见且 private/internal 保持隐藏。
- 已通过完整验证：`cargo fmt`；`cargo clippy --all-targets -- -D warnings`；`python3 tools/spec_fixtures.py check`（`spec fixtures: ok (1)`）；`cargo test --all --all-targets`；`python3 tools/run_fixtures.py`（`fixtures: ok (1607)`）；`git diff --check`。
- 已更新 `TODO.md` 和 `TODO-5.md`，将 `P6-T03` 标记为 `[DONE]` 并填写完成记录；`PLAN.md` 阶段级 sequencing 未变化，无需更新。
- 提交前检查发现未跟踪文件 `REFLECTION.md`，该文件不是本任务产生的改动，不纳入本次提交。
