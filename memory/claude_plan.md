# 本轮执行计划

说明：
- 这里记录可执行计划、关键判断摘要、进度更新与变更原因。
- 不记录逐字内部思维，但会完整记录足以审计本轮工作的步骤、结论与依据。

## 初始计划

1. 检查最新提交，确认是否提到需要先修复的遗留问题。
2. 阅读 `TODO.md`，定位第一个未完成任务。
3. 阅读 `PLAN.md` 与相关上下文，判断该任务是否需要拆分；如果需要，先更新 `PLAN.md` 和 `TODO.md`。
4. 实现当前要执行的单个任务。
5. 运行相关测试、格式化和 lint；若失败则先修复。
6. 更新 `TODO.md`、`PLAN.md` 和本文件，记录完成情况与任何计划调整。
7. 提交一次 git commit，然后停止，不继续下一个任务。

## 进度记录

- 已创建本文件并写入初始计划。
- 已检查最新提交 `8b81976071b904eff49d8d742078539308317231`：提交信息仅为 `Update plan`，未明确提及需要先修复的遗留问题，因此当前无需插入额外“提交遗留问题修复”步骤。
- 已阅读 `TODO.md` / `PLAN.md`：当前第一个未完成任务为 `T2003c0b2b`，主题是 LLVM mixed-arm handle dispatch 中 sibling escape-continuation 的 richer site matrix。
- 下一步将审计 mixed-arm immediate-resume + sibling escape-continuation 的现有 lowering、诊断分支与 fixtures，判断该任务是否可在本轮直接完成，或是否必须先拆成更小子任务并回写 `PLAN.md` / `TODO.md`。
- 已完成复杂度审计：`T2003c0b2b` 同时要求解决 post-immediate site matrix 与 pre-immediate escape site；后者需要 continuation step 在恢复后继续命中 sibling immediate-resume state machine，明显比前者复杂一层。
- 已将原任务拆成：
  - `T2003c0b2b1`：post-immediate multiple direct sites；
  - `T2003c0b2b2`：post-immediate indirect/direct+indirect site matrix；
  - `T2003c0b2b3`：pre-immediate top-level sites。
- 本轮实际执行目标已切换为 `T2003c0b2b1`。
- 实现 `T2003c0b2b1` 时尝试的复用方案：
  - 把 mixed-arm body 重写成“outer immediate-resume handle + inner single-arm escape-continuation handle tail”；
  - 先在编译器里做了实验性 HIR 重写；
  - 再用手写等价程序 `/tmp/debug_mixed_nested_manual.scoop` 对照验证。
- 关键结论：
  - 手写等价程序当前也会在 LLVM codegen 报 `scoop::llvm::unsupported_main_body: value coercion`；
  - 这说明缺的不是 mixed-arm 分流本身，而是“immediate-resume tail 中 nested handle 作为结果表达式继续 lowering”的前置能力。
- 决策：
  - 撤回实验性代码改动，不把未完成的 mixed-arm 方案留在工作区；
  - 在 `TODO.md` / `PLAN.md` 中新增前置任务 `T2003c0b2b0`；
  - 将 `T2003c0b2b1` 移到 `T2003c0b2b0` 之后，等待该前置能力补齐。
- 当前工作区状态：已确认只剩 `TODO.md`、`PLAN.md`、`memory/claude_plan.md` 三处计划性变更，可按“路障/依赖重排”提交并停止。
