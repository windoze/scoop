# T4016T8 执行计划与进展记录

## 当前目标

- 本轮只处理 `TODO.md` 中首个未完成任务 `T4016T8`：收口无锁 `Task` 的 compiler/runtime/substrate handoff 与 trap 合同。
- 在完成该任务前，不推进后续任务。
- 若在 probing、测试、实现中发现既有问题或规范不匹配，必须先修复；不能通过改 fixture 形状、缩小覆盖面或引入特判来绕过。

## 已确认前置状态

- 最新提交 `9a5985e` 为 `Update plan`，未直接声明新的待修代码问题。
- `TODO.md` 首个未完成项确认是 `T4016T8`。
- 生产代码中的 `Task` 已从 per-task mutex 切换为 atomic claim。
- `SCOOP_RUNTIME.md` / `SCOOP_TASK.md` 里仍有“当前 checkpoint 是 per-task mutex”的旧叙事，但这属于后续 `T4016T9` 的文档收口，不是本轮主任务。

## 已发现的既有问题

- 旧回归 `tests/fixtures/run-pass/task_step_cross_thread_sequential_handoff_basic.scoop` 名义上覆盖“顺序 handoff”，实际上包含竞态。
- 在 `SCOOP_GC_STRESS=1` 或 `SCOOP_GC_MOVE=1 SCOOP_GC_STRESS=1` 下，该回归会在 `outer-before` 后直接失败。
- 这属于 `T4016T8` 范围内的既有问题，必须先修。

## 已完成的改动

1. 修正顺序 handoff 基础 fixture 的竞态写法
   - 文件：`tests/fixtures/run-pass/task_step_cross_thread_sequential_handoff_basic.scoop`
   - 调整为：由 main 先执行第一次 `outer.step()`，确认发布 `Waiting(...)` 后再 spawn worker。
   - 目的：将“顺序 handoff”与“并发 step trap”拆开，避免测试本身引入竞态。

2. 新增 waiting-path LLVM 回归
   - 文件：`tests/fixtures/build/task_waiting_handoff_atomic_no_mutex_llvm.scoop`
   - 目标：锁定 `__task_drive_waiting::<Int>` 的 waiting-path 仍走 atomic claim，无 mutex，并要求看到 `@scoop_gc_write_barrier`。

3. 新增 GC stress + move 的顺序 handoff 回归
   - 文件：
     - `tests/fixtures/runtime_gc/task_step_cross_thread_sequential_handoff_gc_stress.scoop`
     - `tests/fixtures/runtime_gc/task_step_cross_thread_sequential_handoff_gc_stress.stdout.txt`
   - 目标：覆盖 main 首次 `Pending`、显式 GC、worker 接手 resume、再次显式 GC 的 continuation/GC/thread 路径。

4. 在 `sysroot/task.scoop` 补充 claim 原子语义注释
   - 说明当前依赖 SeqCst 的 acquire/release 与可见性合同。

5. 在 LLVM GC store 路径上进行了修正尝试
   - 文件：`crates/scoopc/src/llvm/codegen/gc.rs`
   - 已做内容：
     - 提取 `store_gc_pointer_slot_with_write_barrier(...)`。
     - 该 helper 改为通过 `build_call_preserving_gc_local_roots(...)` 调 runtime barrier，而不是直接 `build_call`。
     - 对 `CgTy::Enum(enum_ty)` 增加 `try_store_heap_tagged_union_enum_exact(...)`：
       - 若目标是 GC heap 上的 `TaggedUnion` enum 字段，不再整块 `store`。
       - 改为分别写 `gc_ptr`、`word`、`tag` 三个槽位。
       - `gc_ptr` 槽位通过 write barrier 发布。
     - 同步调整 `needs_write_barrier_for_value_ty(...)` 的说明：tagged-union enum 改为在 `store_local_value_exact` 中拆槽写回，不再走原先整值指针 store 的判定。

## 当前诊断现象

1. `task_step_cross_thread_sequential_handoff_basic`
   - 普通运行不再立即 `exit(3)`，但会挂住，随后 core dump。
   - 当前可见输出仅到：
     ```text
     outer-before
     main-pending
     ```
   - 含义：原先“并发 step trap”路径已被拆出，但 worker 接手后没有正确完成。

2. `task_step_manual_basic`
   - 在 `SCOOP_GC_STRESS=1` 或 `SCOOP_GC_MOVE=1 SCOOP_GC_STRESS=1` 下不会 crash，但会永远返回 `Pending`：
     ```text
     outer-before
     step0=pending
     step1=pending
     step2=pending
     ```
   - 含义：即便没有跨线程，第一次 `Pending` 之后 waiting-state 也会在 GC 压力下失效。
   - 结论：根因不止是 handoff 线程切换，很可能是 waiting payload / continuation graph 的 GC 可达性或发布合同出了问题。

3. `sysroot/task.scoop` 中临时加入了 trap 诊断退出码
   - 当前临时值：
     - claim acquire failure: `exit(31)`
     - `continuation.resume(...)` catch `RuntimeError`: `exit(32)`
     - claim 成功后仍观察到 `Running`: `exit(33)`
   - 在 `task_step_cross_thread_sequential_handoff_gc_stress` 下目前拿到的是 `31`，输出为：
     ```text
     outer-before

     main-pending
     gc-after-main
     ```
   - 含义：在 “main 首次 Pending -> GC -> worker 接手” 序列中，worker 的 claim 失败；要么 `__claim` 没回到 0，要么对象状态/对象本身在 GC 后损坏。
   - 但 `task_step_manual_basic` 的“永远 Pending”说明 waiting-state 内容丢失/损坏也仍然存在。

## 当前最可能的根因假设

