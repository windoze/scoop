# Claude 执行计划

## 范围

- 以 `TODO.md` 作为权威任务清单。
- 只完成第一个标题未带 `[DONE]` 的任务，然后停止。
- 选定当前任务前不做开放式历史问题扫描。
- 本文件只记录可执行计划和进度检查点；不记录隐藏推理链。

## 初始计划

1. 读取 `TODO.md`，按标题前缀确认第一个未完成任务。
2. 只检查最新提交中是否有与该任务直接相关的未完成事项。
3. 阅读选定任务的细节、依赖、验证要求和完成记录要求。
4. 只检查该任务需要的代码、测试、fixture、文档和任务说明。
5. 按任务要求实现，不缩小范围，不使用 workaround。
6. 若正确实现被具体 blocker 或缺失前置项阻塞，则在 `TODO.md` 添加最小前置任务，保持当前任务未完成，提交该任务清单变更后停止。
7. 先运行定向验证，再运行任务要求的更宽验证。
8. 修复当前任务范围内的回归或 warning。
9. 将任务标题标记为 `[DONE]`，并更新完成记录。
10. 只有阶段级顺序、依赖、假设或完成条件变化时才更新 `PLAN.md`。
11. 检查 `git status`、`git diff` 和最近提交；用清晰的任务标签提交所有相关变更。
12. 提交后停止，并报告完成任务和验证结果。

## 进度

- 2026-05-20：已在任务选择和实现前写入初始执行计划。
- 2026-05-20：已从 `TODO.md` 选定第一个未完成任务 `P7-T01`：将 `native-build` 扩展到所有 loaded source cones。
- 2026-05-20：下一步检查 P7 计划段落、最新提交、现有 build/toolchain native-build 代码和 source cone graph API，然后新增 dependency-lib native fixture 并定向验证。
- 2026-05-20：最新提交是 `[P6-T02] Implement prelude package list`；没有直接阻塞 `P7-T01` 的未完成最新提交事项。
- 2026-05-20：实现计划已细化：把 `commands/build.rs` 中 consumer-only native 编译替换为 source cone graph node 遍历；object 名包含 cone id/name；每个 node 的 C/C++ flags 只作用于本 node；新增 `bin` 依赖拥有 `native/add.c` 的 `lib` 的 `run_pass_cone` fixture。
- 2026-05-20：新增范围内 cache 正确性步骤：将 graph native C/C++ source 输入纳入 cone build fingerprint，避免 dependency native 变更错误命中增量缓存。
- 2026-05-20：实现检查点已通过定向验证：新增 dependency native fixture、既有 consumer native fixture、incremental fingerprint 单测均通过。下一步运行更宽的 build/lint/fixture 验证。
- 2026-05-20：更宽验证已通过：`cargo build`、`cargo clippy --all-targets -- -D warnings`、`cargo test --all --all-targets`、`cargo run -p scoop -- test tests/fixtures/run_pass_cone/` 和完整 `cargo run -p scoop -- test`。
- 2026-05-20：`TODO.md` 已更新：将 `P7-T01` 标记为 `[DONE]`，写入完成记录，并将下一任务设为 `P7-T02`。`PLAN.md` 未修改，因为阶段级计划没有变化。
