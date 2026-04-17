# 执行计划

## 说明

按要求先记录执行计划与进度。这里不会写入逐字内部思维过程，而是记录可审计的执行步骤、判断依据摘要、风险点与阶段性结论，便于后续检查。

## 当前目标

本轮只完成 `TODO.md` 中第一个未完成任务；如果发现其前置依赖缺失、规范不匹配或任务过大，则先更新 `TODO.md` 与 `PLAN.md`，提交这些调整后停止。

## 执行步骤

1. 检查最近一次提交：
   - 查看最新 commit message 与改动。
   - 确认是否显式提到已知问题、临时修复、待补事项或回归。
   - 若存在“提交中已承认但未修复的问题”，优先修复这些问题，再继续当前任务流。
2. 读取计划与任务文件：
   - 打开 `TODO.md`，定位第一个未完成任务。
   - 打开 `PLAN.md`，核对现有分解与优先级是否一致。
3. 评估首个未完成任务：
   - 判断是否可以在本轮完整落地。
   - 若任务过大或依赖缺失，则把它拆成更小子任务，并更新 `PLAN.md`/`TODO.md`，本轮只执行拆分后的第一个子任务。
4. 实施：
   - 阅读相关模块、测试与规范。
   - 在不引入 workaround、shim、fixture-only hack 的前提下完成实现。
   - 若途中发现规范缺口或实现边界问题，先把缺口补成新的前置任务并调整顺序，然后停止。
5. 验证：
   - 运行与改动相关的测试。
   - 至少补跑必要的 `cargo fmt`、相关测试，以及在可行范围内执行 `cargo clippy --all-targets -- -D warnings`。
6. 文档与任务状态更新：
   - 更新 `memory/claude_plan.md` 记录关键进展、计划变更与结论。
   - 在 `TODO.md` 中把本轮完成的任务标记为完成。
   - 在 `PLAN.md` 中同步当前状态与后续项。
7. 提交：
   - 用清晰的提交信息提交本轮全部改动。
   - 完成一个任务后停止，不继续下一个任务。

## 初始检查重点

- 是否存在最近提交中提到但未解决的问题。
- 首个未完成任务是否依赖尚未实现的语言特性、运行时能力或标准库支持。
- 是否已有未提交改动会影响本轮工作；若有，必须避免覆盖用户改动。

## 进度记录

- 2026-04-17：已创建计划文件，下一步检查最新提交与任务列表。
- 2026-04-17：已检查 `git log -1 --stat`。最新提交 `[T3010b2b1] Close arm-body outward propagation validation` 的提交信息本身未声明新的遗留问题；后续仍需以当前测试结果为准确认是否存在更早未跟踪 blocker。
- 2026-04-17：已读取 `TODO.md` / `PLAN.md`，当前第一个未完成任务是 `T3010b2b`：基于 synthetic resume slot + immediate-resume lowering 接通可执行的 post-suspend continuation tail，禁止 resume 后重放原表达式。
- 2026-04-17：下一步将先复现 `T3010b2b` 当前的实际失败点，优先检查：
  - 全量 LLVM fixture 当前停在何处；
  - 是否仍是 `T3017` 记录的 stale `EXPECT: fail` 截断，还是 `T3010b2b` 已可由更小定向用例直接复现；
  - 该任务是否需要继续拆分成新的前置子任务。
- 2026-04-17：已完成 `T3010b2b` 的验证闭环：
  - `effect_resume_yield_int_basic.scoop`、`effect_resume_finally_normal.scoop`、`async_await_minimal_int_basic.scoop` 继续通过。
  - 扫描当前全部带 `EXPECT: fail` 的 run-pass fixture，未再出现 `member access target` / `comparison lhs|rhs` / `equality lhs` / `integer binary op lhs` 这些 `T3010b2b` 负责的 body-tail fragment 错误。
  - 将 4 条代表性 post-suspend tail xfail fixture 临时去掉 `EXPECT: fail` 头后按普通 run-pass 复验，`fixtures: ok (4)`，证明 resumed tail / nested escape handle / mixed source-path cleanup 已正常通过。
  - `cargo test --all` 与 `cargo clippy --all-targets -- -D warnings` 通过；`cargo run -p scoop --features llvm -- test` 的首个失败点为 `continuation_resume_continuation.scoop` 的 stale `EXPECT: fail`，该问题已由 `T3017` 跟踪。
- 2026-04-17：已同步 `TODO.md` / `PLAN.md`，将 `T3010b2b` 标记为完成，并把执行顺序推进到新的首个未完成任务 `T3010R`。
- 2026-04-17：下一步提交本轮结果并停止，等待下一轮继续处理 `T3010R`。
