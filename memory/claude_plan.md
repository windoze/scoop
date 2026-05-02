## 当前执行计划

1. 读取 `TODO.md`，确认详细任务文件映射关系。
2. 按索引顺序读取对应的 `TODO-Px.md`，定位第一个标题未带 `[DONE]` 的详细任务。
3. 检查最近一次提交是否包含与该任务直接相关且明确未完成的问题；若有，将其视为当前任务的一部分或必要前置。
4. 阅读当前任务涉及的代码、测试、规范与依赖，确认实现边界与验收要求。
5. 直接完成该任务；若存在阻塞且无法按规范完成，则只引入最小必要前置任务，并同步 `TODO.md`。
6. 运行相关验证，至少覆盖任务要求的测试，并补充必要回归测试。
7. 更新 `TODO-Px.md` 的完成记录与标题 `[DONE]` 标记；如索引有变动，同步更新 `TODO.md`；仅在阶段计划变化时更新 `PLAN.md`。
8. 复查工作区中的本次改动，确保不回退非本人改动。
9. 提交本次工作，提交信息使用当前任务编号。

## 进度记录

- 已开始：初始化本次执行计划，下一步定位首个未完成详细任务。
- 已定位当前任务：`TODO-P4.md` 中首个未完成详细任务为 `P4-T02R`（Review schema pool 与 callable facts，确认 identity 和 case contract 已经固定）。
- 最近一次提交为 `[P4-T02] Materialize effect schema pool and callable facts`，与当前 review 直接相关，但提交信息未显式记录新的未完成前置问题；因此按 `P4-T02R` 原要求继续复核实现与验证矩阵。
- 当前执行步骤：
  1. 检查 `crates/scoopc/src/effect_facts/schema.rs`、`facts.rs`、`builder.rs`、`crates/scoopc/src/mir/materialize.rs`。
  2. 重新运行 `P4-T02` 要求的定向测试与搜索命令。
  3. 若 review 通过，则更新 `TODO-P4.md` 与 `TODO.md` 的 `[DONE]` 标记和完成记录；若发现阻塞问题，则先修复或补最小前置任务。
  4. 运行必要的最终校验后提交本次结果。
- 已完成复核：`schema.rs` / `facts.rs` / `builder.rs` / `mir/materialize.rs` 与 `mir/pass_view.rs` 已确认 identity 链路闭合；`StepSchemaId` 依赖稳定的 `instance_keys` 顺序，`InstanceKey` / `instance_fqn` 能区分 `type_args + eff_args`，`CaseTag` / `ConcreteOpKey` / `ContinuationSchemaKey` 已满足当前任务要求。
- 已完成验证：`refactor_effect_schema`、`refactor_continuation_schema`、`refactor_callable_effect_facts_shell`、`refactor_effect_facts_stage`、`materialized_effect_facts_builder_uses_canonical_pass_view_snapshot`、refactor `dump-mir` smoke、以及 `cargo clippy -p scoop -p scoopc --all-targets --no-default-features -- -D warnings` 全部通过。
- 已完成文档收口：`P4-T02R` 已在 `TODO-P4.md` 与 `TODO.md` 中标记为完成；下一步仅剩检查工作区并创建本次提交。
