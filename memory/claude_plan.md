# 当前执行计划

## 范围

- 以 `TODO.md` 为唯一任务顺序来源，找出第一个标题未带 `[DONE]` 的任务。
- 本次只完成这一个任务；完成后更新记录、提交 Git，然后停止。

## 步骤

1. 读取 `TODO.md`，确认第一个未完成任务及其依赖、验证要求和完成记录要求。
2. 检查最近提交信息是否明确提到与该任务直接相关的未完成问题；若存在，将其纳入当前任务或作为前置任务记录到 `TODO.md`。
3. 按任务要求定位相关代码、文档和测试，优先采用最小正确修改。
4. 实现任务；如发现阻塞当前任务的语言特性缺口、规格不匹配或测试失败，优先修复，或在 `TODO.md` 中添加最小前置任务并停止。
5. 运行任务指定测试和相关验证；若观察到未被明确排期的失败，修复或排期后再决定是否完成当前任务。
6. 将当前任务标题加上 `[DONE]`，更新其完成记录；仅在阶段级计划变化时更新 `PLAN.md`。
7. 检查工作区差异，提交所有与本次任务相关的变更，提交信息使用任务编号和简短说明。
8. 停止，不继续执行后续任务。

## 当前状态

- 已确认第一个未完成任务：`P7-T04`（`TODO-6.md`）。
- 最近提交为 `[P7-T04-cR] Review physical ABI layout migration`，未显示直接相关的未完成事项。
- 本任务范围：综合验证 `P7-T04-b` 与 `P7-T04-c` 后的 LLVM stage handoff / physical ABI cleanup，并记录 `TypeId` cross-process stable wire format 推迟到 P8/per-cone artifact serialization 的结论。

## P7-T04 执行计划

1. 搜索文档、LIR facts dump 和 pipeline/codegen 中关于 stage handoff、physical layout、TypeStore owner、wire-format 推迟的现有记录，确认缺口。
2. 仅做必要的文档或 dump 文本同步；不改动已由 `P7-T04-b/-c` 完成的实现路径，除非验证发现真实回归。
3. 运行 `P7-T04` 指定验证：`cargo fmt`、相关 `cargo test`、`dependency-gate`、run-pass fixtures、clippy、`git diff --check`。
4. 若验证失败且不是已有明确排期问题，修复后重跑相关验证；若出现无法在本任务内正确修复的阻塞，按要求更新 `TODO.md` 前置任务并停止。
5. 验证通过后，将 `P7-T04` 在 `TODO.md` 与 `TODO-6.md` 标记为 `[DONE]` 并填写完成记录。
6. 检查 diff/status，提交本任务相关变更并停止。

## 进度记录

- 已完成文档/dump 缺口确认：LIR stable dump 已包含 `wire_format=deferred wire_owner=P8 per-cone build artifact serialization`；主要缺口是 README 与 pipeline 文档仍描述为 P7 前 residual。
- 已同步 `README.md`、`PIPELINE-CLEANUP.md`、`PIPELINE_REFACTOR.md`，把 P7-T04 handoff/physical ABI 合并验证基线与 TypeId wire-format 推迟 owner 记录到文档。
- P7-T04 指定验证已通过：`cargo fmt`；`cargo test -p scoopc --no-default-features llvm_codegen_stage`；`cargo test -p scoopc --no-default-features llvm::codegen::effect_lowered::layout`；`cargo run -p scoop_tools -- dependency-gate`；`cargo run -p scoop -- test --fixtures tests/fixtures/run-pass`（421/421 passed）；`cargo clippy --all-targets -- -D warnings`；`git diff --check`。
- 已更新 `TODO.md` 与 `TODO-6.md`：`P7-T04` 标记为 `[DONE]`，并填写合并验证、文档/dump 同步、wire-format 推迟决策、residual 搜索和验证记录。
- 已提交本任务变更：`[P7-T04] Complete LLVM handoff verification`。
- 本次 invocation 到此停止，不继续执行 `P7-T04R`。
