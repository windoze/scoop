# 当前执行计划

说明：本文件记录本次任务的可执行计划、进度和关键决策；不包含私有推理过程。

## 步骤

1. 读取 `TODO.md`，按文件顺序定位第一个标题未带 `[DONE]` 的任务。
2. 检查该任务的正文、依赖、验证要求和完成记录；必要时查看最近提交是否提到与该任务直接相关的未完成事项。
3. 在不做开放式历史问题清扫的前提下，读取完成该任务所需的最小代码上下文。
4. 如任务可直接完成，实施最小正确修改；如存在必须先修复的具体阻塞项，则把最小前置任务插入 `TODO.md` 并停止。
5. 运行格式化、lint、相关测试，以及任务要求的完整验证；若发现未被明确排期的失败测试或 fixture，修复或在 `TODO.md` 中排期。
6. 将当前任务标题标记为 `[DONE]`，更新其完成记录；仅在阶段级计划确实变化时更新 `PLAN.md`。
7. 检查 git 状态和差异，提交本次任务相关的所有未提交修改。
8. 完成一个任务后停止，不继续处理下一个任务。

## 进度

- 已创建本计划文件，下一步读取 `TODO.md` 定位第一个未完成任务。
- 已定位第一个未完成任务：`P5-T02`（实现 Phase D-E specificity 与 ambiguity diagnostics）。最近提交为 `P5-T01R`，直接前置任务已完成，没有发现需要先处理的未完成提交事项。
- 下一步读取 `OVERLOAD_RESOLUTION.md` 相关章节和当前 call/typecheck 实现，确认最小修改面。
- 已确认当前实现仍在多个 applicable 候选时直接报 `ambiguous_overload`；本次将新增共享 specificity helper，并接入普通函数、成员/扩展/where-bound 方法与构造器选择路径。
- 已完成共享 specificity helper 与主要调用路径接入；新增/更新 infer 与 typecheck targeted fixtures。重建 `scoop`/`scoopc` 后，`tests/fixtures/infer` 与 `tests/fixtures/typecheck` 均已通过。
- 下一步按要求运行 `cargo fmt`、`cargo clippy --all-targets -- -D warnings`、完整 Rust 测试和完整 fixture suite。
- 已通过 `cargo fmt`、`cargo clippy --all-targets -- -D warnings`、`cargo test --all --all-targets`；正在运行完整 `python3 tools/run_fixtures.py`。
- 完整 `python3 tools/run_fixtures.py` 已通过。下一步更新 `TODO.md` / `TODO-5.md` 的 P5-T02 完成记录并提交。
- 已将 P5-T02 在 `TODO.md` 与 `TODO-5.md` 标记为 `[DONE]` 并写入完成记录。下一步检查 git 状态/差异后提交。
