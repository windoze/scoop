# 执行计划

## 当前约束
- 以 `TODO.md` 为唯一任务顺序来源，先完成第一个标题未带 `[DONE]` 的任务。
- 本次只处理 `MIR-T00`，完成并提交后停止，不推进 `MIR-T01`。
- 不采用规避、弱化 fixture、临时 shim 或偏离规格的实现。
- 若遇到阻塞当前任务的真实前置缺口，只更新 `TODO.md` 插入最小前置任务、提交并停止。
- 完成后必须更新 `TODO.md` 标题为 `[DONE]`，补充完成记录，运行任务要求的定向验证并提交。

## 当前任务
- 第一项未完成任务：`MIR-T00：建立 MIR placeholder inventory 与 gap ownership map`。
- 目标：新增可执行 inventory，覆盖 refactor MIR/materialized MIR 可能泄漏的 placeholder/fallback，并为每个 gap 记录 surface、reason、`PIPELINE_GAPS.md` 编号、disposition、owner task、是否必须从 refactor production path 消除。
- 验证要求：`cargo test -p scoopc --no-default-features refactor_mir_placeholder_inventory`，以及搜索 `Todo(` / MIR Todo enum surface 并确认全部可追溯。

## 执行步骤
1. 查看最新提交信息，确认是否有与 `MIR-T00` 直接相关的未完成问题。
2. 用定向搜索定位 MIR/materializer/refactor pipeline 中的 placeholder 构造和 no-op rewrite 点。
3. 读取相关 MIR/HIR inventory、MIR 类型定义、materialize 入口和现有测试组织，确定最小实现位置。
4. 新增或接入 `refactor_mir_placeholder_inventory` 测试模块，建立静态 inventory 与源码扫描断言。
5. 运行 `MIR-T00` 指定测试和搜索命令；如失败，修正 inventory 或实现边界。
6. 更新 `TODO.md`：将 `MIR-T00` 标题加 `[DONE]`，补充完成记录和验证结果。
7. 检查工作区差异，提交本次任务相关改动，提交信息使用 `[MIR-T00] ...` 格式。

## 进度记录
- 已读取 `TODO.md`，确认第一项未完成任务是 `MIR-T00`。
- 已写入本轮可审计计划；下一步检查最新提交并定位相关实现文件。
- 最新提交为 `52da50b5 Update plan`，未提示与 `MIR-T00` 直接相关的未完成问题。
- 已定位当前 placeholder 来源：`mir/lower.rs` 的 MIR Todo 构造、HIR Todo passthrough，以及 `mir/materialize.rs` 的 Todo no-op rewrite/隐式 unwind 不处理路径。
- 下一步新增 `crates/scoopc/src/mir/placeholder_inventory.rs` 并在 `mir/mod.rs` 中以 `#[cfg(test)]` 接入，建立静态 inventory 与源码扫描断言。
- 已新增 `mir::placeholder_inventory` 测试模块，并修正扫描器以区分真实构造、HIR handoff 和 materializer no-op rewrite。
- 已通过：`cargo test -p scoopc --no-default-features refactor_mir_placeholder_inventory`。
- 已执行 `rg 'Todo\(|Item::Todo|StatementKind::Todo|Rvalue::Todo|TerminatorKind::Todo|UnwindAction::Todo' crates/scoopc/src`；命中可归入新 inventory、HIR inventory、MIR/HIR 构造点、materializer no-op、现有 verifier/preflight/summary/pass/codegen consumer 或 legacy/effect-lowered consumer。
- 已通过：`cargo clippy -p scoopc --no-default-features --all-targets -- -D warnings`。
- 已更新 `TODO.md`：`MIR-T00` 标题已标记 `[DONE]`，完成记录包含实现摘要、指定验证、搜索审计和 clippy 结果。
- 下一步检查工作区差异并按要求提交本次任务改动。
