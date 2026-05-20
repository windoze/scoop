# 执行计划

## 当前约束
- 先以 `TODO.md` 为唯一任务排序来源，识别第一个标题未带 `[DONE]` 的任务。
- 本次只完成第一个未完成任务；完成、验证、记录、提交后停止。
- 不使用 workaround；如果发现阻塞当前任务的实现缺口，先在 `TODO.md` 插入最小必要前置任务并提交后停止。
- `PLAN.md` 只在阶段级计划、依赖或完成标准变化时更新。

## 步骤
1. 读取 `TODO.md`，确定第一个未完成任务及其验证要求。
2. 检查最近提交是否明确提到与该任务直接相关的未完成问题；若有，将其纳入当前任务或登记为前置依赖。
3. 根据任务内容读取最小必要代码和测试上下文，确认实现边界。
4. 实施任务所需的最小正确改动，期间在关键步骤完成或计划变化时更新本文件。
5. 运行任务要求的相关测试和必要的质量检查；若失败，修复后重跑。
6. 在 `TODO.md` 中给任务标题加 `[DONE]`，更新完成记录；只在必要时更新 `PLAN.md`。
7. 检查 git 状态和 diff，确认没有意外修改；按要求提交本次任务相关变更。
8. 提交后停止，不继续下一个任务。

## 进度
- 已写入初始执行计划。
- 已读取 `TODO.md`，第一个未完成任务为 `P8-T04：迁移 scoop.thread native implementation 与 thread entry trampoline`。
- 已检查最近提交：`[P8-T03] Migrate scoop.sync native implementation`，未声明与 `P8-T04` 直接相关的未完成问题。
- 已确认当前工作树除本计划文件外无其他未提交修改。
- 已读取 `PLAN.md` P8 要求和 thread/sync/runtime 入口。实施方案：将 `scoop.thread` surface 改为 Scoop wrapper + `@Extern(abi = "scoop")` native primitives；`threadSpawn` 在 Scoop 侧创建 `GcHandle` raw token，cone-local C trampoline 在新 OS 线程 attach 后调用 `@CallingConvention` Scoop entry symbol，正常返回后 drop handle 并 detach；移除 runtime core 中 user-level `scoop_thread_spawn/join/yield/sleep/current_id` 实现和 allowlist entries。
- 已开始实施：`scoop.thread` sysroot surface 已改为 wrappers；新增 `sysroot/lib/scoop.thread/native/scoop_thread.c` 并通过 `[native-build]` 接入；删除旧 `runtime/c/scoop_thread.c`；移除 LLVM thread intrinsic lowering active path；runtime allowlist 已删除 user-level thread symbols。
- 定向调试中发现未被普通 Scoop 代码调用的 `@CallingConvention` body 不会成为 MIR materialization root，导致 native object 可引用的 trampoline 缺少 plain callable body；已修正 materializer，将所有 `@CallingConvention` body 作为 roots。
- 已通过 thread API typecheck、basic thread run-pass、cross-thread GC roots、thread UMB、explicit sysroot thread dependency 与 runtime allowlist 定向验证；下一步运行较大范围 build/clippy/tests。
- 已完成仓库级验证：`cargo build`、`cargo clippy --all-targets -- -D warnings`、`cargo test --all --all-targets` 和 `cargo run -p scoop -- test` 均通过。
- 已在 `TODO.md` 中将 `P8-T04` 标记为 `[DONE]`，更新当前状态、任务索引和完成记录；`PLAN.md` 无阶段级变化，未更新。
- 下一步检查 git diff/status，然后提交本次 `P8-T04` 变更。
