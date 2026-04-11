# 本轮执行计划

## 说明

按要求，先记录本轮的高层执行计划、关键决策依据和后续进展。这里不写内部隐藏推理，只保留可审计的执行步骤、判断结果和变更记录。

## 初始计划

1. 检查最新一条 Git 提交信息，确认是否提到任何已知问题、遗留缺陷或需要先处理的事项。
2. 阅读 `TODO.md`，识别第一个未完成任务。
3. 评估该任务是否可以在本轮完整完成：
   - 如果可完成，直接实现。
   - 如果过大或存在前置依赖，则拆分为更小子任务，并同步更新 `PLAN.md` 与 `TODO.md`，随后执行拆出的第一个子任务。
4. 在实现过程中持续更新本文件，记录：
   - 当前选中的任务
   - 是否发现前置问题
   - 计划是否调整
   - 关键实现步骤是否完成
   - 测试与验证结果
5. 完成当前任务后：
   - 更新 `TODO.md` 的任务状态
   - 更新 `PLAN.md`
   - 运行必要测试、格式化、lint
   - 提交 Git commit
6. 完成一个任务后立即停止，不继续处理后续任务。

## 进展记录

- 已创建本计划文件。
- 已检查最新提交：
  - `HEAD` 为 `7b952d4 [T2002b] Generalize escape continuation payload ABI`。
  - 提交正文未额外提到必须先修的遗留问题；未发现“先于 TODO 执行”的显式 pre-existing issue。
- 已读取 `TODO.md` / `PLAN.md`：
  - 原始首个未完成任务是 `T2003`（immediate-resume / `finally` / 控制流组合语义回归）。
- 已完成范围审计：
  - 当前 immediate-resume codegen 仍只支持单个顶层 `val x = perform(...)`。
  - `crates/scoopc/src/llvm/codegen/effect.rs` 里的 `codegen_handle_expr_immediate_resume` 会直接拒绝 `handle.finally`。
  - escape continuation 路径已经具备更完整的控制流扫描与 `finally` 模式，可作为后续扩展参考。
- 计划调整：
  - 将原 `T2003` 拆为 `T2003a` / `T2003b` / `T2003c`，并同步更新 `TODO.md` / `PLAN.md`。
  - 本轮实际执行目标切换为 `T2003a`：补齐单-perform immediate-resume 的 `finally` cleanup 语义。
