## 当前轮次执行计划

### 说明
- 按要求先将本轮的执行分析与计划写入本文件，再进行仓库检查和后续操作。
- 这里记录的是可审阅的简要分析、假设、风险和执行步骤，不包含逐字内部推理展开。

### 目标
- 先检查最新提交是否提到任何既有问题；若有，优先修复。
- 读取 `TODO.md`，定位第一个未完成任务。
- 如果该任务过大，则把它拆解到 `PLAN.md` 和 `TODO.md`，本轮只执行拆解后的第一个子任务。
- 完成实现、测试、文档更新、提交，然后停止。

### 预期步骤
1. 查看最新一次 git 提交信息，确认是否明确提到待修复的既有问题。
2. 读取 `TODO.md` 与 `PLAN.md`，识别当前最高优先级未完成任务及其上下文。
3. 评估任务是否可在本轮完整落地；若不可，则进行任务拆分并更新计划文件。
4. 阅读相关代码、测试和规范，明确正确实现边界。
5. 实现该任务所需改动。
6. 运行相关测试；必要时补充或修复测试。
7. 更新 `TODO.md`、`PLAN.md`，并在关键节点回写本文件。
8. 检查工作区差异，整理提交，创建一次 git commit。
9. 停止，不继续处理后续任务。

### 当前已知风险
- 最新提交若只模糊提到问题，可能需要结合代码和测试判断其是否仍然存在。
- 第一个未完成任务若依赖缺失特性或存在规范偏差，需要先把阻塞项前置到 `TODO.md`，而不是绕过。
- 仓库可能已有未提交改动，需要避免误覆盖非本轮变更。

### 当前状态
- 已完成：计划文件初始化。
- 已完成：检查最新提交，未发现提交说明中显式提到需先修复的新既有问题。
- 已完成：读取 `TODO.md` / `PLAN.md`，确认当前首个可执行未完成条目为 `T4016d`，即让 `Task` 最终退化为基于 continuation answer type 的薄封装。
- 已完成：盘点相关实现与文档，确认 `T4016d` 已有部分落地，但仍有剩余实现债务：`runtime/c/scoop_task.c` 仍本地复刻 `ScoopContinuation` 布局并直接写入 `resume_word` / `resume_gc_ref`；LLVM continuation resume lowering 也仍直接按结构体字段写 payload。
- 当前判断：本任务无需继续拆分；可在本轮通过新增通用 continuation resume helper、收掉 task/runtime/LLVM 对 payload 字段布局的直接依赖、补回归并同步文档来完整收口。

### 已细化的执行方案
1. 在 runtime 中新增通用 continuation resume helper，让调用方通过统一入口写 resume payload 并接收 delimiter answer，而不是直接触碰 continuation payload 字段。
2. 修改 `runtime/c/scoop_task.c`，删除本地 `ScoopContinuation` 布局镜像，改为完全通过通用 helper 恢复 pending task continuation。
3. 修改 LLVM codegen，使 `Continuation.resume(...)` fresh-path / replay-path / tail-resume fast path 都改走新的通用 helper，而不是先 GEP 写 payload 再调用 `resume_into`。
4. 更新 runtime API 符号声明与相关文档/注释，明确 `Task.poll()/step()` 与普通 `Continuation.resume(...)` 共用同一 continuation payload+answer helper。
5. 补运行时测试与 LLVM IR 回归，覆盖新 helper 路径。
6. 运行相关测试，再按要求更新 `TODO.md`、`PLAN.md` 并提交。

### 关键进展
- 已完成：runtime 新增共享 continuation helper 方案落地，加入新的 `scoop_continuation_resume_with(...)` 导出符号，用于统一“写 payload + resume + 读 answer”。
- 已完成：`runtime/c/scoop_task.c` 已删除本地 `ScoopContinuation` payload 布局镜像，pending task 恢复路径改为完全走共享 helper。
- 已完成：LLVM `Continuation.resume(...)` fresh-path / replay-path / tail fast path 已切到共享 helper，不再由 caller 直接写 continuation payload 字段。
- 已完成：补了 runtime/LLVM 回归骨架，并同步了 `SCOOP_FULL_SPEC.md`、`SCOOP_RUNTIME.md`、`sysroot/core.scoop` 的叙事。
- 已完成：格式化并运行定向测试；期间发现 `scoop_continuation_resume_u64` 误被改成要求 delimiter answer 的路径，导致 `continuation_cross_thread_handler_stack` 异常退出，现已修正回兼容语义。

### 验证结果
- 已通过：`cargo test -p scoop_runtime --test continuation_one_shot --test task_spawn_join --test continuation_cross_thread_handler_stack`
- 已通过：`cargo run -p scoop -- build tests/fixtures/run-pass/task_poll_step_manual_basic.scoop -o /tmp/task_poll_step_manual_basic.out`
- 已通过：执行 `/tmp/task_poll_step_manual_basic.out`，stdout 与 fixture 预期一致。
- 已通过：`cargo run -p scoop -- test --fixtures tests/fixtures/run-pass`，结果 `fixtures: ok (375)`。
- 已通过：`cargo test --all`
- 已通过：`cargo clippy --all-targets -- -D warnings`

### 待收尾
1. 已完成：更新 `TODO.md`，将 `T4016d` 标记为完成。
2. 已完成：更新 `PLAN.md`，记录本轮完成内容并把下一步切到 `T4016R`。
3. 进行中：检查差异并创建本轮提交，然后停止。
