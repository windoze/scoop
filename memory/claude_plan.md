# 当前执行计划

## 约束
- 以 `TODO.md` 为任务排序与完成状态的唯一来源。
- 本轮只完成第一个未标记 `[DONE]` 的任务，然后提交并停止。
- 若遇到阻塞当前任务的规范缺口或实现缺陷，不绕过；最小化新增前置任务并提交任务清单更新后停止。
- `PLAN.md` 只在阶段级计划或依赖发生变化时更新。
- 所有代码变更后运行相关验证，必要时修复验证发现的问题。

## 步骤
1. 读取 `TODO.md`，定位第一个标题未带 `[DONE]` 的任务，并记录其要求、依赖和验证标准。
2. 检查最近提交是否明确提到与该任务直接相关的未完成问题；若有，将其纳入当前任务或作为前置项记录到 `TODO.md`。
3. 按任务要求检查相关源码、测试和文档，避免无关历史问题扫查。
4. 实现第一个未完成任务；若出现阻塞当前任务的规范缺口，更新 `TODO.md` 并停止。
5. 运行任务要求的验证以及必要的相关测试；修复当前任务引入或暴露且阻塞任务完成的问题。
6. 更新 `TODO.md`：在任务标题前添加 `[DONE]`，并填写完成记录与验证结果。
7. 如阶段级计划发生变化才更新 `PLAN.md`；否则不动。
8. 检查工作区差异，提交本轮全部相关变更。
9. 停止，不继续下一个任务。

## 进度
- 已创建本执行计划，下一步读取 `TODO.md`。
- 已读取 `TODO.md` 与 `TODO-3.md`，确认本轮任务为 `P2-T02R：Review HIR stage output 形状`。
- 当前 review 关注点：`HirStageOutput` 是否对外仅作为 HIR + `hir_facts` handoff，dump/preflight 是否以 `hir_facts` 为入口，以及旧 `TypedHirStageOutput` / `TypedHirEffectContracts` 是否只剩迁移期 adapter 或测试说明。
- 下一步检查最近提交是否提示与本 review 直接相关的未完成问题，然后复查 `hir_stage`、`hir_preflight`、`pipeline/mod` 和 HIR fixture 输出。
- 最近提交未声明未完成问题；代码搜索确认 `TypedHirStageOutput` 已无 Rust 代码引用，`HirStageOutput` 是公开 HIR stage 类型。
- 发现 `TypedHirEffectContracts` 仍为 `pub struct` 且注释称其为 typed HIR stage 显式输出，容易误导后续任务；计划将其收紧为 crate-visible 迁移 bridge 并更新注释。
- 已完成边界修正：`TypedHirEffectContracts` 现在是 `pub(crate)` 迁移 bridge，注释明确正式 facts 入口为 `HirStageOutput::hir_facts()`。
- 验证已通过：`cargo fmt`、`cargo test -p scoopc --no-default-features hir_stage`、`cargo test -p scoopc --no-default-features hir_preflight`、`cargo run -p scoop -- test --fixtures tests/fixtures/hir`、`cargo test -p scoopc_hir_facts`、`cargo clippy --all-targets -- -D warnings`。
- 已将 `P2-T02R` 在 `TODO.md` 与 `TODO-3.md` 标记为 `[DONE]` 并填写完成记录；下一步检查最终差异并提交。
