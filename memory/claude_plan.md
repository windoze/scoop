# 执行计划

## 约束说明

- 本轮只处理 `TODO.md` 中第一个未完成任务，完成后提交并停止。
- 在执行任务前先检查最新提交，若最新提交提到既有问题，优先修复这些问题。
- 任何执行中发现的既有 bug、回归、规格不匹配、未完成边界或临时绕行都视为当前范围内问题；必须修复，或在 `TODO.md` 中加入前置任务后提交并停止。
- 不采用 fixture-only hack、弱化测试、改变语义表示等绕行方式。
- 输出、计划更新和最终说明使用中文。

## 初始步骤

1. 查看最新 Git 提交，判断是否提到需要优先处理的既有问题。
2. 阅读 `TODO.md`，找出第一个未完成任务。
3. 阅读 `PLAN.md` 和相关源码/测试，确认任务背景、预期语义和已有实现边界。
4. 若任务过大，先把它拆成更小的可执行子任务，更新 `TODO.md`/`PLAN.md`，提交后停止。

## 执行步骤

1. 实现第一个未完成任务或当前子任务。
2. 添加或更新最小但充分的测试，覆盖正常路径和关键边界。
3. 运行相关测试；若涉及共享行为，再运行更广的测试命令。
4. 若出现规格不匹配或缺失能力，优先修复；若无法在本任务内正确完成，则把缺失能力作为前置任务写入 `TODO.md`，更新 `PLAN.md`，提交后停止。
5. 任务完成后，在 `TODO.md` 中标记完成，并同步更新 `PLAN.md`。
6. 运行格式化和必要验证，尽量保证无警告。
7. 使用清晰的任务标签提交变更。
8. 停止，不继续下一个任务。

## 当前进度

- 已检查最新提交：`[T5000i2] Add MIR non-escaping closure simplification`，提交正文未提到额外既有问题。
- 已阅读 `TODO.md` / `PLAN.md` 的相关段落，确认本轮第一个显式 `[TODO]` 条目是 `T5000i3 让 continuation escaping analysis 进入 effect/state-machine planning 输入面`。
- 当前任务目标：
  - 将 `T5000i1` 产生的 continuation escape facts 接入 `ProgramFacts` / `EffectAnalysisCtx` / effect-state-machine planning 输入面；
  - planning 层能够消费 local-resume-only / escaping / unknown 级别的 continuation 事实；
  - 本轮不改变 backend emitter ABI；
  - facts 缺失时保持保守 `unknown`，不改变现有运行语义。
- 下一步将阅读 `mir/escape.rs`、`mir/pass_view.rs`、`effect_analysis.rs`、`effect_state_machine_analysis.rs` 以及 LLVM effect planning 包装层，确定当前 continuation 是否仍由 backend/planning 现场推断。
- 已完成源码核对：
  - `mir/escape.rs` 已发布 continuation escape facts，但事实当前只按 callable/local 暴露，未携带 resume call span；
  - `EffectAnalysisCtx` 当前只承载 known fun/local metadata、source path 与 `ProgramFacts`，没有 MIR continuation escape side table；
  - `HandlePlanBuilder::classify_builtin_suspend_call(...)` 仍只消费 `ProgramFacts` 中的 `Continuation.resume` call-site 集合与 non-pure 集合；
  - planning 创建 suspend site 时没有记录 local-resume-only / escaping / unknown 状态。
- 当前实施方案：
  1. 给 `ContinuationEscapeFact` 增加 resume call span 列表，使 MIR facts 能稳定投影到 HIR `CallSite`。
  2. 在 `effect_analysis.rs` 中新增 backend-agnostic `ContinuationEscapeState` / `ContinuationEscapeFacts`，并提供从 `MaterializedMirPassView` + callable FQN + source path 构造 call-site facts 的 helper。
  3. 将 `ContinuationEscapeFacts` 挂入 `EffectAnalysisCtx`，默认空 facts 查询为 `Unknown`，nested analysis context 继续继承该 side table。
 4. 让 `HandlePlanBuilder::new_suspend_site(...)` 在遇到 `Continuation.resume` hidden suspend site 时记录 continuation escape 状态；该状态只进入 planning/segments side table，不改变 backend emitter ABI。
 5. 增加回归测试：无 pass facts 时为 `Unknown`，接入 O2 MIR pass view 后，同一 `Continuation.resume` site 能被 planning 看到 `LocalResumeOnly`。
