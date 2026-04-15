# 本轮执行计划

## 当前任务：T3004R — Review（已完成）

### 审查结论
full-state-machine LLVM emitter 只按 state machine 语义发射：
- Op 发射：29 个 HandleStateOp 变体
- Terminator 发射：9 个 UnifiedStateTerminator 变体
- Branch condition：HandleBranchCondition::WhileCond / IfCond
- Arm body：HandleArmKind（ImmediateResume / EscapeContinuation / NonResuming）
- Handle 入口 dispatch：完全基于 contract 数据
- 关键词检索：无 shape-based 生产代码命中
- 与 flag-based unwind 无交互

### 执行状态
- [x] 读取 emitter 主体
- [x] 审查 op / terminator / branch / arm 分支
- [x] 关键词检索
- [x] 记录审查结论
- [x] 更新 TODO.md / PLAN.md
- [ ] 提交 git
