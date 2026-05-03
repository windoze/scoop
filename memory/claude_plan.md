## 当前执行计划

说明：按安全与协作规范，这里记录可审阅的执行计划、关键判断依据与进度更新，不记录私有推理细节。

1. 读取 `TODO.md`，将其仅作为索引使用。
2. 按 `TODO.md` 引用顺序读取对应的 `TODO-Px.md`，定位首个标题未带 `[DONE]` 的详细任务。
3. 检查最近一次提交信息，确认是否存在与该任务直接相关且明确未完成的问题；若有，则将其视为当前任务的一部分，或在相应 `TODO-Px.md` 中登记为前置任务。
4. 阅读当前任务要求、约束、依赖、验证方式与完成记录，确认需要修改的代码、测试和文档范围。
5. 实施最小正确改动，避免规避性方案；若遇到阻塞当前任务的真实缺口或 spec mismatch，则先修复，或最小化新增前置任务并同步 `TODO.md`。
6. 运行与当前任务直接相关的验证；随后运行必要的仓库级检查，至少包括相关测试，以及在可行时运行 `cargo fmt`、`cargo test --all`、`cargo clippy --all-targets -- -D warnings`。
7. 完成后更新对应 `TODO-Px.md`：将任务标题标记为 `[DONE]`，补全完成记录；若任务索引信息变化，同步更新 `TODO.md`。仅在阶段计划发生变化时更新 `PLAN.md`。
8. 检查工作区是否存在与本次任务相关的未提交变更；按要求将本次应提交内容一起提交，提交信息以任务 id 为前缀。
9. 停止，不继续处理下一个任务。

## 进度日志

- 已写入初始执行计划，下一步开始读取 `TODO.md` 和对应详细任务文件。
- 已读取 `TODO.md` 与 `TODO-P5.md`，定位首个未完成详细任务为 `P5-T07a`。
- 已检查最近一次提交：`[P5-T07a] Track late-lowering projection blocker`，与当前任务直接相关。
- 已复现任务描述中的阻塞：
  - `dump-effect-facts` 显示 `main -> run` 的 call site 已解析出 callee 的 compiler-generated `scoop.core.Raise.raise<scoop.core.RuntimeError>` case；
  - `dump-effect-lowered` 与 refactor `build` 均在 P5 late-lowering 失败，错误为 `MissingProjectedStepCase`，确认问题发生在 P5 而不是 P6 LLVM body lowering。
- 当前判断：
  - pure caller 仍需要保留 call boundary，因为它调用的是已发布 `Step_F` contract 的 callee；
  - 但 caller 自身的 `StepSchema` 不应因此被无端扩大；
  - 因此需要在 P5 call-boundary contract 中显式记录“由 boundary 本地消费的 ordinary runtime-error case”，而不是把该 case 强行投影到 caller outward `StepSchema`，也不能静默丢弃。
- 下一步：
  1. 修改 `effect_lowered` IR / materialize 逻辑，为 call boundary 增加显式的本地 runtime-error case contract；
  2. 增加针对 `effect_resume_if_else_branch_single_perform.scoop` 的回归测试；
  3. 运行任务要求的定向验证与必要的格式化/测试/lint；
  4. 更新 `TODO-P5.md` / `TODO.md` 完成记录并提交。
- 已完成实现：
  - `LateLoweredCallBoundaryLowering` 现在可显式发布 `consumed_runtime_error_case`；
  - pure caller 调用带 compiler-generated one-shot `Raise<RuntimeError>` 上界的 callee 时，P5 保留 call boundary / complete dispatch，但不再把该 case 错误投影回 caller outward `StepSchema`；
  - `dump-effect-lowered` 的稳定输出也会展示该本地消费 contract。
- 已完成回归：
  - 新增 `refactor_boundary_lowering_keeps_local_runtime_error_contract_for_pure_caller_calls`，覆盖 `tests/fixtures/run-pass/effect_resume_if_else_branch_single_perform.scoop`。
- 已完成验证：
  - `cargo test -p scoopc --no-default-features refactor_boundary_lowering`
  - `cargo test -p scoopc --no-default-features refactor_effect_lowered_stage`
  - `cargo run -q -p scoop -- --effect-pipeline refactor dump-effect-facts tests/fixtures/run-pass/effect_resume_if_else_branch_single_perform.scoop`
  - `cargo run -q -p scoop -- --effect-pipeline refactor dump-effect-lowered tests/fixtures/run-pass/effect_resume_if_else_branch_single_perform.scoop`
  - `cargo run -q -p scoop -- --effect-pipeline refactor build tests/fixtures/run-pass/effect_resume_if_else_branch_single_perform.scoop -o /tmp/p5_t07a_probe.out`
  - `cargo test -p scoop --no-default-features effect_lowered`
  - `cargo clippy -p scoopc -p scoop --all-targets -- -D warnings`
- 当前状态：
  - `P5-T07a` 已满足完成条件；
  - `build` 已不再被 P5 `MissingProjectedStepCase` 阻塞，而是按预期前进到 P6 fail-fast；
  - 下一步只剩整理工作区并提交本次任务。
