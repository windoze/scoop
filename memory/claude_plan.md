# 本轮执行计划

## 说明

根据要求，我会在开始实际检查和修改前，先把本轮的执行计划写入此文件，并在关键步骤完成或计划发生变化时持续更新。

出于协作与安全边界考虑，这里记录的是可公开的执行思路摘要、步骤计划、判断依据和当前状态，不包含逐字逐句的内部推理草稿。

## 总目标

只完成 `TODO.md` 中当前排在最前面的一个未完成任务，然后停止。

## 初始步骤

1. 检查最新一次 Git 提交的信息，确认是否提到了尚未修复的既有问题。
2. 阅读 `TODO.md`，定位第一个未完成任务。
3. 阅读 `PLAN.md`，了解现有计划与任务上下文。
4. 如果第一个未完成任务过大或存在明确前置依赖，则把任务拆分为更小的子任务，并同步更新 `PLAN.md` 与 `TODO.md`。
5. 只执行当前排在最前面的那个可实施任务。

## 执行标准

1. 实现任务，不接受规避方案或偏离规范的临时做法。
2. 运行相关测试，并在必要时补充测试。
3. 运行质量检查，至少覆盖：
   - `cargo test --all`
   - `cargo clippy --all-targets -- -D warnings`
   - 如任务影响格式或文档结构，再运行相应检查
4. 更新任务文档：
   - 在 `TODO.md` 中标记该任务完成，或在阻塞时重排任务顺序并保留为待办
   - 在 `PLAN.md` 中记录当前状态与后续依赖
   - 在本文件中记录关键进展
5. 提交 Git commit，然后停止，不继续做下一个任务。

## 阻塞处理规则

如果当前任务因缺失功能、规范不匹配、已有缺陷或错误依赖而无法按规范完成：

1. 不把任务标记为完成。
2. 在 `TODO.md` 中新增或拆分真正的前置修复任务，并把它们移动到当前任务之前。
3. 更新 `PLAN.md`，写明阻塞原因、依赖关系和调整后的顺序。
4. 更新本文件记录本轮判断结果。
5. 提交这些计划调整，然后停止。

## 当前状态

- 状态：已完成本轮仓库检查、任务定位、生产代码复审与验证；当前任务可收口并提交。
- 已完成：
  - 已检查最新提交 `6900c7ef1602c0755ff2cdd15ca3bf2848aaedaf`，未发现需要在本轮先行插队修复、且未被现有任务跟踪的新问题。
  - 已定位本轮首个未完成任务为 `T3013R`，并确认其属于 review 任务，不需要再拆分。
  - 已复审 `crates/scoopc/src/llvm/codegen/effect/mod.rs`、`crates/scoopc/src/llvm/codegen/effect/state_machine_emitter.rs`、`crates/scoopc/src/llvm/codegen/runtime_abi.rs` 与 `runtime/c/scoop_runtime.c`。
  - 已确认 `perform` binder、handle result、resume payload 三条链路统一复用同一套 transport helper，composite 值通过 typed GC box + `resume_gc_ref`/`gc_ref` 传递，未发现 `ptr <-> int` 编码回流。
  - 已确认 continuation 与 effect frame 的 GC trace 合同覆盖新的 composite transport 槽位。
  - 已确认 handle 入口只注册首个 op-tag 的旧问题仍存在，但该问题已由现有 `T3014` 跟踪，不属于本轮 `T3013R` transport review 的新增缺口。
- 已完成验证：
  - `cargo run -p scoop -- run tests/fixtures/run-pass/handle_compound_result.scoop`
  - `cargo run -p scoop -- run tests/fixtures/run-pass/effect_nonresuming_payload_struct_indirect.scoop`
  - `cargo run -p scoop -- run tests/fixtures/run-pass/continuation_resume_continuation.scoop`
  - `cargo run -p scoop -- run tests/fixtures/run-pass/continuation_resume_struct.scoop`
  - `cargo run -p scoop -- run tests/fixtures/run-pass/continuation_resume_tuple.scoop`
  - `cargo run -p scoop -- run tests/fixtures/run-pass/continuation_resume_struct_with_ref.scoop`
  - `cargo test -p scoopc async_await_ir_preserves_continuation_slot_and_perform_payload -- --nocapture`
  - `cargo test --all`
  - `cargo clippy --all-targets -- -D warnings`
  - `cargo run -p scoop --features llvm -- test`：仍只停在已跟踪的 stale `EXPECT: fail` `effect_escape_continuation_indirect_perform_closure_tail_return_string.scoop`（`T3017`）
- 已更新：
  - 已在 `TODO.md` 中将 `T3013R` 标记为完成，并补充复审进展、验证记录与审查结论。
  - 已在 `PLAN.md` 中记录本轮 `T3013R` 复审摘要，并将 effect 主线下一项推进到 `T3009b`。
- 下一步：
  - 提交本轮变更并停止。
