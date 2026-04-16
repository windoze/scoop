# 执行计划

## 当前阶段
- `T3008aR` 已完成审查、验证与文档同步；下一步是提交本轮变更并停止。

## 初始计划
1. 检查最新一次提交信息，确认是否提到任何既有问题；若有，优先修复。
2. 阅读 TODO.md，定位第一个未完成任务。
3. 阅读 PLAN.md，核对任务背景、依赖与现有计划。
4. 评估首个未完成任务的规模；若过大，则先拆分任务并更新 TODO.md / PLAN.md。
5. 实现当前要执行的任务。
6. 运行相关测试与必要的质量检查，包括至少与改动相关的测试以及 `cargo clippy --all-targets -- -D warnings`（若适用）。
7. 更新 memory/claude_plan.md、TODO.md、PLAN.md，记录进展并标记任务完成。
8. 使用清晰的提交信息提交本次变更，然后停止。

## 说明
- 该文件记录摘要化的推理、执行计划、关键决策与进度，不包含逐字内部思维。

## 当前判断
- 最新提交信息未在 commit message 中额外声明需先修的既有问题。
- `TODO.md` 中首个未完成项为 `T3008aR`，属于生产代码 review 任务，规模可直接执行，无需进一步拆分。
- 相邻任务 `T3009` 的依赖写成了不存在的 `T3008R`；本轮已将其修正为 `T3008aR`。

## 本轮执行重点
1. 审查 `crates/scoopc/src/llvm/codegen/effect/state_machine_emitter.rs`、`runtime_abi.rs`、`runtime_symbols.rs` 及相关调用点，确认不存在 raw-frame / verifier-hack 残留。
2. 定向检索 `malloc`、native `ptr` state、局部 bitcast 绕过地址空间、缺失 typed trace descriptor 等风险模式。
3. 若发现真实生产缺口，直接修复并复审；若未发现，则记录审查结论并更新 TODO/PLAN。
4. 运行与任务匹配的质量验证，再提交并停止。

## 审查结果
- 未发现 `T3008aR` 所针对的生产代码缺口；本轮无需修改 `crates/scoopc/src/llvm/codegen/**` 或 runtime 生产实现。
- `codegen_handle_expr_via_state_machine` 已稳定走 `scoop_alloc_typed` 分配 effect frame，并且只清零对象头之后的 payload，不存在 raw-frame `malloc` 残留。
- `emit_effect_step_function`、`declare_runtime_continuation_alloc`、`llvm_continuation_struct_type` 与 runtime `ScoopContinuation`/`scoop_continuation_trace` 在 `state` / `resume_gc_ref` 的 GC 语义上保持一致；编译器侧统一使用 `addrspace(1)`，runtime 侧显式 trace `state` 与 `resume_gc_ref`。
- effect frame type descriptor 通过 `get_or_create_effect_frame_type_desc_global` 调用通用 trace bitmap 生成逻辑；`--emit-llvm` 生成物中可见 `__scoop_type_desc_effect_frame__*__trace_bitmap` 与 `@scoop_continuation_alloc(ptr addrspace(1), ptr)`，说明不是靠 verifier hack 压过去。

## 已执行验证
1. `cargo check -p scoopc`
2. `cargo clippy --all-targets -- -D warnings`
3. `cargo test --all`
4. `cargo run -p scoop -- run tests/fixtures/run-pass/effect_multi_nonresuming_custom_indirect.scoop`
5. `cargo run -p scoop -- run tests/fixtures/run-pass/try_catch_raise_runtime_error_basic.scoop`
6. `cargo run -p scoop -- build tests/fixtures/run-pass/effect_multi_nonresuming_custom_indirect.scoop --emit-llvm -o /tmp/t3008a_effect.ll`

## 待完成收尾
1. 复查工作区差异，仅保留本轮文档更新与计划记录。
2. 提交本轮变更并停止。
