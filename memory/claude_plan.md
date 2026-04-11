# 执行计划

## 目标

本轮只完成 `TODO.md` 中第一个未完成任务，然后停止。

## 约束与执行顺序

1. 先检查最新一次 Git 提交，确认是否提到了需要先修复的既有问题。
2. 读取 `TODO.md`，定位第一个未完成任务。
3. 读取 `PLAN.md`，确认该任务是否已有拆分计划或依赖说明。
4. 如果该任务过大或存在明确前置依赖：
   - 在 `PLAN.md` 中补充拆分方案或依赖说明；
   - 在 `TODO.md` 中将该任务替换或扩展为更小的子任务；
   - 本轮只执行拆分后的第一个子任务。
5. 实现目标任务。
6. 运行相关验证，至少覆盖：
   - `cargo fmt --check`
   - `cargo clippy --all-targets -- -D warnings`
   - 与改动直接相关的测试；若影响面较大，则运行更完整的测试集。
7. 更新文档与跟踪文件：
   - 在 `TODO.md` 中将本轮完成项标记为已完成；
   - 在 `PLAN.md` 中更新当前状态、后续顺序和必要说明；
   - 在本文件记录关键进展、计划变更和验证结果。
8. 使用清晰的 Git 提交信息提交本轮改动。
9. 停止，不继续处理下一个任务。

## 预期检查点

- 检查点 A：确认最新提交是否包含待先修问题。
- 检查点 B：确认首个未完成任务的具体内容与可执行性。
- 检查点 C：完成实现并通过格式化、lint 和测试。
- 检查点 D：更新 `TODO.md`、`PLAN.md`、`memory/claude_plan.md` 并提交。

## 进度记录

- 已创建初始执行计划。
- 已检查最新提交 `76f63c8f56bace797574cc4e20bb750cadbfd85f`（`[T2003c0b2b0] Fix nested handle result lowering`），提交信息未额外声明需先修复的遗留问题。
- 已读取 `TODO.md` 与 `PLAN.md`，当前第一个未完成任务为 `T2003c0b2b1`：支持 mixed-arm 场景下 immediate site 之后的多个 top-level direct escape sites。
- 已检查 `crates/scoopc/src/llvm/codegen/effect.rs`：
  - 现有 direct mixed-arm 路径在扫描阶段只允许一个 top-level direct escape site；
  - `T2003c0b2b0` 已提供“immediate-resume tail 中嵌套 handle 结果表达式”的 lowering 前置能力；
  - HIR 的 `Block` / `Stmt` / `Expr` / `HandleExpr` / `HandleArm` 都支持 `Clone`，可以构造合成的 tail handle。
- 当前实现方案：
  1. 保留外层 immediate-resume 状态机；
  2. 对“immediate site 之后的多个 top-level direct escape sites”新增专用 direct 路径；
  3. 在 `resume(...)` 后把 tail 合成为单-arm escape `handle`，复用既有 multi-perform escape-continuation lowering；
  4. 继续保留本阶段门禁：pre-immediate direct site、nested/control-flow direct site、direct+indirect 混合仍报稳定诊断；
  5. 补 run-pass fixture，并把旧 build-fail fixture 改成仍未支持的组合。
- 已完成首轮代码修改：
  - `effect.rs` 新增 mixed direct site 扫描 helper；
  - single-site direct 路径现会在检测到 post-immediate multiple direct sites 时切换到新的 nested-tail lowering；
  - 已新增 run-pass fixture `effect_resume_mixed_escape_direct_multiple_post_immediate`；
  - 已把旧 build-fail fixture 改为覆盖 pre-immediate direct site 的稳定诊断。
- 验证阶段发现更底层的前置缺口：
  - 计划复用的 single-arm escape-continuation multi-perform direct primitive 在 `Unit` 结果 handle 上可用；
  - 但最小 non-`Unit` 结果样例（single-arm + multiple direct sites）仍会在 LLVM codegen 报 `unknown local value` / `value coercion`；
  - 因此当前 mixed-arm `T2003c0b2b1` 不能按原计划直接落地。
- 已执行回退：
  - 撤回未完成的 `effect.rs` / fixture 代码改动；
  - 恢复仓库到“仅调整任务计划”的状态；
  - 在 `TODO.md` / `PLAN.md` 中新增前置任务 `T2003c0b2b0c`，并把 `T2003c0b2b1` 移到其后。
- 本轮结论：按用户约束，本次提交只记录依赖重排与原因说明，停止在这里，等待下次调用先处理 `T2003c0b2b0c`。
