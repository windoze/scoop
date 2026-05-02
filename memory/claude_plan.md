# Claude Plan

## Planning note

按你的要求，我会在这个文件里维护本次执行计划、关键决策、进度更新和任何阻塞信息。
出于安全与协作考虑，这里记录的是可执行计划、检查点、依据与结论，而不是逐字的内部推理草稿。

## Initial execution plan

1. 读取 `TODO.md`，把它当作索引使用，不把它当作任务细节来源。
2. 按 `TODO.md` 引用顺序读取对应的 `TODO-Px.md` 文件，定位第一个标题未带 `[DONE]` 的详细任务。
3. 检查最近一次提交是否明确提到与该任务直接相关但未完成的问题；若是，则将其视为当前任务的一部分或必要前置。
4. 阅读当前任务涉及的代码、测试、文档与约束，确认需要变更的最小范围。
5. 若任务可直接完成：实现任务、补充或更新测试、运行必要验证，直到通过。
6. 若存在阻塞当前任务的真实缺口：在相应 `TODO-Px.md` 中插入最小必要前置任务，更新当前任务依赖，并同步 `TODO.md`；仅当阶段计划被改变时才更新 `PLAN.md`。
7. 完成后在对应 `TODO-Px.md` 中将该任务标题显式标记为 `[DONE]`，补全完成记录；若索引受影响，同步更新 `TODO.md`。
8. 检查工作区变更，确保不回退非本人改动；按要求创建一次清晰的 git 提交，然后停止，不继续下一个任务。

## Progress log

- 已创建计划文件。
- 已读取 `TODO.md`，并按索引确认首个未完成详细任务为 `TODO-P4.md` 中的 `P4-T03R`。
- 已核对 `TODO-P4.md`：`P4-T03` 已显式标记为 `[DONE]`，`P4-T03R` 仍未完成，因此本次执行单元就是该 review 任务。
- 下一步：检查最近一次提交是否明确提到与 `P4-T03R` 直接相关且未完成的问题；随后阅读本任务涉及实现与测试，进行 review、必要修复、验证、文档更新和提交。

## Current invocation update

- 本轮从头重新核对 `TODO.md` 索引与对应 `TODO-Px.md` 明细，不依赖之前日志里的任务定位结果。
- 在运行任何 shell 命令前，先把本轮执行计划写入本文件；后续每完成关键检查、发现阻塞、调整计划、完成验证或完成提交前，都会继续更新本文件。
- 本轮步骤：
  1. 读取 `TODO.md` 作为索引，并按顺序读取被引用的 `TODO-Px.md`，重新定位第一个未完成详细任务。
  2. 若当前任务存在相关的最近一次未完成提交线索，则把它纳入当前任务或作为前置依赖处理。
  3. 阅读该任务涉及的代码、测试、文档和已有完成记录，判断是直接实现/修复，还是必须先新增前置任务。
  4. 对当前任务做最小正确改动，补齐必要测试与验证，不接受规避性方案。
  5. 更新 `TODO-Px.md` 的任务状态与完成记录；若索引、标题或顺序变化，同步更新 `TODO.md`；仅在阶段计划变化时更新 `PLAN.md`。
  6. 检查并提交当前任务涉及的全部未提交改动，然后停止，不继续下一个任务。

## Current progress

- 已重新读取 `TODO.md` 与 `TODO-P4.md`，确认当前第一个未完成详细任务仍是 `P4-T03R`（`P4-T03` 已显式标记为 `[DONE]`，`P4-T03R` 尚未带 `[DONE]`）。
- 下一步：检查最近一次提交是否与 `P4-T03R` 直接相关且留有未完成事项；随后阅读 `effect_facts` 相关实现与测试，依据 review 结果决定是：
  1. 直接完成 review 并补齐完成记录；或
  2. 若发现阻塞 `P4-T04` 的真实缺口，则先最小化修复；或
  3. 若当前任务无法在本轮直接修复，则在 `TODO-P4.md` 中新增必要前置任务并同步 `TODO.md` 后停止。
- 已检查最近提交：`HEAD` 为 `[P4-T03] Materialize body and site effect facts`，提交信息未显式记录新的未完成事项；因此继续以代码与验证结果为准完成本次 review。
- 已按任务要求搜索 `LoweredHir|hir::HandleExpr|continuation_resume_call_sites|effect_op_call_sites|Span`：
  - `effect_facts` 主实现中未发现回 HIR side table 取 contract 的命中；`Span` 仅出现在 `builder.rs` 测试模块里。
  - `effect_refactor_pipeline` 中 `LoweredHir` / `Span` 的命中集中在既有 `hir_stage`、stage wrapper 和测试注释，不在 `effect_facts` 主分析逻辑中。
- review 过程中发现一个与当前任务结论直接相关的小缺口：`CallSiteEffectFacts` 对 `KnownInstance` 的 effectful direct call，会在 `resolved_cases` 仍是 T03 保守上界时提前标成 `EffectPrecision::Precise`；这与 `P4-T03` 的完成记录里“保守 resolved_cases 与 precision”的承诺不一致，也会让 `P4-T03R` 不能严格成立。
- 修复计划：把这类 pre-solver known-instance call site 的初始 precision 改为保守值，并补一条定向断言锁定该 contract；之后重跑 `P4-T03` / `P4-T03R` 所需验证，再决定是否可将 `P4-T03R` 标记完成。
- 已完成最小修复：`crates/scoopc/src/effect_facts/builder.rs` 现在只对 empty case-set 的 known-instance call site 维持 `EffectPrecision::Precise`；对仍依赖 T04 求解的非空 known-instance outward case，初始 precision 改为保守的 `EffectPrecision::Widened`。
- 已补测试：`refactor_site_effect_facts_capture_call_target_modes_and_resume_contracts` 现在显式锁定“带 outward case 的 known direct call 在 P4-T03 阶段必须保守标宽，而不是提前声称精确”。
- 已重跑并通过验证：
  - `cargo test -p scoopc --no-default-features refactor_site_effect_facts`
  - `cargo test -p scoopc --no-default-features refactor_body_effect_facts`
  - `cargo test -p scoopc --no-default-features refactor_nested_handle_classification`
  - `cargo test -p scoopc --no-default-features materialized_effect_facts_builder_uses_canonical_pass_view_snapshot`
  - `cargo test -p scoopc --no-default-features refactor_effect_facts_stage`
  - `cargo clippy -p scoopc --all-targets --no-default-features -- -D warnings`
  - `cargo clippy -p scoopc --all-targets -- -D warnings`
- 结论：当前未再发现会阻塞 `P4-T04` 的 body/site facts contract 缺口；可以进入文档与索引同步、提交前检查阶段。
- 已完成文档同步：`TODO-P4.md` 已将 `P4-T03R` 标记为 `[DONE]` 并补齐 review 完成记录，`TODO.md` 索引也已同步；`PLAN.md` 仍无需改动。
- 下一步：执行本轮提交，提交内容应只包含 `P4-T03R` review 修复、验证记录、索引同步与计划日志，然后停止。
