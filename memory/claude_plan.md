# 当前执行计划

## 目标

按 `TODO.md` 的权威顺序完成第一个标题未带 `[DONE]` 的任务，完成验证、更新记录、提交 Git，然后停止。

## 执行步骤

1. 读取 `TODO.md`，只定位第一个未完成任务，不做开放式历史问题扫查。
2. 查看最新提交信息；仅当其明确提到与当前任务直接相关的未完成问题时，将其纳入当前任务或作为前置依赖记录到 `TODO.md`。
3. 阅读当前任务涉及的计划、代码、测试和夹具，确认验收标准与验证命令。
4. 若任务可直接完成，实施最小且完整的代码或文档修改；若遇到阻塞当前任务的真实缺口，按要求把最小前置任务加入 `TODO.md` 并停止。
5. 运行当前任务要求的相关测试，并在需要时运行更广范围的验证；修复由当前任务引入或暴露且阻塞当前任务的问题。
6. 更新 `TODO.md`：给已完成任务标题加 `[DONE]`，补全完成记录和验证记录。仅当阶段级计划变化时才更新 `PLAN.md`。
7. 检查 Git 状态、差异和近期提交，确认只提交本次任务相关内容。
8. 提交更改，提交信息使用任务编号和简洁说明。
9. 停止，不继续处理下一个任务。

## 进度记录

- 已创建本计划文件，下一步读取 `TODO.md` 定位首个未完成任务。
- 已定位首个未完成任务：`P4-T03`，目标是收口 `EffectFactsStageOutput` 并让 P5 显式消费 MIR handoff 与 effect facts handoff。最新提交 `P4-T02R` 为已完成 review，未发现需要优先处理的直接未完成项。
- 下一步聚焦读取 `effect_facts_stage.rs`、`effect_lowering_stage.rs`、pipeline orchestration、dump 和相关测试，按任务要求做最小完整修改。
- 已完成相关代码定位：当前 P4 输出仍持有 `MirStageOutput` 并转发 `materialized_pass_view()` / `mir_facts()` / `types()` 等查询；P5 builder 和 LLVM orchestration 通过该 wrapper 回读上游 handoff。
- 调整方案：`EffectFactsStageOutput` 改为仅持有 `MaterializedEffectFacts`；effect facts stage 只借用 `MirStageOutput` 构造 facts；新增显式 `EffectLoweringStageInput` 同时携带 MIR handoff 和窄 P4 output；P4 stable dump 改为只基于 effect facts / snapshot binding 渲染。
- 已实施核心改造：P4 output 不再保存或转发 P3 `MirStageOutput`；P5 late-lowering 通过 `EffectLoweringStageInput` 显式接收 MIR handoff + effect facts handoff；LLVM orchestration 与相关测试已改为分开传递二者。
- 已重新生成 `tests/fixtures/effect_facts/*.effectfacts`，因为 P4 dump 现在不再使用 MIR pass view 的 body/site label 上下文。
- 已通过验证：`cargo test -p scoopc --no-default-features effect_facts_stage`、`cargo test -p scoopc --no-default-features effect_lowering_stage`、`cargo test -p scoopc --no-default-features effect_facts`、`cargo test -p scoopc --no-default-features effect_lowered`、`cargo run -p scoop -- test --fixtures tests/fixtures/effect_facts`、`cargo run -p scoop -- test --fixtures tests/fixtures/effect_lowered`、`cargo clippy --all-targets -- -D warnings`。
- 已将 `TODO.md` / `TODO-5.md` 中 `P4-T03` 标记为 `[DONE]` 并填写完成记录；`git diff --check` 已通过，已检查 git 状态、差异和近期提交。
- 下一步提交本次 `P4-T03` 相关变更并停止。
