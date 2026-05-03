# Claude Plan

## 约束说明

- 不记录或暴露内部私有推理；这里仅维护可审计的执行计划、决策依据摘要、进度和验证结果。
- 本次调用只处理第一个未完成的详细任务；完成后提交并停止。

## 执行计划

1. 读取 `TODO.md`，确认索引结构、任务顺序和对应的 `TODO-Px.md` 文件。
2. 按索引顺序读取相关 `TODO-Px.md`，以标题是否带有 `[DONE]` 为准，定位第一个未完成的详细任务。
3. 检查最近一次提交是否直接提到该任务相关的未完成事项；如果是，将其并入当前任务范围，或在对应 `TODO-Px.md` 中记录为前置依赖。
4. 阅读当前任务涉及的代码、测试、规范和依赖位置，确认实现边界，避免擅自缩小范围或采用规避性方案。
5. 实现该任务所需的最小正确改动；若遇到阻塞当前任务的真实缺口或回归，先修复；若无法在本次内直接修复，则在正确位置添加最小前置任务并同步 `TODO.md`。
6. 运行与当前任务直接相关的验证，再运行必要的仓库级验证，至少覆盖任务要求和回归风险；若失败则修复后重试。
7. 更新 `memory/claude_plan.md`，记录关键发现、计划变化、实现步骤完成情况和验证结果。
8. 在对应 `TODO-Px.md` 中把已完成任务标题改为 `[DONE]` 并补全完成记录；若任务索引、顺序或标题变化，同步更新 `TODO.md`；仅当阶段级计划变化时才更新 `PLAN.md`。
9. 检查工作区变更，保留用户已有改动；按要求提交本次任务相关的全部未提交文件，提交信息使用当前任务 id。
10. 停止，不继续处理后续任务。

## 进度日志

- 已创建本计划文件。
- 已读取 `TODO.md` 索引并定位首个未完成详细任务为 `P6-T03`（`TODO-P6.md`）。
- 已检查最近一次提交：`[P6-T02h] Publish local runtime-error terminal contract`。提交标题未引入额外未完成事项，当前继续按 `P6-T03` 任务本身执行。
- 已检查工作区：除本计划文件外无未提交改动。
- 已确认当前实现状态：
  - `crates/scoopc/src/llvm/codegen/effect_refactor/body.rs` 仍是占位文件；
  - `crates/scoopc/src/llvm/emit.rs` 仍先做 `materialize_refactor_program_abi(...)`，随后沿用旧的 `codegen_top_level_mir_fun` / HIR fallback；
  - `ensure_refactor_effect_lowering_is_supported(...)` 仍会对 outward case / boundary / resume-state 等 `P6-T03` 路径 fail fast。
- 已阅读 `LateLoweredStateGraph`、boundary lowering contract、frame schema、continuation/resume shell、refactor ABI query 与现有相关 fixtures，确认 P6-T03 需要同时覆盖：callable direct/dynamic entry、state CFG、boundary dispatch、continuation surface resume / interface method body，以及 whole-body source-slice lowering。
- 已复现当前失败：`effect_resume_if_else_branch_single_perform.scoop` 在 refactor 运行路径上被 `refactor_effect_lowering_unsupported` fail fast 拒绝。
- 已继续验证 `dump-effect-lowered` / `dump-effect-facts`，确认 `P6-T03` 需要直接消费 synthetic `invoke_args_tuple_ty`、`ResumeSurface::*` / `CallSurface::*` step schema，以及这些 schema 上的 payload / answer carrier。
- 已尝试接入新的 body emitter，并在 direct-entry / surface-wrapper 的实际落线中确认新的 blocker：这些 synthetic source type 并不总存在于 legacy codegen 使用的 `hir::LoweredHir.types` / `CgTy` 键空间；继续实现将被迫把 refactor handoff 类型回塞旧 `TypeStore`，或在 backend 现场猜 carrier 形状，均违背 contract-first 约束。
- 已撤回本次半成品代码改动，恢复仓库到稳定状态。
- 下一步：在 `TODO-P6.md` / `TODO.md` 中新增最小前置任务 `P6-T02i`，把 blocker 明确记录为 `P6-T03` 的先决条件，然后提交并停止。
