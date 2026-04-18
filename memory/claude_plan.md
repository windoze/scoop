# 执行记录

## 说明

按要求先写入本文件，再开展仓库检查与实现工作。

出于安全与协作边界，我不会写入不可审计的私有推理细节；这里记录的是可公开的决策摘要、执行计划、进度与变更原因。

## 当前目标

本轮只完成 `TODO.md` 中第一个未完成任务，完成后测试、更新计划与任务状态、提交 Git，然后停止。

## 初始执行计划

1. 检查最新一次 Git 提交，确认提交说明中是否提到任何既有问题；若有，先修复这些问题。
2. 阅读 `TODO.md`，定位第一个未完成任务。
3. 读取 `PLAN.md`，核对现有计划与任务依赖。
4. 判断该任务是否过大或是否存在前置缺口。
5. 若任务过大或被前置缺口阻塞：
   - 在 `PLAN.md` 中细化子任务或记录阻塞原因。
   - 在 `TODO.md` 中调整排序、补充前置任务，并保证当前轮只处理新的首个可执行任务。
6. 若任务可直接执行：
   - 阅读相关代码与测试。
   - 实现任务所需改动。
   - 运行相关测试，并补充必要测试。
   - 运行格式化、lint 与必要的全量或针对性检查，确保无警告。
7. 更新 `TODO.md`、`PLAN.md` 与本文件，记录完成情况或阻塞原因。
8. 使用清晰的提交信息创建一次 Git 提交，然后停止。

## 进度

- 已创建计划文件，准备开始仓库检查。
- 已查看最新提交 `e746ad4068652eeff3836ccc0850ed0a25801e77`，其内容主要是把 expectation cleanup 暴露出的既有 blocker 重排进 `TODO.md` / `PLAN.md`，未包含代码修复。
- 已定位当前首个未完成任务为 `T3016f`：修正 top-level 多个 direct/indirect suspend site 与 multi-escape 组合下，resumed-body 错误重放已完成 prefix 的回归。
- 已完成问题复现：4 个目标 fixture 都会在第二次 `resume(...)` 时额外输出上一条 top-level statement 的已完成前缀（如 `after_first` / `after_a1`），与 `TODO.md` 记录一致。
- 已确认根因：`attach_escape_resume_targets()` 先前直接复制“上一个 `ResumeAfterSite` 之后的整段 owner-state 后缀”，导致 later top-level statement 的 replay state 混入更早已完成的语句。
- 已完成修复：replay actions 现按当前 suspend site 的 `source_path.top_level_stmt_idx` 裁剪到“当前 top-level statement 边界”，只保留仍应在下一次 replay 前执行的同 statement 前缀。
- 已补结构测试：
  - `source_plan_preserves_same_statement_escape_replay_prefix_for_nested_block_call_site`
  - `source_plan_trims_escape_replay_to_current_top_level_statement`
- 已完成验证：
  - 4 条 `T3016f` 目标 fixture 输出与 `.stdout` 一致。
  - 1 条 `T3016b` block regression 输出与 `.stdout` 一致。
  - `cargo test --all` 通过。
  - `cargo clippy --all-targets -- -D warnings` 通过。
- 待完成的收尾步骤：更新 `TODO.md` / `PLAN.md` 状态并创建本轮 Git 提交。

## 针对 T3016f 的执行计划

1. 阅读 `TODO.md` / `PLAN.md` 中 `T3016f` 与相邻已完成任务（尤其 `T3016b`、`T3015a`、`T3009b2c`）的描述，明确该任务与已有 replay/resume-path 修复面的边界。
2. 运行 `T3016f` 描述中的 4 个目标 fixture，记录实际输出与期望输出的差异，确认回归是否稳定复现。
3. 检查 unified state-machine 的 plan/segments/transform/emitter 中与 resumed-body replay、resume-path、top-level multi-site 调度相关的生产代码与测试。
4. 实施修复，并优先补充最小但足以锁定问题的单元/回归测试，避免仅靠 fixture 观察。
5. 运行定向测试、相关 Rust 测试、`cargo test --all`、`cargo clippy --all-targets -- -D warnings`。
6. 更新 `TODO.md`、`PLAN.md` 与本文件，标记 `T3016f` 完成，并准备提交一次 Git commit 后停止。
