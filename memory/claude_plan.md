# Claude Plan

## 说明
按要求先记录可审阅的高层推理摘要与执行计划。这里不写内部逐词思维过程，只记录任务判断依据、执行顺序、阻塞条件与完成标准，后续在关键进展后持续更新。

## 当前目标
完成本轮实际执行目标 `T4017e1`：把 `pending_continuation` 与 `continuation_resume_active` 从原始 TLS source-of-truth 收口到显式 resume-driver scope，并在完成后停止。本轮不继续推进 `T4017e2`。

## 已确认事实
1. 最新提交为 `[T4017d] Switch ordinary effectful calls to explicit effect ABI`，提交信息正文未提到额外待修复 issue。
2. 当前工作区只有 `memory/claude_plan.md` 被修改，没有其它未提交改动。
3. 原始首个未完成条目是 `T4017e`；因实现面过大，现已拆成 `T4017e1 -> T4017e2 -> T4017e3`，并把 `T4017e1` 作为本轮目标。
4. `PLAN.md` 与 `TODO.md` 已同步更新顺序；下一轮入口将是 `T4017e2`。

## 本轮实现摘要
1. 已将 `T4017e` 拆分为三个子任务：
   - `T4017e1`：runtime resume-driver scope bookkeeping
   - `T4017e2`：`Continuation.resume(...)` replay token 显式化
   - `T4017e3`：ordinary indirect callee `callee_suspend_state` 迁出 TLS
2. `runtime/c/scoop_runtime.c` 已引入 `ScoopContinuationResumeScope`：
   - 删除 `__scoop_continuation_resume_pending_continuation`
   - 删除 `__scoop_continuation_resume_active`
   - 当前线程仅保留 `__scoop_continuation_resume_scope` 作为 active scope 指针
3. `scoop_continuation_resume_publish_pending_continuation()` 现在只写入当前 active scope 的 `pending_continuation` 字段；`scoop_continuation_resume_common()` 通过 `prev` 链正确隔离 nested resume。
4. `crates/scoop_runtime/tests/continuation_one_shot.rs` 新增定向回归：
   - `continuation_publish_pending_continuation_is_scoped_to_active_resume_driver`
   - 锁定 scope 外 publish 为 no-op，scope 内 publish 会被包装成 replay-state，而不是泄漏 raw continuation 指针。

## 验证结果
1. 基线检查通过：
   - `cargo test -p scoop_runtime --test continuation_one_shot --test continuation_cross_thread_handler_stack`
   - 相关 LLVM 定向单测
2. 本轮实现后的验证通过：
   - `cargo fmt --all`
   - `cargo test --all`
   - `cargo run -p scoop -- test` → `fixtures: ok (1169)`
   - `cargo clippy --all-targets -- -D warnings`

## 结论
1. 没有遇到新的前置 blocker。
2. `T4017e1` 已完成并已写回 `TODO.md` / `PLAN.md`。
3. 下一轮应从 `T4017e2` 开始。

## 完成标准
- 最新提交提及的既有问题已处理。
- 本轮只完成一个任务或一个新拆分出的首个子任务。
- 所有相关测试通过，且不存在编译/Clippy 警告。
- `TODO.md`、`PLAN.md`、`memory/claude_plan.md` 已同步更新。
- 改动已提交到 Git。

## 进度记录
- 已检查最新提交；未发现提交信息中声明的额外既有 issue。
- 已确认原始首个未完成任务为 `T4017e`，并完成任务拆分。
- 已实现并验证 `T4017e1`。
- 下一步：检查 diff、提交本轮改动，然后停止。
