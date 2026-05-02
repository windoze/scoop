## 当前执行计划

说明：此文件记录可公开的执行摘要、决策依据和步骤计划，不记录逐字内部推理。

1. 读取 `TODO.md` 作为索引，并按索引顺序检查对应的 `TODO-Px.md`。
2. 根据详细任务标题是否带有 `[DONE]`，确定第一个未完成的详细任务。
3. 检查最近一次提交是否提到与该任务直接相关且未完成的问题；如果是，则将其视为当前任务的一部分或按要求补充前置任务。
4. 阅读当前任务涉及的代码、测试、文档与约束，确认实现边界与验收标准。
5. 如无阻塞，直接实现当前任务；如遇到阻塞当前任务的真实缺陷或缺失能力，则在对应 `TODO-Px.md` 中补充最小前置任务并同步 `TODO.md`。
6. 运行与当前任务直接相关的测试、格式化、以及必要的 `cargo clippy --all-targets -- -D warnings` / 其他验证命令；若失败则继续修复直到通过，或确认存在必须先处理的前置阻塞。
7. 更新任务记录：在对应 `TODO-Px.md` 中将任务标题标记为 `[DONE]` 并填写完成记录；若任务索引或顺序变化，同步更新 `TODO.md`；仅在阶段计划真实变化时更新 `PLAN.md`。
8. 检查工作区差异，确保不回退非本次任务相关的已有改动；如用户此前调用中断且留下未提交变更，在完成当前任务时一并纳入提交。
9. 使用清晰的提交信息提交本次任务相关改动，然后停止，不继续处理下一个任务。

## 进度记录

- 已初始化计划文件，尚未读取任务索引与详细任务文件。
- 已读取 `TODO.md` 与 `TODO-P4.md`，按 `[DONE]` 规则确认首个未完成详细任务是 `P4-T02aR`：Review canonical pass-view 对 ordinary callable body 的发布结果，确认 P4 不再需要 raw/fallback 键空间。
- 最近一次提交为 `[P4-T02a] Publish ordinary callables in canonical pass view`，与当前 review 直接相关；当前需要验证该提交是否完整满足 `P4-T02aR` 的 review 目标，若发现会阻塞 `P4-T03` 的问题，则必须先补前置任务或直接修复。
- 当前 review 计划：
  1. 读取并复核 `mir/materialize.rs`、`mir/callables.rs`、`mir/pass_view.rs`、`effect_refactor_pipeline/mir_stage.rs`、`effect_refactor_pipeline/effect_facts_stage.rs` 的实现边界。
  2. 检查 `pass_view().instances()`、`owner_of_callable()`、`root_body()`、`callable_bodies()` 的 canonical owner/family 行为，以及 `effect_facts` builder 是否仍存在 raw/fallback 键空间依赖。
  3. 运行 `P4-T02aR` 要求的定向测试与相关验证命令。
  4. 若 review 通过，则在 `TODO-P4.md` 与 `TODO.md` 标记 `P4-T02aR` 为 `[DONE]` 并补全完成记录；若发现阻塞，则按要求补充最小前置任务并同步索引。
- review 过程中发现一个直接相关回归：`cargo test -p scoopc --no-default-features materialized_effect_facts_builder_uses_canonical_pass_view_snapshot` 失败，报 `MissingCallableRoot { fqn: "sample.exercise" }`。原因是 `effect_facts::builder::collect_callable_seeds(...)` 仍把“pass-view 中保留 family 身份但当前 canonical snapshot 已无 root body”的 family 当作硬错误，而不是按当前 canonical snapshot 直接跳过。
- 已完成修复：`crates/scoopc/src/effect_facts/builder.rs` 现在会对这类无 canonical root body 的 family 直接跳过，明确不回 raw MIR，也不要求 fallback 键空间；仅对当前 snapshot 中仍存在 root body 的 family 构建 callable/body facts。
- 已复验通过：
  1. `cargo test -p scoopc --no-default-features materialized_effect_facts_builder_uses_canonical_pass_view_snapshot`
  2. `cargo test -p scoopc --no-default-features materialized_pass_view_non_generic`
  3. `cargo test -p scoopc --no-default-features refactor_effect_facts_stage_non_generic`
  4. `cargo test -p scoopc --no-default-features refactor_effect_facts_stage`
  5. `cargo test -p scoopc --no-default-features caller_side_inlining_keeps_non_generic_pass_roots_visible`
  6. `cargo test -p scoopc production_codegen_observes_caller_side_mir_inlining_for_non_generic_body`
  7. `cargo clippy -p scoopc --all-targets --no-default-features -- -D warnings`
  8. `cargo clippy -p scoopc --all-targets -- -D warnings`
