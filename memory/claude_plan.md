# 当前执行计划

## 约束
- 只处理第一个未完成的详细任务，完成后停止。
- `TODO.md` 只作为索引读取；详细需求、顺序和完成状态以对应 `TODO-Px.md` 为准。
- 任务只有在详细文件标题带 `[DONE]` 时才视为完成，并同步 `TODO.md` 中对应索引项。
- 如遇阻塞性缺失功能或规格不一致，不绕过；新增最小前置任务、同步索引、提交后停止。
- `PLAN.md` 仅在阶段级计划变化时更新。

## 步骤
1. 读取 `TODO.md`，按索引顺序确认引用的详细任务文件。
2. 打开对应 `TODO-Px.md` 文件，定位第一个标题未带 `[DONE]` 的详细任务。
3. 检查最近提交是否明确提到与该任务直接相关的未完成问题；若有，将其纳入当前任务或作为前置任务记录。
4. 阅读当前任务要求、依赖、验证方式和完成记录，确定需要修改的代码、测试和文档范围。
5. 按任务要求实现最小正确改动；不采用替代表示、fixture-only hack 或规格弱化。
6. 添加或更新相关测试和 fixture，运行任务要求的验证命令；若失败，定位并修复。
7. 将完成状态写回对应 `TODO-Px.md`，在任务标题加 `[DONE]` 并更新完成记录；同步 `TODO.md` 的 `[DONE]` 标记。
8. 更新本文件记录关键进度和实际验证结果。
9. 检查 git 状态和差异，提交所有与本次任务相关的未提交更改。
10. 停止，不继续下一个任务。

## 当前状态
- 已读取 `TODO.md` 和 `TODO-P5.md`，第一个未完成详细任务是 `P5-T08：让 NoOutward 在 late-lowered handoff 中保持 plain callable，不物化 Step / continuation / state-machine 壳`。
- 最近提交为 `74a7cf77 Update plan`，未明确记录与 `P5-T08` 直接相关的未完成 issue。
- 已检查 P5 IR/builder/materialize/dump/opt、P4 `CallableAbiKind` facts、stage 测试与相关 golden。当前 P5 builder 仍对所有 callable 调用 `step_schema()` 并物化 complete-only `Step`/continuation/state graph，这是 `P5-T08` 需要修复的核心。
- 已将 `LateLoweredCallable` 改为显式 `Plain` / `EffectStep` ABI 分支；Plain 分支只保留普通签名、source slices 与 source-slice call-site ABI contracts，不携带 `Step_F`、continuation object、state graph、frame、boundary 或 resume map。
- 已更新 builder/materialize/dump/opt/stage 测试与 `tests/fixtures/effect_lowered/*.effectlowered` golden。
- 已通过定向验证：`cargo test -p scoopc --no-default-features refactor_effect_lowered_no_outward_plain_callable`、`refactor_late_lowered_ir`、`refactor_source_slice_plain_call_keeps_ordinary_call_contract`、`refactor_effect_lowered_stage`、`refactor_continuation_object`、`refactor_resume_interface_completeness`、`refactor_impl_plan_lowering_keeps_no_outward`，以及 `cargo run -p scoop --no-default-features -- test --effect-pipeline refactor --fixtures tests/fixtures/effect_lowered`、direct dump smoke、`cargo clippy -p scoopc -p scoop --no-default-features --all-targets -- -D warnings`。
- 下一步更新 `TODO-P5.md` / `TODO.md` 完成记录，检查 git diff 后提交。
