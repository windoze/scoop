执行计划

1. 读取 `TODO.md`，按文件顺序找到第一个标题未带 `[DONE]` 的任务，并只处理该任务。
2. 查看该任务的要求、依赖、验证命令和完成记录；必要时查看 `PLAN.md` 与最近提交，确认是否存在直接相关的未完成事项。
3. 建立最小实现范围，优先修复会阻塞当前任务或使其行为不符合规格的问题，不做绕路实现。
4. 按需修改代码、测试、夹具或文档；若发现必须先处理的新前置条件，则更新 `TODO.md` 并停止在该前置任务上。
5. 运行格式化、lint、相关测试，并按要求逐步扩大到完整验证；发现未排期失败时，修复或在 `TODO.md` 中排入正确位置。
6. 完成后将当前任务标题加上 `[DONE]`，更新完成记录；仅在阶段计划变化时更新 `PLAN.md`。
7. 检查 git 状态与差异，提交本次任务的所有相关修改，然后停止，不继续下一个任务。

进度记录

- 已创建本执行计划，下一步读取 `TODO.md`。
- 已确认首个未完成任务为 `P10-T04-c-2`：把 cache-hit dependency cone artifact 中的 `LateLoweredProgram`、`LirFacts`、`TypeStore` 与对象文件 handoff 到 LLVM stage，使 dep callable layout 查询不再依赖 dep AST 出现在 consumer 自身 LIR program。
- 最近提交 `e82e94c5 [P10-T04-c-1] Stabilize LIR callable ABI keys` 正是当前任务强前置，未发现需要改排的额外前置。
- 下一步检查 artifact 读写、frontend/pipeline stage 输入和 LLVM ABI materializer 的现有实现，按最小改动接入多 dep cached handoff。
- 已开始实现：artifact schema 增加 `type_store.bin`，`FrontendOutput` 聚合 cache-hit dep 的 decoded LIR/LirFacts/TypeStore/object 路径，LLVM stage/output/emit handoff 增加 `CachedDepArtifactHandoff`，ABI materializer 开始按 primary/cached-dep origin 合并 callable 与 step layout 查询面。
- 已完成实现与验证：`P10-T04-c-2` 已接通 cached dep LIR/LirFacts/TypeStore 到 LLVM ABI materializer，处理旧 artifact schema 为 cache miss，补充定向回归测试，并完成 `cargo fmt`、`cargo clippy --all-targets -- -D warnings`、`cargo test --all --all-targets`、`cargo run -p scoop -- test --fixtures tests/fixtures/run_pass_cone`、`cargo run -p scoop -- test`。
- 已将 `TODO.md` 与 `TODO-7.md` 中 `P10-T04-c-2` 标记为 `[DONE]` 并写入完成记录；下一步检查 diff 并提交本任务。
