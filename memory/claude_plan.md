# 当前执行计划

说明：这里记录简明执行计划与关键决策，不包含不可见的原始推理细节。

## 初始计划

1. 读取 `TODO.md`，按索引定位相关的详细任务文件（如 `TODO-P0.md`、`TODO-P1.md` 等）。
2. 在详细任务文件中按顺序查找第一个标题未标记 `[DONE]` 的任务，并将其视为本次唯一执行目标。
3. 检查最近一次提交信息，确认是否存在与该任务直接相关且明确未完成的问题；如果有，将其并入当前任务范围或按要求记录为前置依赖。
4. 阅读该任务的详细要求、约束、依赖、验证方式，以及相关代码和测试位置，确认实现边界。
5. 直接实现该任务；若遇到阻塞当前任务的真实缺陷或缺失能力，不做绕过，而是在对应 `TODO-Px.md` 中补充最小前置任务并同步 `TODO.md`。
6. 运行相关验证，包括任务要求的测试，以及必要的 `cargo fmt`、`cargo test`、`cargo clippy --all-targets -- -D warnings`（若适用且能在当前任务范围内完成）。
7. 更新文档记录：
   - 将对应详细任务标题改为 `[DONE]`。
   - 补充完成记录。
   - 若任务索引或标题变化，同步更新 `TODO.md`。
   - 仅在阶段计划实际变化时更新 `PLAN.md`。
8. 提交本次变更，提交信息使用当前任务 id 和简明描述。
9. 停止，不继续处理下一个任务。

## 进度记录

- 已创建本计划文件，下一步开始读取任务索引并确定当前任务。
- 已读取 `TODO.md` 与 `TODO-P6.md`，确认首个未完成详细任务为 `P6-T02j`：发布 `HandleDispatch` / completion-state lowering contract。
- 最近一次提交标题为 `[P6-T02j] Track handle-dispatch completion-state prerequisite`，说明当前任务已被识别为 `P6-T03` 的真实前置项，但尚未完成实现。
- 当前工作树中除本计划文件外没有其他未提交改动，可按单任务原子实现推进。
- 下一步：检查 `effect_lowered` 与 `llvm/codegen/effect_refactor` 中现有 `HandleDispatch`、handle boundary、completion/state 槽位与 dump/query 发布面，找出需要补充的 authoritative contract 字段与 fail-fast 校验点。
- 已确认当前缺口：`HandleDispatch` 只有 `body_state/arm_states/finally_state/exit_state/boundary_ids` 等状态边，缺少“body/arm/finally 完成后如何通过 completion/state carrier 继续到 arm/finally/exit/outward”的显式 contract。
- 计划中的实现形状：
  1. 在 `effect_lowered::ir` 为 `HandleDispatch` 新增结构化 contract，显式发布 handled case -> arm state、body/arm/finally completion target、body/arm/finally outward case 集、published outward emissions、pending completion token，以及 abandon 与 finally/cleanup 的分流边界。
  2. 在 `effect_lowered::materialize` 基于 `HandleSiteEffectFacts` + handle boundary lowering 物化该 contract，并在缺失 handled-arm 映射、缺失 outward emission、或 boundary/source 漂移时 fail fast。
  3. 在 `effect_lowered::dump` 公开渲染这层 contract，便于 `dump-effect-lowered` 审阅。
  4. 在 `llvm/codegen/effect_refactor::{types,layout}` 中把该 contract 接到 `RefactorAbiQuery`，补充 `StateTag/CompletionTag/ResumePayloadCarrier` 字段索引与 completion tag identity 发布面，并在 ABI materialization 阶段校验 contract/slot/tag 一致性。
  5. 增加定向测试：late-lowered contract 正例、dump 暴露、LLVM query 正例，以及缺失 handled-arm/completion-state contract 时的 fail-fast。
- 以上 5 步已完成。
- 已完成验证：
  - `cargo fmt --all`
  - `cargo test -p scoopc refactor_handle_dispatch_contract`
  - `cargo test -p scoopc refactor_completion_state_contract`
  - `cargo run -p scoop -- --effect-pipeline refactor dump-effect-lowered tests/fixtures/run-pass/effect_resume_if_else_branch_single_perform.scoop`
  - `cargo run -p scoop -- --effect-pipeline refactor dump-effect-lowered tests/fixtures/run-pass/effect_multi_escape_indirect_direct_while.scoop`
  - `cargo clippy -p scoopc -p scoop --all-targets -- -D warnings`
- 已更新 `TODO-P6.md` 与 `TODO.md`：`P6-T02j` 已标记为 `[DONE]`，并补齐完成记录与验证记录。
- 下一步：检查工作树、创建 `P6-T02j` 提交，然后停止。
