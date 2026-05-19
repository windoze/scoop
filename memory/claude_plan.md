执行计划

状态：已确定当前任务。

计划：
1. 读取 `TODO.md`，按文件顺序找到第一个标题未带 `[DONE]` 的任务。
2. 检查最近提交是否明确提到与该任务直接相关的未完成问题；如有，作为当前任务范围或在 `TODO.md` 中补充前置依赖。
3. 阅读该任务涉及的代码、测试和文档，只做与当前任务相关的上下文调查。
4. 若任务可直接完成，则实现最小且完整的变更；若发现阻塞性的缺失功能或规格不匹配，则在 `TODO.md` 中插入最小前置任务并停止。
5. 运行任务要求的验证命令和必要的回归测试；遇到失败则修复当前任务范围内的问题并重测。
6. 任务完成后，将该任务标题加上 `[DONE]`，更新完成记录；仅当阶段级计划发生变化时才更新 `PLAN.md`。
7. 检查工作区差异，提交本次任务涉及的所有未提交文件，提交信息使用任务编号开头。
8. 完成一个任务后停止，不继续处理后续任务。

进度记录：
- 已创建初始执行计划。
- 已读取 `TODO.md`，第一个未完成任务为 `P1-T03：删除或改写 archive fixtures 与 archive-only tests`。
- 已检查最近提交摘要，最新提交为 `[P1-T02] Remove archive dependency build flow`，未发现需要先处理的直接未完成事项。
- 当前任务重点：移除 `tests/fixtures/typecheck_cone_archive/**` 对 active fixture suite 的影响，并删除或禁用 `crates/scoop/src/fixtures/mod.rs` 中 archive fixture runner active path。
- 调查结论：`deps_visibility_filter` 与 `typealias_export_generic` 可改写为 source-only `typecheck_cone` fixtures；`program_boundary_export_entry_points` 可改写为 manifest-backed `run_pass_cone` fixtures；`deps_api_injection`、`annotation_retention_export`、`pre_specialize_*` 覆盖旧 `.cone` API/metadata 注入，删除不迁移。
- 已完成实现草稿：删除 `tests/fixtures/typecheck_cone_archive/**` 文件，新增 source-only 替代 fixtures，并从 active fixture routing 中移除 `typecheck_cone_archive`；历史 archive runner helper 以 `#[cfg(any())]` 隔离，不能被 `scoop test` 路由调用。
- 已完成验证：定向迁移 fixtures、retired archive path、`cargo test -p scoop --bin scoop`、`cargo test -p scoopc cone:: -- --nocapture`、`cargo build`、`cargo clippy --all-targets -- -D warnings`、完整 `cargo run -p scoop -- test`、`cargo test --all --all-targets` 均通过。
- 已更新 `TODO.md`：将 `P1-T03` 标记为 `[DONE]`，补充完成记录、fixture 删除/改写理由和验证结果；`PLAN.md` 无阶段级变化，未更新。
