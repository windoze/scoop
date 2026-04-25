# 本轮执行计划

## 约束说明

- 本轮目标：只完成 `TODO.md` 中第一个未完成任务，然后停止。
- 必须先检查最新提交是否提到预存问题；若提到，先修复该问题，再处理计划任务。
- 若在探查、测试、实现过程中发现任何既有问题、规约不匹配、缺失能力或依赖缺口，必须优先修复；如果当前轮次无法直接修复，则需要先在 `TODO.md` 中插入前置任务、更新 `PLAN.md`，提交后停止。
- 在没有确认上下文之前，不假设任务规模、实现位置或测试范围。

## 当前已知执行思路

1. 读取最新一次 Git 提交信息，检查是否显式提到需要先修复的遗留问题。
2. 读取 `TODO.md`，定位第一个未完成任务。
3. 读取 `PLAN.md`，理解当前计划与任务编号、依赖关系、已有拆分情况。
4. 根据任务内容检查相关代码、测试与文档；必要时先运行最小范围验证，确认是否存在阻塞性的既有缺陷。
5. 若任务过大，则先把任务拆分为更小子任务，并同步更新 `TODO.md` 与 `PLAN.md`；本轮只执行拆分后的第一个子任务。
6. 实现当前目标任务。
7. 运行与改动直接相关的测试，再补充必要的全量或更大范围校验；同时运行格式化/静态检查，至少覆盖本次改动影响面，并尽量满足 `cargo clippy --all-targets -- -D warnings`。
8. 更新文档与计划状态：
   - 在 `TODO.md` 中把本轮完成任务标记为已完成。
   - 在 `PLAN.md` 中记录当前进展、后续依赖或拆分调整。
   - 若执行过程中计划发生变化，实时更新本文件。
9. 检查工作区改动，确保不误覆盖用户已有更改。
10. 使用清晰的 Git 提交信息提交本轮成果，然后停止。

## 风险与检查点

- 若最新提交只“提到”问题但未说明范围，需要结合相关文件与测试确认是否仍然存在。
- 若发现 `memory/`、`PLAN.md`、`TODO.md`、`README.md` 或测试基线本身存在不一致，需判断其是否构成当前任务的前置问题。
- 若任务涉及语言特性、编译链路、运行时或标准库边界，必须验证真实链路，不接受仅靠夹具或局部绕过的“完成”。

## 进度记录

- 已完成：检查最新提交 `d7bfb1ab [T5000c3bR] Review shared concrete-type helper direction`；提交正文未额外提到需要优先修复的遗留问题。
- 已完成：定位本轮首个未完成任务为 `T5000c3R Review：确认共享分析消费者已脱离 LLVM backend 源文件依赖`。
- 已完成：审计 shared analysis / consumer 的依赖方向。
  - `crates/scoopc/src/lib.rs` 只在 `#[cfg(not(feature = "llvm"))]` 下暴露 `effect_step_summary`；
  - `crates/scoopc/src/effect_step_summary.rs` 当前直接 `include!("effect_state_machine_analysis.rs")`；
  - `crates/scoopc/src/llvm/codegen/effect/state_machine_plan.rs` 只保留 backend 薄包装；
  - `crates/scoopc/src/program_facts.rs`、`crates/scoopc/src/effect_analysis.rs`、`crates/scoopc/src/expr_facts.rs` 已覆盖 planning 与 direct-step summary 的共同输入，且不依赖 `crate::llvm` / inkwell；
  - `crates/scoopc/src/effect_state_machine_analysis.rs` 中剩余 `MainCodegen` 相关入口全部位于 `#[cfg(feature = "llvm")]` 下，仅作为 backend 调用 shared analysis 的接缝。
- 已完成验证：
  - `cargo check -p scoopc --lib`
  - `cargo fmt --all --check`
  - `cargo test -p scoopc llvm::tests::lowered_call_results_keep_concrete_types_for_local_bindings`
  - `cargo test -p scoopc --no-default-features direct_step_effect_rows_include_direct_effectful_call_after_escape_site`
  - `cargo test -p scoopc --no-default-features`
  - `cargo test --all`
  - `cargo clippy --all-targets -- -D warnings`
- 已完成：回写 `TODO.md` / `PLAN.md`，已将 `T5000c3R` 标记为完成，并把下一条待执行任务切换为 `T5000cR`。
- 当前结论：未发现必须插入到 `T5000cR` 之前的新前置缺陷任务；下一步只剩提交本轮结果并停止。
