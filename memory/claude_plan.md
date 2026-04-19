# 当前执行计划

更新时间：2026-04-19

说明：按要求先写入计划与执行步骤摘要。这里记录可审阅的分析结论、行动顺序、关键判断与进度更新，不包含不适合公开的完整内部推理原文。

## 任务目标

本次调用只完成 `TODO.md` 中第一个未完成任务，然后停止。若发现阻塞该任务的前置缺陷，则先把缺陷显式加入 `TODO.md` / `PLAN.md`，调整顺序，提交后停止。

## 执行顺序

1. 检查最新一次 git 提交信息，确认是否提到已知问题、遗留缺陷或必须先修复的事项。
2. 阅读 `TODO.md`，定位第一个未完成任务。
3. 阅读 `PLAN.md`，核对该任务上下文、依赖与现有分解。
4. 若任务过大，先细分为更小的子任务，并同步更新 `TODO.md` 与 `PLAN.md`；本次只做拆分后的第一个子任务。
5. 阅读实现与相关测试代码，确认当前行为、规范要求与可能的前置缺陷。
6. 实现该任务，必要时补充或重构测试。
7. 运行相关验证：
   - 最小相关测试
   - 受影响模块测试
   - 如改动范围允许，再运行更高层验证
   - 按要求检查 `cargo clippy --all-targets -- -D warnings`
8. 更新文档状态：
   - 在 `TODO.md` 标记当前任务完成
   - 在 `PLAN.md` 记录完成情况与后续调整
   - 在本文件记录关键进度
9. 查看 git diff，确认仅包含合理改动。
10. 提交 git commit，提交信息对应当前任务。
11. 停止，不继续下一个任务。

## 风险与处理原则

- 如果最新提交提到问题，则先修复这些问题，再进入 `TODO.md` 任务。
- 如果实现中发现规范不匹配、语言特性缺失、运行时缺陷或测试只能靠变通方案通过，则不能绕过，必须先把该问题加入 `TODO.md` 作为前置任务，并更新 `PLAN.md` 后提交停止。
- 不回退用户已有改动；若发现冲突性未预期修改，先判断是否影响当前任务，再决定是否继续。

## 进度记录

- 2026-04-19：已创建本计划文件，下一步开始检查最新提交与 `TODO.md`。
- 2026-04-19：已确认本次首个未完成任务为 `T4005R`（Elvis review）。
- 2026-04-19：复审与临时 probe 已覆盖 `safe-call + Elvis`、顶层初始化 Elvis、struct rhs Elvis；这些路径可执行。
- 2026-04-19：发现一条真实缺口：`val xs: Array<Int> = maybe ?: []` 当前会在 typecheck 阶段报 `array_lit_type_annotation_required`。原因是 Elvis rhs 仍按“无 expected type 的独立表达式”推断，没有把 lhs 的 nullable inner type 向 rhs 传播。
- 2026-04-19：计划调整为先修复 Elvis rhs expected-type 传播，再补回归（优先覆盖空数组 rhs 与 lambda rhs），随后重跑定向测试、全量测试与 clippy，再更新 `TODO.md` / `PLAN.md` 并提交。
- 2026-04-19：已修复 Elvis rhs expected-type 传播：rhs 现统一使用 lhs nullable inner type 做 expected-context typecheck。
- 2026-04-19：复审中继续发现第二条 Elvis 可执行裂缝：`noneThunk ?: { 7 }` 在降成 `when` 后，arm body 的 `Closure` 没有接入 expected-context codegen，LLVM 会报 `expression kind` unsupported；现已修复为 `codegen_expr_in_expected_context` 直接走闭包 codegen 主线。
- 2026-04-19：已新增回归 `tests/fixtures/run-pass/elvis_rhs_expected_context_basic.scoop`，覆盖 `noneArray ?: []` 与 `noneThunk ?: { 7 }` 两条先前失败路径。
- 2026-04-19：已完成验证：新增 Elvis run-pass、既有 Elvis run-pass、`safe-call + Elvis` 临时 probe、`cargo run -p scoop -- test --fixtures tests/fixtures/typecheck`、`cargo test --all`、`cargo clippy --all-targets -- -D warnings` 全部通过。
- 2026-04-19：扩展 probe 还暴露出一个与 Elvis 修复独立的既有 callable bug：pattern binder 承载函数值时，`Some(f) -> f()` 仍会在 LLVM 侧报 `call callee` unsupported。该问题已按流程登记进 `TODO.md` / `PLAN.md` 作为后续顺序任务，本次调用仍只完成 `T4005R` 并停止。
