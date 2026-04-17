# 执行计划与决策记录

说明：
- 该文件记录本轮任务的执行计划、关键决策、进度更新与阻塞原因，便于审计。
- 不写入逐字内部推理；改为提供足够详细的步骤、依据与结果。
- 我会在完成关键步骤、调整计划或发现阻塞时及时更新本文件。

## 本轮目标

按仓库 `TODO.md` 的顺序，只完成第一个未完成任务，然后停止。

## 初始执行步骤

1. 检查最新一次 Git 提交：
   - 阅读提交信息与改动摘要。
   - 判断其中是否提到已知问题、遗留缺陷或需要先处理的事项。
   - 若存在，则先修复这些问题，并在本文件记录。
2. 读取 `TODO.md`：
   - 找到第一个未完成任务。
   - 判断该任务是否可在本轮完整完成。
3. 如任务过大：
   - 将其拆解为更小的子任务。
   - 更新 `PLAN.md`。
   - 在 `TODO.md` 中替换或补充这些子任务，并保证依赖顺序正确。
   - 选择拆分后的第一个子任务作为本轮执行对象。
4. 实施任务：
   - 先阅读相关代码与测试。
   - 再做最小且正确的实现修改，避免绕过规范。
   - 如遇规范缺口、实现边界或现有 bug，会先把它们显式加入 `TODO.md`，并调整当前任务依赖，而不是采用临时 workaround。
5. 验证：
   - 运行与本次修改直接相关的测试。
   - 运行必要的格式化、静态检查与 lint（至少覆盖受影响范围；若可行则按要求执行 `cargo clippy --all-targets -- -D warnings`）。
   - 修复发现的问题直到通过，或在无法继续时按阻塞流程处理。
6. 文档与任务状态更新：
   - 在 `TODO.md` 中将本轮完成的任务标记为已完成。
   - 在 `PLAN.md` 中反映最新状态和后续计划。
   - 在本文件追加关键结果与验证结论。
7. 提交：
   - 使用清晰的 Git 提交信息提交本轮改动。
   - 提交后停止，不继续下一个任务。

## 更新规则

- 完成“检查最新提交”“确认首个未完成任务”“开始实现”“完成验证”“更新任务文档”“完成提交”等关键节点后，都会更新本文件。
- 如果计划发生变化，会追加“计划调整”段落，说明原因与影响。
- 如果发现阻塞，会追加“阻塞记录”段落，明确缺失能力、规范不匹配、添加到 `TODO.md` 的新前置任务，以及为什么必须先解决它。

## 当前状态

- 已创建本文件。
- 已检查最新一次 Git 提交：`4ab58d3efdc6062d67ff45d26cb927450608c81d`（`[T3010b2b1b1] Sync handle_perform MIR golden`）。
- 提交信息与改动摘要未引入新的“必须先修复的前置问题”；本次仍按 `TODO.md` 当前顺序推进。
- 已读取 `TODO.md` / `PLAN.md` 并确认首个未完成任务为 `T3010b2b1`：收口 handle arm body nested/indirect non-resuming effect 的剩余外传 / self-inactive / finally 验收。
- 初步判断：该任务很可能仍可围绕同一类 arm-body outward propagation 缺口完成；先做最小复现与失败路径定位，再决定是否需要进一步拆分。

## 执行进展

- 已重新阅读 `T3010b2b1` 的验收 fixture，确认覆盖 immediate-resume arm raise、escape continuation arm raise、pure non-resuming multi-arm finally，以及 nested handle / indirect perform outward propagation。
- 已执行并通过以下定向命令：
  - `cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/effect_resume_finally_arm_raise.scoop`
  - `cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/effect_escape_continuation_finally_arm_raise.scoop`
  - `cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/effect_multi_nonresuming_raise_custom_finally.scoop`
  - `cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/effect_escape_continuation_nested_arm_indirect_performs_outer.scoop`
- 定向验收结果：
  - 四个 fixture 全部通过。
  - 未再观察到 arm body 在 unmatched non-resuming effect 后错误落回当前 handle 正常完成路径。
  - `finally` / cleanup 的执行顺序与 outward propagation 语义符合任务要求。
- 已执行仓库级验证：
  - `cargo test --all` 通过。
  - `cargo clippy --all-targets -- -D warnings` 通过。
  - `cargo run -p scoop --features llvm -- test` 不再在 `T3010b2b1` 相关语义上更早失败；当前首个失败点是 `tests/fixtures/run-pass/continuation_resume_continuation.scoop` 的 stale `EXPECT: fail`，该问题已由后续任务 `T3017` 跟踪。
- 结论：
  - `T3010b2b1` 已可判定完成，无需新增生产代码修改，也不需要继续拆分子任务。
  - 已同步更新 `TODO.md` 与 `PLAN.md`：将 `T3010b2b1` 标记为完成，并把当前执行顺序推进到 `T3010b2b`。

## 下一步

1. 复核文档 diff，确保 `TODO.md` / `PLAN.md` / 本文件一致。
2. 提交本轮变更。
3. 停止，等待下一次调用。
