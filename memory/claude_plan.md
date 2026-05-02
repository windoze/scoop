# 当前执行计划

说明：这里记录对外可见的执行计划、决策依据摘要、关键进展与变更；不记录内部私有推理细节。

## 初始计划

1. 读取 `TODO.md`，确认它只是索引，并找出引用的详细任务文件。
2. 按任务顺序检查相关 `TODO-Px.md`，定位第一个标题未带 `[DONE]` 的详细任务；若 `TODO.md` 与详细文件不一致，以详细文件为准。
3. 检查最近一次提交是否直接提到与该任务相关且未完成的问题；若这是当前任务的直接组成部分或前置依赖，则纳入当前执行范围。
4. 阅读该任务涉及的代码、测试、规范和相关文档，确认实现边界、约束、依赖与验收要求。
5. 实现当前任务；若遇到阻塞当前任务的真实缺陷或缺失能力，不绕过，改为补齐该阻塞项或在对应 `TODO-Px.md` 中新增最小前置任务并同步 `TODO.md`。
6. 运行与当前任务直接相关的验证，包括必要的测试、格式化、lint；若仓库要求且改动影响范围较大，再运行更广泛验证。
7. 更新 `memory/claude_plan.md` 记录关键进展与计划变更。
8. 在对应 `TODO-Px.md` 中将完成的任务标题标记为 `[DONE]`，补充完成记录；若任务索引、标题、顺序或状态变化，同步更新 `TODO.md`。
9. 仅在阶段级计划、依赖或完成标准发生变化时更新 `PLAN.md`。
10. 按仓库提交风格创建一次 git 提交，然后停止，不继续下一个任务。

## 当前任务定位

- 已读取 `TODO.md` 与 `TODO-P5.md`。
- 首个未完成的详细任务是 `P5-T07`：新增 `dump-effect-lowered` / snapshot 基线，并冻结 P5 -> P6 handoff contract。
- `TODO.md` 与 `TODO-P5.md` 在这一点上一致；目前无需先同步索引。
- 最近一次提交为 `[P5-T06R] Preserve dedicated drop paths in late opt review`，内容直接修复了 late opt 对 dedicated `drop_state` 的回归，属于 `P5-T06R` 的收尾，不构成新的、需要先插入到 `P5-T07` 前面的未跟踪前置任务。

## 当前任务执行分解

1. 检查 `scoop` CLI、命令分发、fixture runner、以及 `effect_refactor_pipeline`/`effect_lowered` 中现有 late-lowered dump 能力，确定最小改动方案。
2. 新增 `dump-effect-lowered` 命令模块，并为 legacy pipeline 提供稳定 unsupported 诊断；refactor 路径统一走 `load_effect_lowered_stage_output_for_dump(...)`。
3. 扩展 fixture phase：新增 `effect_lowered` phase、`.effectlowered` golden 比对、对应错误诊断与 phase-name 识别测试。
4. 建立 `tests/fixtures/effect_lowered/` 基线：复用既有 `.scoop` 源文件内容，补齐任务要求的最少 10 个 late-lowered 专属样本与 golden。
5. 把 P5 -> P6 handoff contract 进一步固定到代码注释/稳定 dump surface 中，确保从代码层面明确“P6 只翻译 late-lowered representation，不重做高层 lowering 设计”。
6. 运行定向验证：CLI 命令测试、fixture 测试、任务要求的若干 `cargo run -p scoop --no-default-features ...` 命令，以及 `cargo clippy -p scoop --no-default-features --all-targets -- -D warnings`；必要时运行 `cargo fmt --all`。
7. 更新 `TODO-P5.md` 与 `TODO.md` 的完成状态及完成记录；仅在阶段计划变化时才改 `PLAN.md`。
8. 检查工作区状态并创建一次 git 提交，然后停止。

## 当前进展

- 已完成 `dump-effect-lowered` CLI plumbing：新增 `crates/scoop/src/commands/dump_effect_lowered.rs`，并接入 `cli.rs` / `commands/mod.rs`。
- 已完成 late-lowered dump surface 加固：`RefactorEffectLoweredStageOutput::stable_dump()` 现在显式展示 `opt_level`、`snapshot_binding` 与 `post_opt_program`，并补写了 P5 -> P6 handoff contract 注释。
- 已完成 fixture phase：`crates/scoop/src/fixtures/mod.rs` 新增 `effect_lowered` phase、`.effectlowered` golden 比对、对应诊断与 phase-name 测试。
- 已完成 snapshot 基线：`tests/fixtures/effect_lowered/` 现已包含任务要求的 10 个 `.scoop` 样本与对应 `.effectlowered` golden。
- 已完成定向验证：
  - `cargo test -p scoop --no-default-features effect_lowered`
  - `cargo test -p scoopc --no-default-features effect_lowered_stage`
  - 任务要求的 refactor `dump-effect-lowered` / `scoop test --fixtures ...` 命令
  - legacy unsupported 诊断验证
  - `cargo fmt --all --check`
  - `cargo clippy -p scoop -p scoopc --no-default-features --all-targets -- -D warnings`
- 计划未变更：`PLAN.md` 无需修改。
- 下一步：检查最终 diff，创建一次 `[P5-T07] ...` 提交，然后停止。

## 计划更新规则

- 每当定位到当前任务、发现阻塞、调整实现路径、完成关键实现、开始验证、完成文档同步、完成提交时，更新本文件。
- 如果任务无法按原样完成，本文件要明确记录阻塞点、新增前置任务位置、以及本次为什么停止。
