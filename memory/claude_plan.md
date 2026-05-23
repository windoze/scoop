# 执行计划

## 约束说明

- 我会记录可审计的计划、决策依据和执行进度，但不会写入私有逐字推理链。
- `TODO.md` 是任务排序和完成状态的唯一来源；只完成第一个未以 `[DONE]` 标记的任务。
- 如遇到阻塞当前任务的缺陷、缺失功能或未调度的测试失败，会先修复；无法立即修复时，将最小必要前置任务写入 `TODO.md` 并提交后停止。
- 除非阶段级计划确实变化，否则不更新 `PLAN.md`。

## 初始执行计划

1. 读取 `TODO.md`，确认第一个未完成任务的编号、内容、依赖和验证要求。
2. 查看最近提交，仅判断是否存在与该任务直接相关的未完成事项。
3. 检查工作区状态，避免覆盖用户或其他代理的改动。
4. 按任务要求阅读相关代码、测试和文档，确认实现边界。
5. 实施最小正确修改；若发现当前任务被具体前置缺陷阻塞，则按要求更新 `TODO.md` 并停止。
6. 运行相关测试；若发现未调度失败，修复或新增必要前置任务。
7. 更新 `TODO.md`：将完成任务标题前缀改为 `[DONE]`，补充完成记录。
8. 必要时更新本文件记录关键进展。
9. 检查差异并提交本次任务相关改动。
10. 提交后停止，不继续下一个任务。

## 当前状态

- 已读取 `TODO.md` 与 `TODO-6.md`。
- 第一个未完成任务是 `P7-T05R`：`Review P7 全包完成度`，任务详情位于 `TODO-6.md`。
- `P7-T05R` 要求复审 `P7-T05`：确认 LLVM backend 只消费 `LIR + LIR facts + base context`，没有 HIR/raw MIR/effect facts/stage output wrapper residual，并判断 P8 是否只剩最终验证和文档冻结。

## 本轮执行计划（P7-T05R）

1. 查看最近提交和工作区状态，只识别与 `P7-T05R` 直接相关的未完成事项或待提交改动。
2. 复审 `P7-T05` 已改动区域和 dependency gate，确认 review 范围内的边界检查已覆盖 LLVM stage handoff、emit handoff、reachability 与 backend 去虚化残余。
3. 执行 `P7-T05R` 要求的额外 residual 搜索，范围覆盖 `crates/scoopc/src/llvm` 与 `crates/scoopc/src/pipeline` 中上游 stage output、HIR、raw MIR、effect facts wrapper、ordinary dispatch devirtualization 等命中。
4. 重新运行 `P7-T05` 指定验证：`cargo fmt`、`cargo run -p scoop_tools -- dependency-gate`、`cargo test -p scoopc_lir_facts`、`cargo test -p scoopc --no-default-features llvm_codegen_stage`、`cargo run -p scoop -- test --fixtures tests/fixtures/run-pass`、`cargo clippy --all-targets -- -D warnings`、`git diff --check`。
5. 如发现未调度失败或阻塞 residual，优先修复；无法在本 review 内正确修复时，在 `TODO.md` / `TODO-6.md` 添加最小必要前置任务并停止。
6. 若 review 通过，更新 `TODO.md` 与 `TODO-6.md`：将 `P7-T05R` 标记为 `[DONE]`，填写完成记录与验证结果。
7. 检查最终 diff，提交本任务所有改动，然后停止，不进入 `P8-T01`。

## 本轮进展

- 已检查最近提交：最新提交是 `23c0d463 [P7-T05] Complete backend cleanup gate`。
- 工作区进入本轮时仅有本计划文件改动。
- `P7-T05R` residual 搜索发现阻塞项：`crates/scoopc/src/llvm/emit.rs` 仍把 `base_context.materialized_pass_view()` 传入 production `CompilationUnitCodegenInputs`，`crates/scoopc/src/llvm/codegen/mod.rs` 仍保存并暴露 `MaterializedMirPassView` accessor，`crates/scoopc/src/llvm/codegen/call/lowering.rs` 仍按 `MaterializedMirPassView -> HIR owner fun -> fun_index` 顺序回退获取 callable signature。
- 该 residual 使 `P7-T05R` 不能如实写出“P7 完成、P8 只剩最终验证/文档冻结”。
- 已按规则在 `TODO-6.md` 与 `TODO.md` 插入最小前置任务 `P7-T05-a`，并把 `P7-T05R` 依赖改为 `P7-T05-a`；本轮将提交该任务重排后停止。
