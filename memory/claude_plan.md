# Claude Plan

## 当前接手状态

- 最新提交 `[T4008b1b] Add resumed-step boundary summaries` 已检查，暂未发现提交信息中额外声明且尚未处理的先修问题。
- 本轮只处理 `TODO.md` 中第一个未完成任务 `T4008b2`，完成后立即停止，不继续推进后续任务。
- 已知上一轮已经完成了 `T4008b2` 的主体实现，但在全量回归时仍有一个 LLVM effect state machine 相关回归需要收尾：
  - `llvm::codegen::effect::state_machine_emitter::tests::when_arm_try_resume_nested_handle_ir_keeps_binder_scope_for_inner_resume`
- 已知根因是把所有 `Continuation.resume` 都按会 outward suspend 的调用处理，破坏了 pure continuation 在嵌套 `try/catch` / `handle` 下应保留的 hidden runtime-raise 语义。
- 已有半完成修复方向：引入 “non-pure continuation resume call site side table”，仅对非 pure continuation 的 `resume` 走 replay 主线；pure continuation 的 `resume` 继续保留 runtime-raise 分类。

## 执行计划

1. 先检查工作树和相关源码，确认 side table 分流改动目前停在哪一步，避免重复实现或覆盖已有修改。
2. 补齐 AST / parser / typecheck / HIR / LLVM codegen / tests helper 之间的 `non_pure_continuation_resume_call_sites` 传递链路。
3. 修正并补齐相关测试断言，确保 pure 与 non-pure continuation resume 的分类行为都符合预期。
4. 先跑最小化测试集定位剩余问题：
   - `cargo test -p scoopc continuation_resume -- --nocapture`
   - `cargo test -p scoopc when_arm_try_resume_nested_handle_ir_keeps_binder_scope_for_inner_resume -- --nocapture`
   - `cargo run -q -p scoop -- test --fixtures target/t4008b2-fixtures/infer`
   - `cargo run -q -p scoop -- test --fixtures target/t4008b2-fixtures/run-pass`
5. 若最小化测试通过，再跑大范围验证：
   - `cargo run -q -p scoop -- test --fixtures tests/fixtures/typecheck`
   - `cargo test --all`
   - `cargo clippy --all-targets -- -D warnings`
6. 全部通过后，更新文档与任务状态：
   - `TODO.md`：将 `T4008b2` 标记为完成，并写清本轮交付内容。
   - `PLAN.md`：记录本轮完成情况与语义边界。
   - `memory/claude_plan.md`：补充已完成关键步骤、测试结果和最终结论。
7. 运行 `cargo fmt`，检查 `git status`，提交一个只覆盖本轮任务所需内容的 commit，commit message 使用 `[T4008b2] Re-type escape continuations from resumed-step summaries`。
8. 停止，不继续处理后续任务。

## 关键约束与判断标准

- 不接受 workaround、fixture-only hack 或规避 spec 的实现。
- 如发现当前任务仍依赖新的语言特性缺口或 spec mismatch，必须先把该缺口写入 `TODO.md` / `PLAN.md`，调整依赖顺序并提交后停止。
- 不回退用户已有改动；若遇到与当前任务直接冲突的外部修改，需要先确认影响再决定如何兼容。
- 所有输出、记录与后续说明保持中文。

## 已完成关键步骤

1. 已核对最新提交 `[T4008b1b] Add resumed-step boundary summaries`，未发现提交信息中仍未处理的额外先修问题。
2. 已确认 `TODO.md` 首个未完成任务确为 `T4008b2`；本轮不继续推进 `T4008c`。
3. 已补齐并确认 pure / non-pure `Continuation.resume` 分流链路：
   - AST / parser / typecheck / HIR / LLVM 均已贯通 `non_pure_continuation_resume_call_sites` side table。
   - pure continuation resume 继续保留 hidden `RuntimeRaise` 分类。
   - non-pure continuation resume 走 outward-suspending replay 主线。
4. 已确认 escape continuation binder 重新定型主线可用：
   - `check_file_exprs` 现为两阶段，先完成常规 expr typecheck，再基于 lowered HIR + resumed-step summary 回填 escape binder 的精确 `EffectRow`。
   - `, k ->` 不再默认注入 `Continuation<T, eff Pure>`，而是使用同一份 resumed-step summary 推得 `Continuation<T, eff E>`。
5. 已修完先前全量回归中的遗留问题：
   - `when_arm_try_resume_nested_handle_ir_keeps_binder_scope_for_inner_resume` 现已通过。
   - 额外为 `clippy::too_many_arguments` 将 suspendability side table 引用收成 `SuspendCallProgramFacts`，并同步修正 `state_machine_segments.rs` / `state_machine_transform.rs` 中的 test helper 初始化。

## 验证结果

- 已通过最小 Rust 单测：
  - `cargo test -p scoopc continuation_resume -- --nocapture`
  - `cargo test -p scoopc when_arm_try_resume_nested_handle_ir_keeps_binder_scope_for_inner_resume -- --nocapture`
- 已通过本任务定向 fixtures：
  - `cargo run -q -p scoop -- test --fixtures target/t4008b2-fixtures/infer` -> `fixtures: ok (2)`
  - `cargo run -q -p scoop -- test --fixtures target/t4008b2-fixtures/run-pass` -> `fixtures: ok (1)`
- 已通过相关全量验证：
  - `cargo run -q -p scoop -- test --fixtures tests/fixtures/typecheck` -> `fixtures: ok (331)`
  - `cargo test --all`
  - `cargo clippy --all-targets -- -D warnings`

## 最终结论

- `T4008b2` 已可标记完成。
- 下一次调用应从 `T4008c` 开始。
- 本轮剩余工作只有：
  - 检查 `git status`
  - 提交本轮相关文件
  - 停止
