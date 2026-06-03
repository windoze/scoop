# 执行计划

## 当前目标

- 按 `TODO.md` 的顺序识别第一个标题未带 `[DONE]` 的任务。
- 完成且只完成该任务；如遇到阻塞当前任务的缺口，则按要求把最小前置任务写入 `TODO.md` 并停止。

## 步骤

1. 读取 `TODO.md`，定位第一个未完成任务，并检查该任务的依赖、验证要求和完成记录。
2. 检查最近提交是否明确提到与该任务直接相关的未完成问题；只处理会阻塞当前任务的问题。
3. 阅读当前任务相关代码、测试、fixture 和文档，确认需要修改的最小范围。
4. 实现任务要求，避免 workaround、fixture-only hack 或偏离规格的替代方案。
5. 按要求运行格式化、lint、相关测试，并在需要时运行完整测试和 fixture 套件。
6. 将任务标题在 `TODO.md` 中标记为 `[DONE]`，更新完成记录；仅在阶段计划变化时更新 `PLAN.md`。
7. 检查 git diff/status，提交本次任务相关全部变更，然后停止。

## 进度记录

- 已创建初始执行计划，下一步读取 `TODO.md` 定位首个未完成任务。
- 已读取 `TODO.md` / `TODO-3.md`，当前首个未完成任务为 `T3-04R：Review T3-04`，依赖 `T3-04L`。下一步检查最近提交并审查 T3-04 fact-only/fail-fast/gate 残余。
- 最近提交为 `[T3-04L] Close twelfth fact-only gaps`，未在标题中声明新的未完成问题。当前工作区除本计划文件外有未跟踪 `FACT_REFACTOR.md`，先视为非本次变更。下一步进行 targeted review。
- Targeted review 已确认 `T3-04L` 后仍有阻塞 `T3-04R` 的事实自包含缺口，包括 P6 path/span 与 root scan fallback、MIR/LIR fact 合成、dispatch/value-box 文本恢复、verifier/gate 漏锁。已在 `TODO-3.md` 插入最小前置任务 `T3-04M`，并把 `T3-04R` 依赖更新为 `T3-04M`。
- 本次只修改任务记录和计划记录，未改编译产物；完整 fmt/clippy/test/fixture 验证按文档-only 变更跳过。已运行 `git diff --check -- TODO.md TODO-3.md memory/claude_plan.md` 通过。
