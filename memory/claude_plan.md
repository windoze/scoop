# Claude Plan

## Current Invocation Goal
- 完成 `TODO.md` 索引所指向的第一个未完成详细任务，然后停止。
- 已定位首个未完成详细任务：`P4-T04`（`TODO-P4.md`）。

## Current Task
- `P4-T04`：实现 `resolved_outward_cases` SCC/dataflow 求解，并完成 `needs_reentry` / `impl_plan` / final block facts 回填。
- 直接前置：`P4-T03R` 已完成；`TODO.md` 与 `TODO-P4.md` 当前对任务顺序一致。
- 最近提交：`[P4-T03R] Close body/site facts review gap`。已检查最近提交列表，暂未看到显式声明且需要先于 `P4-T04` 单独插入的新未完成事项；后续若在提交正文或代码中发现会阻塞 `P4-T04` 的真实前置缺口，再按详细 TODO 规则处理。

## Progress
- 已完成 solver 主体：为 `MaterializedEffectFactsSolver` 落地 callable graph / SCC / dataflow / budget widening，并把 `resolved_outward_cases`、`needs_reentry`、`impl_plan` 从 builder 保守壳层收口为最终结果。
- 已完成 body/block finalization：在 `BodyEffectFacts` 内补充 solver 所需的 block->site / successor / handled-context 结构输入，并在 solver 中回填 final site facts 与 final `BlockEffectFacts.ambient_cases/outward_cases`。
- 已完成 stage 接通：`MaterializedMir` 现显式携带 `opt_level`，effect-facts stage 会从 canonical snapshot 派生 solver config，避免预算/优化等级走测试私货。
- 已完成定向验证：
  - `cargo test -p scoopc --no-default-features refactor_effect_solver`
  - `cargo test -p scoopc --no-default-features refactor_impl_plan`
  - `cargo test -p scoopc --no-default-features refactor_block_effect_facts`
  - `cargo test -p scoopc --no-default-features refactor_effect_facts_stage`
  - `cargo test -p scoopc --no-default-features refactor_site_effect_facts`
  - `cargo test -p scoopc --no-default-features refactor_body_effect_facts`
  - `cargo test -p scoopc --no-default-features refactor_nested_handle_classification`
  - `cargo clippy -p scoopc --all-targets --no-default-features -- -D warnings`
  - `cargo clippy -p scoopc --all-targets -- -D warnings`

## Execution Plan
1. 读取 `TODO.md`，确认详细任务文件与顺序。
2. 按顺序检查对应 `TODO-Px.md`，定位第一个标题未带 `[DONE]` 的详细任务。
3. 检查最新提交是否有与该任务直接相关且未完成的问题；若有，视为当前任务范围内内容或前置依赖。
4. 阅读 `P4-T04` 的任务要求、约束、验收条件，以及 `effect_facts` / `effect_refactor_pipeline` / `mir pass_view` 的现状实现。
5. 先确认 solver 当前是否已有壳层与未完成接口，再实现：call graph/SCC、case-set dataflow、budget widening、`needs_reentry`、`impl_plan`、site/block finalization。
6. 为 `P4-T04` 增补定向测试，覆盖 direct/candidate/dynamic、budget exhaustion、`O0` vs 非 `O0`、nested handle 与 final block facts。
7. 运行与当前任务直接相关的测试和 `clippy`，修复发现的问题。
8. 更新 `memory/claude_plan.md` 记录关键进展或计划变化。
9. 在 `TODO-P4.md` 中将 `P4-T04` 标记为 `[DONE]` 并补写完成记录；同步更新 `TODO.md`。仅当阶段计划变化时才更新 `PLAN.md`。
10. 按要求创建一次 git 提交，提交信息包含 `P4-T04`，然后停止。

## Planning Notes
- 不跳过未标记 `[DONE]` 的任务。
- 不以变通方案规避规范缺口；若存在阻塞，先显式建前置任务并同步索引。
- 仅处理一个详细任务；完成或记录阻塞后立即停止。
