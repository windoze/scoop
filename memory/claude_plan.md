# 执行计划（初始）

## 约束与执行原则

- 本次只处理 `TODO.md` 中第一个未完成任务，完成后立即停止。
- 在推进计划任务前，先检查最新一次 Git 提交是否提到已知问题；若提到，则该问题优先处理。
- 在执行、测试、审查过程中发现的任何既有缺陷、规约不匹配、回归、不完整实现边界或临时绕过，都视为立即在范围内的问题，必须先修复，或在 `TODO.md` 中插入为前置任务后停止。
- 不接受绕过实现、不接受缩小规格、不接受仅为夹具或测试定制的特判。
- 所有进展、关键决策、计划调整都需要同步更新本文件。

## 初始步骤计划

1. 查看最新 Git 提交信息，确认是否显式提到需要优先修复的既有问题。
2. 阅读 `TODO.md`，定位第一个未完成任务。
3. 阅读 `PLAN.md`，确认当前总计划、依赖关系与任务上下文。
4. 判断该任务是否过大：
   - 若过大，则拆分为更小子任务；
   - 更新 `PLAN.md`；
   - 更新 `TODO.md`，将拆分后的子任务放到正确依赖顺序中；
   - 选择新的第一个子任务作为本次执行目标。
5. 阅读并理解与目标任务相关的代码、规格、测试与最近变更。
6. 实施任务；若过程中发现既有问题，则优先修复或将其作为前置任务加入 `TODO.md`。
7. 运行相关验证：
   - 最小相关测试；
   - 必要的更广泛测试；
   - `cargo fmt`；
   - `cargo clippy --all-targets -- -D warnings`；
   - 与任务直接相关的命令。
8. 更新文档与计划：
   - 在 `TODO.md` 标记任务完成，或在阻塞时重排任务顺序；
   - 更新 `PLAN.md`；
   - 更新本文件记录实际执行情况与偏差。
9. 查看工作区差异，确认仅包含本次应提交内容。
10. 提交 Git commit，提交信息清晰描述本次任务。
11. 停止，不继续处理下一个任务。

## 决策记录

- 已检查最新提交 `8442091c [T5000bR] Review LLVM codegen backend boundary`。
  - 提交标题与变更内容未引入新的待修既有缺陷；提交中提到的注释错配已在该提交内修复。
- 已读取 `TODO.md` 与 `PLAN.md`，确认当前第一条未完成任务是 `T5000c 抽离 backend-agnostic 的 ProgramFacts / EffectAnalysisCtx / shared side tables`。
- 已完成 `T5000c` 定界：
  - `state_machine_plan.rs` 同时在 `HandlePlanContext::from_codegen(...)`、`ensure_known_fun_body_may_outward_effect_cache(...)` 与多个测试 helper 中重复拼装 `SuspendCallProgramFacts`；
  - `effect_step_summary.rs` 仍通过 `include!` 直接复用 backend 源文件；
  - 因此 `T5000c` 单轮过大，已拆成 `T5000c1`～`T5000c3`。
- 当前本轮唯一执行目标已切换为 `T5000c1 抽出 backend-agnostic 的 ProgramFacts 数据结构与统一 builder`。
- `T5000c1` 已完成实现与验证：
  - 新增 `crates/scoopc/src/program_facts.rs`，定义 backend-agnostic `ProgramFacts`，并通过 `ProgramFacts::from_lowered(&hir::LoweredHir)` 从 lowering side tables 统一构造；
  - `crates/scoopc/src/llvm/emit.rs` / `crates/scoopc/src/llvm/codegen/mod.rs` 现会在进入 backend 前构造并持有共享 `Rc<ProgramFacts>`；
  - `crates/scoopc/src/llvm/codegen/effect/state_machine_plan.rs` 删除了临时 `SuspendCallProgramFacts` 拼装结构，`HandlePlanContext`、known-fun suspendability cache 与 higher-order function-value suspendability 查询现统一复用同一份 `ProgramFacts`；
  - `crates/scoopc/src/llvm/codegen/effect/state_machine_segments.rs` 与 `state_machine_transform.rs` 的测试 helper 也改为从 `LoweredHir` 统一构造 `ProgramFacts`；
  - 本轮同时修复了一个既有无告警构建问题：`crates/scoopc/src/effect_step_summary.rs` 在 `--no-default-features` 路径下因 `include!` 整个 `state_machine_plan.rs` 而暴露大量 intentional dead-code / unused-import warnings；现已把告警边界收口在 `effect_step_summary.rs` 自身。
- 当前下一条任务已切换为 `T5000c1R Review：确认 ProgramFacts 已成为 backend-agnostic 的共享 side table`，本轮不继续执行。

## 进度状态

- [x] 已创建初始计划文件。
- [x] 已检查最新提交。
- [x] 已读取 `TODO.md` 与 `PLAN.md`。
- [x] 已确定当前顶层未完成任务是 `T5000c`。
- [x] 已评估并拆分 `T5000c`。
- [x] 已完成 `T5000c1` 的实现与验证。
- [ ] 待更新任务状态并提交。

## 当前执行计划

1. 检查工作区差异，确认仅包含 `T5000c1` 与本轮顺手修复的无告警构建问题。
2. 提交 commit。
3. 停止，不进入 `T5000c1R`。
