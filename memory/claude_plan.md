# 本次执行计划

## 约束与执行原则
- 先以 `TODO.md` 作为索引，再读取对应 `TODO-Px.md`，确定第一个未完成的详细任务。
- 仅完成一个详细任务；若遇到阻塞，则为其补充最小前置任务并同步 `TODO.md`，随后停止。
- 不采用变通方案、夹具特判、弱化规格或绕过缺失能力的做法。
- 在执行过程中，如计划变化、发现阻塞、完成关键步骤，会及时更新本文件。

## 初始步骤计划
1. 读取 `TODO.md`，确认任务索引与详细任务文件映射。
2. 按索引顺序读取相关 `TODO-Px.md`，找到第一个标题未带 `[DONE]` 的详细任务。
3. 检查最近一次提交是否直接提到与该任务相关的未完成问题；若是，则将其视为当前任务的一部分或前置条件。
4. 阅读当前任务要求、约束、依赖、验收标准，并检查相关代码与测试位置。
5. 实施最小且正确的代码修改，必要时补充或调整测试。
6. 运行与该任务相关的验证；如任务影响范围要求较大，再运行更完整的 `cargo` 检查、测试与 `clippy`。
7. 更新对应 `TODO-Px.md` 的完成记录并将任务标题标记为 `[DONE]`；如索引有变化，同步更新 `TODO.md`。
8. 若阶段级计划未变化，则不修改 `PLAN.md`；仅在阶段依赖或完成标准改变时更新。
9. 检查工作区状态，按要求提交一次 Git commit，然后停止，不继续下一个任务。

## 当前状态
- 已读取 `TODO.md` 索引，并确认第一个未完成详细任务为 `TODO-P5.md` 中的 `P5-T03R`。
- 已检查最近提交：`[P5-T03] Build fact-driven segmentation skeleton`，提交标题未显式声明额外未完成前置问题。

## 针对当前任务的执行计划
1. 阅读 `P5-T03R` 指定的实现与相关事实定义，重点检查 boundary 选择、owner/resume 显式映射，以及 expression 内 boundary 切分是否只依赖 P3/P4 显式结果。
2. 运行 `P5-T03R` 要求的文本搜索，确认新主线实现没有依赖 `Span`、HIR、statement-only 快捷路径或 code-shape 特判作为事实来源。
3. 重新运行 `P5-T03` 要求的测试与校验命令，确认 review 结论成立。
4. 若 review 发现阻塞性问题：优先修复；若无法在当前任务内直接正确落地，则在详细 TODO 中补充最小前置任务并同步索引后停止。
5. 若 review 通过：把 `P5-T03R` 标记为 `[DONE]`，填写完成记录，必要时同步 `TODO.md`，然后提交并停止。

## 已完成的关键检查
- 已审阅 `crates/scoopc/src/effect_lowered/segment.rs`：boundary 选择直接读取 `BodyEffectFacts::site(...)`，并把 owner/resume 绑定固化到 `LateLoweredBoundaryMap` / `LateLoweredResumeStateMap`；state graph 中的切分基于 canonical MIR block/statement cursor，而不是源码 AST / span。
- 已审阅 `crates/scoopc/src/effect_lowered/builder.rs`：late-lowering 只消费 canonical pass-view 与 P4 facts；对 declaration-only family 采用空壳 shell，对有 body 但缺 facts 的 family 继续报错，没有引入 legacy 回退。
- 已审阅 `crates/scoopc/src/effect_facts/facts.rs`：`BodyEffectFacts` 向 P5 暴露的 authoritative 输入为 block/site facts 与 solver facts 结构，没有额外的源码形状查询接口供 P5 依赖。
- 已执行 `rg -n "Span|hir::|single perform|tail-resume|linear body|statement-only" crates/scoopc/src/effect_lowered crates/scoopc/src/effect_refactor_pipeline`：
  - `effect_lowered` 主实现未命中这些回退条件；仅 `effect_lowered/ir.rs` 的测试代码使用 `Span` 构造测试样本。
  - `effect_refactor_pipeline` 的命中集中在前序 HIR stage / stage dispatcher，本次 P5 主实现未据此分流 boundary 或 segmentation。
- 已执行并通过：
  - `cargo test -p scoopc --no-default-features refactor_late_boundary_selection`
  - `cargo test -p scoopc --no-default-features refactor_late_segmentation`
  - `cargo test -p scoopc --no-default-features refactor_owner_resume_state`
  - `cargo test -p scoopc --no-default-features refactor_late_lowered_ir`
  - `cargo test -p scoopc --no-default-features refactor_effect_lowered_stage`
  - `cargo clippy -p scoopc --no-default-features --all-targets -- -D warnings`

## 当前结论
- 目前未发现阻塞 `P5-T03R` 的实现缺口。
- `TODO-P5.md` 已将 `P5-T03R` 标记为 `[DONE]` 并补全完成记录；`TODO.md` 也已同步索引状态。
- `PLAN.md` 本轮无需变更；下一步仅剩按任务要求提交 Git commit 并停止。
