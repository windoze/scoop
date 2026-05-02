# Claude Plan

## 执行摘要

- 目标：先定位 `TODO.md` 对应的首个未完成详细任务，然后只完成该任务并停止。
- 约束：以 `TODO-Px.md` 为任务真源；若遇到阻塞，只补最小前置任务并同步 `TODO.md`；完成后必须测试、更新任务记录并提交 Git。
- 说明：这里记录的是可审计的执行计划与进度，不包含私有推理细节。

## 步骤计划

1. 读取 `TODO.md`，确认它引用了哪些详细任务文件。
2. 按任务顺序检查对应 `TODO-Px.md`，定位首个未完成的详细任务，并核对最近一次提交是否存在与该任务直接相关的未完成事项。
3. 阅读该任务所需的相关源码、测试、规范与上下文，确认验收标准、依赖与当前实现状态。
4. 如果任务可直接完成：实现最小正确修改，并补充或调整测试。
5. 如果存在真实阻塞：先修复阻塞；若当前调用内无法直接修复，则在正确的 `TODO-Px.md` 中插入最小前置任务，保持当前任务未完成，并同步 `TODO.md`。
6. 运行与该任务相关的验证命令，随后运行必要的格式化、测试与 `clippy` 检查，确保无警告。
7. 更新任务记录：在对应 `TODO-Px.md` 标记完成情况；如任务索引有变化，同步 `TODO.md`；仅在阶段计划变化时更新 `PLAN.md`。
8. 检查工作区变更，使用清晰提交信息创建一次 Git 提交，然后停止，不继续下一个任务。

## 进度日志

- 已创建本计划文件。
- 已确认 `TODO-P0.md`、`TODO-P1.md`、`TODO-P2.md` 全部条目均有明确完成记录；首个未完成详细任务为 `TODO-P3.md` 中的 `P3-T01`。
- 已核对最近一次提交信息：`[P2-T04R] Confirm typed HIR handoff is ready for P3`，其中未显式记录与 `P3-T01` 直接相关的未完成事项，因此按既定顺序直接执行 `P3-T01`。
- 当前执行重点：阅读 `P3-T01` 约束与现有 `dump-mir` / MIR pipeline 代码，建立 refactor direct-style MIR stage 入口、显式 stage 输出类型，以及 `dump-mir` 对该 stage 的新路由。
- 下一步：
  1. 检查 `effect_refactor_pipeline`、`dump_mir`、`fixtures` 与现有 MIR lowering/materialization 入口的当前组织方式。
  2. 识别哪些旧 MIR helper 可以作为中立 API 复用，哪些只允许留在 legacy 路径。
  3. 以最小改动实现新的 refactor MIR stage 与命令路由，并补齐定向测试。
- 已完成 `P3-T01` 实现：新增 `effect_refactor_pipeline::mir_stage` 与 `RefactorMirStageOutput`，把 refactor `dump-mir` 显式改成 `TypedHirStageOutput -> MIR stage output`，并将 callable body 查询面与可选 `materialized_mir` handoff 收口到 stage 输出上。
- 已完成验证：`cargo test -p scoopc --no-default-features refactor_direct_mir_stage`、`cargo test -p scoopc --no-default-features effect_refactor_pipeline`、`cargo test -p scoop --no-default-features dump_mir`、`cargo test -p scoop --no-default-features parity`、三条 `dump-mir` smoke，以及 `cargo clippy -p scoop -p scoopc --all-targets --no-default-features -- -D warnings` 全部通过。
- 已确认工作区里另有与本任务无关的现有改动（如 `dump_ir.rs`、`hir/lower/expr.rs`、`hir/lower/mod.rs`、`parser/tests.rs`）；本次提交只会包含 `P3-T01` 相关文件，不回退也不夹带这些改动。
- 待完成收尾：检查工作区 diff，确认 `TODO-P3.md`/`memory/claude_plan.md` 已回写，然后创建 `P3-T01` 提交并停止。
