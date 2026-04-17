# 执行计划与进度记录

## 约束说明

- 本文件记录可审阅的执行计划、关键决策、发现的问题、进度更新与后续动作。
- 出于安全与协作边界，本文件不记录原始内部思维链路，只记录必要的计划、依据与结论。

## 初始执行计划

1. 检查最近一次提交，确认提交信息或提交内容中是否提到已知问题、待修复事项或明显遗留缺陷。
2. 阅读 `TODO.md`，识别第一个未完成任务。
3. 评估该任务是否足够小且可在本轮完整交付。
4. 如果任务过大：
   - 在 `PLAN.md` 中拆分为更小的可执行子任务。
   - 同步调整 `TODO.md` 中的顺序与依赖。
   - 选择拆分后的第一个子任务作为当前执行目标。
5. 针对当前目标实现代码修改。
6. 运行相关检查与测试，至少覆盖：
   - 与改动直接相关的测试。
   - 必要的格式化、编译、lint 检查。
7. 若在实现或测试中发现规范不匹配、缺失能力或现存缺陷：
   - 先判断是否属于当前任务的前置依赖。
   - 如属于阻塞项，则按要求更新 `TODO.md` / `PLAN.md`，说明依赖与顺序调整，然后提交并停止。
8. 若任务完成：
   - 更新 `TODO.md` 标记完成。
   - 更新 `PLAN.md` 记录当前状态与后续计划。
   - 更新本文件记录关键结果。
   - 提交一次清晰的 Git commit。
9. 完成后停止，不继续处理下一项任务。

## 进度日志

- 2026-04-18：已创建本文件并写入初始执行计划。下一步将检查最近一次提交与 `TODO.md`。
- 2026-04-18：已检查最近一次提交 `c7b12ffe4fd88c230e96f890d72dab8563cd06d3`，提交信息为 `[T3015aR] Review resumed-segment handler redispatch`。提交信息本身未声明新的未修复遗留问题。
- 2026-04-18：已读取 `TODO.md` 与 `PLAN.md`，当前第一个未完成任务为 `T3009b2`：收口 escaped continuation indirect callee 的 shared resumed-body / caller-tail 验收矩阵。

## 当前任务理解

- 目标不是只补单个 fixture，而是把 escaped continuation + indirect callee + resumed-body caller-tail 的共享语义矩阵真正收口。
- 需要重点覆盖：
  - ordinary callee suspend 后恢复自身 post-suspend body；
  - 然后把真实 call result 交回 outer caller-tail；
  - shared matrix 要覆盖 multi-site 与 nested statement-container source-path 变体；
  - 不能引入按 fixture 名称或代码形状分流的补丁。

## 下一步执行项

1. 检查当前工作区状态，避免覆盖用户已有修改。
2. 阅读 `T3009b2` 附近的 `TODO.md` / `PLAN.md` 细节与现有相关 fixture。
3. 运行与 `T3009b2` 直接相关的定向测试，复现当前真实缺口。
4. 根据失败点定位到 compiler/runtime 生产代码，实施修复。
5. 回归定向测试，再跑必要的全量检查：
   - `cargo test --all`
   - `cargo clippy --all-targets -- -D warnings`
6. 更新 `TODO.md`、`PLAN.md` 与本文件，提交一次 commit，然后停止。

## 当前进展补充

- 已检查工作区状态：仅有本轮新增/更新的 `memory/claude_plan.md`，无其他用户未提交改动。
- 已完成 `T3009b2` 验收清单中的 8 条定向运行，全部成功：
  - `effect_escape_continuation_indirect_perform_basic.scoop`
  - `effect_escape_continuation_indirect_perform_closure_locals.scoop`
  - `effect_escape_continuation_indirect_perform_resume_string.scoop`
  - `effect_escape_continuation_indirect_perform_resume_struct_with_ref.scoop`
  - `effect_escape_continuation_indirect_perform_tail_return_int.scoop`
  - `effect_escape_continuation_indirect_perform_closure_tail_return_string.scoop`
  - `effect_multi_escape_indirect_callee_suspend_matrix.scoop`
  - `effect_escape_continuation_indirect_perform_statement_container_matrix.scoop`
- 当前判断：`T3009b2` 很可能已被前序任务的生产修复一并满足，但 `TODO.md` / `PLAN.md` 尚未完成状态同步。
- 下一步：补跑 `cargo test --all` 与 `cargo clippy --all-targets -- -D warnings`；若通过，则直接更新任务文档并提交。

## 结果更新

- `cargo test --all` 已通过。
- `cargo clippy --all-targets -- -D warnings` 已通过。
- 结论：本轮未发现新的生产缺口；`T3009b2` 已被前序修复实际满足，当前需要做的是状态收口而不是继续修改 compiler/runtime 代码。
- 已更新 `TODO.md`：将 `T3009b2` 标记为完成，并记录 8 条定向 fixture + 全量测试/质量门槛通过。
- 已更新 `PLAN.md`：记录本轮验收结论，并将当前执行顺序推进到 `T3009b2R`。
- 计划提交信息：`[T3009b2] Close shared resumed-body caller-tail matrix task`
