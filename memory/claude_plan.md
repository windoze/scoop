# 执行计划与进度记录

## 说明

用户要求先把计划写入本文件，再执行任何命令。基于安全与协作边界，这里记录可验证的执行计划、决策摘要、关键发现和后续调整，不写入不可审计的内部长篇推理。

## 初始计划

1. 检查最新一次 Git 提交信息，确认是否明确提到任何既有问题、回归、规约不匹配或待修复事项。
2. 如果最新提交提到了既有问题，先定位并修复该问题，再继续后续流程。
3. 读取 `TODO.md`，识别第一个未完成任务。
4. 读取 `PLAN.md`，核对当前计划与 `TODO.md` 是否一致。
5. 判断首个未完成任务是否足够小且可在本轮完整完成：
   - 若可完成，直接实现。
   - 若过大或存在缺失前置条件，则把任务拆解为更小子任务，并更新 `PLAN.md` / `TODO.md`，本轮只处理拆解后的第一个子任务。
6. 实现任务时，若在探查、测试、评审或修复过程中发现任何既有问题：
   - 立即将其视为当前范围内事项。
   - 若该问题阻塞当前任务，则先修复；若无法在本轮直接修复，则把它作为前置任务插入 `TODO.md` 当前任务之前，更新 `PLAN.md`，提交后停止。
7. 完成实现后执行相关验证，至少覆盖：
   - 受影响范围的测试；
   - `cargo fmt`；
   - `cargo test --all`（若成本可接受且与变更相关）；
   - `cargo clippy --all-targets -- -D warnings`（若当前仓库状态允许，且与本轮修改范围匹配）。
8. 更新文档与任务状态：
   - 在 `TODO.md` 标记本轮完成项；
   - 在 `PLAN.md` 记录当前状态、拆分、依赖或阻塞调整；
   - 本文件同步记录关键进展。
9. 使用清晰的 Git 提交信息提交本轮更改。
10. 停止，不进入下一项任务。

## 进度记录

- 已检查最新提交信息、`TODO.md` 与 `PLAN.md`。
- 最新提交为 `b2b9a0e5 [T5000c1R] Review ProgramFacts shared side table`；提交正文未额外点名需要先行修复的既有问题，因此不触发新的优先修复项。
- 当前顺序上的首个未完成任务为 `T5000c2 抽出 backend-agnostic 的 EffectAnalysisCtx 与 shared local metadata`。
- 已初步确认 `T5000c2` 的工作范围主要集中在 `crates/scoopc/src/llvm/codegen/effect/state_machine_plan.rs` 中：
  - `HandlePlanContext::from_codegen(...)` 仍直接从 `MainCodegen.function_cx.env` 取 known locals / metadata / source path；
  - `SuspendCallAnalysis` 仍平铺保存 known-fun facts、local metadata、source path 与 `ProgramFacts`；
  - `state_machine_segments.rs` / `state_machine_transform.rs` 的测试 helper 仍手工拼装同类上下文。
- 当前判断：`T5000c2` 可以在本轮直接完成，不需要再拆子任务。

## 当前执行方案

1. 新增共享分析上下文模块，抽出 `EffectAnalysisCtx`、`KnownLocalMetadata` 以及 local metadata / synthetic symbol / source-path 相关构造辅助。
2. 让 `HandlePlanContext` / `SuspendCallAnalysis` 基于共享 `EffectAnalysisCtx` 工作，减少或移除直接从 backend 上下文反取分析状态的路径。
3. 把 `state_machine_segments.rs` / `state_machine_transform.rs` 测试 helper 的手工上下文拼装改为复用同一套共享 builder。
4. 运行 `cargo fmt --all`、相关测试和 `cargo clippy --all-targets -- -D warnings`；若发现既有问题，先修复再继续。
5. 更新 `TODO.md`、`PLAN.md` 和本文件，然后提交并停止。

## 当前结果

- `T5000c2` 已完成，不再需要继续拆分。
- 已新增 `crates/scoopc/src/effect_analysis.rs`，引入 backend-agnostic 的 `EffectAnalysisCtx` 与 `KnownLocalMetadata`。
- `crates/scoopc/src/llvm/codegen/effect/state_machine_plan.rs` 现将 `HandlePlanContext` 收口为 `EffectAnalysisCtx` 别名，`SuspendCallAnalysis` 改为直接消费共享分析上下文。
- `HandlePlanContext::from_codegen(...)` 已移除；LLVM backend 现通过 `MainCodegen::effect_analysis_ctx()` 生成共享分析输入。
- `state_machine_segments.rs` 与 `state_machine_transform.rs` 测试 helper 已统一改为复用 `collect_effect_analysis_context_for_fun(...)`。
- 验证已完成并通过：
  - `cargo fmt --all`
  - `cargo test -p scoopc llvm::`
  - `cargo test -p scoopc --no-default-features`
  - `cargo test --all`
  - `cargo clippy --all-targets -- -D warnings`
- 验证过程中发现并修复了两个告警回退：测试模块中残留的 `HashMap` unused import。
- `TODO.md` 与 `PLAN.md` 已更新；下一条待执行任务应为 `T5000c2R Review：确认 EffectAnalysisCtx 已脱离 LLVM backend 现场取数`。
