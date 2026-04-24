# 本轮执行计划（T4016T8）

## 当前任务

- `TODO.md` 的第一个未完成任务仍是 `T4016T8`，本轮目标是完成它后立即停止。
- 最新提交 `2de651e1 [T4016T7a] Fix GC-safe aggregate ordinary-call transport` 没有额外点名必须先修的旧问题。
- 但在执行 `T4016T8` 的验证过程中，已经确认存在一个真实的既有实现缺口：`thread.join()` 对应 safepoint 没有把调用后仍活跃的 GC locals 全部纳入 `gc-live`，导致跨线程 moving GC 后 caller frame 保留 stale 指针。这个问题属于当前任务范围，必须先修。

## 已确认事实

- runtime 侧已经修过一个真实 bug：`scoop_thread_spawn()` 在 child 线程完成 runtime 注册前过早 `unpin` closure env；对应修复和测试 helper 已经落地。
- 新增 fixture `tests/fixtures/runtime_gc/task_step_cross_thread_sequential_handoff_gc_stress.scoop` 后，开启 moving GC 与 root verification 时仍能稳定复现 stale root。
- 通过 stackmap / objdump / IR 交叉定位，当前真正失败点在 `main` 的 stackmap record 212，位置在 `worker.join()` 返回后、下一次 `scoop_gc_collect_safepoint()` 之后。
- 对应 LLVM IR 中，`worker.join()` 的 statepoint 只携带了 `worker`，没有携带 `inner` / `outer` 这类“调用后仍会继续使用”的 GC locals：

```llvm
%load_ref103 = load ptr addrspace(1), ptr %worker, align 8
%statepoint_token240 = call token ... @scoop_thread_join ... [ "gc-live"(ptr addrspace(1) %load_ref103) ]
```

- 但 `join` 返回后程序仍会从本地 root slot 重新读取 `inner` / `outer`：

```llvm
%gc_root_keepalive_23104 = load ptr addrspace(1), ptr %inner, align 8
%gc_root_keepalive_29105 = load ptr addrspace(1), ptr %outer, align 8
%gc_root_keepalive_34106 = load ptr addrspace(1), ptr %worker, align 8
%statepoint_token241 = call token ... @scoop_gc_collect_safepoint ... [ "gc-live"(...) ]
```

- 结论：`join()` 期间 worker 线程触发 moving GC 时，caller frame 上的 `inner` / `outer` local root slot 没被前一个 statepoint 更新；`join()` 返回后 reload 出旧地址，最终在 verifier 中暴露为 stale roots。

## 本轮执行策略

1. 检查 LLVM codegen 中 native/safepoint call 的 keepalive 组装逻辑，重点看：
   - `crates/scoopc/src/llvm/codegen/gc.rs`
   - `crates/scoopc/src/llvm/codegen/mod.rs`
2. 修复 `thread.join()` / 类似 safepoint call 的 lowering，使“调用后仍活跃、且保存在真实 local root slot 中的 GC locals”一并进入 `gc-live`，不能只保留直接参数。
3. 重新生成 IR，确认 `statepoint_token240` 之类的 `gc-live` 已包含 `inner` / `outer` / `worker`。
4. 用 fixture 和运行时开关验证：
   - `SCOOP_GC_MOVE=1`
   - `SCOOP_GC_MOVE=1 SCOOP_GC_STRESS=1`
   - `SCOOP_GC_MOVE=1 SCOOP_GC_STRESS=1 SCOOP_GC_VERIFY_ROOTS=1`
5. 运行相关测试，再跑全量质量门：
   - `cargo run -p scoop -- test --fixtures tests/fixtures/runtime_gc`
   - `cargo test -p scoopc --features llvm task_step_ir_uses_seqcst_atomic_claim_and_trap_without_mutex -- --nocapture`
   - `cargo test --all`
   - `cargo clippy --all-targets -- -D warnings`
6. 如果修复过程中发现更底层 blocker，必须先把 blocker 写入 `TODO.md` / `PLAN.md` 作为前置任务，提交后停止，不能绕过。
7. 若修复完成，更新 `memory/claude_plan.md`、`TODO.md`、`PLAN.md`，提交一次并停止。

## 当前已知风险

- `runtime/c/scoop_gc_backend_immix.c` 里加入了临时诊断开关 `SCOOP_GC_TRACE_STACKMAP_VISITS=1`；最终需要判断是否删除或收敛，避免把临时调试噪音带进最终提交。
- safepoint keepalive 修复可能影响其他 native call lowering，需要至少检查相关 LLVM 单测和 runtime_gc fixture，防止引入新的 stackmap / relocation 回归。

## 进度更新

- 已确认 compiler 侧的直接缺口不只 `thread.join()` 一处：`thread.spawn`、`thread.join`、`thread.sleepMillis`、`thread.yield` 这四条线程 runtime 调用路径都还在直接 `builder.build_call(...)`，没有经过 `build_call_preserving_gc_local_roots(...)`。
- runtime 语义已经核对完毕：
  - `thread.spawn` 内部调用 `scoop_alloc(...)` 分配 `Thread` 对象，本身就是 safepoint；
  - `thread.yield` 显式执行 `scoop_gc_safepoint_poll()`；
  - `thread.join` / `thread.sleepMillis` 会进入 native 并阻塞，期间其他线程可触发 moving GC；
  - 因此这四条 lowering 都必须保活并写回当前 frame 的 GC locals，不能继续当作普通 leaf call。
- 已在 `crates/scoopc/src/llvm/codegen/mod.rs` 中把上述四条线程调用改为统一走 `build_call_preserving_gc_local_roots(...)`。
- `thread.join()` 的 pass 后 IR 已确认修复生效：statepoint 现在显式保留 `inner / outer / worker` keepalive，并在返回后把 relocated 值写回三个 local root 槽位。
- 已新增 LLVM 单测，直接对 `tests/fixtures/runtime_gc/task_step_cross_thread_sequential_handoff_gc_stress.scoop` 的 pass 后 IR 断言上述 `thread.join()` keepalive 形状。
- `runtime/c/scoop_gc_backend_immix.c` 中这轮为定位而加的 `SCOOP_GC_TRACE_STACKMAP_VISITS` / `SCOOP_GC_TRACE_UPDATE_SLOTS` 临时诊断已全部删除，避免把纯调试噪音带入最终提交。
- 在执行全量 `cargo run -p scoop -- test` 时，又暴露出一个既有 fixtures runner 缺口：`--fixtures tests/fixtures/run_pass_cone` 与 `--fixtures tests/fixtures/run_pass_cone/<case>` 过去会误把 case 名当成 phase 名，无法走 cone-package runner。该问题已一并修复：
  - `crates/scoop/src/fixtures/mod.rs` 现在能识别 `run_pass_cone` 根目录与单 case 目录输入；
  - 已补 runner 单测；
  - `cargo run -p scoop -- test --fixtures tests/fixtures/run_pass_cone` 已复验通过。
- 当前状态：
  1. 关键功能修复、LLVM 单测、`runtime_gc` 子集、`run_pass_cone` 子集、`cargo test --all`、`cargo clippy --all-targets -- -D warnings` 都已通过；
  2. 重新执行的全量 `cargo run -p scoop -- test` 也已通过，最终计数为 `fixtures: ok (1169)`；
  3. `TODO.md` / `PLAN.md` 已更新为 `T4016T8` 完成；
  4. 剩余工作只剩检查工作树、提交一次，然后停止。
