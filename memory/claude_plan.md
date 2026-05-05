# 当前执行计划

## 执行原则
- 先读取 `TODO.md` 作为索引，再按索引打开对应 `TODO-Px.md`，以详细任务文件为准确定第一个未完成任务。
- 只处理第一个未完成的详细任务；完成、验证、记录并提交后立即停止。
- 若遇到阻塞当前任务的缺失特性或规格不一致，不做绕路实现；在对应详细 TODO 文件中加入最小 prerequisite，同步 `TODO.md`，提交后停止。
- 只在阶段级计划确实变化时更新 `PLAN.md`；常规进度仅记录在详细 TODO 与本文件中。

## 步骤
1. 检查任务索引与详细任务文件，确定当前第一个未完成任务。
2. 阅读该任务要求、约束、依赖和验证方式，必要时查看相关代码与测试。
3. 按任务要求做最小正确实现，不回退或覆盖无关工作区变更。
4. 运行相关测试；若失败，修复与当前任务直接相关的问题并复测。
5. 更新对应 `TODO-Px.md` 的任务标题为 `[DONE]` 并补齐完成记录，同时同步 `TODO.md` 中同一任务的 `[DONE]` 状态。
6. 更新本文件记录关键完成步骤。
7. 按要求提交本次任务的全部相关改动，然后停止。

## 当前进度
- 已写入初始计划。
- 已根据 `TODO.md` 与 `TODO-P7.md` 确认当前第一个未完成详细任务为 `P7-T02S`：修复默认 build fixture 中暴露的 refactor LLVM/lowering 缺口。
- 下一步查看最新提交与相关代码/fixture，确认是否存在直接关联的未完成问题，然后按任务三类缺口逐项修复并验证。
- 已复现三类定向失败：f-string 在 MIR 中仍是 `Todo`；integer literal overflow 在 effect facts 前被包装为 frontend prepare failure；`HandleDispatch` completion source 只看 state slice 最后一条赋值。
- 正在实施最小修复：显式 MIR f-string rvalue 与 refactor lowering、LLVM stage literal 预检查、P5 contract 层反向扫描 state slice completion source。
- 初轮修复后：默认 `9223372036854775808.toString()` 已恢复 `scoop::llvm::invalid_literal`；f-string 已不再是当前 extern fixture 阻塞；原 task fixture 的 completion-source 错误已消失。剩余直接后续失败为 `GC.handleNew` 函数值调用 lowering、负号/窄整数字面量预检查覆盖不足、以及 task fixture 的 source-type ABI value 对类型参数 `T` 的 contract 缺口。
- 已继续修复 `GC.handleNew/Drop`、负号/窄整数字面量、generic resume surface ABI、resume-boundary wrapper complete projection、plain local-effect closure、enum ctor、task transport、atomic 与 panic 的直接后续阻塞。
- 当前 `task_atomic_claim_no_mutex_llvm` 停在 `scoop.core.Task<T>` generic class constructor/layout handoff：refactor class ctor 仍按未实例化 generic class field layout 取 LLVM payload，触发 `class field type`。该缺口需要新增 prerequisite 任务先固定 generic class instance layout contract；`P7-T02S` 保持未完成。
