# 执行计划

## 当前状态

- 本轮目标：按照 `TODO.md` 的顺序完成第一个未标记 `[DONE]` 的任务，然后停止。
- 约束：先识别第一个未完成任务，不做开放式历史问题扫描；如遇到阻塞当前任务的真实前置问题，则按要求更新 `TODO.md` 并提交后停止。
- 说明：此文件记录执行计划、关键决策依据和进度更新，不包含私密逐步推理。

## 初始步骤

1. 读取 `TODO.md`，按标题是否带 `[DONE]` 判断第一个未完成任务。
2. 读取最新提交信息，确认是否明确提到与当前任务直接相关的未完成事项。
3. 读取当前任务相关的代码、测试、规范和已有记录，只收集完成该任务所需上下文。
4. 判断任务能否直接完成；如存在必须先修复的缺口，则在 `TODO.md` 中加入最小前置任务并提交后停止。
5. 如可直接完成，实施最小正确修改，避免 workaround、fixture-only hack 或弱化任务要求。
6. 运行当前任务要求的验证命令，并补充必要的定向测试。
7. 更新 `TODO.md`：在任务标题前加 `[DONE]`，填写或刷新完成记录。
8. 仅当阶段级计划变化时更新 `PLAN.md`。
9. 更新本文件的执行记录。
10. 将本轮所有相关变更提交到 Git，然后停止。

## 进度记录

- 已识别第一个未完成任务：`HIR-T05：为 typealias/type/object/extension property 建立 HIR declaration graph`。
- 最新提交 `47cc9ab9 [HIR-T04] Record declaration graph prerequisite` 只记录 `HIR-T04` 依赖 `HIR-T05`，与当前任务直接相关但不改变当前执行顺序。
- 当前工作树已有本文件新增变更；后续提交需包含本轮所有相关变更。
- 已实施核心结构：`hir::File` 新增 `decls` declaration graph，lowering 为 typealias、nominal type、object、extension property 生成结构化声明，移除 `typealias`/`type`/`object`/`extension_property_no_getter` 的 HIR `Item::Todo` 构造点。
- 已更新 placeholder inventory 与 refactor HIR stage 定向测试，使 `refactor_hir_decls` 覆盖声明图关键字段。
- 已将 declaration graph lowering helper 拆分到 `crates/scoopc/src/hir/lower/decls.rs`，避免继续扩大 `hir/lower/mod.rs`。
- 已将 property declaration check 接入 refactor typed HIR 入口，确保缺 getter 的 extension property 在进入 HIR 前给出专用 typecheck diagnostic。
- 已新增 `tests/fixtures/hir/refactor_decl_graph.scoop/.hir`，并同步受声明图输出影响且可通过 no-Todo gate 的既有 HIR golden。
- 尝试运行整个 `tests/fixtures/hir` 目录时，两个既有 fixture 仍因 `ExprKind::Missing` 被当前 no-Todo verifier 拒绝；这是 HIR-T11 范围的历史占位问题，不阻塞 HIR-T05 的声明图验证。误截空的两个 golden 已恢复。
- 已完成验证：`refactor_hir_decls`、`refactor_hir_placeholder_inventory`、`refactor_hir_no_todo`、更新后的 HIR fixtures、`dump-hir refactor_decl_graph`、`cargo test -p scoop --no-default-features dump_hir`、`cargo clippy -p scoopc -p scoop --no-default-features --all-targets -- -D warnings` 均通过。
- 已更新 `TODO.md`，将 `HIR-T05` 标记为 `[DONE]` 并写入完成记录。

## 当前任务执行计划

1. 检查 refactor HIR 的 `Item`、stable dump、placeholder inventory/verifier 与 lowering 入口，定位 `typealias`、`type`、`object`、`extension_property_no_getter` 当前构造点。
2. 检查 AST、resolver/typecheck 输出中可用的 declaration 元数据，确定 HIR declaration graph 需要消费的最小稳定数据。
3. 扩展 HIR 表达或 side table，覆盖 typealias、nominal type declaration、object declaration、extension property getter/setter contract。
4. 修改 refactor HIR lowering，生成 declaration graph，不再输出上述 `Item::Todo`。
5. 修改 stable dump 与 completeness verifier/inventory 测试，使 declaration graph 可见且 no-Todo 检查继续生效。
6. 新增或更新 HIR fixtures 与单测，覆盖 typealias、class、struct、enum、interface、object、extension property。
7. 运行 `cargo test -p scoopc --no-default-features refactor_hir_decls`、相关 `dump-hir` fixture 验证、必要 no-Todo 测试与 clippy。
8. 更新 `TODO.md` 完成记录并标记 `[DONE]`，提交本轮变更后停止。当前状态：已完成，待提交。
