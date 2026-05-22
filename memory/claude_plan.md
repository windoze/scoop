# 执行计划

## 约束
- 以 `TODO.md` 为唯一任务顺序和完成状态来源。
- 只处理第一个标题未带 `[DONE]` 的任务，完成后停止。
- 若遇到阻塞当前任务的缺陷或缺失能力，先在 `TODO.md` 中加入最小必要前置任务并提交，然后停止。
- 不使用规避方案；当前任务必须按规格完成或明确记录前置阻塞。
- 仅当阶段级计划发生变化时更新 `PLAN.md`。
- 完成后更新 `TODO.md` 标题和完成记录，运行相关验证，并提交全部相关改动。

## 步骤
1. 读取 `TODO.md`，定位第一个标题未带 `[DONE]` 的任务。
2. 查看最新提交信息，判断是否有与该任务直接相关的未完成问题需要纳入当前任务或前置任务。
3. 阅读该任务涉及的代码、测试和文档，确认需求、依赖和验证命令。
4. 如任务可直接完成，进行最小正确实现，并同步添加或更新必要测试。
5. 如发现当前任务被具体缺陷或缺失功能阻塞，更新 `TODO.md` 记录最小前置任务，必要时更新 `PLAN.md`，提交后停止。
6. 运行任务要求的验证以及必要的相关测试；若失败，定位并修复后重新验证。
7. 在 `TODO.md` 中将当前任务标题前缀改为 `[DONE]`，更新完成记录。
8. 更新本文件记录关键进展与验证结果。
9. 检查 `git status`、`git diff`、近期提交，确认只提交相关改动。
10. 使用描述性提交信息提交本次任务改动，然后停止，不继续下一个任务。

## 当前进展
- 已写入初始执行计划。
- 已读取 `TODO.md`，第一个未完成任务是 `P5-T04R：Review LIR optimization family`。
- 已读取 `TODO-5.md` 中 `P5-T04` / `P5-T04R` 正文；最新提交为 `db48b46a [P5-T04] Add LIR optimization pipeline`，与当前 review 任务直接相关，需要复查该提交引入的 LIR opt pipeline、metadata、verifier 与 fixtures。
- 下一步复查 `effect_lowered::opt`、post-opt verifier、LIR stage metadata、`scoopc_lir_facts` 和 effect-lowered fixtures，重点确认 LIR opt 不读取 HIR/MIR/effect solver 输入，且 dangling references verifier 覆盖任务要求。
- Review 发现并修复一个 P5-T04R 范围内的问题：post-opt verifier 未校验 boundary lowering 与 `HandleDispatch` contract 内部的嵌套 state/boundary/frame/continuation-object/StepSchema 引用。已扩展 `opt_verify` 校验这些引用，并新增回归测试，构造带悬空 handle contract state 的 post-opt LIR 以确认 verifier 会拒绝。
- 已完成验证：`cargo fmt`；`cargo test -p scoopc --no-default-features effect_lowered::opt`；`cargo test -p scoopc_lir_facts`；`cargo run -p scoop -- test --fixtures tests/fixtures/effect_lowered`；`cargo clippy --all-targets -- -D warnings`；`git diff --check`。
- 已同步任务记录：`TODO.md` 将 `P5-T04R` 标为 `[DONE]`，`TODO-5.md` 将 `P5-T04R` 标为 `[DONE]` 并填写 review 结论、修复内容、搜索结果、验证命令和残余风险。
- 下一步检查 diff/status，确认只包含本次 review 相关改动，然后提交并停止。
