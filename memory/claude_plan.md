# 执行计划

1. 检查最新一次 git 提交信息，确认是否提到需要先修复的既有问题；若有，优先处理。
2. 阅读 `TODO.md` 与 `PLAN.md`，确定第一个未完成任务，并判断是否需要拆分。
3. 如任务可直接完成，定位相关代码与测试，实施最小正确修改；如被既有问题阻塞，则先把前置修复任务插入 `TODO.md`/`PLAN.md`。
4. 运行与该任务相关的测试，以及必要的格式化、lint、编译检查；修复发现的问题。
5. 更新 `memory/claude_plan.md`、`TODO.md`、`PLAN.md` 记录进展。
6. 按仓库提交规范创建一次 git commit，然后停止，不继续下一个任务。

## 当前执行

- 当前首个未完成任务：`T5001d3R Review`。
- 本轮 review 将重点核对三件事：
  1. 所有跨 safepoint roots 是否都已经拥有稳定 explicit frame home slot；
  2. `extra_gc_root_slots` / root-slot-id 旧机制是否仍以别名形式残留；
  3. effect / continuation / state-machine / resume 路径是否与 ordinary lowering 使用同一 home-slot 合同。
- 若 review 发现真实 correctness 缺口，本轮优先修复该缺口；若无法在本轮直接修复，则先把前置任务插入 `TODO.md`/`PLAN.md` 后停止。

## 当前发现

- 最新提交 `[T5001d3]` 未在提交信息中留下额外待修问题，可继续执行当前 review。
- 已复核 `TODO.md` / `PLAN.md`：当前首个未完成任务仍是 `T5001d3R Review`，不需要再拆分子任务。
- 已确认一处真实 correctness 缺口：`crates/scoopc/src/llvm/codegen/effect/mod.rs` 的 `resume_continuation_with_encoded_payload(...)` 此前直接发射 `scoop_continuation_resume_with` runtime call，没有经过 `build_call_preserving_gc_local_roots(...)`，导致 `Continuation.resume` / replay / state-machine tail-resume 路径未统一走与 ordinary safepoint 相同的 explicit-frame home-slot keepalive 合同。
- 已保留并继续整理中的修复：让 `resume_continuation_with_encoded_payload(...)` 接收 `span` 并统一走 `build_call_preserving_gc_local_roots(...)`；调用点已同步补传 `span`。
- 正在补一条 LLVM 回归，锁定 `Continuation.resume` 运行时调用窗口必须出现 GC keepalive、explicit frame home-slot 参与以及调用后 write-back 痕迹。

## 已完成

- 已完成代码修复：`resume_continuation_with_encoded_payload(...)` 现统一通过 `build_call_preserving_gc_local_roots(...)` 发射 runtime call，`Continuation.resume` fresh/replay/tail-resume 调用点均已补传 `span`。
- 已完成 LLVM 回归：`state_machine_multi_payload_perform_uses_tuple_transport` 现额外锁定 `Continuation.resume` 调用窗口存在 `gc_root_keepalive_*`、`explicit_gc_root_slot_*` 与调用后 write-back 痕迹。
- 已完成验证：
  - `cargo test -p scoopc state_machine_multi_payload_perform_uses_tuple_transport`
  - `cargo test -p scoopc when_arm_try_resume_nested_handle_ir_keeps_binder_scope_for_inner_resume`
  - `cargo test -p scoopc --lib`
  - `cargo clippy -p scoopc --all-targets -- -D warnings`
- 下一步仅剩按仓库规范检查差异并创建本轮提交，然后停止。

## 说明

出于安全与协作边界，我不会记录详细内部推理，但会在此文件持续记录可执行计划、关键发现、阻塞原因与完成状态。
