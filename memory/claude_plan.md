# Claude Plan

## 执行摘要
- 本次调用先记录执行计划，再读取 `TODO.md` 作为索引，定位第一个未完成的详细任务。
- 然后检查该任务对应的详细要求、依赖、最近提交是否有与该任务直接相关的未完成事项。
- 若任务可直接完成，则实现、测试、更新 TODO 记录并提交一次 git commit 后停止。
- 若存在阻塞且必须先补前置任务，则仅新增最小必要前置任务、同步 `TODO.md`，提交后停止。

## 步骤计划
1. 读取 `TODO.md`，确认索引结构与指向的详细任务文件。
2. 按索引顺序读取相关 `TODO-Px.md`，定位第一个标题未标记 `[DONE]` 的详细任务。
3. 查看最近提交，判断是否存在与该任务直接相关但未完成的问题需要并入当前任务或记录为前置依赖。
4. 阅读当前任务要求、限制、验证方式与依赖，确定需要改动的代码和测试范围。
5. 实现任务；若发现阻塞当前任务且不能规避的问题，则在对应 `TODO-Px.md` 中添加最小前置任务并同步 `TODO.md`。
6. 运行相关验证，至少覆盖任务要求的测试；如有必要，运行 `cargo fmt`、`cargo test`、`cargo clippy --all-targets -- -D warnings` 或更小范围等效验证。
7. 更新 `memory/claude_plan.md` 记录关键进展与计划变更。
8. 任务完成后，将对应详细任务标题标记为 `[DONE]`，补全完成记录，并在需要时同步 `TODO.md`；仅在阶段计划变化时更新 `PLAN.md`。
9. 按任务 id 生成清晰的提交信息，提交当前所有应纳入本次任务的未提交改动，然后停止，不继续下一个任务。

## 进度记录
- 已创建初始执行计划，待开始读取任务索引与详细任务文件。
- 已读取 `TODO.md`，定位首个未完成详细任务为 `TODO-P6-part2.md` 中的 `P6-T02n`。
- 已检查最近提交：`[P5-T07b] Demote resume interfaces to packings` 与当前任务直接相关，因此下一步先核对现有实现/测试是否已覆盖 `P6-T02n` 的全部要求，再决定补齐范围。
- 已完成首轮实现：`crates/scoopc/src/llvm/codegen/effect_refactor/{types,layout}.rs` 中把 LLVM ABI/query 的公开叙事从 `resume interface` 收口到 `resume packing`，新增 `surface_resume_method_layout(...)` 直接查询，并同步更新 continuation/surface-resume 相关测试用法与断言文案。
- 下一步：运行 `cargo fmt`、定向 `cargo test`、`dump-effect-lowered` 验证以及 `cargo clippy`，若通过则更新 `TODO-P6-part2.md` / `TODO.md` 并提交。
- 已完成验证：`cargo fmt --all`、3 组定向 `cargo test -p scoopc ...`、`cargo test -p scoopc refactor_llvm_`、2 个 `dump-effect-lowered` fixture，以及 `cargo clippy -p scoopc -p scoop --all-targets -- -D warnings` 全部通过。
- 已更新 `TODO-P6-part2.md` 与 `TODO.md`，将 `P6-T02n` 标记为 `[DONE]` 并写入完成记录；下一步只剩检查工作区并提交本次任务。
