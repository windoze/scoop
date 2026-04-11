## 当前执行计划

说明：
- 按要求先记录可审计的执行思路与步骤。
- 这里记录的是任务分析摘要、执行计划、关键决策与进度更新。
- 不写入不可验证的内部详细思维链路；后续如计划变化，会继续更新本文件。

### 初始分析摘要

当前尚未读取仓库状态，因此先按固定流程推进：
1. 查看最新一次 Git 提交，检查提交信息中是否提到已知问题、遗留问题或待补修项。
2. 阅读 `TODO.md`，定位第一个未完成任务。
3. 阅读 `PLAN.md`，理解现有路线与任务上下文。
4. 评估该任务是否能在本轮内完整完成；若过大，则将其拆分并更新 `TODO.md` / `PLAN.md`。
5. 实现当前应执行的第一个任务。
6. 运行相关测试、格式化、lint，修复发现的问题。
7. 更新 `TODO.md`、`PLAN.md` 与本文件，记录完成状态。
8. 提交 Git commit，然后停止，不继续做下一个任务。

### 执行步骤

- [ ] 检查最新提交及其可能提到的遗留问题
- [ ] 读取 `TODO.md`，确认第一个未完成任务
- [ ] 读取 `PLAN.md`，确认是否需要拆分任务
- [ ] 如需拆分，先更新 `TODO.md` / `PLAN.md`
- [ ] 实现当前目标任务
- [ ] 运行测试与质量检查
- [ ] 更新文档与任务状态
- [ ] 提交变更并停止

### 进度日志

- 已创建初始计划文件，下一步将检查最新提交与任务列表。
- 已检查最新提交 `bd5ca57328cd0687257aee9b3a04b8c5dc560042`，提交信息未额外声明需要先单独修复的遗留 issue。
- 已读取 `TODO.md` 与 `PLAN.md`；当前第一个未完成任务为 `T2003c0b2`：扩展 LLVM mixed-arm handle dispatch 中 sibling escape-continuation 到 indirect / richer mixed 组合。
- 下一步：审计相关 codegen / fixtures，判断 `T2003c0b2` 是否需要先拆分为更小的子任务。
- 审计结果：
  - mixed-arm escape 当前入口 `codegen_handle_expr_immediate_resume_with_escape_sibling_direct` 只支持 top-level `val = perform` 的 direct single-site。
  - 单-arm escape continuation 已有较完整的 indirect-perform 路径：`scan_for_indirect_perform_call_sites` + `codegen_handle_expr_escape_continuation_indirect`。
  - 但 `T2003c0b2` 当前描述把“先补 indirect 单站点”与“再补 richer mixed 组合”绑在一起，单轮改动面过大。
- 决策：
  - 先把 `T2003c0b2` 拆成两个子任务：
    1. `T2003c0b2a`：mixed-arm immediate-resume + sibling escape-continuation 支持单个 top-level indirect perform call site。
    2. `T2003c0b2b`：继续扩展 richer mixed 组合（多 site / 更复杂 direct+indirect / 与 sibling non-resuming 混用等）。
  - 本轮执行 `T2003c0b2a`，并保留其余 richer mixed 组合的稳定诊断。
- 已更新 `TODO.md` / `PLAN.md`：
  - `T2003c0b2` 已拆为 `T2003c0b2a`（single indirect site）与 `T2003c0b2b`（richer mixed）。
  - 当前首个未完成任务已变为 `T2003c0b2a`。
- 当前实现计划：
  1. 在 mixed-arm 入口先区分 escape sibling 是 direct site 还是 indirect call site。
  2. 为 `T2003c0b2a` 新增 dedicated lowering，复用单-arm indirect continuation 的 state capture / step resume / callee suspend 写回逻辑。
  3. 对 multiple indirect sites、direct+indirect 混用、多 escape sites 等 richer mixed 组合维持稳定诊断。
  4. 补 run-pass / build-fail fixtures。
  5. 跑格式化、测试、clippy 后完成文档与提交。
