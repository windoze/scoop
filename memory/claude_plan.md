## 当前回合执行记录

说明：以下内容记录可审计的执行思路摘要与步骤计划，用于跟踪本回合工作，不展开逐字内部推理。

### 目标

完成 `TODO.md` 中第一个未完成任务；若存在最新提交中提到的遗留问题，则先修复这些问题；完成后更新计划与任务状态、运行测试、提交 git commit，然后停止。

### 初始计划

1. 检查最新一次提交信息与改动，确认是否提到了遗留问题或已知缺陷。
2. 读取 `TODO.md`，定位第一个未完成任务。
3. 读取 `PLAN.md`，理解当前总体计划、依赖关系和任务上下文。
4. 判断首个未完成任务是否可在本回合完整完成。
5. 若任务过大，则拆分为更小子任务，并更新 `PLAN.md` 与 `TODO.md`，将第一个子任务作为当前执行目标。
6. 在实现前检查相关代码、测试、规范或夹具，确认真实约束。
7. 实现当前任务，避免引入规避性方案或偏离规范的行为。
8. 运行相关测试；如有必要，补充或修复测试，并确保 `cargo clippy --all-targets -- -D warnings` 不报错。
9. 更新 `TODO.md`、`PLAN.md` 与本文件，记录完成情况或阻塞原因。
10. 使用清晰的提交信息创建 git commit，然后停止，不继续处理下一个任务。

### 执行原则

- 若发现规范不匹配、缺失功能、运行时缺陷或测试仅靠变通方案通过，则必须先把该问题加入 `TODO.md`，调整依赖顺序，再停止本回合。
- 不回退与当前任务无关的用户改动。
- 仅在当前首个任务完整完成后才标记其完成。

### 状态

- 当前阶段：已完成初始化与任务确认。

### 已确认信息

1. 最新提交 `e84b7e1f8952973dcd9bdc443fd6328c4b46d329` 仅更新计划文件，未额外引入“需先修复的遗留 issue”说明。
2. `TODO.md` 中首个未完成任务为 `T4016T1`：将 core task public surface 收口为 `TaskStep<T>` + `step()`，移除 `Poll<T>` 与 `Task.poll()`。
3. 该任务当前不需要继续拆分；主要改动边界已经明确，集中在：
   - `sysroot/core.scoop`
   - `SCOOP_FULL_SPEC.md`
   - `SCOOP_RUNTIME.md`
   - `SCOOP_TASK.md`（如需补充与实现一致的表述）
   - `crates/scoopc/src/llvm/codegen/mod.rs`
   - `crates/scoopc/src/typecheck/expr/error.rs`
   - `tests/fixtures/run-pass/task_step_manual_basic.scoop`
   - 可能新增一个 typecheck fixture，锁定 `poll()` 已被移除
   - `PLAN.md` / `TODO.md`

### 当前执行计划（细化）

1. 将 `sysroot` 的公开 task surface 从 `Poll<T>` / `poll()` 改为 `TaskStep<T>` / `step()`。
2. 将 LLVM codegen 中对 `scoop.core.poll` / `scoop.core.Poll` 的硬编码切换到 `scoop.core.step` / `scoop.core.TaskStep`。
3. 更新与任务 surface 相关的用户提示文本，避免继续向用户推荐 `Task.poll()`。
4. 更新运行回归 fixture，并补一个负向 typecheck fixture，锁定 `poll()` 已不再属于公开 surface。
5. 同步更新 spec/runtime/design/进度文档，使生产文档只保留 `TaskStep<T>` + `step()` 叙事。
6. 运行格式化与相关验证；若 spec code block 有变更，再执行 `spec-fixtures check`。
7. 更新 `TODO.md`、`PLAN.md` 与本文件，提交 commit，然后停止。

### 已完成的关键步骤

1. 已将 `sysroot/core.scoop` 的公开 task surface 收口为 `TaskStep<T>` + `Task.step()`，并删除 `Poll<T>` / `Task.poll()`。
2. 已将 LLVM codegen 从 `scoop.core.poll` / `scoop.core.Poll` 切换到 `scoop.core.step` / `scoop.core.TaskStep`，避免留下 compatibility 分支。
3. 已更新结构化并发 deferred 诊断与 runtime 注释，确保用户提示和实现叙事都只指向 `Task.step()`。
4. 已同步 `SCOOP_FULL_SPEC.md`、`SCOOP_RUNTIME.md`、`SCOOP_TASK.md`、`ISSUES.md` 与 `STDLIB_COMPLETENESS.md`。
5. 已将 run-pass 回归改为 `task_step_manual_basic.scoop`，并新增两个 typecheck 负向 fixture，分别锁定 `Task.poll()` 与 `Poll<T>` 已移除。

### 验证结果

- `cargo run -p scoop_tools -- spec-fixtures check`：通过
- `cargo run -p scoop -- test --fixtures tests/fixtures/typecheck`：通过
- `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass`：通过
- `cargo run -p scoop -- test`：通过
- `cargo test --all`：通过
- `cargo clippy --all-targets -- -D warnings`：通过

### 当前状态（收尾前）

- `T4016T1` 已实现并验证完成。
- 下一步应在提交后停止，本轮不继续进入 `T4016T2`。
