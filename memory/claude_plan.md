## 当前执行计划

1. 读取 `TODO.md`，按文件顺序识别第一个标题未带 `[DONE]` 的任务。
2. 检查该任务的要求、依赖和验证条件；必要时查看最近提交，确认是否有直接相关的未完成事项。
3. 基于任务内容定位相关代码、测试和文档，避免做开放式历史问题扫描。
4. 完整实现当前任务；如果遇到阻塞当前任务的缺失特性、规格不匹配或测试失败，优先修复，或在 `TODO.md` 中插入最小必要前置任务并停止。
5. 按要求运行格式化、lint、测试和 fixture 验证；发现未被计划覆盖的失败时修复或登记为正确顺序的任务。
6. 更新 `TODO.md`，将完成任务标题加上 `[DONE]` 并填写完成记录；仅在阶段级计划变化时更新 `PLAN.md`。
7. 提交本次任务相关全部变更，然后停止，不继续下一个任务。

## 当前状态

- 已读取 `TODO.md`，第一个未完成任务是 `P5-T01R：Review Phase A-C resolution`。
- 已读取 `TODO-5.md` 任务正文；该 review 依赖已完成的 `P5-T01a`，需要复核 Phase A-C 候选收集、visibility-before-applicability、no-applicable diagnostics，并确认未提前做 specificity / effect fallback。
- 当前工作区已有本次新增的 `memory/claude_plan.md`，另有未跟踪 `REFLECTION.md`，我不会修改或回退该未跟踪文件。
- 初步 review 发现仍有阻塞 P5-T02 的问题：普通调用对混合 function/constructor candidates 的消费不完整，多个 call path 在 Phase A-C 后提前做 specificity/default tie-break，部分 targeted fixtures 对 shadow/no-applicable/late extension 覆盖不足。下一步先核对代码并做最小修复。
- 已修改核心 Phase A-C 路径：移除普通/member/constructor/operator 调用中的提前 specificity/default tie-break；普通调用会把同一调用组中的 function 与 constructor candidates 一起做 applicability；where-bound member 调用点会写回完整 bound method candidates。
- 已补强 fixtures：增加 local shadow applicability 失败、function/constructor 混合候选、where-bound 多候选、cross-file ctor vararg；补齐 no-applicable 候选签名/rejection reason 断言；将本阶段不应通过的 most-specific 正例改为 ambiguity 负例。
- 验证已完成：`cargo fmt`、`cargo clippy --all-targets -- -D warnings`、`cargo test --all --all-targets`、`python3 tools/run_fixtures.py` 均通过；受影响 targeted fixtures 也已单独运行通过。
- 已将 `TODO.md` 与 `TODO-5.md` 中 `P5-T01R` 标记为 `[DONE]` 并填写完成记录；当前准备提交本次 review 变更。
