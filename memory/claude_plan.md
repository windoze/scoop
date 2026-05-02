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

- 已重新读取 `TODO.md` 与 `TODO-P5.md`。
- 首个未完成的详细任务是 `P5-T07R`：Review P5 阶段退出条件，确认 P6 只需把 late-lowered representation 翻译到 LLVM。
- `TODO.md` 与 `TODO-P5.md` 在这一点上一致：`P5-T07` 已标记 `[DONE]`，`P5-T07R` 尚未完成。
- 最近一次提交为 `[P5-T07] Freeze late-lowered dump surface for P6`，它正是当前 review 任务要复核的对象；目前没有新的未提交改动，优先对该提交及其依赖阶段进行 review 与定向验证。

## 当前任务执行分解

1. 复核 `TODO-P5.md` 中 `P5-T01` 到 `P5-T07` 的完成记录与代码入口，确认本 review 需要覆盖的 contract 面：late-lowering stage、IR shape、segmentation/frame lifting、boundary lowering、late opt、CLI dump、fixture/golden、P5 -> P6 handoff 注释。
2. 阅读核心实现与入口文件：`crates/scoopc/src/effect_lowered/**`、`crates/scoopc/src/effect_refactor_pipeline/**`、`crates/scoop/src/commands/dump_effect_lowered.rs`、`crates/scoop/src/fixtures/mod.rs`，确认 CLI/测试/P6 共用同一 P5 stage 输出，而不是旁路或回落到 legacy/高层分析。
3. 重新运行 `P5-T01` ~ `P5-T07` 要求的定向测试与命令；若发现阻塞 P6 的真实缺陷，则先修复，或在 `TODO-P5.md` / `TODO.md` 中插入最小前置任务并停止。
4. 若 review 未发现新的阻塞项，则在 `TODO-P5.md` 中将 `P5-T07R` 标记为 `[DONE]` 并补齐完成记录；`TODO.md` 同步该 `[DONE]` 状态；仅在阶段级计划变化时才更新 `PLAN.md`。
5. 提交本次 review 结论与文档更新，然后停止，不推进 `P6-T01`。

## 当前进展

- 已确认当前分支在本次开始时无未提交改动；本次是对上一提交 `[P5-T07] Freeze late-lowered dump surface for P6` 的 review 收尾。
- 已完成核心入口复核：
  - `crates/scoopc/src/effect_refactor_pipeline/mod.rs` 中 `build_effect_lowered_stage_output(...)` / `load_effect_lowered_stage_output_for_dump(...)` 已固定为 P5 stage 的共同入口，并在注释中写死 P5 -> P6 canonical handoff contract。
  - `crates/scoopc/src/effect_refactor_pipeline/effect_lowering_stage.rs` 的 `RefactorEffectLoweredStageOutput` 文档注释与 `stable_dump()` 已明确：P6 只能翻译 late-lowered output 到 LLVM，不得重新做 boundary 识别、segmentation、frame lifting、continuation capture 设计或 `ImplPlan` 选择。
  - `crates/scoop/src/commands/dump_effect_lowered.rs` 与 `crates/scoop/src/fixtures/mod.rs` 统一经 `load_effect_lowered_stage_output_for_dump(...)` / `render_effect_lowered_output(...)` 走同一 P5 stage helper，没有发现 CLI、fixture 或测试旁路去拼高层文本。
- 已完成依赖/边界搜索：
  - `crates/scoopc/src/effect_lowered/**` 中未发现对 `crate::llvm`、`crate::effect::state_machine`、`state_machine_bridge` 或 `production_lowered_hir` 的生产依赖。
  - `crates/scoopc/src/effect_refactor_pipeline/effect_lowering_stage.rs` 自带测试 `refactor_effect_lowered_stage_has_no_legacy_state_machine_or_llvm_imports` 继续锁定了这一边界。
- 已完成 `P5-T01` ~ `P5-T07` 定向验证矩阵，全部通过：
  - `cargo test -q -p scoopc --no-default-features refactor_effect_lowered_stage`
  - `cargo test -q -p scoopc --no-default-features refactor_late_lowered_ir`
  - `cargo test -q -p scoopc --no-default-features refactor_body_version_key`
  - `cargo test -q -p scoopc --no-default-features refactor_late_boundary_selection`
  - `cargo test -q -p scoopc --no-default-features refactor_late_segmentation`
  - `cargo test -q -p scoopc --no-default-features refactor_owner_resume_state`
  - `cargo test -q -p scoopc --no-default-features refactor_frame_lifting`
  - `cargo test -q -p scoopc --no-default-features refactor_late_control_flow`
  - `cargo test -q -p scoopc --no-default-features refactor_dropped_continuation`
  - `cargo test -q -p scoopc --no-default-features refactor_runtime_error_boundary`
  - `cargo test -q -p scoopc --no-default-features refactor_step_materialization`
  - `cargo test -q -p scoopc --no-default-features refactor_boundary_lowering`
  - `cargo test -q -p scoopc --no-default-features refactor_continuation_object`
  - `cargo test -q -p scoopc --no-default-features refactor_resume_interface_completeness`
  - `cargo test -q -p scoopc --no-default-features refactor_late_opt`
  - `cargo test -q -p scoop --no-default-features effect_lowered`
  - `cargo run -q -p scoop --no-default-features -- --effect-pipeline refactor dump-effect-lowered tests/fixtures/effect_lowered/dispatch_and_resume_call.scoop`
  - `cargo run -q -p scoop --no-default-features -- --effect-pipeline refactor dump-effect-lowered tests/fixtures/effect_lowered/handle_finally_boundary.scoop`
  - `cargo run -q -p scoop --no-default-features -- --effect-pipeline refactor test --fixtures tests/fixtures/effect_lowered/dispatch_and_resume_call.scoop`
  - `cargo run -q -p scoop --no-default-features -- --effect-pipeline refactor test --fixtures tests/fixtures/effect_lowered/handle_perform.scoop`
  - `cargo run -q -p scoop --no-default-features -- --effect-pipeline refactor test --fixtures tests/fixtures/effect_lowered/single_case_impl_plan.scoop`
  - `cargo run -q -p scoop --no-default-features -- --effect-pipeline refactor test --fixtures tests/fixtures/effect_lowered/dropped_continuation_abandons_remaining_work.scoop`
  - `cargo run -q -p scoop --no-default-features -- --effect-pipeline legacy dump-effect-lowered tests/fixtures/effect_lowered/dispatch_and_resume_call.scoop`（按预期失败，并复核稳定 unsupported 诊断文案）
  - `cargo fmt --all --check`
  - `cargo clippy -q -p scoop -p scoopc --no-default-features --all-targets -- -D warnings`
- 当前结论：未发现新的阻塞项；可以把 `P5-T07R` 标记为完成，并同步 `TODO.md` 后提交。
- 已完成文档同步：`TODO-P5.md` 已将 `P5-T07R` 标记为 `[DONE]` 并补齐完成记录；`TODO.md` 已同步根索引状态；`PLAN.md` 仍无需修改。
- 下一步：检查最终 `git diff` / `git status`，创建一次 `[P5-T07R] ...` 提交，然后停止。

## 计划更新规则

- 每当定位到当前任务、发现阻塞、调整实现路径、完成关键实现、开始验证、完成文档同步、完成提交时，更新本文件。
- 如果任务无法按原样完成，本文件要明确记录阻塞点、新增前置任务位置、以及本次为什么停止。
