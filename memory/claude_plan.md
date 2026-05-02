## 当前执行计划

说明：此文件记录本次执行的可操作计划、关键判断、进展与变更原因，不记录不可导出的内部推理细节。

1. 先读取 `TODO.md`，确认索引结构、任务文件映射与完成标记约定。
2. 按 `TODO.md` 给出的顺序读取对应 `TODO-Px.md`，以任务标题是否带 `[DONE]` 作为唯一完成判定，定位第一个未完成的详细任务。
3. 检查最近一次提交是否直接提到与该任务相关且未完成的问题；如果这是当前任务的直接组成部分或前置阻塞，则将其纳入当前处理范围。
4. 阅读当前任务涉及的实现、测试、文档与约束，确认是否可以在本次调用内完整落地。
5. 若存在阻塞当前任务的真实缺口或规格不匹配：
   - 在对应 `TODO-Px.md` 中插入最小必要前置任务并调整依赖顺序；
   - 同步 `TODO.md`；
   - 仅当阶段计划发生变化时更新 `PLAN.md`；
   - 提交并停止。
6. 若无阻塞，则直接实现当前任务，保持改动尽量小且符合既有结构。
7. 运行与任务直接相关的测试、格式化、`cargo clippy --all-targets -- -D warnings`，修复发现的问题。
8. 在对应 `TODO-Px.md` 中将当前任务标题标记为 `[DONE]` 并补全完成记录；如索引需要同步，则更新 `TODO.md`。
9. 检查工作区状态，按仓库既有风格创建一次原子提交，然后停止，不继续处理下一项任务。

## 进展

- 已创建本计划文件，准备开始读取任务索引并定位当前应执行的第一项未完成详细任务。
- 已读取 `TODO.md` 与 `TODO-P5.md`，确认当前首个未完成详细任务为 `P5-T01R`：`Review late-lowering stage 边界，确认新路径没有借壳 legacy effect/state_machine 或 LLVM backend`。
- 已检查最新提交：`c68f76a9 [P5-T01] Establish refactor late-lowering stage boundary`。该提交正是当前 review 任务的直接前置实现，没有发现另一个必须先处理的、被显式提到的未完成问题；因此本次按 `P5-T01R` 的 review/验证要求继续执行。
- 下一步：
  1. 阅读 `effect_lowered`、`effect_refactor_pipeline/effect_lowering_stage.rs`、`lib.rs` 以及相关 legacy 目录，确认边界是否干净。
  2. 运行任务要求的搜索与验证命令，检查是否存在把 refactor 分支混入 legacy `effect/state_machine/**` 或 `llvm/codegen/effect/**` 的情况。
  3. 若 review 通过，则把 `P5-T01R` 标记为 `[DONE]`、补全完成记录、同步 `TODO.md`，并创建提交；若发现阻塞，则按要求插入最小前置任务后提交并停止。

## 当前结论

- review 结论：`P5-T01R` 可完成，未发现需要新增的前置阻塞任务。
- 证据摘要：
  1. `git diff --name-only HEAD^ HEAD -- crates/scoopc/src/effect_lowered crates/scoopc/src/effect_refactor_pipeline crates/scoopc/src/effect/state_machine crates/scoopc/src/llvm/codegen/effect` 结果表明，上一提交只改动了 `effect_lowered/**` 与 `effect_refactor_pipeline/**`，没有改动 legacy `effect/state_machine/**` 或 `llvm/codegen/effect/**` 业务实现。
  2. `crates/scoopc/src/effect_refactor_pipeline/refactor.rs` 中，`LateLowering` 的 refactor 路径直接调用 `effect_lowering_stage::run(...)`；`crates/scoopc/src/effect_lowered/{mod,builder,ir}.rs` 构成独立 late-lowered 子系统。
  3. 针对 legacy 目录的关键字搜索未发现新增 `EffectPipelineMode` / `refactor` 分支被混入 legacy `effect/state_machine/**` 或 `llvm/codegen/effect/**` 主实现。
  4. 针对新模块的依赖搜索显示：`effect_lowered/**` 不包含 `crate::effect::state_machine` 或 `crate::llvm` 引用；`effect_lowering_stage.rs` 的防回归测试也显式断言了这一点。
  5. 验证通过：`cargo test -p scoopc --no-default-features refactor_effect_lowered_stage`、`cargo clippy -p scoopc --no-default-features --all-targets -- -D warnings`。

- 收尾动作：更新 `TODO-P5.md` 与 `TODO.md` 的 `[DONE]` 标记和完成记录，然后提交并停止。

## 收尾进展

- 已更新 `TODO-P5.md`：将 `P5-T01R` 标题标记为 `[DONE]`，并写入 review 结论、搜索结果、路径边界与验证命令。
- 已同步 `TODO.md`：为索引中的 `P5-T01R` 加上 `[DONE]` 标记。
- `PLAN.md` 无需修改；下一步只剩检查工作区、创建原子提交并停止。
