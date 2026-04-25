# 本轮执行计划

## 约束

- 本轮只处理 `TODO.md` 中第一个未完成任务，完成后立即停止。
- 在推进计划任务前，先检查最新提交是否提到需要先修复的既有问题；若有，优先修复。
- 任何在探查、测试、实现过程中发现的既有缺陷、规格不匹配、实现边界缺口，都必须立即纳入当前范围：要么直接修复，要么作为前置任务插入 `TODO.md` 并更新 `PLAN.md`，然后停止。
- 不通过变更目标形状、缩窄测试、增加特判或其他规避方式绕过问题。
- 代码修改后需要补充验证，更新 `TODO.md` / `PLAN.md`，并提交 git commit。

## 执行步骤

1. 查看最新一次 git 提交信息，确认是否已经记录需要优先处理的既有问题。
2. 阅读 `TODO.md`，定位第一个未完成任务。
3. 阅读 `PLAN.md`，核对该任务的上下文、依赖与当前整体计划。
4. 判断该任务是否过大：
   - 若可在本轮完整落地，则直接实施。
   - 若过大，则把它拆成更小的前置子任务，更新 `PLAN.md` 与 `TODO.md`，并执行拆分后的第一个子任务。
5. 为实施任务收集最小必要上下文：
   - 定位相关源码、测试、规范或夹具。
   - 若发现既有缺陷或前置能力缺失，先处理该问题或把它整理成新的前置任务。
6. 实现当前目标任务，保持改动与规格一致。
7. 运行相关验证：
   - 至少执行与改动直接相关的测试。
   - 若改动影响面较广，补充执行更高层级验证。
   - 若环境允许，执行 `cargo fmt` 与 `cargo clippy --all-targets -- -D warnings`，确保无新增格式或 lint 问题。
8. 更新文档状态：
   - 在 `TODO.md` 中标记该任务完成，或在阻塞时调整任务顺序与前置依赖。
   - 在 `PLAN.md` 中记录本轮完成情况、发现的问题与后续顺序调整。
   - 按进展同步更新本文件。
9. 检查工作区改动，确认只包含本轮需要的变更。
10. 提交一次 git commit，提交信息与任务对应，然后停止。

## 进度记录

- 已完成：创建本轮执行计划文件。
- 已完成：检查最新提交 `c3c2bb4a2383f53889b036a107c04eb136186564`，提交主题为 `[T5000c2R] Review EffectAnalysisCtx backend boundary`，未发现提交说明里要求先修复的既有问题。
- 已完成：读取 `TODO.md` 与 `PLAN.md`，确认首个未完成任务为 `T5000c3 迁移 effect/state-machine planning 与 direct-step summary 到 shared facts / analysis 层`，其前置 `T5000c2R` 已完成。
- 已完成：检查 `state_machine_plan.rs`、`effect_step_summary.rs`、`effect_analysis.rs` 与 `program_facts.rs` 的边界；确认 `T5000c3` 仍然过大，已按稳定边界拆分为：
  - `T5000c3a`：先抽出共享 `effect_state_machine_analysis.rs` 源文件，并清理 `effect_step_summary.rs` 对 backend 文件的 `include!`；
  - `T5000c3b`：再收口 concrete-type / field-type / receiver exactness helper 的消费方向。
- 已完成：更新 `TODO.md` 与 `PLAN.md`，当前本轮实际执行目标改为 `T5000c3a`。
- 已完成：实施 `T5000c3a`。
  - 原 `crates/scoopc/src/llvm/codegen/effect/state_machine_plan.rs` 的 pure analysis 主体已迁到新的共享源文件 `crates/scoopc/src/effect_state_machine_analysis.rs`；
  - backend 侧 `state_machine_plan.rs` 已收口为薄包装；
  - `crates/scoopc/src/effect_step_summary.rs` 已改为直接复用共享源文件，不再 `include!` backend 源文件。
- 已完成：验证 `T5000c3a`。
  - `cargo fmt --all --check`
  - `cargo test -p scoopc llvm::`
  - `cargo test -p scoopc --no-default-features`
  - `cargo test --all`
  - `cargo clippy --all-targets -- -D warnings`
  - 以上均通过。
- 已完成：更新 `TODO.md` / `PLAN.md`，将 `T5000c3a` 标记完成并把下一条待执行任务切换为 `T5000c3aR`。
- 下一步：检查工作区改动、整理提交信息并创建本轮 commit，然后停止。
