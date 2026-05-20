# Claude 执行计划

## 范围

- 以 `TODO.md` 作为权威任务列表。
- 定位第一个标题未带 `[DONE]` 的任务。
- 只完成该任务，提交后停止，不继续下一个任务。

## 执行步骤

1. 读取 `TODO.md`，确认第一个未完成任务及其验证要求。
2. 按任务说明检查最近提交，只处理与当前任务直接相关的未完成问题。
3. 只阅读实现当前任务所需的代码、测试和设计说明。
4. 按规格实现任务，不引入 workaround 或削弱行为。
5. 先运行定向验证，再运行任务或仓库要求的更广验证。
6. 在 `TODO.md` 中给完成任务标题添加 `[DONE]`，并填写完成记录。
7. 关键步骤完成或计划变化时更新本文件。
8. 提交前检查 `git status`、`git diff` 和最近提交记录。
9. 用清晰的任务标签提交本任务全部相关改动。
10. 停止，不开始 `TODO.md` 中的下一个任务。

## 进度

- 已记录初始执行计划。
- 已读取 `TODO.md`；第一个未完成任务是 `P5-T04`：生成 per-cone init routine 与 final system entry 调用骨架。
- 最近提交为 `[P5-T03] Preserve cone identity through compilation`，属于相关背景，但没有记录会阻塞 `P5-T04` 的未完成事项。
- 实现路径：从已按 source cone graph 扁平化的 source 顺序派生 linked cone 顺序，生成稳定 internal cone init stub，并在 runtime 初始化后、用户 `main` 调用前发出 final entry 调用。
- 已实现 cone init routine plan、稳定 internal stub emission，以及 `main` wrapper 中用户入口前的 cone init 调用。
- 已新增 LLVM build fixture `tests/fixtures/build/per_cone_init_routine_emit_llvm.scoop`，检查 stub 定义和调用顺序。
- 验证已完成：定向 fixture、build fixture suite、`cargo build`、`cargo clippy --all-targets -- -D warnings`、`cargo test --all --all-targets` 和完整 `cargo run -p scoop -- test` 均通过；期间更新了 entry-wrapper 单测，使其在统计用户 entry 调用时忽略 compiler-private cone init calls。
- 已更新 `TODO.md`：`P5-T04` 标记为 `[DONE]`，当前状态改为下一任务 `P6-T01`，并写入完成记录。
