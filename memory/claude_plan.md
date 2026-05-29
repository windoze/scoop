# 当前执行计划

## 目标

- 以 `TODO.md` 为唯一任务顺序和完成状态来源。
- 找到第一个标题未带 `[DONE]` 的任务。
- 完整实现该任务，完成验证，更新任务记录，提交一次 Git commit，然后停止。

## 执行步骤

1. 读取 `TODO.md`，确定第一个未完成任务及其验收要求。
2. 检查最近提交是否明确提到与该任务直接相关的未完成问题。
3. 读取该任务涉及的代码、测试、规格或文档，确认实现边界和依赖。
4. 如任务存在必须先修复的具体阻塞问题，按要求把最小前置任务写入 `TODO.md`，提交并停止。
5. 如无阻塞，实施当前任务所需的最小正确修改。
6. 运行 `cargo fmt`。
7. 运行 `cargo clippy --all-targets -- -D warnings`。
8. 运行当前任务相关的定向测试或 fixture。
9. 如代码变更影响编译或运行行为，运行完整验证：`cargo test --all --all-targets` 和 `python3 tools/run_fixtures.py`，均使用不少于 30 分钟超时。
10. 修复验证中发现且未被明确排期的失败；若失败需要新增前置任务，则更新 `TODO.md`、提交并停止。
11. 将当前任务标题前缀更新为 `[DONE]`，并补全 completion record，包括修改摘要和验证命令结果。
12. 检查 Git 状态和 diff，确保只提交本次任务相关变更以及必须包含的未提交状态。
13. 使用符合仓库风格的提交信息提交变更。
14. 停止，不继续处理下一个任务。

## 进度

- 已创建本计划文件。
- 已读取 `TODO.md`，第一个未完成任务是 `P6-T01R`：Review P6-T01 spec 回写完整性。
- 已读取 `TODO-5.md` 中的 `P6-T01R` 正文；最近提交为 `[P6-T01] Update active language specs`，未显式记录直接相关的未完成 issue。
- 已完成初步只读复核，发现 active spec 需要修补的回写缺口：full spec 中 tuple `var` 解构表述矛盾、closure `var` capture 替代建议缺少 fold/higher-order accumulation、split spec 的 `ref` / `value` bound 细则不足、overload 章节缺少 effective type / override / constructor / diagnostics 细节。
- 已修补 `SCOOP_FULL_SPEC.md` 与 `docs/spec/language_spec-part2.md` / `part3.md`，使 full/split spec 对上述规则保持一致。
- 验证已通过：`python3 tools/spec_fixtures.py check` 输出 `spec fixtures: ok (1)`；`git diff --check` 无输出。
- 因本轮只改 Markdown 正文与任务记录，不运行 Rust 编译、clippy 或全量 fixture 矩阵，沿用上一任务完成记录中的最近完整绿色矩阵。
- 已更新 `TODO.md` 与 `TODO-5.md`，`P6-T01R` 标题和索引均标记为 `[DONE]`，完成记录已填写。
- 下一步：检查 Git 状态、diff 和最近提交，确认提交范围后创建本任务 commit。
