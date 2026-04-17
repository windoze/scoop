# 2026-04-18 当前轮执行计划

## 目标
完成 `TODO.md` 中当前首个未完成任务 `T3009b2b`，完成后立即停止，不进入下一个任务。

## 已知上下文
- 上一轮已完成该任务的核心实现，修复 ordinary indirect callee 在 effect suspend / resume 场景下的 resumed-body replay 问题。
- 代码改动已覆盖 LLVM codegen、effect state machine、runtime TLS publish，以及相关 run-pass fixtures。
- 目前已知 `cargo check -p scoopc` 与 `cargo test --all` 通过，但还需要补齐本轮要求的最终验收、文档更新与提交。

## 执行步骤
1. 检查最近一次提交信息，确认是否提到任何需要先修复的 pre-existing issue；若存在，优先处理。
2. 确认 `TODO.md` 中首个未完成任务仍为 `T3009b2b`，并核对 `PLAN.md` 当前状态，避免与现有计划冲突。
3. 运行剩余必须的验证命令：
   - `cargo clippy --all-targets -- -D warnings`
   - 按任务验收方式运行相关 LLVM fixture。
4. 若验证失败，直接修复问题并重新验证；若验证通过，更新文档：
   - `TODO.md` 将 `T3009b2b` 标记为完成，并记录必要说明。
   - `PLAN.md` 记录本轮完成情况与后续边界。
   - `memory/claude_plan.md` 追加进度与结果。
5. 检查工作区差异，确认只包含本任务相关修改。
6. 使用明确提交信息创建 git commit，例如 `[T3009b2b] Restore ordinary indirect callee resumed-body replay`。
7. 停止，不继续处理后续任务。

## 注意事项
- 不采用 workaround；若发现新的 spec mismatch，必须先更新 `TODO.md` / `PLAN.md` 后再停止。
- 不回退未授权的现有改动。
- 对外记录仅写可审阅计划、进度与结论，不记录不可审阅的内部推理细节。

## 进度更新
- 已检查最近一次提交 `f533a1e [T3009b2aR] Seal callee suspend TLS ABI bypass`，未发现需要在本轮任务前插队修复的额外 pre-existing issue。
- 已确认 `TODO.md` 当前首个未完成任务仍为 `T3009b2b`，`PLAN.md` 当前执行顺序也与此一致。
- `cargo clippy --all-targets -- -D warnings` 初次执行时失败，暴露 `crates/scoopc/src/llvm/codegen/effect/state_machine_emitter.rs` 中 `emit_resume_after_call_site` helper 参数过多的真实 lint 问题。
- 已通过删除未使用的 `_state` 参数收紧该 helper 形状；行为未改动。
- 重新执行 `cargo clippy --all-targets -- -D warnings` 后已通过。
- 已按 `TODO.md` 验收命令通过 4 条关键 LLVM fixture：
  - `effect_escape_continuation_indirect_perform_basic.scoop`
  - `effect_escape_continuation_indirect_perform_closure_locals.scoop`
  - `effect_escape_continuation_indirect_perform_resume_string.scoop`
  - `effect_escape_continuation_indirect_perform_resume_struct_with_ref.scoop`
- `cargo test --all` 已重新通过，确认本轮对 ordinary indirect callee resumed-body restore 的实现与最后的 lint 修正未引入回归。
- 已更新 `TODO.md` 将 `T3009b2b` 标记为完成，并更新 `PLAN.md` 记录本轮完成情况；下一项待执行任务应为 `T3009b2bR`。
- `cargo fmt` 与 `cargo fmt --check` 已通过，确认当前工作区格式对齐。
- 下一步只剩检查工作区、创建提交 `[T3009b2b] Restore ordinary indirect callee resumed-body replay`，然后停止。
