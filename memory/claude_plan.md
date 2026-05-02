## 当前计划

1. 读取 `TODO.md` 作为索引，并检查相关 `TODO-Px.md` 详细任务文件，定位第一个未完成的详细任务（以标题是否带 `[DONE]` 为准）。
2. 读取该任务的详细要求、约束、依赖与完成记录，并检查最新一次 git 提交是否提到与该任务直接相关的未完成事项。
3. 实现当前任务所需代码改动；若发现阻塞该任务的真实前置问题，则仅新增最小必要前置任务，并同步 `TODO.md`。
4. 运行与该任务直接相关的验证命令；在可行范围内补充或修复测试，确保通过。
5. 更新任务记录：在对应 `TODO-Px.md` 中将任务标题标记为 `[DONE]` 并填写完成记录；如任务索引有变化，同步更新 `TODO.md`；仅在阶段计划变化时更新 `PLAN.md`。
6. 复查工作区变更，按要求创建一次 git 提交，然后停止，不进入下一个任务。

## 进度记录

- 已创建本计划文件，准备开始定位首个未完成详细任务。
- 已读取 `TODO.md` 与 `TODO-P4.md`，确认首个未完成详细任务为 `P4-T03`：构建 `BodyEffectFacts` / `SiteEffectFacts` 与 local-case 结构化分析。
- `P4-T03` 的详细任务文件中记录过一次已处理的前置阻塞（新增 `P4-T02a` / `P4-T02aR`），两项现已完成；接下来需要检查最新提交是否还留下与 `P4-T03` 直接相关的未完成事项，并阅读当前 `effect_facts` / MIR 代码边界后实施该任务。
- 已完成首轮实现补齐：
  - 在 `crates/scoopc/src/effect_facts/builder.rs` 中为 callable step schema 增加 body-derived effect row 收集，确保本地 `perform` / `resume` / `handle` 即使被局部吸收，也仍有稳定 case/tag 可供 `BodyEffectFacts` / `SiteEffectFacts` 使用。
  - 在 `crates/scoopc/src/effect_facts/facts.rs` 中补充按 `BasicBlockId` / `SiteId` / `InstanceKey` 的直接查询 API，便于 T04 与定向测试只消费 facts，不回 MIR/HIR 重新推断。
  - 已新增 `refactor_site_effect_facts_*`、`refactor_body_effect_facts_*`、`refactor_nested_handle_classification_*` 定向测试，覆盖 direct/fun-value/dispatch/resume、perform/handle、以及 nested handle 分类。
- 已完成验证：
  - `cargo test -p scoopc --no-default-features refactor_site_effect_facts`
  - `cargo test -p scoopc --no-default-features refactor_body_effect_facts`
  - `cargo test -p scoopc --no-default-features refactor_nested_handle_classification`
  - `cargo test -p scoopc --no-default-features materialized_effect_facts_builder_uses_canonical_pass_view_snapshot`
  - `cargo test -p scoopc --no-default-features refactor_effect_facts_stage`
  - `cargo clippy -p scoopc --all-targets --no-default-features -- -D warnings`
  - `cargo clippy -p scoopc --all-targets -- -D warnings`
- 已同步文档状态：`TODO-P4.md` 已把 `P4-T03` 标记为 `[DONE]` 并补齐完成记录，`TODO.md` 索引也已同步。
- 下一步：执行提交前 git 复核，创建 `P4-T03` 提交，然后停止。
