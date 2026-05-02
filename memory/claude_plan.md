# 当前执行计划

## 说明
- 不记录内部私有推理细节；这里维护可审阅的执行计划、关键判断依据、进度与变更记录。

## 本次目标
- 按 `TODO.md` 索引与对应 `TODO-Px.md` 详细任务文件，找到第一个未完成的详细任务。
- 完成该任务，执行相关验证，更新任务记录，并创建一次 git 提交。
- 本次只处理一个详细任务，完成后停止。

## 初始执行步骤
1. 读取 `TODO.md`，确认索引中的任务顺序、对应详细文件引用以及 `[DONE]` 标记状态。
2. 按索引顺序读取相关 `TODO-Px.md` 文件，依据“标题是否带 `[DONE]`”判断完成状态，定位第一个未完成详细任务。
3. 检查最近提交是否直接提及与该任务相关但未完成的问题；如果该问题构成当前任务的直接组成部分或前置阻塞，则纳入本次处理范围或补录为前置任务。
4. 阅读当前任务涉及的源码、测试、规范与文档，明确约束、依赖与验收要求。
5. 实施最小且正确的代码改动；若遇到阻塞当前任务的真实缺口，不做绕过，而是在相应 `TODO-Px.md` 中补充最小前置任务并同步 `TODO.md`。
6. 运行与当前任务直接相关的测试、格式化、检查与必要的 lint；若失败则修复后重跑。
7. 更新 `TODO-Px.md` 的任务标题为 `[DONE]`（仅在真正完成时），补全完成记录；若任务索引状态或顺序变化，同步更新 `TODO.md`；只有阶段级计划变化时才更新 `PLAN.md`。
8. 检查工作区中本次任务相关改动，按任务编号撰写清晰提交信息并提交。
9. 停止，不继续处理下一个任务。

## 进度记录
- 已创建本计划文件。
- 已读取 `TODO.md` 与 `TODO-P4.md`，确认当前首个未完成详细任务为 `P4-T01R`（该任务已有完成记录，但标题尚未带 `[DONE]`，按规则仍视为未完成）。
- 已读取 `PLAN.md` 与 review 要求列出的关键文件，当前 review 聚焦于：
  - `crates/scoopc/src/effect_facts/**`
  - `crates/scoopc/src/effect_refactor_pipeline/effect_facts_stage.rs`
  - `crates/scoopc/src/lib.rs`
  - `crates/scoopc/src/program_facts.rs`
  - `crates/scoopc/src/mir/summary.rs`
  - `crates/scoopc/src/effect/analysis.rs`
- 已执行新 facts 术语搜索，当前结果显示命中集中在新 `effect_facts` 子系统、refactor stage 与注释中，未在 `program_facts.rs`、`mir/summary.rs`、`effect/analysis.rs` 中发现相应业务命中。
- 已完成复验：
  - `cargo test -p scoopc --no-default-features refactor_effect_facts_stage`
  - `cargo run -q -p scoop --no-default-features -- --effect-pipeline refactor dump-mir tests/fixtures/mir_refactor/dispatch_and_resume_call.scoop`
  - `cargo clippy -p scoop -p scoopc --all-targets --no-default-features -- -D warnings`
- 复核结论：当前未发现 `MaterializedEffectFacts` / `StepSchema` / `ContinuationSchema` / `resolved_outward_cases` / `impl_plan` 渗入 `ProgramFacts`、`InstanceSummary` 或 `EffectAnalysisCtx` 的业务实现；`P4-T01R` 可完成。
- 已更新 `TODO-P4.md` 与 `TODO.md`，为 `P4-T01R` 补齐 `[DONE]` 标记并追加本次复核记录。
- 下一步：检查当前工作区差异，只暂存并提交本次任务相关文件，然后停止。
