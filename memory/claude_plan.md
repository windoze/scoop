# 当前执行计划

## 范围

- 以 `TODO.md` 为唯一任务顺序来源，找出第一个标题未带 `[DONE]` 的任务。
- 本次只完成这一个任务；完成后更新记录、提交 Git，然后停止。

## 步骤

1. 读取 `TODO.md`，确认第一个未完成任务及其依赖、验证要求和完成记录要求。
2. 检查最近提交信息是否明确提到与该任务直接相关的未完成问题；若存在，将其纳入当前任务或作为前置任务记录到 `TODO.md`。
3. 按任务要求定位相关代码、文档和测试，优先采用最小正确修改。
4. 实现任务；如发现阻塞当前任务的语言特性缺口、规格不匹配或测试失败，优先修复，或在 `TODO.md` 中添加最小前置任务并停止。
5. 运行任务指定测试和相关验证；若观察到未被明确排期的失败，修复或排期后再决定是否完成当前任务。
6. 将当前任务标题加上 `[DONE]`，更新其完成记录；仅在阶段级计划变化时更新 `PLAN.md`。
7. 检查工作区差异，提交所有与本次任务相关的变更，提交信息使用任务编号和简短说明。
8. 停止，不继续执行后续任务。

## 当前状态

- 已确认第一个未完成任务：`P7-T04R`（`TODO-6.md`）。
- `TODO.md` 与 `TODO-6.md` 均显示 `P7-T04` 已完成，下一项为 `P7-T04R` review。
- 本任务范围：复审 `P7-T04` 的 LLVM stage handoff 与 physical ABI cleanup 收口结果，并在发现问题时直接修复或记录最小前置任务。

## P7-T04R 可审计推理摘要

- 任务性质是 review 任务，不应跳过；完成条件要求明确写出 LLVM backend 输入边界是否已收口。
- 复审重点来自 `TODO-6.md`：`LlvmCodegenStageOutput` / `StageEmitInput` 不再传播 P5 wrapper 或 HIR scaffold；`LirStageOutput` 不再保留 LLVM residual pass-view context；physical ABI/layout 只做 backend-private 物理化；`TypeId` cross-process stable wire format 处置必须已冻结到未来 owner。
- 若 residual 搜索或验证发现真实回归，优先在本 review 内修复；若需要新增 prerequisite，更新 `TODO.md` / `TODO-6.md`、提交并停止，不把当前任务标记为完成。
- 不更新 `PLAN.md`，除非复审发现阶段级边界或依赖结构需要改变。

## P7-T04R 执行计划

1. 检查最近提交信息，确认是否显式提到与 `P7-T04R` 直接相关的未完成事项。
2. 读取并复审 `P7-T04`、`P7-T04-bR`、`P7-T04-cR` 完成记录及相关文档，确认 review 的预期边界。
3. 执行 residual 搜索：`hir_compat_scaffold`、`llvm_residual_pass_view`、`EffectLoweredStageOutput`、`MaterializedMirPassView`、`EffectFactsStageOutput` 在 LLVM/pipeline handoff 中是否还有生产命中；同时抽查 `LlvmCodegenStageOutput`、`StageEmitInput`、`LirStageOutput` 的公开输入形状。
4. 如搜索发现不合规 residual，定位并做最小正确修复；修复后补充或调整测试。
5. 重新运行 `P7-T04` 的验证：`cargo fmt`、`cargo test -p scoopc --no-default-features llvm_codegen_stage`、`cargo test -p scoopc --no-default-features llvm::codegen::effect_lowered::layout`、`cargo run -p scoop_tools -- dependency-gate`、`cargo run -p scoop -- test --fixtures tests/fixtures/run-pass`（至少 30 分钟 timeout）、`cargo clippy --all-targets -- -D warnings`、`git diff --check`。
6. 若所有验证通过且无未排期失败，将 `P7-T04R` 在 `TODO.md` 与 `TODO-6.md` 标记为 `[DONE]` 并填写完成记录，记录 review 结论、residual 搜索、TypeId wire-format owner 和验证结果。
7. 检查工作区状态和 diff，提交本任务相关变更，提交信息使用 `[P7-T04R] ...`，然后停止。

## 进度记录

- 已写入本执行计划，并在写入后开始执行验证。
- 最近提交为 `[P7-T04] Record final plan status`，未提到与 `P7-T04R` 直接相关的未完成事项。
- residual 搜索结果：`EffectLoweredStageOutput`、`llvm_residual_pass_view`、`hir_compat_scaffold` 在 LLVM handoff 生产路径无命中；`EffectFactsStageOutput` 命中仅在 P5 `effect_lowering_stage` 输入边界；`MaterializedMirPassView` 仍在 `LlvmStageBaseContext` / `CompilationUnitCodegenCx` base-context residual 出现，应在 review 完成记录中明确归类为 P7-T05/P8 清场继续跟踪而非 P5 wrapper 回退。
- physical ABI/layout 搜索结果：`effect_lowered/layout` 生产命中为 `LirFacts.physical_layout.class_vtables/class_itables` 字段读取；HIR side-table 名称命中只在 layout tests 的空表注入或普通 `interfaces` 文本，未发现 `crate::hir::mangle_nominal_fqn` 生产残留。
- P7-T04R 指定验证已通过：`cargo fmt`；`cargo test -p scoopc --no-default-features llvm_codegen_stage`；`cargo test -p scoopc --no-default-features llvm::codegen::effect_lowered::layout`；`cargo run -p scoop_tools -- dependency-gate`；`cargo run -p scoop -- test --fixtures tests/fixtures/run-pass`（421/421 passed）；`cargo clippy --all-targets -- -D warnings`；`git diff --check`。
- 已更新 `TODO.md` 与 `TODO-6.md`：`P7-T04R` 标记为 `[DONE]`，完成记录写入 review 结论、residual 搜索分类、TypeId wire-format owner 和验证结果。
- 已提交本任务变更：`[P7-T04R] Review LLVM handoff cleanup`（`026ebe0b`）。
- 本次 invocation 到此停止，不继续执行 `P7-T05`。
