# 本轮执行计划

## 说明

无法提供或持久化逐字的内部思维过程，但会在此记录可审计的执行计划、决策依据、关键发现和进度更新，确保你可以随时检查当前状态。

## 初始计划

1. 检查最新一次 Git 提交信息，确认是否提到任何已知遗留问题。
2. 如果最新提交中提到需要先修复的遗留问题，先定位并修复这些问题，再继续后续流程。
3. 读取 `TODO.md`，识别第一个未完成任务。
4. 评估该任务是否过大：
   - 如果可直接完成，则直接实施。
   - 如果过大，则拆分为更小的子任务，并同步更新 `PLAN.md` 与 `TODO.md`，然后只执行拆分后的第一个子任务。
5. 实现当前要执行的任务，确保实现符合规范，不引入临时绕过方案。
6. 运行相关验证：
   - 最小必要测试；
   - 受影响范围测试；
   - `cargo fmt`；
   - `cargo clippy --all-targets -- -D warnings`；
   - 如有必要，运行更完整的 `cargo test --all` 或针对性测试。
7. 更新文档与任务状态：
   - 在 `TODO.md` 中标记本轮完成的任务；
   - 在 `PLAN.md` 中更新当前状态、依赖关系与后续计划；
   - 若执行过程中发现规范缺口或前置问题，按要求将其转化为更早的任务并调整顺序。
8. 检查工作区变更，确认只提交与本轮任务相关且需要保留的内容。
9. 使用清晰的 Git 提交信息提交本轮成果。
10. 停止，不继续处理下一个任务。

## 当前状态

- 已创建本计划文件。
- 已检查最新提交：`6da40d6fda7781d66e22b4570c3da673409ec250 [T3016lR] Review RuntimeError transport contract`。
- 已读取 `TODO.md` / `PLAN.md`，本轮执行的首个未完成任务为 `T3017`：回收 `T3006` 暂时 xfail fixtures，恢复 effect run-pass 基线。
- 已确认最新提交说明未引入新的、尚未记录但必须优先修复的遗留问题；提交中提到的 runtime double-resume transport 问题已经在该提交内修复。
- 已扫描 `tests/fixtures/run-pass` 中残留的 `EXPECT: fail`，当前共 6 条：
  - `effect_resume_double_resume_exit.scoop`
  - `gc_continuation_multi_thread_concurrent_alloc_resume.scoop`
  - `stderr_mismatch_distinguishable.scoop`
  - `not_null_assert_basic.scoop`
  - `exit_code_mismatch.scoop`
  - `timeout_should_fail.scoop`
- `cargo run -p scoop --features llvm -- test` 已完整通过，结果为 `fixtures: ok (992)`。
- `cargo test --all` 已通过。
- `cargo clippy --all-targets -- -D warnings` 已通过。
- 已完成 `TODO.md` / `PLAN.md` 更新：`T3017` 已标记完成，新的首个未完成任务为 `T3017R`。
- 已检查 diff，仅包含 `TODO.md`、`PLAN.md` 与本记忆文件的本轮状态更新。

## 当前判断

- 已确认 `T3017` 不需要再拆分；当前不存在比它更前置的新生产 blocker。
- `run-pass` 下已无 `T3006` 临时 xfail 注释，`EXPECT: fail` 只剩 6 条真实归因 fixture。
- 当前剩余工作：
  1. 提交 Git commit。
  2. 停止，等待下一轮执行从 `T3017R` 开始。
