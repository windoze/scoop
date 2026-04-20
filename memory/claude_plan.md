## 当前执行计划

### 目标

本次调用只完成 `TODO.md` 中第一个未完成任务，然后停止。在进入实现前，先检查最新提交是否提到需要先修复的遗留问题；如果有，则这些问题优先于 `TODO.md` 任务处理。

### 约束与执行原则

- 先写本文件，再执行任何命令或代码。
- 不接受规避式实现；如果发现规范缺口、实现边界或前置依赖缺失，必须先把问题写入 `TODO.md`/`PLAN.md`，调整顺序后停止。
- 只完成一个任务或一个新拆分出的首个子任务。
- 任务完成后必须更新 `TODO.md`、`PLAN.md`，运行充分测试，并创建 Git 提交。
- 如果过程中计划发生变化，或完成了关键节点，需要继续更新本文件。

### 初始步骤

1. 查看最新一次 Git 提交，确认提交说明中是否提到尚未修复的问题、已知缺陷或需要跟进的事项。
2. 读取 `TODO.md`，定位第一个未完成任务。
3. 读取 `PLAN.md`，核对该任务是否已有上下文、依赖或阶段计划。
4. 判断该任务是否足够小且可以在本次调用内完整交付。
5. 如果任务过大或存在前置缺口：
   - 将任务拆成更小的子任务；
   - 更新 `PLAN.md`；
   - 更新 `TODO.md` 的任务顺序和依赖；
   - 只执行拆分后的第一个子任务。
6. 实现任务并补充/调整测试。
7. 运行相关验证，至少覆盖受影响测试；若任务影响范围较大，再补充工作区级检查。
8. 更新 `TODO.md`、`PLAN.md`、本文件。
9. 进行 Git 提交，并停止。

### 预期检查项

- 相关功能行为与规范一致。
- 相关测试通过。
- 若可行，执行 `cargo fmt`、相关测试命令，以及必要的 `cargo clippy --all-targets -- -D warnings`。
- 不覆盖或回退用户已有修改。

### 停止条件

满足以下任一条件即停止：

- 首个未完成任务已完整实现、验证、更新文档并提交。
- 发现该任务被未实现前置依赖阻塞，已在 `TODO.md`/`PLAN.md` 中重排并提交。
- 发现最新提交指出的遗留问题需要先处理，且本次已完成该问题的修复、验证、文档更新和提交。

### 当前状态

- 已完成：创建计划文件。
- 已完成：检查最新提交；提交信息仅为 `[T4016a1] Define answer-returning continuation surface`，未额外指出需要优先修复的遗留问题。
- 已完成：读取 `TODO.md` / `PLAN.md`，确认首个未完成任务为 `T4016a2`。
- 当前任务：对齐 `sysroot/core.scoop` 与必要实现注释中的 continuation / `Task` 过渡合同，使其与 `T4016a1` 文档口径一致。

### 本次实现聚焦

1. 检查 `sysroot/core.scoop` 中 `Continuation`、`Task`、`__TaskStepResult` 等注释，找出仍把 `resume` 或旧 `Task` 驱动写成稳定设计结论的表述。
2. 检查 runtime / compiler 中少量最关键注释，尤其是 `Task` 仍依赖 frame peek hack 的位置，确认是否需要把其明确写为过渡债务。
3. 仅修改必要注释与计划文档，不提前实现 `T4016b/c` 的行为改动。
4. 运行最小但足够的验证，确保注释更新未引入格式或测试问题；若改动触及编译面，再扩大验证范围。
5. 更新 `TODO.md`、`PLAN.md` 与本文件，标记 `T4016a2` 完成后提交并停止。

### 已完成的关键步骤

- 已检查 `sysroot/core.scoop` 中 `Continuation` / `Task` / `__TaskStepResult` 注释，并把它们改成“当前仅是过渡 surface”的表述：
  - `Continuation<T, eff E>` 目前只显式暴露 payload 与 required effects，不应被理解为 `resume` 最终返回 `Unit`；
  - 用户态 handler surface 只保留 `Effect.op(args) -> expr` 与 `Effect.op(args), k -> expr`；
  - `Task`/`__TaskStepResult` 已明确标注为“私有 step-result continuation answer 的过渡承载”。
- 已更新 `runtime/c/scoop_task.c` 注释，明确当前 “resume 后回读 frame 前缀得到 `__TaskStepResult`” 是 task-only 过渡债务，等待 `T4016c/d` 移除。
- 已更新 `runtime/c/scoop_runtime.c` 注释，补充当前 C runtime 仍只显式记录 resume payload transport，而 delimiter answer 仍属待显式化的 continuation ABI 过渡状态。

### 当前下一步

1. 运行验证命令，确认注释更新未引入任何构建、fixture 或 lint 回归。
2. 若验证通过，更新 `TODO.md` / `PLAN.md` / 本文件并提交。

### 验证结果

- `cargo test --all`：通过。
- `cargo run -p scoop -- test`：通过，结果为 `fixtures: ok (1112)`。
- `cargo clippy --all-targets -- -D warnings`：通过。
- `git diff --check`：通过。

### 收尾状态

- 已将 `TODO.md` 中的 `T4016a2` 标记为完成，并同步把父任务 `T4016a` 标记为完成。
- 已将 `PLAN.md` 的当前状态前移到 `T4016b`，说明 `T4016a2` 已完成的具体注释对齐内容。
- 下一步仅剩：检查最终 diff，创建本次任务提交，然后停止。
