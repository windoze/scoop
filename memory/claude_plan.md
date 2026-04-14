# Claude Execution Plan

## 当前目标

- 本轮只处理 `TODO.md` 中第一个未完成任务 `T2003r3d3d`。
- 任务范围限定为 unified `1 immediate + 1 escape` mixed leaf 在 `post-immediate` current legal source-path / site matrix 的 LLVM codegen 支持。
- 不扩展到 `pre-immediate escape site`；这类能力已有后续独立任务承接。

## 已知前置结论

- 已检查最新提交 `05ec6b4 [T2003r3d3c] Unify multi-escape indirect callee-suspend matrix`，提交信息未声明必须先修的额外遗留问题。
- 当前工作树包含一轮未完成实现，主要集中在 `crates/scoopc/src/llvm/codegen/effect/multi_resuming_mixed.rs`。
- 已新增若干 helper，用于把 mixed leaf 的 immediate 恢复后 tail/replay 接到 unified helper，而不是走旧的 shape-based 特判路线。
- step trampoline 已部分切到 `scanned_sites: Vec<MultiResumingEscapeSitePlan<'hir>>` 驱动，但主路径仍残留旧的单一 `escape_site` / top-level direct-only 假设。

## 本轮执行计划

1. 检查当前工作树、`TODO.md`、`PLAN.md` 和 `multi_resuming_mixed.rs`，确认实际未完成点仍与接手摘要一致。
2. 完成 `multi_resuming_mixed.rs` 主路径改造：
   - 移除 `state1` 后残留的单 site direct-only 流程。
   - 让 immediate 恢复后的后续 replay/tail 统一走 `codegen_multi_resuming_mixed_continue_after_immediate_site(...)`。
   - 将主路径 escape arm、binder 绑定、continuation materialization 与 step trampoline 对齐为 `scanned_sites` 驱动。
   - 保留现有稳定诊断，尤其是 `pre-immediate` site 的 reject 行为。
3. 若实现过程中发现规范缺口或真实前置 bug：
   - 停止继续实现。
   - 在 `TODO.md` / `PLAN.md` 中补充前置任务与依赖说明。
   - 更新本文件并按要求提交后停止。
4. 为该任务补最小但足够的回归覆盖：
   - 在 `state_machine_plan_tests.rs` 增加定向 unified codegen 测试。
   - 在 `tests/fixtures/run-pass/` 增加一个 representative fixture，覆盖 nested immediate + post-immediate direct/indirect + source-path matrix。
5. 运行验证：
   - 先做针对性编译/测试，必要时修复。
   - 通过后运行 `cargo clippy --workspace --all-targets -- -D warnings`。
6. 完成文档与状态更新：
   - 在 `TODO.md` 标记 `T2003r3d3d` 完成。
   - 在 `PLAN.md` 记录完成情况与下一步上下文。
   - 视关键进展更新本文件。
7. 提交本轮变更并停止，不继续处理下一个任务。

## 当前风险点

- `multi_resuming_mixed.rs` 可能仍有旧变量引用，如 `escape_site`、`resolved_escape_site`、`escape_init`、`escape_op`，需要清理并对齐新结构。
- 新增 helper 可能存在未使用参数、签名不一致或控制流接线错误，需通过编译与测试暴露后修复。
- 如果 unified mixed 主路径 contract 与 `multi_resuming_heap.rs` 的 helper 预期不完全一致，可能需要先修正接口或新增前置任务。

## 进度记录

- 2026-04-14：接手前一轮未完成实现，重写本计划文件，准备开始核对代码与补齐主路径。
- 2026-04-14：已完成 `multi_resuming_mixed.rs` 主路径收口。`state1` 不再手写单一 top-level direct escape site；post-immediate tail/replay、direct/indirect continuation materialization 与 escape arm dispatch 现统一复用 `scanned_sites + MultiResumingEscapeSitePlan` 和 heap continuation helper。
- 2026-04-14：已把 mixed leaf 的 `state0` immediate 进入点切到 `codegen_immediate_resume_prefix_to_site(...)`，并新增 per-site escape arm entry/body/unwind blocks，保持 detach same-handle handler-stack 后再执行 arm body 的既有语义。
- 2026-04-14：已新增 LLVM 定向单测 `unified_multi_resuming_codegen_emits_single_immediate_single_escape_source_path_matrix_sample` 与 representative fixture `effect_resume_mixed_source_path_matrix`；`cargo fmt --all`、`cargo test -p scoopc llvm::codegen::effect::tests:: -- --nocapture`、`cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/effect_resume_mixed_source_path_matrix.scoop`、`cargo clippy --workspace --all-targets -- -D warnings` 全部通过。
