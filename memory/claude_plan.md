# 当前执行计划

## 约束

- 以 `TODO.md` 为任务顺序与完成状态来源；只处理第一个标题未带 `[DONE]` 的任务。
- 当前任务是 review 任务，不能跳过；若发现阻塞项，应在本 review 内修复，或按规则插入最小前置任务并停止。
- 不做开放式历史问题扫查；搜索范围限定为 `P7-T05R` 要求的 LLVM/backend stage residual、任务指定验证，以及验证暴露的未排期失败。
- 不能接受 workaround、fixture-only hack、silent fallback 或与 P7 边界不一致的实现。
- 完成时必须同步更新 `TODO.md` 与 `TODO-6.md`，将 `P7-T05R` 标题标记为 `[DONE]` 并填写完成记录；仅当阶段计划变化时才更新 `PLAN.md`。
- 完成后提交本轮所有相关更改并停止，不继续 `P8-T01`。

## 当前任务

- 第一个未完成任务：`P7-T05R：Review P7 全包完成度`。
- 任务位置：`TODO.md` 索引第 143 行；`TODO-6.md` 第 1344 行起。
- 任务目标：确认 LLVM backend 只消费 `LIR + LIR facts + base context`，无 HIR/raw MIR/effect facts/stage output wrapper residual，并给出 P7 完成、P8 可开始的 review 结论；若发现阻塞项则在本 review 内修复。
- 任务验证：重新运行 `P7-T05` 的验证，并额外搜索 `crates/scoopc/src/llvm` 与 `crates/scoopc/src/pipeline` 中的上游 stage output、HIR、MIR residual。

## 执行步骤

1. 查看 `git status` 与最近提交，确认是否有直接关联 `P7-T05R` 的未完成问题，并记录当前工作树状态。
2. 读取 `P7-T05`、`P7-T05-a`、`P7-T05-b-0`、`P7-T05-b` 完成记录和相关 dependency gate 规则，明确 review 检查清单。
3. 对 `crates/scoopc/src/llvm`、`crates/scoopc/src/pipeline` 和 `tools/scoop_tools/src/dependency_gate.rs` 做 targeted residual 搜索，重点检查 `EffectLoweredStageOutput`、`EffectFactsStageOutput`、`LoweredHir`、`HirFacts`、`MaterializedMirPassView`、`materialized_pass_view`、`source_signatures`、`fun_index`、HIR/raw MIR body fallback、backend-local dispatch devirtualization、class ctor HIR body lowering。
4. 对搜索命中逐项分类：合法测试/helper/LIR-owned source payload/base-context 窄合同，或生产 residual。若是生产 residual，直接修复并补 gate/test；若是明确更大 blocker，按规则插入前置任务并停止。
5. 运行任务要求验证：`cargo fmt`、`cargo run -p scoop_tools -- dependency-gate`、`cargo test -p scoopc_lir_facts`、`cargo test -p scoopc --no-default-features llvm_codegen_stage`、`cargo run -p scoop -- test --fixtures tests/fixtures/run-pass`、`cargo clippy --all-targets -- -D warnings`、`git diff --check`；如 review 修复触及 `llvm::codegen`，补跑对应目标测试。
6. 若验证或 residual 搜索发现未排期失败，修复后重跑对应范围；不带失败标记完成。
7. 更新 `TODO.md` 与 `TODO-6.md`：将 `P7-T05R` 标题标为 `[DONE]`，写入 review 结论、residual 分类、验证结果和是否允许进入 P8。
8. 更新本文件记录关键进度；检查 `git status`、`git diff`、`git log --oneline -10`，提交本轮相关更改。

## 进度记录

- 已读取 `TODO.md`，第一个未完成任务确认为 `P7-T05R`。
- 已读取 `TODO-6.md` 中 `P7-T05R` 的任务体；本轮不会推进 `P8-T01`。
- 已查看最近提交；`[P7-T05-b] Clear HIR callable residuals` 是当前 review 的直接前置完成提交，未声明新的未完成 blocker。
- 静态 residual review 发现 `P7-T05R` 不能直接完成：LLVM production codegen 仍接收/保存 `fun_index`、`HirFacts`、HIR-derived `callable_signatures` fallback，并且 `LlvmStageBaseContext` 仍持有完整 `MaterializedMir` / `MaterializedEffectFacts` wrapper；dependency gate 也未覆盖这些形态。
- 已按阻塞处理规则新增最小前置任务 `P7-T05-c`，并将 `P7-T05R` 依赖更新为 `P7-T05-c`；`P7-T05R` 保持 `[TODO]`。
- 已运行 `cargo run -p scoop_tools -- dependency-gate`，当前 gate 通过；这同时确认新增任务中记录的 gate 覆盖不足是待修复内容，而不是现有 gate 已能拦截的失败。
- 已运行 `git diff --check`，无 whitespace 错误。
- 下一步：检查最终 git 状态/diff/最近提交，提交本轮任务重排与 blocker 记录后停止。
