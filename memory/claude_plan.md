# Claude Plan

## 当前目标
- 按照 `TODO.md` 作为索引、`TODO-Px.md` 作为任务真源的规则，定位首个未完成的详细任务。
- 只完成这一个任务；若遇到真实阻塞，则按要求补充前置任务、同步索引并停止。

## 执行步骤
1. 读取 `TODO.md`，识别其引用的详细任务文件与顺序。
2. 按索引顺序检查对应 `TODO-Px.md`，定位第一个标题未带 `[DONE]` 的详细任务。
3. 检查最近提交是否存在与该任务直接相关且明确未完成的问题；若有，则将其并入当前任务范围或作为前置依赖记录。
4. 阅读当前任务的详细要求、约束、依赖、验证要求与完成记录，确认需要修改的代码、测试与文档范围。
5. 在不采用变通方案的前提下实现该任务；如发现阻塞当前任务的真实缺陷或缺失特性，则先修复，或在相应 `TODO-Px.md` / `TODO.md` 中最小化插入新的前置任务并停止。
6. 运行与当前任务相关的验证，包括最小必要测试，以及仓库要求的格式、测试、lint 检查，确保无警告。
7. 更新任务记录：在对应 `TODO-Px.md` 中将任务标题标记为 `[DONE]` 并补全完成记录；若索引需要同步，则更新 `TODO.md`；仅在阶段计划实际变化时更新 `PLAN.md`。
8. 检查工作区是否有本次任务相关的未提交更改；按要求创建一次 git 提交，提交信息使用当前任务编号。
9. 停止，不继续下一个任务。

## 进度记录
- 已创建本计划文件，后续会在关键步骤完成或计划调整时持续更新。
- 已读取 `TODO.md` 与 `TODO-P4.md`，确认首个未完成详细任务为 `P4-T01`。
- 该任务在 `TODO-P4.md` 的完成记录中已有实现说明，但任务标题尚未加 `[DONE]`；根据执行规则，仍需把它作为当前任务处理。
- 接下来将核对 `P4-T01` 涉及的代码与测试是否已满足要求；若确认实现已齐备，则补齐任务标题与索引同步，并完成规定验证和提交。
- 已完成复验：`cargo test -p scoopc --no-default-features refactor_effect_facts_stage`、`cargo run -q -p scoop --no-default-features -- --effect-pipeline refactor dump-mir tests/fixtures/mir_refactor/dispatch_and_resume_call.scoop`、`cargo clippy -p scoop -p scoopc --all-targets --no-default-features -- -D warnings` 均通过。
- 额外执行了 `MaterializedEffectFacts|StepSchema|ContinuationSchema|resolved_outward_cases|impl_plan` 搜索；命中仍集中在 `effect_facts` 子系统、refactor stage 与注释，未发现重新写回 legacy `ProgramFacts` / `mir::summary` / `effect::analysis` 的情况。
- 当前工作区还存在其他未提交改动，其中包含后续任务相关内容；按当前任务边界，接下来只补齐 `P4-T01` 的完成标记与索引同步，不主动改动这些已有工作。
