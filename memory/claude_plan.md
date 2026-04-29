## 说明

出于安全与协作边界，我不会写入不可审阅的内部逐词推理；这里记录可审阅的执行计划、决策依据和进展更新，供你随时检查。

## 初始执行计划

1. 查看最新一次 git 提交，判断是否显式提到需要先修复的既有问题。
2. 阅读 `TODO.md`，定位第一个未完成任务。
3. 阅读 `PLAN.md`，确认当前计划、依赖关系与任务背景。
4. 如果首个未完成任务过大，则把它拆分成更小的子任务，并更新 `TODO.md` 与 `PLAN.md`。
5. 实现当前应执行的首个任务。
6. 运行与该任务相关的测试、格式化和必要检查；若发现既有问题，优先修复或把其整理为前置任务。
7. 更新 `memory/claude_plan.md`、`TODO.md`、`PLAN.md` 以反映进展。
8. 按仓库约定创建一次 git 提交，然后停止，不继续下一个任务。

## 进展

- 已创建本文件并记录初始计划。
- 已检查最新提交、`TODO.md` 与 `PLAN.md`。
- 已确认首个未完成任务为 `T5001f1R Review：确认 await/task waiting transport 合同重新闭合`。
- 最新提交标题为 `[T5001f1] Fix async waiting continuation transport`；提交标题本身未额外声明新的待修问题。
- `PLAN.md` 记录了一个独立既有失败：`cargo run -p scoop -- test --fixtures tests/fixtures/run-pass` 已越过原 async 阻塞，但会在 `class_init_order_primary_secondary_basic.scoop` 处失败。当前需要在 review 过程中确认这是否影响本次任务的收口以及后续任务排序。
- 已复核 `crates/scoopc/src/llvm/codegen/effect/state_machine_emitter.rs` 中 await waiting-path 的关键写回逻辑：escaped continuation 会先经 `store_local_value_exact(...)` 同步到 ordinary local 与 explicit-frame home slot，再调用 `__task_step_pending(...)`，与 waiting-task resume 合同一致。
- 已通过定向 LLVM / 运行时验证：`async_task_pending_path_stores_escape_continuation_before_waiting_helper`、`async_task_resume_ir_does_not_replay_original_await_site`、`single_file_minimal_ir_supports_handled_async_await`、`task_step_manual_basic.scoop`、`async_await_minimal_int_basic.scoop`、`async_await_string_basic.scoop`、`async_fun_task_runtime_basic.scoop` 与 `cargo clippy -p scoopc --all-targets -- -D warnings`。
- 已再次确认独立 blocker 仍存在：`cargo run -p scoop -- run tests/fixtures/run-pass/class_init_order_primary_secondary_basic.scoop` 目前只输出 `start` / `Primary.a`，未完成预期的类初始化顺序主线。
- 决策：将 `T5001f1R` 标记完成，并按 `PROMPT.md` 在 `T5001g` 前插入新的前置任务 `T5001f2/T5001f2R` 来处理类初始化顺序回归；本次提交在完成该计划更新后停止。
- 下一步：
  1. 更新 `TODO.md` / `PLAN.md`，把 `T5001f1R` 标记完成并插入 `T5001f2/T5001f2R`。
  2. 检查 diff 与 git 状态，整理本轮仅涉及 review/计划收口的改动。
  3. 按仓库约定创建一次提交，然后停止，等待下一轮处理 `T5001f2`。
