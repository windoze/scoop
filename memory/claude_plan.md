## 当前执行计划

1. 检查最新一次 Git 提交说明，确认是否提到了需要先修复的既有问题。
2. 阅读 `TODO.md` 与 `PLAN.md`，定位第一个未完成任务，并确认是否需要先拆分为更小的子任务。
3. 基于仓库当前状态更新本文件，记录实际执行步骤、阻塞项与任务依赖。
4. 实现当前应执行的首个任务或首个子任务，避免引入规避式实现。
5. 运行与该任务直接相关的测试、格式化、lint 与必要的回归检查；若发现既有问题，优先修复。
6. 更新 `TODO.md` 与 `PLAN.md`，反映任务完成情况或新的前置依赖。
7. 按仓库约定创建一次 Git 提交，然后停止，不继续处理下一个任务。

## 说明

- 这里只记录可公开的执行计划与进度，不记录内部推理细节。
- 如果在检查、实现或测试过程中发现既有缺陷、规格不匹配或缺失能力，会先更新本文件，再优先修复或把它加入 `TODO.md` 作为前置任务。

## 当前任务确认（2026-04-30）

- 最新提交说明与 `TODO.md`/`PLAN.md` 一致：上一轮已修复 `T5001f2`，并在继续跑 `cargo run -p scoop -- test` 时暴露出新的既有阻塞 `T5001f3`。
- 本轮需要完成的首个未完成任务是：`T5001f3 修复 effect escape continuation GC-stress golden regression，解除 run-pass 后续阻塞`。

## 本轮具体步骤

1. 复现 `tests/fixtures/run-pass/effect_escape_continuation_gc_stress_multi_string.scoop` 的失败，并确认实际输出与 golden 的差异。
2. 检查与该路径直接相关的 lowering / runtime 实现，重点关注 escaped continuation、effect transport、resume payload 与 GC-stress 下的 source-of-truth。
3. 进行最小正确修复，不通过修改 golden 或收窄 fixture 来规避问题。
4. 为该问题补充或更新最小定向回归，锁定 multi-string payload 在 escape/resume 后的输出顺序和值。
5. 运行定向测试，以及必要的库测试 / fixture 测试 / lint；若过程中暴露新的既有 blocker，则先更新 `TODO.md`、`PLAN.md` 与本文件。
6. 若 `T5001f3` 完成，则更新 `TODO.md` 与 `PLAN.md`，创建一次 Git 提交，然后停止。

## 当前进展

- 已检查最新提交 `6a646a16 [T5001f2] Fix ctor-inline this root sync`，其说明与 `TODO.md`/`PLAN.md` 记录一致，没有额外需要先插队修复的问题。
- 已先验证上一轮未提交补丁的大方向：`effect/state_machine_emitter.rs`、`gc.rs` 与 `mod.rs` 把 effect frame 指针、heap-frame slot GEP 和 safepoint spill write-back 收口为可 reload 的 source-of-truth，避免 state-machine / escaped continuation 路径继续依赖跨 safepoint 的旧 SSA 指针。
- 在按 fixture 环境 `SCOOP_GC_STRESS=1` 复现后，确认 `T5001f3` 的直接根因是另一处既有缺口：fresh `Continuation.resume(...)` lowering 会先把 continuation receiver 读成 `%load_ref`，再分配 `String` payload；GC stress 触发搬迁后，`scoop_continuation_resume_with(...)` 仍拿到 stale continuation 指针，因此第一次 resume 就被错误地当成 `ContinuationAlreadyResumed`。
- 已完成修复：`crates/scoopc/src/llvm/codegen/effect/mod.rs` 现会先把 continuation receiver 通过 `defer_gc_sensitive_cg_value(...)` spill 成 tracked root，再在 payload materialize 完成后经 `continuation_resume_receiver_reload` reload，最后才调用 `scoop_continuation_resume_with(...)`。
- 已新增 LLVM 回归 `continuation_resume_reloads_receiver_after_gc_sensitive_payload_materialization`，锁定“GC-sensitive payload 分配后必须 reload continuation receiver，再进入 runtime resume helper”的 IR 顺序。
- 已完成验证：
  - `cargo test -p scoopc continuation_resume_reloads_receiver_after_gc_sensitive_payload_materialization -- --nocapture`
  - `cargo test -p scoopc when_arm_try_resume_nested_handle_ir_keeps_binder_scope_for_inner_resume -- --nocapture`
  - `SCOOP_GC_STRESS=1 cargo run -p scoop -- run tests/fixtures/run-pass/effect_escape_continuation_gc_stress_multi_string.scoop`
  - `cargo clippy -p scoopc --all-targets -- -D warnings`
- 继续执行 `cargo run -p scoop -- test` 时，suite 已越过 `effect_escape_continuation_gc_stress_multi_string.scoop`，当前新的既有 blocker 已切换为 `tests/fixtures/run-pass/gc_cross_function_class_object_graph.scoop`（仅在 `SCOOP_GC_STRESS=1` 下触发，默认环境单跑可通过）。按 `PROMPT.md`，本轮将把该问题前置为新的 TODO 任务，更新计划后提交并停止。
