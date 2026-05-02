# Claude Plan

## 目标
- 按 `TODO.md` 索引与对应 `TODO-Px.md` 详细任务文件，找到第一个未完成的详细任务。
- 只完成这一个任务；若遇到阻塞，则按要求补充最小前置任务、同步索引并停止。

## 执行计划
1. 读取 `TODO.md`，确认详细任务文件引用与任务顺序。
2. 按顺序读取对应 `TODO-Px.md`，以标题是否带 `[DONE]` 判断完成状态，定位第一个未完成任务。
3. 检查最近一次提交信息，确认是否有与当前任务直接相关且未完成的问题需要一并处理或登记为前置。
4. 阅读当前任务要求、约束、依赖与验收方式，结合代码现状确定最小正确改动方案。
5. 实现任务所需代码与测试；如发现阻塞当前任务的真实缺口或规格不匹配，则先补充前置任务并同步 `TODO.md`。
6. 运行相关验证，至少覆盖任务要求的测试；若有必要，运行更广的检查（含 `cargo fmt`、相关测试、`cargo clippy --all-targets -- -D warnings`）。
7. 更新任务记录：在对应 `TODO-Px.md` 中将任务标题标记为 `[DONE]` 并补全完成记录；若任务索引受影响，同步 `TODO.md`；仅在阶段计划变更时更新 `PLAN.md`。
8. 复核工作区中与本次任务相关的改动，按要求创建一次 Git 提交，然后停止，不继续下一项任务。

## 进度记录
- 已初始化计划文件；待开始读取任务索引并定位当前执行目标。
- 已读取 `TODO.md` 并确认首个未完成详细任务为 `TODO-P4.md` 中的 `P4-T05a`：把 compiler-generated continuation 的 one-shot runtime error 纳入 canonical `StepSchema` / facts handoff。
- 已检查最近一次提交：`[P4-T05a] Track compiler continuation runtime-error prerequisite`，其主题与当前任务直接相关；接下来需要确认该前置是否仍未实现，以及代码中 runtime-error case 当前只覆盖到什么范围。

## 当前任务细化计划
1. 阅读 `P4-T05a` 任务描述与 `TODO-P5.md` 中 `P5-T05` 的相关约束，明确 one-shot runtime error 在 P4 handoff 中必须出现的位置。
2. 检查 `effect_facts` 的 schema/facts/builder 与相关测试，定位当前 runtime-error ordinary effect 仅覆盖源码 `Continuation.resume(...)` 还是已经部分覆盖 compiler-generated continuation。
3. 若现有实现可直接扩展，则做最小代码改动，把 compiler-generated continuation 的 one-shot runtime error 正式纳入 canonical schema/facts 与 dump 输出；若发现真实阻塞，则按要求回写新的前置任务并停止。
4. 补充或更新最小但充分的定向测试与 fixture，覆盖“应加入 runtime-error case”和“pure/no-outward 不应被误扩张”两类行为。
5. 运行任务要求的定向测试、必要的 dump 命令与格式/静态检查；修复所有相关失败。
6. 完成后更新 `TODO-P4.md`（标记 `[DONE]` 与完成记录），如有需要同步 `TODO.md`；若阶段计划未变则不改 `PLAN.md`。
7. 复核工作区并创建一次提交，然后停止。

## 当前实现决策
- 现有 `effect_facts` builder 只会因源码 `Resume` site 把 `Raise<RuntimeError>` 放进 `StepSchema`；这不足以覆盖 compiler-generated continuation object 的 one-shot runtime error。
- 计划采用最小侵入的两次构建策略：
  1. 先按现状运行 builder + solver，得到当前最终 `needs_reentry` 集合；
  2. 仅对这些最终确实会进入 resumable lowering 的 callable/version，在第二次 builder 中把 compiler-generated continuation one-shot runtime error 追加到 callable `StepSchema` 上界；
  3. 再次运行 solver，得到最终 P4 handoff。
- 这样可以避免把 truly no-outward callable 无端扩张为带 runtime-error case，同时保留 `single_case_impl_plan` 这类样本的 `resolved_outward_cases` / `impl_plan`，除非真实 body/site 贡献本就包含 runtime error。

## 已完成步骤
- 已在 `crates/scoopc/src/effect_facts/builder.rs` 增加 compiler-generated continuation runtime-error 覆盖集合，并让第二次 builder 可把 `Raise<RuntimeError>` 追加到目标 callable 的 canonical `StepSchema` 上界。
- 已在 `crates/scoopc/src/effect_refactor_pipeline/effect_facts_stage.rs` 落地两次构建流程：第一次求出最终 `needs_reentry` 集合，第二次仅对这些 callable 重建 schema 后再交给 solver。
- 已补充定向单测，覆盖：runtime-error case 被补入 reentry callable 的 step schema；未选中的 truly no-outward callable 不被误扩张；最终 stage 输出中 runtime-error 只进入 schema 上界而不无端扩大 `resolved_outward_cases`。
- 已更新 `tests/fixtures/effect_facts/{single_case_impl_plan,dynamic_fallback_widening,nested_handle_self_contained_vs_outward}.effectfacts` golden，使 dump 能稳定暴露新的 handoff contract。
- 已把 `TODO-P4.md` 中的 `P4-T05a` 标记为 `[DONE]` 并补全完成记录，同时已把 `TODO.md` 索引同步为 `[DONE]`；`PLAN.md` 保持不变。
- 已完成验证：`cargo fmt --all`、`cargo test -p scoopc --no-default-features compiler_continuation_runtime_error`、`cargo test -p scoopc --no-default-features refactor_effect_schema`、`cargo test -p scoopc --no-default-features refactor_callable_effect_facts_shell`、`cargo test -p scoopc --no-default-features refactor_effect_facts_stage`、`cargo test -p scoopc --no-default-features refactor_impl_plan`、`cargo test -p scoopc --no-default-features refactor_effect_solver`、`cargo test -p scoop --no-default-features dump_effect_facts`、`cargo run -q -p scoop --no-default-features -- --effect-pipeline refactor dump-effect-facts tests/fixtures/effect_facts/single_case_impl_plan.scoop`、`cargo run -q -p scoop --no-default-features -- --effect-pipeline refactor test --fixtures tests/fixtures/effect_facts`、`cargo clippy -p scoop -p scoopc --all-targets --no-default-features -- -D warnings`。

## 剩余收口
1. 复核变更清单并创建 Git 提交。

## 说明
- 这里记录的是可审计的执行计划与进度摘要，不包含私有推理细节。
