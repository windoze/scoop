# 当前执行计划

说明：按安全与协作要求，这里记录可审计的执行摘要、判断依据与步骤计划，不展开内部逐词推理。

## 目标

本轮只完成 `TODO.md` 中第一个未完成任务；如果发现其前置依赖缺失、规范不匹配或最新提交已指出的遗留问题，则先修复这些阻塞项，并同步更新 `TODO.md`、`PLAN.md` 与本文件。

## 初始步骤

1. 检查最新一次 Git 提交的提交信息与改动，确认是否明确提到已知问题、遗留 bug 或待修复事项。
2. 阅读 `TODO.md`，识别第一个未完成任务。
3. 阅读 `PLAN.md`，理解当前任务顺序、约束与上下文。
4. 如首个未完成任务过大或存在隐藏依赖，先把任务拆解为更小子任务，并更新 `PLAN.md`/`TODO.md` 后再执行最前面的子任务。

## 执行策略

1. 先定位相关代码、测试、规范或夹具。
2. 确认实现边界，避免用临时绕过方案掩盖真实缺陷。
3. 完整实现当前任务。
4. 运行相关测试；必要时补充或调整测试，直到相关测试稳定通过。
5. 运行质量检查，至少覆盖与改动相关的构建、测试与 lint；若成本可接受，优先执行全量检查。
6. 更新 `TODO.md`、`PLAN.md`，记录完成状态或依赖调整。
7. 提交 Git commit，然后停止。

## 阻塞处理规则

1. 如果发现任务依赖尚未实现的语言特性、运行时能力或规范修复，不做规避实现。
2. 将阻塞项作为更前置的任务写入 `TODO.md`，调整顺序与依赖描述。
3. 在 `PLAN.md` 与本文件记录阻塞原因、已核实的信息与下一步。
4. 提交这些计划性调整后停止，等待下一轮执行。

## 进度记录

- 已完成：创建本计划文件。
- 已完成：检查最新提交、`TODO.md`、`PLAN.md`。最新提交信息未直接声明新的遗留 bug；首个未完成任务原为 `T2003c0c2`。
- 已完成：审计 `codegen_handle_expr_multi_arm`。确认它同时硬编码了三类门禁：`multiple immediate-resume arms`、`multiple escape-continuation arms`、以及 `multi-arm without immediate-resume`，单轮整体推进风险过高。
- 已完成：把原 `T2003c0c2` 拆分为 `T2003c0c2a`～`T2003c0c2d`，并同步更新 `TODO.md` / `PLAN.md`。
- 已完成：实现 `T2003c0c2a`。`codegen_handle_expr_multi_arm` 现已在“无 immediate-resume / 无 escape-continuation”的 multi-arm 组合下分流到新的 pure non-resuming lowering。
- 已完成：新增 run-pass 回归 `effect_multi_nonresuming_custom_indirect` 与 `effect_multi_nonresuming_raise_custom_finally`，分别覆盖 multiple custom + indirect dispatch，以及 Raise + custom + finally + sibling handler scope 外执行。
- 已完成：验收命令通过：
  - `cargo fmt --all`
  - `cargo test --all`
  - `cargo run -p scoop -- test`
  - `cargo run -p scoop --features llvm -- test`
  - `cargo clippy --workspace --all-targets -- -D warnings`
- 进行中：更新任务文档并准备提交。

## T2003c0c2a 实施计划

1. 阅读单 arm non-resuming lowering 与 `immediate + sibling non-resuming` dispatch，提炼 pure non-resuming multi-arm 可复用的 push/pop、dispatch trampoline、sibling detach/restore 与 `finally` 流程。
2. 在 LLVM codegen 中新增无 immediate-resume 的 pure non-resuming multi-arm lowering，并调整 `codegen_handle_expr_multi_arm` 入口分流。
3. 新增最小 run-pass 回归：
   - multiple custom non-resuming；
   - `Raise.raise` + custom non-resuming；
   - 至少一例 indirect perform 命中 multi-arm。
4. 运行相关测试与 lint，修复问题。
5. 更新 `TODO.md` / `PLAN.md` / 本文件中的完成记录并提交。

## 结果摘要

- 本轮任务：完成 `T2003c0c2a`。
- 下一轮起点：`T2003c0c2b`。
