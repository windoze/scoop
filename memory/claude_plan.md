执行计划

1. 读取 TODO.md，按任务标题是否带有 [DONE] 判断第一个未完成任务。
2. 读取该任务相关的计划、依赖、验证要求和最近提交信息，确认是否存在直接相关的未完成前置问题。
3. 在不做开放式历史问题清扫的前提下，定位当前任务需要修改的代码、测试和文档。
4. 完整实现当前任务；如果发现阻塞当前任务的规格缺口或实现缺陷，则先修复，或在 TODO.md 中插入最小必要前置任务并停止。
5. 运行与当前任务相关的测试；必要时运行更宽的回归验证，修复由当前任务引入或暴露且阻塞当前任务的问题。
6. 更新 TODO.md：完成时在任务标题前加 [DONE] 并填写完成记录；若任务被阻塞，则保持未完成并记录前置任务关系。
7. 仅在阶段级计划发生变化时更新 PLAN.md；常规任务日志不写入 PLAN.md。
8. 检查 git 状态、差异和最近提交，提交本次任务相关的所有变更，然后停止。

进度记录

- 已创建初始执行计划，下一步读取 TODO.md 识别首个未完成任务。
- 已识别首个未完成任务：TODO-5.md 中的 P4-T03R（Review `EffectFactsStageOutput` 收口结果）。最新提交 `e365b74b [P4-T03] Narrow effect facts stage output` 正是该 review 的对象。
- 本轮执行边界：只复审 P4-T03，不推进 P4-T04 或 P5；若发现 P4-T03 未满足窄输出 / 显式 P5 输入 / helper 清理要求，则在本 review 内修正。
- 关键检查项：`EffectFactsStageOutput` 不嵌套或转发 P3 output；`EffectLoweringStageInput` 或等价输入显式携带 MIR handoff 与 effect facts handoff；生产与测试 helper 不再把 P4 output 当作 P3/P4/P5 bundle。
- 计划中的验证：重新运行 P4-T03 的验证命令，并额外搜索 `mir_stage_output\(|materialized_mir\(|materialized_pass_view\(|mir_facts\(`，确认这些上游转发不再存在于 `EffectFactsStageOutput` impl 上。
- 已完成代码复查初步结论：`EffectFactsStageOutput` 当前只有 `effect_facts` 字段，未发现 `mir_stage_output()`、`materialized_mir()`、`materialized_pass_view()`、`mir_facts()`、`file()`、`types()` 等上游转发；P5 orchestration 和测试通过 `EffectLoweringStageInput::new(mir_stage_output, effect_facts_stage_output)` 显式传入 MIR handoff 与 effect facts handoff。
- 下一步执行 P4-T03R 要求的验证；如验证失败，先修复本 review 范围内的直接问题。
- 验证已通过：`cargo fmt`；额外搜索确认 `EffectFactsStageOutput` impl 上无上游转发，且无通过 `effect_facts_stage_output` 回看上游的调用；`cargo test -p scoopc --no-default-features effect_facts_stage`；`cargo test -p scoopc --no-default-features effect_lowering_stage`；`cargo test -p scoopc --no-default-features effect_facts`；`cargo test -p scoopc --no-default-features effect_lowered`；`cargo run -p scoop -- test --fixtures tests/fixtures/effect_facts`；`cargo run -p scoop -- test --fixtures tests/fixtures/effect_lowered`；`cargo clippy --all-targets -- -D warnings`。
- 下一步更新 TODO.md / TODO-5.md，将 P4-T03R 标记为完成并填写 review 结论与验证记录。
- 已更新 TODO.md / TODO-5.md：P4-T03R 标记为 `[DONE]`，完成记录写入 review 结论、P5 显式输入边界、helper/test 复查、搜索结果、验证命令和 P5 残余风险。
- 下一步执行 `git diff --check`，检查状态和差异，然后提交本次 P4-T03R 变更。
