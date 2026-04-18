# 当前执行计划

更新时间：2026-04-18

说明：按要求先记录执行计划。这里提供的是可审计的计划与关键判断摘要，不包含逐字内部推理。

## 目标

本轮只完成 `TODO.md` 中第一个未完成任务；如果发现其前置缺陷、规范不匹配或最新提交提到的遗留问题，先修复这些问题，再继续当前任务；完成后更新文档并提交一次 git commit，然后停止。

## 执行步骤

1. 检查最新一次 git commit 的提交信息与改动摘要，确认是否明确提到已知问题、TODO、FIXME 或需要先处理的遗留缺陷。
2. 检查当前工作树状态，避免覆盖用户现有未提交修改；如果存在无关改动，则在不回退它们的前提下继续工作。
3. 阅读 `TODO.md`，定位第一个未完成任务。
4. 阅读 `PLAN.md`，核对该任务的上下文、依赖与当前阶段计划。
5. 判断该任务是否可在本轮完整落地：
   - 若可直接完成，进入实现。
   - 若过大或前置能力缺失，则把任务拆成更小子任务，更新 `PLAN.md` 与 `TODO.md`，本轮只执行拆分后的第一个子任务；如果只是依赖缺失导致无法正确实现，则把缺失项前置到 `TODO.md` 并停止。
6. 实现本轮目标，保持改动模块化，不引入规避性实现或仅为测试通过的 hack。
7. 运行相关验证：
   - 最小相关测试；
   - 必要时运行更大范围测试；
   - 运行 `cargo fmt`；
   - 运行 `cargo clippy --all-targets -- -D warnings`；
   - 若任务影响广泛，再运行 `cargo test --all` 或足够覆盖本次改动的测试集。
8. 若测试暴露规范不匹配或遗留 bug：
   - 先修复能在本轮解决的问题；
   - 若无法在本轮直接完成且需要新增前置任务，则按要求更新 `TODO.md` / `PLAN.md`，提交后停止。
9. 完成后更新：
   - 在 `TODO.md` 标记本轮任务完成；
   - 在 `PLAN.md` 记录状态变化、剩余风险与后续顺序；
   - 如执行过程中计划有调整，实时回写本文件。
10. 使用清晰的提交信息创建 git commit，然后停止，不继续下一个任务。

## 当前已知约束

- 不回退或覆盖非本轮引入的现有修改。
- 不使用规避规范的临时方案。
- 如果发现缺失语言特性或规范实现边界不足，必须先把该问题转化为前置任务并调整任务顺序。
- 本轮结束前需要给出实际验证结果；若某项验证无法运行，需要明确记录原因。

## 待执行检查项

- [x] 查看最新提交
- [x] 查看工作树状态
- [x] 读取 `TODO.md`
- [x] 读取 `PLAN.md`
- [x] 确定本轮目标
- [x] 实现与测试
- [x] 更新 `TODO.md` / `PLAN.md` / 本文件
- [ ] git commit

## 当前进展

- 已检查最新提交 `4f51cede`，提交信息为 `[T3016k] Add trace-hook blocker before T3017`；其中记录的遗留问题与当前首个未完成任务一致。
- 已确认当前工作树除本文件外无额外未提交改动。
- 已定位 `TODO.md` 中第一个未完成任务为 `T3016k`：恢复 unified non-resuming effect 的 trace hook line/col 合同。
- 已阅读 `PLAN.md` 中相应阻塞说明，并确认当前失败现象为 `effect_raise_trace_hook_basic.scoop` 输出 `0/0`，根因是 codegen 仅调用 `scoop_effect_set_active()`，未走 `scoop_effect_set_active_with_trace(...)`。
- 已初步定位到需要修改的生产面：
  - `crates/scoopc/src/llvm/codegen/runtime_symbols.rs`
  - `crates/scoopc/src/llvm/codegen/runtime_abi.rs`
  - `crates/scoopc/src/llvm/codegen/effect/mod.rs`
  - `crates/scoopc/src/llvm/codegen/effect/state_machine_emitter.rs`
- 已完成生产实现：
  - 恢复 `scoop_effect_set_active_with_trace(...)` 的 runtime symbol 与 LLVM ABI 声明。
  - 在 `effect/mod.rs` 中新增统一的 span → `(line, col)` 映射 helper，并让 `codegen_perform_expr()` / `emit_raise_runtime_error_variant()` 统一通过它发出 traceful activation。
  - 在 `state_machine_emitter.rs` 中修正 `UnifiedStateTerminator::Suspend`：direct `Perform` site 发布 traceful activation；已经由 callee 或内层 boundary 激活的 active-path 不再重复 set-active 覆盖 trace。
  - 新增 2 条 emitter IR 回归，分别锁定 direct perform trace hook 与 outer suspend 保留 callee trace。
- 已完成验证：
  - `cargo test -p scoopc direct_perform_suspend_ir_uses_traceful_activation_hook -- --nocapture`
  - `cargo test -p scoopc outer_suspend_does_not_reset_callee_trace_hook -- --nocapture`
  - `cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/effect_raise_trace_hook_basic.scoop`
  - `cargo test --all`
  - `cargo clippy --all-targets -- -D warnings`
- 已完成文档同步：
  - `TODO.md` 已将 `T3016k` 标记为完成，并记录实现与验收结果。
  - `PLAN.md` 已新增本轮完成记录，并将当前执行顺序推进到 `T3016kR`。
- 下一步：创建 git commit，并在提交后停止本轮执行。
