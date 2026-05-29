# 当前执行计划

说明：此文件记录可审查的执行计划与进度，不包含私密推理链路。

## 初始计划

1. 读取 `TODO.md`，按标题是否带 `[DONE]` 判断第一个未完成任务。
2. 检查最近提交信息；若它明确提到与当前任务直接相关的未完成问题，将其纳入当前任务或作为前置任务记录到 `TODO.md`。
3. 阅读当前任务相关的代码、测试、文档与约束，确认实现范围与验证要求。
4. 按任务要求做最小正确实现；若发现阻塞性的缺失特性或规格不匹配，先更新 `TODO.md` 添加最小前置任务并停止。
5. 运行格式化、lint、相关测试；若有未被明确排期的失败，修复或在 `TODO.md` 中排入必要任务。
6. 任务完成后，更新 `TODO.md`：在任务标题前加 `[DONE]`，并补充完成记录与验证结果。
7. 仅在阶段计划发生变化时更新 `PLAN.md`。
8. 检查工作区差异，提交本次任务相关全部变更，然后停止，不进入下一个任务。

## 进度记录

- 已建立初始执行计划，下一步读取 `TODO.md` 确认当前任务。
- 已确认第一个未完成任务为 `P6-T01：回写 SCOOP_FULL_SPEC.md 与 split spec 的全部语言变更`。
- 最近提交 `53ed1da1 [P5-T05R] Update execution memory` 未明确留下与 P6-T01 直接相关的未完成实现问题。
- 当前工作区存在非本次创建的 `REFLECTION.md` 未跟踪文件；本任务不会修改或提交该文件，除非后续发现其与当前任务直接相关。
- 已完成只读审计：`SCOOP_FULL_SPEC.md` 和 `docs/spec/language_spec-part*.md` 均已有部分更新，但仍需补齐 enum `with` mismatch panic、refutable `val` panic、`!!` / `as` panic、closure `var` capture 禁止、default `internal`、operator requirement、overload rules，以及旧 sealed marker / `RuntimeError` 相关正向描述清理。
- 已完成 spec 回写与 split spec 手工同步；未发现 split spec 生成器。已通过 `python3 tools/spec_fixtures.py check` 与 `git diff --check`。
- 已将 `P6-T01` 在 `TODO.md` 与 `TODO-5.md` 标记为 `[DONE]`，并记录 docs-only 验证策略。
- 提交前复查 `git status --short`、`git diff`、`git log --oneline -10`；确认仅暂存本任务文件，保留无关未跟踪 `REFLECTION.md`。
