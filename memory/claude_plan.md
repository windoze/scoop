## 本轮计划

1. 先记录本轮执行计划与约束，确保后续每个关键步骤完成后同步更新本文件。
2. 检查最新一条 Git 提交信息，确认是否提到了需要先修复的既有问题；若提到，则优先处理该问题。
3. 阅读 `TODO.md`，找出第一个未完成任务。
4. 视任务复杂度决定是否需要拆分：
   - 若任务可在本轮完整落地，则直接执行。
   - 若任务过大或被既有问题阻塞，则先更新 `PLAN.md` 与 `TODO.md`，把前置子任务放到当前任务之前，并只完成第一个子任务或前置整理工作。
5. 在实现前阅读必要代码，定位最小正确改动点，避免引入绕过方案。
6. 完成实现后运行相关测试，并补跑必要的质量检查；若发现任何既有缺陷、回归或规范不匹配，立即优先修复，或按要求写入 `TODO.md` 作为前置任务。
7. 更新 `TODO.md`、`PLAN.md` 与本文件，记录完成情况、阻塞关系与计划调整。
8. 按仓库提交风格创建一次 Git 提交，然后停止，不继续处理下一个任务。

## 说明

- 这里记录的是可审计的执行计划摘要，不包含内部推理细节。
- 若执行过程中计划变化、发现阻塞、完成关键步骤或调整任务顺序，会及时更新本文件。

## 进度更新

- 已检查最新提交 `c307aeacca57e7e0ef9710367586641fc8d61c78`（`[T5002aR] Record final progress state`），提交信息与变更内容未提到需要先修复的既有问题，因此无需在 `TODO.md` 既定顺序之外插入额外前置修复。
- 已阅读 `TODO.md` 与 `PLAN.md`，当前首个未完成任务为 `T5002b`：引入 managed `EffectCtx` / `EffectHandlerNode` 与显式 hidden effect ABI。
- 下一步将对照 `CONTINUATION_RUNTIME_REFACTOR.md` 与当前 codegen/runtime 实现，判断 `T5002b` 是否可在本轮完整落地；若发现实际存在新的前置缺口，会先把前置任务写回 `TODO.md` / `PLAN.md` 再停止。
- 已确认 `T5002b` 同时覆盖 ordinary call ABI、callee-resume、state-machine dispatch、handle 入口、arm self-inactive、outer redispatch 与 cross-thread resume，单轮直接完成风险过高；因此已改为四段子任务：direct-call token 显式化、剩余 ABI surface 扩展、managed ctx/node graph、derived ctx + redispatch/cross-thread。
- `T5002b1` 已完成：top-level outward-effect direct-call wrapper 现在显式接收 `incoming_resume_token_ref`；fresh direct call 会传 `null` token；wrapper 内会在 legacy call 前 `publish` token，在 `consume_current_effect_outcome(...)` 后清空 TLS token scratch。
- 已完成验证：`cargo test -p scoopc effect_contract_struct_types_are_registered_for_effect_codegen -- --nocapture`、`cargo test -p scoopc direct_call_with_real_outward_effect_uses_wrapper_and_explicit_outcome -- --nocapture`、`cargo clippy --all-targets -- -D warnings` 均通过。
- `cargo fmt --check` 暴露了仓库内若干与本轮无关的既有格式漂移（`crates/scoop_runtime/tests/continuation_cross_thread_handler_stack.rs`、`crates/scoop_runtime/tests/continuation_one_shot.rs`、`crates/scoopc/src/llvm/codegen/effect/state_machine_emitter.rs` 等）；当前未改动这些文件，也未在本轮提交中擅自整理它们。
