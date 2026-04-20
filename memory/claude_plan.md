## 当前计划（初始）

### 目标
- 本轮只完成 `TODO.md` 中第一个未完成任务，并在完成后停止。
- 在开始任何实现前，先检查最近一次提交中是否提到待修复问题；若有，先修复这些问题。

### 执行步骤
1. 查看最近一次提交信息，确认是否存在明确提到的遗留问题或待修复项。
2. 阅读 `TODO.md`，定位第一个未完成任务。
3. 读取 `PLAN.md`，核对该任务的上下文、依赖与已有分解。
4. 判断该任务是否足够小且可直接完成：
   - 若可以，直接实现。
   - 若不可以，先把任务拆分为更小子任务，并同步更新 `PLAN.md` 与 `TODO.md`，然后只执行拆分后的第一个子任务。
5. 实现任务时，若发现任何与规格不符、缺失能力或现有缺陷：
   - 不绕过、不打补丁式规避。
   - 先在 `TODO.md` 中补充前置修复任务并调整顺序。
   - 在 `PLAN.md` 记录阻塞原因与依赖关系。
   - 若因此无法继续当前任务，则提交这些计划性变更后停止。
6. 为实现结果补充或更新测试，并运行相关验证：
   - 至少运行与本任务直接相关的测试。
   - 若改动影响面较广，补充运行更高层级验证。
   - 目标是确保 `cargo clippy --all-targets -- -D warnings` 无告警。
7. 完成后更新文档状态：
   - 在 `TODO.md` 中标记任务完成。
   - 在 `PLAN.md` 中记录完成情况与后续影响。
   - 视需要更新 `README.md` 或代码注释。
8. 提交本轮所有改动，提交信息应清晰描述本轮完成的任务。
9. 停止，不继续处理下一个任务。

### 记录原则
- 在执行过程中，如计划变化、发现阻塞、完成关键步骤或测试结果明确，及时更新本文件。
- 这里记录的是面向任务执行的简明推理摘要与操作计划，不记录冗长草稿。

## 当前进展

### 已确认事项
- 最近一次提交为 `[T4016b2] Thread continuation answer type through static surface`，提交信息中未额外点名需要先修复的遗留问题。
- `TODO.md` 中首个未完成子任务是 `T4016c`：收口 state machine / runtime / ABI，使 continuation result 成为一等返回通道。
- `PLAN.md` 已明确当前顺序是 `T4016b2 -> T4016c -> T4016b3`，因此本轮不应越过 `T4016c` 去实现 expression-position `k.resume(...): Answer` 的完整语言面。

### 当前细化执行计划
1. 盘点 continuation resume 的现有实现路径：
   - runtime ABI；
   - LLVM/codegen 的 continuation lowering；
   - `Task` 当前如何在 `resume` 之后回读 step result。
2. 判断 `T4016c` 是否能在本轮直接完成；若范围仍过大，则把它进一步拆分到 `TODO.md` / `PLAN.md`，并只执行第一个子任务。
3. 若可直接完成，则实现统一 answer-return channel，并同步必要注释/文档。
4. 补充并运行定向测试、工作区测试与 `clippy`。
5. 更新 `TODO.md`、`PLAN.md` 与本文件，然后提交本轮改动并停止。

### 最新判断
- `T4016c` 可直接实施，不需要继续拆分。
- 现有问题的最小正确收口路径是：
  1. 在 runtime 新增统一 continuation answer helper，负责“执行 resume + 从标准化状态机 transport 读取 delimiter answer”；
  2. 保留旧 `scoop_continuation_resume` / `scoop_continuation_resume_u64` 兼容入口，但让 compiler 与 `Task` 改走新 helper；
  3. `Task` 不再直接读取 continuation heap frame 前缀；
  4. 更新 runtime/codegen 测试与文档，明确底层仍可复用 frame transport，但该细节已收进通用 continuation ABI，而不再暴露给 task-only 代码。

### 已完成的关键实现
- runtime 已新增显式 answer-return helper：`scoop_continuation_resume_into(...)`。
- 旧 `scoop_continuation_resume(...)` / `scoop_continuation_resume_u64(...)` 保留兼容形状，但内部已统一转到新 helper。
- `runtime/c/scoop_task.c` 已改为通过新 helper 取得 `__TaskStepResult`，不再自己读取 continuation heap frame 前缀。
- LLVM continuation resume lowering 与 state-machine tail-resume fast path 已切到新 helper，避免 compiler/runtime 继续围绕旧 void ABI 分裂。
- 已补 runtime 回归与 LLVM IR 断言，下一步是格式化、编译与定向测试。

### 验证结果
- `cargo test -p scoop_runtime --test continuation_one_shot -- --nocapture`：通过。
- `cargo test -p scoopc state_machine_multi_payload_perform_uses_tuple_transport -- --nocapture`：通过。
- `cargo test -p scoopc when_arm_try_resume_nested_handle_ir_keeps_binder_scope_for_inner_resume -- --nocapture`：通过。
- `cargo run -p scoop -- run tests/fixtures/run-pass/task_poll_step_manual_basic.scoop`：通过。
- `cargo run -p scoop -- run tests/fixtures/run-pass/continuation_resume_surface_named_tuple_and_unit_basic.scoop`：通过。
- `cargo test --all`：通过。
- `cargo clippy --all-targets -- -D warnings`：通过。

### 收尾状态
- `T4016c` 已达到“共享 continuation answer-return channel 落地”的目标，可在 `TODO.md` 中标记完成。
- 下一步应转入 `T4016b3`：把当前 runtime helper 真正接到 expression-position `Continuation.resume(...): Answer` 的 typecheck / lowering 主线。
