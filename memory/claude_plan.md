# 本轮执行计划

## 说明

按你的要求，我会先把本轮的执行计划记录在这里，再开始读取任务文件和执行实现。

出于安全与协作边界，我不会写入逐字的内部推理过程；这里记录的是可审阅的执行计划、判断依据摘要、进度更新和关键决策。

## 初始计划

1. 读取 `TODO.md`，确认它只是索引，并按索引顺序找到对应的 `TODO-Px.md` 文件。
2. 依次检查相关 `TODO-Px.md` 中的任务标题，定位第一个标题未带 `[DONE]` 的详细任务。
3. 阅读该任务的完整要求、约束、依赖、验证要求，以及最近提交中是否有与该任务直接相关的未完成项。
4. 检查工作区当前状态，避免覆盖我未创建的改动；只在当前任务所需范围内修改文件。
5. 实现该任务，优先做最小且正确的改动；如果遇到真实阻塞，则按要求在相应 `TODO-Px.md` 中补充最小前置任务并同步 `TODO.md`。
6. 运行与该任务直接相关的测试、必要的仓库级校验，以及无警告检查；若失败则继续修复直到通过，或确认存在必须先解决的新前置任务。
7. 更新文档记录：
   - 在对应 `TODO-Px.md` 中把该任务标题标记为 `[DONE]` 并补全完成记录；如果未完成则记录阻塞与新前置任务。
   - 若任务索引发生变化，同步更新 `TODO.md`。
   - 仅在阶段计划确实变化时更新 `PLAN.md`。
8. 按任务要求创建一次 git 提交，然后停止，不继续处理下一个任务。

## 进度记录

- 已创建本计划文件，下一步开始读取任务索引并定位首个未完成详细任务。
- 已读取 `TODO.md` 索引并确认首个未完成详细任务为 `TODO-P4.md` 中的 `P4-T05b`：修正 `ContinuationSchema.surface_ty` 与 `out_step_schema` 的 contract 边界。
- 已检查当前工作区与最近提交：当前未提交改动仅包含本计划文件；最近一次提交信息为 `Update plan`，未显式记录与 `P4-T05b` 直接相关的未完成项。

## 当前执行细化

1. 阅读 `crates/scoopc/src/effect_facts/{schema.rs,builder.rs,dump.rs}` 与相关测试，确认 `surface_ty` / `out_step_schema` 当前是如何构造、缓存、展示和断言的。
2. 找出把 synthetic step upper bound 反推回 `ContinuationSchema.surface_ty` 的代码路径，并判断是否同时影响 ordinary continuation case 与 resume-site synthetic schema。
3. 以最小改动修正 contract：
   - 保留 `out_step_schema` 的 runtime-error 上界；
   - 让 `surface_ty` 只反映 source-visible residual row；
   - 确保 identity / dump 仍稳定可比较。
4. 更新或新增定向测试与 `.effectfacts` golden，重点覆盖 `Pure` / 非 `Pure` residual row、runtime-error upper bound 分离、以及 `resolved_outward_cases` / `impl_plan` 不漂移。
5. 运行任务要求中的测试与必要校验；若 golden 需要刷新，则按仓库现有方式更新并复跑验证。
6. 任务完成后更新 `TODO-P4.md`、同步 `TODO.md`，并创建一次 git 提交后停止。

## 当前发现

- `P4-T05b` 的主要错误路径已经定位在 `crates/scoopc/src/effect_facts/builder.rs`：
  - 普通 callable 的 `step_effect_row` 在 `collect_callable_seeds(...)` 中会为特定 reentry callable 额外补入 compiler-generated `Raise<RuntimeError>`；
  - `intern_step_schema(...)` 又直接用这个扩张后的 row 构造每个 case 的 `ContinuationSchema.surface_ty`；
  - 因而 `surface_ty` 被错误扩大为“source residual row + internal one-shot runtime-error upper bound”。
- `resume` site synthetic schema 目前是正确分层的：`out_step_schema` 取 `resume.out_effects + runtime_error_effect_ty`，而 `continuation_schema.surface_ty` 直接取 `resume.continuation_ty`，不需要改回退路径。
- 预计的最小修正是为普通 callable 显式保留“source-visible continuation residual row”和“step-schema upper bound row”两条独立数据路径，并只让后者承载 compiler-generated runtime-error case。

## 已完成步骤

1. 已修改 `crates/scoopc/src/effect_facts/builder.rs`：
   - 普通 callable 现在分别保留 `surface_effect_row` 与 `step_effect_row`；
   - `ContinuationSchema.surface_ty` 改为只由 source-visible residual row 构造；
   - `out_step_schema` 仍可额外携带 compiler-generated `Raise<RuntimeError>` case；
   - `resume` synthetic step schema 也已按同一边界分离 `continuation_surface_row` 与 step upper bound。
2. 已补充/更新定向测试：
   - builder 测试覆盖 compiler-generated runtime-error upper bound 不得扩大 `surface_ty`；
   - builder 测试覆盖 `resume` synthetic schema 在 `Pure` / `Boom` residual row 下仍保持 source-visible `surface_ty`；
   - stage 测试覆盖 P4 authoritative handoff 中 `surface_ty` 与 step upper bound 分离。
3. 已更新受影响 golden：
   - `tests/fixtures/effect_facts/single_case_impl_plan.effectfacts`
   - `tests/fixtures/effect_facts/dynamic_fallback_widening.effectfacts`
   - `tests/fixtures/effect_facts/nested_handle_self_contained_vs_outward.effectfacts`
   - `tests/fixtures/effect_facts/dispatch_and_resume_call.effectfacts`

## 验证结果

- 已通过：
  - `cargo fmt --all`
  - `cargo test -p scoopc --no-default-features refactor_continuation_schema_surface_ty`
  - `cargo test -p scoopc --no-default-features compiler_continuation_runtime_error`
  - `cargo test -p scoopc --no-default-features refactor_site_effect_facts_capture_call_target_modes_and_resume_contracts`
  - `cargo test -p scoopc --no-default-features refactor_callable_effect_facts_shell_uses_final_shape_and_runtime_error_case`
  - `cargo test -p scoopc --no-default-features refactor_effect_facts_stage_surface_ty`
  - `cargo test -p scoop --no-default-features dump_effect_facts`
  - `cargo run -q -p scoop --no-default-features -- --effect-pipeline refactor dump-effect-facts tests/fixtures/effect_facts/single_case_impl_plan.scoop`
  - `cargo run -q -p scoop --no-default-features -- --effect-pipeline refactor dump-effect-facts tests/fixtures/effect_facts/dynamic_fallback_widening.scoop`
  - `cargo run -q -p scoop --no-default-features -- --effect-pipeline refactor dump-effect-facts tests/fixtures/effect_facts/nested_handle_self_contained_vs_outward.scoop`
  - `cargo run -q -p scoop --no-default-features -- --effect-pipeline refactor dump-effect-facts tests/fixtures/effect_facts/dispatch_and_resume_call.scoop`
  - `cargo run -q -p scoop --no-default-features -- --effect-pipeline refactor test --fixtures tests/fixtures/effect_facts`
  - `cargo clippy -p scoop -p scoopc --all-targets --no-default-features -- -D warnings`

## 待收尾

1. 在 `TODO-P4.md` 中把 `P4-T05b` 标记为 `[DONE]` 并写入完成记录。
2. 同步 `TODO.md` 索引中的 `P4-T05b` 状态。
3. 检查最终 diff，提交 git commit，然后停止。