1. `Waiting(awaited, continuation)` 的 boxed payload trace metadata 不完整
   - `__TaskState.Waiting` 是 boxed variant，payload 里有两个 GC ref：
     - `awaited: Task<(Int, Any)>`
     - `continuation: Continuation<(Int, Any), __TaskStepResult<T>>`
   - 如果 enum boxed payload object 的 type descriptor / bitmap 只追踪了其中一个字段，GC 后会导致：
     - continuation 丢失或损坏，表现为一直 `Pending`；
     - waiting payload 被部分破坏，连带影响 claim 或状态观察。

2. `Task.__state` 整体写回虽已拆槽，但相关 boxed payload object / descriptor 的 trace 路径仍有缺口
   - 也可能不是 heap store 本身，而是 boxed payload object 的 type desc 生成错误。

## 当前执行计划

1. 检查 enum boxed payload 的 type descriptor / trace bitmap 生成逻辑
   - 重点查看：
     - `get_or_create_enum_boxed_payload_type_desc_global(...)`
     - boxed payload object 的 bitmap 如何依据 variant fields 生成
     - `__TaskState.Waiting` / `__TaskStepResult.Pending` 这类双 ref payload 是否两个槽位都被标记

2. 增加最小化 GC 回归验证 boxed enum payload tracing
   - 设计一个不依赖 task 的最小 rich-enum GC stress case：
     - boxed enum variant 持有两个 refs
     - 存入 heap object 字段
     - 显式 GC 后再读取两个 refs
   - 如果该用例失败，可直接锁定问题在 generic enum boxed payload GC tracing，而不是 task 特例。

3. 若确认 descriptor/bitmap 存在缺陷，则修复并回归验证
   - 必跑：
     - `task_step_manual_basic` 在 `SCOOP_GC_STRESS=1`
     - `task_step_cross_thread_sequential_handoff_basic`
     - `task_step_cross_thread_sequential_handoff_gc_stress`
     - `task_waiting_handoff_atomic_no_mutex_llvm`

4. 清理临时诊断
   - 修复完成后，将 `sysroot/task.scoop` 中临时的 `31/32/33` 退出码恢复为正式 trap 合同所需值。

5. 完成任务收口
   - 更新 `TODO.md` / `PLAN.md`
   - 运行完整相关测试，确保无 warning
   - 提交一次清晰的 git commit

## 进行中状态

- 当前进入第 1 步：检查 enum boxed payload 的 descriptor / trace bitmap 生成逻辑，并准备最小复现用例。
- 在定位完成前，不更新 `TODO.md` / `PLAN.md` 的任务完成状态，也不提交。

## 最新进展（本轮后半段）

1. 已排除“boxed payload descriptor / trace bitmap 漏标第二个引用”这一假设
   - 直接检查 LLVM IR 可见：
     - `__TaskState.Waiting` 的 boxed payload type 为 `{ ptr addrspace(1), ptr addrspace(1) }`
     - 对应 type descriptor bitmap 为 `3`
     - `__TaskStepResult.Pending` 同样是双 GC 指针 payload，bitmap 也是 `3`
   - 结论：Waiting/Pending boxed payload 的 type descriptor 本身没有只追踪一个字段的问题。

2. 已确认并部分修正了“aggregate 值分配前后裸跨 safepoint”问题
   - 已做：
     - `DeferredCgValue` 从“只 spill 直接 GC 指针”扩展为可 spill 含嵌套 GC refs 的 aggregate。
     - enum boxed payload ctor 改成：先保活 field deferred roots，再 `scoop_alloc_typed`，然后用 relocate 后的值重建 payload。
     - `EffectValueBox` 分配改成走 `build_call_preserving_gc_local_roots(...)`，并在 alloc 后 reload 再写 payload。
   - 现状：
     - LLVM IR 已能看到 `__task_step_pending::<Int>` / `__task_restore_waiting::<Int>` 的 alloc safepoint 把双 ref payload roots 放进 `gc-live`。
     - `EffectValueBox` alloc 也开始保活/relocate `TaskStep` / `__TaskStepResult` 的 payload GC 指针。

3. 但任务仍未完成，因为又暴露出更底层的 blocker
   - 新的关键发现：
     - ordinary/statepoint call 仍会把“含 GC refs 的 by-value aggregate 实参”以旧的 SSA aggregate 形式直接传给 callee。
     - IR 中虽然已经能看到 aggregate 的 leaf GC refs 被单独 keepalive，但真正传给 ordinary helper 的 `TaskStep` / `__TaskStepResult` 实参并没有在 call 前用 relocate 后的叶子重建。
   - 这会导致：
     - `task_step_manual_basic` 在 `SCOOP_GC_STRESS=1` 和 `SCOOP_GC_MOVE=1 SCOOP_GC_STRESS=1` 下仍然在 `step0=pending` 后 `EXIT=139`。
     - `task_step_cross_thread_sequential_handoff_gc_stress` 仍然落在 claim failure（此前诊断为 `31`）路径，说明顺序 handoff 仍被更早的 aggregate transport 缺口破坏。
   - 结论：
     - 这不是 `T4016T8` 独有的 task 局部问题，而是更基础的 compiler contract 缺口：
       ordinary/statepoint call 还不能安全传递含 GC refs 的 by-value aggregate 值。

4. 因此本轮应按阻塞流程处理
   - 需要把这个 compiler blocker 作为前置任务插入 `TODO.md` 当前 `T4016T8` 之前。
   - `T4016T8` 本身保持 `[TODO]`，并改为依赖该新前置任务。
   - 本轮提交应以“登记 blocker、更新计划与依赖顺序”为收口，不把 `T4016T8` 标记为完成。
