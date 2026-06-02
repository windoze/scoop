# 执行计划

## 当前约束

- 以 `TODO.md` 为任务顺序和完成状态的唯一来源。
- 只完成第一个标题未带 `[DONE]` 的任务，完成后提交并停止。
- 如遇阻塞当前任务的缺失功能、规格不匹配或未排期失败测试，先修复或在 `TODO.md` 中插入最小前置任务并提交后停止。
- `PLAN.md` 只在阶段级计划、依赖或完成标准变化时更新。
- 交互和记录使用中文。

## 步骤

1. 检查 `TODO.md`，找出第一个标题未标记 `[DONE]` 的任务，并阅读该任务的依赖、要求、验证和完成记录。
2. 检查最近提交信息；如果它明确提到与当前任务直接相关的未完成问题，将其纳入当前任务或作为前置任务记录到 `TODO.md`。
3. 读取当前任务相关代码、测试、规格或文档，确认需要修改的最小范围。
4. 按任务要求实现；如发现阻塞性规格缺口或缺失语言功能，停止实现并改为更新 `TODO.md` 的前置任务。
5. 运行 `cargo fmt`，然后运行 `cargo clippy --all-targets -- -D warnings`。
6. 在 lint 通过后运行相关测试；若代码变更影响全局行为，再运行 `cargo test --all --all-targets` 和 `python3 tools/run_fixtures.py`，超时不少于 30 分钟。
7. 修复所有未排期失败测试或在 `TODO.md` 中添加最小必要任务，不能留下未处理失败。
8. 更新 `TODO.md`：将完成任务标题加 `[DONE]`，填写完成记录、验证命令和结果；仅在阶段级计划变化时更新 `PLAN.md`。
9. 更新本文件记录关键进展。
10. 检查 `git status`、`git diff`、`git log --oneline -10`，确认只提交相关变更；提交一次清晰的任务提交。
11. 停止，不处理下一个任务。

## 当前状态

- 已确认第一个未完成任务为 `TODO-3.md` 的 `T3-04D`。
- 最近提交 `[T3-04R] Schedule fourth review follow-up` 明确排期该任务；无需新增前置任务即可先处理 `T3-04D` 本身。
- 下一步聚焦搜索任务列出的残余 source-span、ABI 合成、intrinsic/root、dispatch/text fallback、verifier 与 dependency gate 缺口。
- 已实施核心修复：移除 LIR reflection source-span facts；LIR intrinsic/source-call metadata 不再从 HIR `(source_path, span)` map 发布；MIR intrinsic facts 改为消费 HIR 显式 named intrinsic call-site metadata而非扫描 source signatures；P4/P5 verifier 开始拒绝 direct/dispatch 的 DynamicFallback/bodyless target；LLVM class ctor source-span handoff 已从 P6 context 移除；reachability 缺 target 改为报错。
- 已完成 T3-04D 并更新 `TODO.md` / `TODO-3.md`。最终验证通过：`cargo fmt`、`cargo clippy --all-targets -- -D warnings`、`python3 tools/dependency_gate.py`、`cargo test --all --all-targets`、`cargo build -p scoop -p scoopc`、`python3 tools/spec_fixtures.py check`、`python3 tools/run_fixtures.py`（1664 checks）。
- 下一步检查 diff/status/log 后提交本任务变更。
