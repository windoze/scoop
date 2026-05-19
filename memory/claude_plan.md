# Claude 执行计划

## 范围

- 只处理 `TODO.md` 中第一个未完成任务。
- 以 `TODO.md` 作为任务顺序、依赖、验证要求和完成状态的唯一来源。
- 完成该任务后停止；若被具体前置问题阻塞，则只添加/提交必要前置任务并停止。

## 执行步骤

1. 读取 `TODO.md`，识别标题未加 `[DONE]` 的第一个任务。
2. 仅检查最新提交中是否有与该任务直接相关的未完成事项。
3. 阅读该任务要求，并检查最小相关代码范围。
4. 按规格实现任务，不使用 workaround，不弱化行为。
5. 添加或更新该任务所需的聚焦测试/夹具。
6. 运行任务指定验证，以及必要的相关目标测试。
7. 如果验证暴露阻塞当前任务的实现/规格缺口，则在 `TODO.md` 添加最小前置任务，保持当前任务未完成，提交后停止。
8. 如果任务完成，则在 `TODO.md` 标题中加 `[DONE]` 并补充完成记录。
9. 文档回写后运行最终相关验证。
10. 检查 git status/diff/log，只提交本任务相关改动。
11. 提交后停止，不开始下一个任务。

## 进度记录

- 已在任务发现前写入初始计划。
- 已选择第一个未完成任务：`P1-T02`（`移除 normal build 的 .cone dependency flow`）。
- 直接目标：从 normal `scoop build/run` 和 frontend active path 中移除 `.cone` archive dependency 搜索、加载和注入，并用目标 grep 与 `cargo build` 验证。
- 检查发现 active flow：`commands/build/deps.rs` 搜索 `SCOOP_CONE_PATH` / `cone` / `deps`，`load_build_context_with_options` 加载 archive deps，`ProjectContext` 携带 `Vec<ConeArchiveApi>`，`run_frontend` 注入 archive public API。
- 编辑计划：删除 active build dependency loader 模块，将 `ProjectContext` 简化为 input-only，移除 frontend 注入，并把消费 archive 的 build 测试替换为 negative/no-consume 断言。
- 实现完成：从 active build 删除 `commands/build/deps.rs`，`ProjectContext` 改为 input-only，frontend archive 注入已移除，并新增 build 单元测试证明 manifest dependency 旁的无效 `.cone` 会被 normal build 忽略。
- 验证完成：grep 确认 Rust 源码中无 `SCOOP_CONE_PATH` / `load_dependency_graph`，`crates/scoop/src/commands` 中无 archive API 符号，`frontend.rs` 中无 archive API 符号；`cargo fmt`、限定目标单元测试、`cargo build`、`cargo clippy --all-targets -- -D warnings`、`cargo test -p scoop --bin scoop` 均通过。
