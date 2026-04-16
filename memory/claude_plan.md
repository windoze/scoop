# 本次执行计划（可审计摘要）

## 约束说明

- 按仓库与用户要求，本次只处理 `TODO.md` 中第一个未完成任务，完成后停止。
- 在执行过程中，我会持续更新本文件，记录阶段进展、计划调整、阻塞原因与已完成步骤。
- 出于安全与策略边界，这里记录的是完整的可执行计划、判断依据摘要与结果，不包含逐字内部思维链路。

## 总体步骤

1. 检查最新一次 Git 提交，确认提交信息是否提到既有问题、已知缺陷或待修复事项。
2. 如果最新提交暴露出“必须先修复的既有问题”，先定位并修复这些问题，再进入 `TODO.md` 的任务执行。
3. 读取 `TODO.md`，定位第一个未完成任务。
4. 评估该任务是否足够小且可在一次提交中完整实现。
5. 如果任务过大或存在明确前置依赖：
   - 拆分为更小的子任务；
   - 更新 `PLAN.md`；
   - 更新 `TODO.md` 的任务顺序与依赖；
   - 本次仅执行拆分后的第一个子任务；
   - 若只是进行任务重排/拆分，则提交后停止。
6. 如果任务可执行：
   - 阅读相关实现与测试；
   - 直接实现，不做规避性 workaround；
   - 若遇到规范缺口、实现边界或前置缺失，立即把问题转成更前置的 `TODO.md` 任务并更新 `PLAN.md`，提交后停止。
7. 对实现结果进行验证：
   - 运行最小相关测试；
   - 运行必要的更广测试；
   - 运行 `cargo fmt`；
   - 运行 `cargo clippy --all-targets -- -D warnings`（如与本次修改相关且可承受）；
   - 确认没有新增 warning。
8. 更新文档状态：
   - 在 `TODO.md` 标记该任务完成；
   - 在 `PLAN.md` 反映当前进度、后续顺序、任何风险或调整；
   - 如本次变更影响说明文档，补充 `README.md` 或代码注释。
9. 提交本次变更，提交信息应清晰描述任务内容。
10. 停止，不继续执行下一个任务。

## 初始判断准则

- “最新提交提到的问题”优先级高于 `TODO.md`。
- 不接受夹带 workaround 的“假完成”；如果规范要求与实现不一致，必须先把缺口转成前置任务。
- 若当前任务需要的语言特性、运行时能力或标准库行为不存在，则不能绕过，必须回填任务依赖。
- 只要任务能被拆小，就优先拆小并保证一次提交能闭环。

## 进度记录

- [已开始] 已写入本计划文件，后续开始检查最新提交与任务列表。
- [已完成] 已检查最新提交：最新提交信息为 `[T3009a] Lower immediate-resume arm payloads directly`，提交信息本身未显式声明新的既有问题；同时已核对 `TODO.md` / `PLAN.md`，确认首个未完成任务原为 `T3009aR`。
- [已完成] 已对 `T3009a` 的生产代码做预审查，并做了定向验证：
  - `effect_resume_yield_int_basic.scoop` build 通过；
  - `effect_resume_finally_normal.scoop` build 通过；
  - `effect_resume_if_else_branch_single_perform.scoop` 不再报 `call callee`，而是前进到既有 `value coercion` 缺口；
  - `effect_resume_double_resume_exit.scoop` 当前在 codegen 阶段报 `unsupported_main_body: unknown local value`。
- [计划调整] `T3009aR` 不能直接完成。审查结果表明：`T3009a` 只覆盖了 tail-position `resume(value)` 的 dedicated lowering；同一 immediate-resume arm 中更早出现的 `resume(...)` 仍会漏到 generic local-call，而 `resume_placeholder` 已删除，因此失败形态从 `call callee` 变成了 `unknown local value`。
- [已完成] 已按阻塞流程更新 `TODO.md` / `PLAN.md`：
  - 新增前置任务 `T3009a1`，用于收紧 immediate-resume 的 typecheck/HIR/codegen 合同；
  - 将 `T3009aR` 保持为 `[TODO]`，并改为依赖 `T3009a1`；
  - 调整 `PLAN.md` 当前执行顺序，确保下一轮先处理真实前置缺口。
- [下一步] 检查工作区差异，提交本轮计划重排与阻塞记录，然后停止。
