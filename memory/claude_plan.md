# 本轮执行计划

更新时间：2026-04-12

说明：
- 按要求先写入执行计划，再进行任何仓库检查或命令执行。
- 我不会记录逐字的内部推理细节，但会持续记录可审计的执行步骤、依据、决策和进展。

## 目标

本轮只完成 `TODO.md` 中第一个未完成任务；如果发现该任务被前置缺陷阻塞，则先把阻塞项整理进 `TODO.md` / `PLAN.md`，提交后停止。

## 执行步骤

1. 查看最新一次 Git 提交，确认是否提到了需要先处理的遗留问题。
2. 阅读 `TODO.md`，定位第一个未完成任务。
3. 阅读 `PLAN.md`、相关代码与测试，判断该任务是否足够明确且可在一轮内完整完成。
4. 如果任务过大或被前置问题阻塞：
   - 细化成更小子任务，更新 `PLAN.md` 与 `TODO.md`；
   - 或把发现的规范缺陷/缺失能力转成新的前置任务并调整顺序；
   - 提交这些规划性修改后停止。
5. 如果任务可执行：
   - 实现任务；
   - 运行相关测试、`cargo fmt`、`cargo clippy --all-targets -- -D warnings`，必要时补测；
   - 修复测试/告警问题直到通过。
6. 更新文档与任务状态：
   - 在 `TODO.md` 标记完成；
   - 在 `PLAN.md` 记录已完成内容和后续状态；
   - 在本文件中追加进展记录。
7. 提交本轮变更，提交信息应清晰描述本轮完成的任务。
8. 停止，不继续处理下一个任务。

## 进展记录

- 已创建本文件，尚未开始仓库检查。
- 已检查最新提交：提交信息仅为 `[T2003c0c2b1a] Materialize indirect escape arm binders`，未额外声明需要先修的遗留问题。
- 已读取 `TODO.md` / `PLAN.md`，当前首个未完成任务为 `T2003c0c2b1b`：single-arm indirect escape-continuation 的 callee tail-perform resume path。
- 已完成初步代码审计：
  - `scan_for_callee_suspend` 目前只识别 `val x = perform(...)` 形状；
  - top-level function / closure 的 suspendable resume path 目前只会重放 `perform` 之后的语句；
  - 若 `perform` 是 callee block 的尾表达式，现有实现没有建立对应的 suspend/resume 语义。
- 已用最小样例复现当前缺陷：
  - 样例：`fun fetch(): Int / (Ask) { val key: Int = 7; Ask.get(key) }`
  - 行为：程序打印 `body_start / arm / result / 99` 后在 `k.resume(...)` 崩溃，退出码 `139`。
  - 结论：tail-return indirect perform 没有正确准备/恢复 callee suspend state，属于真实后端缺口。
- 下一步：
  - 扩展 callee suspend 的预扫描与信息结构，识别 block 尾表达式 `perform`；
  - 同步修正 top-level function 与 closure 的 resume path，使 tail-perform 在恢复后直接产出函数返回值而不是走默认返回/空状态；
  - 补 run-pass 回归并跑格式化、测试、clippy。
- 已完成实现：
  - `scan_for_callee_suspend` 已扩展到 `val x = perform(...)` 之外的 tail-return 形状，包括 block 尾表达式与 `return perform(...)`。
  - top-level function / closure 的 callee resume path 已区分“恢复值绑定到局部”与“恢复值直接返回”两种模式。
  - closure expression-body（例如 `{ Ask.ask(key) }`）现会合成最小 block 并复用同一套 callee-suspend 扫描与 resume lowering。
- 已新增回归：
  - `tests/fixtures/run-pass/effect_escape_continuation_indirect_perform_tail_return_int.scoop`
  - `tests/fixtures/run-pass/effect_escape_continuation_indirect_perform_closure_tail_return_string.scoop`
- 已完成验收：
  - `cargo test --all`
  - `cargo run -p scoop --features llvm -- test`
  - `cargo clippy --workspace --all-targets -- -D warnings`
- 本轮任务状态：
  - `T2003c0c2b1b` 已完成。
  - 下一轮首个未完成任务将是 `T2003c0c2b2`。
