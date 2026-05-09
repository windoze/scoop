# Claude Plan

说明：按你的要求，我会在这里维护可执行计划和关键进展记录。出于安全与隐私限制，这里不写内部逐字推理，而是写清晰的任务分解、判断依据摘要、执行步骤、阻塞点和验证结果。

## 当前轮目标

完成 `TODO.md` 中第一个未标记为 `[DONE]` 的任务；如果存在直接阻塞该任务的具体前置问题，则先把该前置问题以最小必要粒度写入 `TODO.md`、提交并停止。

## 初始执行计划

1. 读取 `TODO.md`，定位第一个未完成任务，并确认其要求、依赖、验收方式与完成记录格式。
2. 检查最近提交信息，确认是否存在与该任务直接相关且明确未完成的事项；若有，将其视为当前任务的一部分或写成前置依赖。
3. 阅读与当前任务直接相关的代码、测试、夹具和文档，不做开放式问题扫荡。
4. 判断是否可以直接完整实现当前任务：
   - 若可以，实施最小正确修改。
   - 若不可以，精确定义阻塞点，按要求在 `TODO.md` 中插入最小必要前置任务，并保持当前任务未完成。
5. 运行当前任务要求的验证：至少包括相关测试；若任务涉及通用质量门禁，则补跑格式化、测试和 `cargo clippy --all-targets -- -D warnings`（在范围和时间允许时）。
6. 更新文档与任务记录：
   - 完成任务时，在 `TODO.md` 对应标题前加 `[DONE]`，并补充完成记录。
   - 仅在阶段级计划变化时更新 `PLAN.md`。
   - 本文件同步记录关键进展、计划调整和验证结果。
7. 按仓库约定创建一次 git 提交，然后停止，不继续下一个任务。

## 进展记录

- 已创建本计划文件。
- 已读取 `TODO.md`，定位首个未完成任务为 `P7-T04`：`运行 GC env 全开验证，并冻结 P7 -> P8 handoff：legacy 仅剩显式 compare/rollback 入口`。
- 已检查最近提交：`[P7-T03R] Review default full regression handoff`，未见比当前任务更早且直接相关的未完成事项；当前继续执行 `P7-T04`。
- 已读取 `TODO-P7.md` 中 `P7-T04` / `P7-T04R` 详细要求。当前判断：优先执行默认 refactor 的 GC env 全开矩阵；若矩阵通过，再补显式 legacy smoke、更新 handoff 文档与任务记录；若矩阵失败，则按失败根因决定修复或插入最小前置任务。
- 首轮 GC env `run-pass` 失败于 `continuation_escape_binder_resume_effect_row_runtime_basic.scoop`；已收敛为 composed continuation 在 wrapper allocation safepoint 之后错误复用旧 SSA callee pointer。修复：在 `create_continuation_object_with_state_tag` 中把 extracted callee continuation 先 root 到专用 slot，分配 wrapper 后再从该 slot reload 再写入 composed edge。
- 复验中发现此前工作树里未提交的 O0 LLVM pipeline 改动会让 `effect_escape_continuation_arm_nested_handle_replay_tail_basic.scoop` 回归（默认路径输出错误），说明它不属于本任务所需修复；已回退该改动，保留 body-side root reload 修复。
- 当前状态：定向通过 `continuation_escape_binder_resume_effect_row_runtime_basic.scoop`（GC env）与 `effect_escape_continuation_arm_nested_handle_replay_tail_basic.scoop`（default）。下一步重新执行完整 GC env `run-pass` / `runtime_gc` 矩阵，并在通过后补 legacy smoke、文档和 TODO 记录。
- 继续执行 `run-pass` GC env 时，`effect_escape_continuation_indirect_perform_binder_string_use.scoop` 暴露 handler arm/outward payload SSA 穿过 continuation materialization safepoint 后失效。已修复：在 `handle_boundary_case` 中对会跨 safepoint 的 arm/outward payload 先 deferred，再在实际绑定/发射 `Step` 前 reload。该样本现已在 GC env 下通过。
- 目前剩余 blocker：`effect_multi_escape_custom_nonresuming_direct_indirect_block_multi.scoop` 在第一次 resume 进入 mixed replay 后触发 runtime `verify-roots`，报告 explicit-frame slots 中残留 invalid roots；对应 LLVM IR 仍可观察到大量 `ptr poison` spill/writeback 形状。这是比当前 `P7-T04` 更底层的 explicit-frame stale-root contract 问题。
- 已决定不继续在本轮直接完成 `P7-T04`，而是按要求把该问题前置为新任务 `P7-T03S`，更新 `TODO-P7.md` / `TODO.md` 后提交当前增量并停止。
- 已完成收尾验证：`cargo fmt --all`；`cargo test -p scoopc --lib cross_call_escape_resume_roots_do_not_degrade_to_poison_in_explicit_frame` 通过。当前待提交内容为：两项 safepoint root/payload 修复、Immix write-barrier poll 顺序修正、以及 `P7-T03S` blocker 任务插入。
