## 当前执行计划

说明：按安全要求，这里记录的是可审阅的执行计划与决策摘要，不包含逐字内部推理。

1. 先检查最新一次 Git 提交信息，确认是否明确提到需要先修复的既有问题；若有，则优先处理该问题。
2. 阅读 `TODO.md`，定位第一个未完成任务；同时阅读 `PLAN.md`，理解现有分解、依赖和约束。
3. 评估该任务是否足够小且可在本轮完整完成；若过大，则把它拆分为更小的前置子任务，并同步更新 `TODO.md` 与 `PLAN.md`。
4. 针对当前应执行的首个任务，先阅读相关代码与测试，确认实现边界、现状以及是否存在阻塞性的既有缺陷或规格不匹配。
5. 如果在探查、实现或测试过程中发现任何既有问题、规格不匹配、回归、未完成边界或依赖缺失：
   - 若能在本轮修复，则先修复并验证；
   - 若不能直接完成，则把该问题作为前置任务插入 `TODO.md` 中当前任务之前，更新 `PLAN.md` 说明阻塞原因，然后提交并停止。
6. 对当前任务做最小且正确的实现修改，避免引入变通方案、夹层兼容代码或仅针对夹具/测试的特殊处理。
7. 运行与修改相关的测试、必要的构建检查，以及用户要求的质量检查；若失败则继续修复直到通过，或按阻塞流程处理。
8. 完成后更新文档状态：在 `TODO.md` 中标记该任务完成，在 `PLAN.md` 中记录结果、测试和后续影响。
9. 检查工作区状态，仅提交与本轮任务相关的改动；按仓库习惯撰写提交信息并创建一次非 amend 的提交。
10. 提交完成后停止，不继续处理下一个任务。

## 计划更新规则

- 一旦发现新的阻塞、任务拆分、实现方案显著变化、关键测试结论或任务完成状态，会立即更新本文件。
- 若最新提交里提到需要先修复的问题，本文件会补充该问题的结论与处理状态。

## 当前状态更新（第一次上下文收集后）

- 已检查最新提交：`37a9cba [T5002b2b2a2] Stage segmented arm replay groundwork`。提交信息本身没有额外的独立“先修 issue”正文；当前需要优先处理的既有问题，与 `TODO.md` 中首个未完成任务 `T5002b2b2a2` 一致。
- 已读取 `TODO.md` 与 `PLAN.md`。当前执行目标是：补齐 non-tail escape arm segmented-body 的 resume-fragment 合同，确保 arm body 内 nested handle / `try { k.resume(...) } catch ...` replay 后还能继续执行 arm tail，而不是过早把 inner 表达式值当成整个 arm 结果。
- 已发现工作区存在与该任务直接相关的未提交改动（主要在 `crates/scoopc/src/llvm/codegen/effect/*`、`crates/scoopc/src/llvm/codegen/mir_body.rs`、`crates/scoopc/src/llvm/tests.rs`、若干 runtime tests 与新 fixture）。后续会先审阅这些改动与相关代码，再决定补全实现、测试或必要的任务前插。
- 下一步：阅读相关 diff 与实现点，确认当前 groundwork 已经覆盖到哪里、还缺哪条 replay/source-path 合同，再做最小正确修改。

## 当前状态更新（定位根因后）

- 已确认一个具体缺口：`attach_suspend_resume_paths()` 会遍历 handle arms，但 `attach_suspend_source_paths()` 目前只遍历 `handle.body`，不会为 segmented arm body 内部的 suspend site（尤其是 nested handle / `try { k.resume(...) } catch ...`）记录 source path。
- 进一步确认：`attach_suspend_source_paths_in_expr()` 当前只在 `Call` / `Perform` 分支里显式 `record_suspend_source_path(...)`，所以 `SuspendSiteKind::NestedHandleBoundary` 对应的 `hir::ExprKind::Handle` 站点其实在任何位置都不会拿到 source path。这与当前症状一致：nested-handle boundary 的 wrapper continuation 只能退回粗糙 replay，无法稳定接回 arm tail。
- 因此，`attach_escape_resume_targets()` 里的 `escape_replay_actions_for_site()` 依赖 `source_path` 来把 replay prefix 裁剪到“当前语句/表达式边界”；缺少 arm-body / nested-handle source path 时，只能退回粗糙的 `state.actions[1..]`，从而在当前路径上把 inner 结果过早当成整个 arm 结果。
- 当前计划已收敛为：
  1. 为 suspend source path 引入显式 root（至少覆盖 handle body stmt、arm body、finally stmt），而不是只记录 `handle.body` 的 top-level stmt。
  2. 让 source-path 收集在遍历任意表达式时统一尝试记录站点，使 `NestedHandleBoundary` 也能拿到 source path；同时覆盖 `handle.body`、handle arms 与 finally。
  3. 更新 `escape_replay_actions_for_site()` 与 ordinary-callee 相关 helper，使其按新的 source root 裁剪 replay 边界，同时保持现有 top-level body 语义不变。
  4. 补一条 focused analysis/source-plan 回归，锁定 nested-handle boundary 的 escape replay state 里仍包含 `inner_arm_after_resume` 与 arm tail。
  5. 补一条 run-pass 最小探针，验证输出确实经过 `inner_arm_after_resume` 与最终 arm tail。

## 当前状态更新（实现与验证完成）

- 已完成实现：
  - `SuspendSourcePath` 已从“只会指向 `handle.body` 顶层 stmt”扩成显式 source root（handle body stmt / arm body / finally stmt）；
  - `attach_suspend_source_paths_in_expr()` 现在会在任意表达式入口统一尝试登记站点，因此 `SuspendSiteKind::NestedHandleBoundary` 也能拿到 source path；
  - segment / unified transform 的 contract 校验已同步放行 nested-handle-boundary 的 source-path metadata；
  - `state_machine_emitter.rs` 现已在 nested-handle boundary outward suspend 时，把 raw inner replay token 保留在 frame slot 里供 same-frame replay 使用，同时把 outward `EffectOutcome.signal.resume_token` 改写成 wrapper continuation，使更外层 resume 会先回到 arm tail。
- 已新增回归：
  - analysis：`non_tail_escape_arm_nested_handle_boundary_escape_replay_keeps_arm_tail`
  - run-pass：`tests/fixtures/run-pass/effect_escape_continuation_arm_nested_handle_replay_tail_basic.scoop`
- 已完成验证：
  - `cargo test -p scoopc non_tail_escape_arm_nested_handle_boundary_escape_replay_keeps_arm_tail -- --nocapture`
  - `cargo test -p scoopc non_tail_escape_arm_with_outward_suspend_builds_inner_resume_site -- --nocapture`
  - `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/effect_escape_continuation_arm_nested_handle_replay_tail_basic.scoop`
  - `SCOOP_GC_MOVE=1 SCOOP_GC_STRESS=1 SCOOP_GC_VERIFY_ROOTS=1 cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/effect_escape_continuation_arm_nested_handle_replay_tail_basic.scoop`
  - `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/effect_resume_nested_escape_handle_tail_multi_perform_nonunit.scoop`
  - `cargo clippy --all-targets -- -D warnings`
- 当前任务 `T5002b2b2a2` 已可标记完成；下一步按顺序进入 `T5002b2b2a2R`。
