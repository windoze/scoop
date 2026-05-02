## 当前执行计划

1. 读取 `TODO.md`，确认它只作为索引使用，并提取按顺序引用的详细任务文件。
2. 依次检查相关 `TODO-Px.md`，定位第一个未明确记录为已完成的详细任务。
3. 查看最近一次提交信息，判断是否存在与该任务直接相关且明确标注未完成的问题；若有，按要求并入当前任务或作为前置任务处理。
4. 阅读当前任务涉及的代码、测试、规格与依赖约束，确认需要修改的最小范围。
5. 实现当前任务；如果遇到阻塞当前任务且必须先解决的问题，则在对应 `TODO-Px.md` 中补充最小前置任务并同步 `TODO.md`。
6. 运行与当前任务直接相关的验证命令；如有失败，继续修复直到通过，或在确有阻塞时按流程记录前置任务并停止。
7. 更新 `memory/claude_plan.md` 记录关键进展，更新对应 `TODO-Px.md` 的完成记录；仅在任务索引变化时同步 `TODO.md`，仅在阶段计划变化时更新 `PLAN.md`。
8. 按仓库约定创建一次 git 提交，然后停止，不继续下一个任务。

## 记录约束

- 这里记录的是可审阅的执行计划与关键决策，不包含冗长的内部推理草稿。
- 若执行中发现阻塞、范围变化、验证结果或完成状态变化，会及时补充更新。

## 当前进展

- 已按 `TODO.md -> TODO-P0.md -> TODO-P1.md -> TODO-P2.md` 顺序检查完成记录。
- 当前首个未完成详细任务：`TODO-P2.md` 中的 `P2-T04R`（Review P2 阶段退出条件，确认 P3 不再需要回 AST/typecheck 猜语义）。
- 最近一次提交为 `[P2-T04] Emit typed HIR effect contract tables`，未显式记录与 `P2-T04R` 直接相关的未完成事项。
- 当前工作区存在未提交改动：`crates/scoop/src/commands/dump_ir.rs`、`crates/scoop/src/commands/dump_mir.rs`、`crates/scoopc/src/hir/lower/expr.rs`、`crates/scoopc/src/hir/lower/mod.rs`、`crates/scoopc/src/parser/tests.rs`，以及本文件；后续处理 `P2-T04R` 时不会回退这些现有改动，只在必要时审慎协作。
- 下一步：抽查 `hir_stage` / `dump_hir` / `typecheck` / `sysroot` 的实现是否满足 `P2-T04R` review 关注点，然后运行 `P2-T01 ~ P2-T04` 要求的定向验证并据结果决定是否补前置问题或填写完成记录。

## 关键发现

- `crates/scoopc/src/effect_refactor_pipeline/hir_stage.rs` 已将 `TypedHirStageOutput` 与 `TypedHirEffectContracts` 固化为 P2 -> P3 handoff：稳定暴露 `function_effects`、`call_site_kinds`、`continuation_resume_sites`、`perform_sites`、`handle_sites`，并通过 `stable_dump()` 以确定顺序渲染。
- `crates/scoop/src/commands/dump_hir.rs` 已明确分流：`legacy` 继续走 `scoopc::hir::lower_for_dump(...)`，`refactor` 走 `effect_refactor_pipeline::load_typed_hir_stage_output_for_dump(...)`，并直接打印 typed contract 区块。
- `crates/scoopc/src/typecheck/expr/call.rs` 中 `try_infer_continuation_resume_call_expr_type(...)` 继续显式记录 `Out` effects 与额外的 `Raise<RuntimeError>` ordinary effect，并把 `k.resume()` 的 zero-arg sugar 收口到 typed 阶段 helper，而不是回 AST/parser 特判。
- `crates/scoopc/src/typecheck/interfaces.rs` 仍在 interface/typecheck 阶段拒绝用户实现 compiler-owned `scoop.core.Continuation`；`sysroot/core.scoop` 也保持 `interface Continuation<Resume, Answer, eff E = Pure>` 与 `resume(value): Answer / (E + Raise<RuntimeError>)` 的 surface contract。
- 已复读 `TODO-P3.md` 开头前置条件与 `P3-T01` 约束；当前 P2 side tables 提供的 resume / perform / handle / function-effect contract 已满足“P3 不再回 AST/typecheck 猜语义”的入场前提。

## 验证结果

- 核心单元测试与静态检查通过：`refactor_typed_hir_stage`、`effect_refactor_pipeline`、`dump_hir`、`parity`、`continuation_resume`、`unit_single_param_zero_arg`、`refactor_continuation_typecheck`、`refactor_typed_hir`、`cargo clippy -p scoop -p scoopc --all-targets --no-default-features -- -D warnings`。
- `P2-T01 ~ P2-T04` 所要求的 `dump-hir` / fixture smoke 已全部重跑通过，包括 `tests/fixtures/hir/minimal.scoop`、`continuation_resume_surface_named_tuple_and_unit_basic.scoop`、`continuation_runtime_error_surface_basic.scoop`、`handle_perform.scoop`，以及相关 continuation/typecheck fixtures。
- 额外搜索 `crates/scoopc/src/hir`、`crates/scoopc/src/typecheck` 中的 `EffectPipelineMode|effect_pipeline|legacy|refactor` 后，命中仅来自测试、既有 `legacy_*` 命名和诊断文本，未发现 pipeline selector 下沉到旧 HIR/typecheck 业务函数的新增分支。
- 测试后工作区未新增额外改动；仍只有执行前就存在的用户/并行改动与本次更新的 `memory/claude_plan.md`。

## 当前收尾状态

- 已将 review 结论回写到 `TODO-P2.md` 的 `P2-T04R` 完成记录。
- 未修改 `TODO.md` 与 `PLAN.md`：本次 review 未引入任务重排，也未改变阶段计划。
- 下一步仅需暂存并提交 `TODO-P2.md` 与 `memory/claude_plan.md`，避免夹带现有无关改动，然后停止。