- 新增回归首次运行暴露一个前置 MIR escape 精度缺口：
  - `TerminatorKind::Handle` 只是结构化 handler boundary，已经通过 CFG successor 暴露 body/arms/finally；
  - `Todo("handle body/arm/finally exit pending")` 是 handle 子块出口占位，不携带值使用；
  - `Rvalue::Todo("handle result pending")` 只是 handle 表达式结果占位，不读取 continuation 值；
  - 当前 escape analysis 把这些都算作 unknown MIR，导致 handle 内已 lowering 出来的本地 `Continuation.resume` 被错误降级为 `Unknown`。
- 处理决定：先修正 escape analysis 对这些 handle 结构占位的分类；其它未建模 `Todo` 继续保持保守 `Unknown`。
- 已完成当前实现：
  - `ContinuationEscapeFact` 现在记录 `resume_call_spans`；
  - 新增 `ContinuationEscapeState::{LocalResumeOnly, Escaping, Unknown}` 与 call-site keyed `ContinuationEscapeFacts`；
  - `EffectAnalysisCtx` 默认携带空 continuation escape facts，查询缺失 facts 时返回 `Unknown`；
  - production/shared analysis context 会从 `MaterializedMirPassView::escape_facts()` 投影当前 callable 的 continuation facts；
  - `SuspendSitePlan` / segment suspend site 现在记录 `Continuation.resume` hidden suspend site 的 continuation escape 状态，但不改变 unified emitter ABI；
  - escape analysis 已修正 handle 结构占位导致的错误 unknown 降级。
- 已通过局部验证：
  - `cargo test -p scoopc continuation_escape -- --nocapture`
  - `cargo test -p scoopc escaping_continuation_facts_enter_handle_planning_input -- --nocapture`
  - `cargo test -p scoopc mir::escape -- --nocapture`
- 下一步运行 effect state-machine skeleton 相关测试、`--no-default-features`、全量测试与 clippy。

## 2026-04-28 接手复核计划

- 当前仓库已有 `T5000i3` 相关未提交改动，先保护这些改动，不回退任何已有工作。
- 先复核当前 diff 与相关源码，确认 continuation escape facts 是否已经正确进入 `EffectAnalysisCtx`、state-machine plan 与 segment 输入面。
- 运行前一轮计划中尚未完成的验证命令，优先处理编译错误、测试失败或 clippy warning。
- 若验证暴露真正的既有实现缺口，按 `PROMPT.md` 要求优先修复；如果不能在本任务内正确修复，则写入 `TODO.md` 前置任务并停止。
- 完成后更新 `TODO.md` 与 `PLAN.md` 的 `T5000i3` 完成记录，运行格式化与必要验证，最后以 `[T5000i3] ...` 提交并停止。

## 2026-04-28 接手验证结果

- 已复核当前 diff：continuation escape facts 已从 MIR escape side table 投影到 `EffectAnalysisCtx`，并进入 `SuspendSitePlan` / segment suspend-site side table；本轮未改变 emitter ABI。
- 已确认 `current_callable_fqn` 在 HIR body 与 MIR body emission 入口设置，production effect analysis context 可按当前 callable 从 `MaterializedMirPassView::escape_facts()` 取 facts；缺失路径保持 `Unknown`。
- 已完成验证：
  - `cargo fmt --all --check`
  - `cargo test -p scoopc continuation_escape -- --nocapture`
  - `cargo test -p scoopc escaping_continuation_facts_enter_handle_planning_input -- --nocapture`
  - `cargo test -p scoopc mir::escape -- --nocapture`
  - `cargo test -p scoopc llvm::codegen::effect -- --nocapture`
  - `cargo test -p scoopc --no-default-features`
  - `cargo test --all`
  - `cargo clippy --all-targets -- -D warnings`
  - `cargo run -p scoop -- test`（`fixtures: ok (1202)`）
- 已更新 `TODO.md` / `PLAN.md`，将 `T5000i3` 标记为完成并记录实现、前置缺口修复和验证命令。
- 下一步只做最终 diff/status 检查，然后提交 `[T5000i3] Connect continuation escape facts to effect planning` 并停止。
