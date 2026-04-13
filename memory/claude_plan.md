# 本轮执行计划（推理摘要）

说明：这里记录的是可审计的执行思路摘要与计划，不包含逐字内部思维展开；后续如果计划调整、发现阻塞、完成关键步骤，我会持续更新此文件。

## 当前目标

按 `TODO.md` 中顺序完成第一个未完成任务；如果发现前置缺陷、规范不匹配或任务过大，需要先修复/拆分并更新 `TODO.md`、`PLAN.md`，然后仅完成当前应该执行的第一项后停止。

## 初始步骤

1. 检查最新一次 Git 提交，确认是否提到了需要先处理的既有问题。
2. 阅读 `TODO.md`，定位第一个未完成任务。
3. 阅读 `PLAN.md` 及相关上下文，确认该任务是否已有拆分或依赖说明。
4. 如任务过大或存在前置缺陷：
   - 拆分为更小子任务；
   - 更新 `PLAN.md`；
   - 调整 `TODO.md` 的顺序与依赖；
   - 本轮只执行调整后排在最前面的那一项。
5. 实现目标任务，补充或修改相关代码与测试。
6. 运行必要的验证，包括与改动直接相关的测试，以及尽量满足项目要求的格式化 / lint / 测试命令。
7. 更新文档状态：
   - 在 `TODO.md` 标记完成，或在阻塞时重排并说明依赖；
   - 在 `PLAN.md` 记录当前状态；
   - 在本文件记录关键进展。
8. 使用清晰的提交信息提交本轮改动，然后停止。

## 当前已知约束

- 不能用规避方案替代规范要求。
- 若发现规范缺口、实现边界或 bug 阻塞当前任务，需要先把该问题转化为更靠前的任务。
- 只完成一个任务，不继续做下一个。
- 输出与进度记录使用中文。

## 进度记录

- 已创建本计划文件，准备开始检查最新提交与任务列表。
- 已检查最新提交：提交信息未显式提到需要先修复的既有问题。
- 已定位第一个未完成任务原为 `T2003r3`，并确认其范围过大。
- 审计后发现一个真实前置缺口：`HandleStateMachinePlan` / `HandleSegmentList` 的 state/segment 动作仍主要是字符串 label，发射阶段若直接切统一 emitter，将被迫解析字符串或重新按源码形状回扫 HIR。
- 已据此把 `T2003r3` 拆为 `T2003r3a`～`T2003r3d`，并将当前执行项收口为新的第一项 `T2003r3a`：先补齐 typed/source-linked emit contract。
- 下一步：修改 effect state-machine plan / segment contract，把字符串动作与分支注释收口成结构化元数据，并保持 pretty dump、segment round-trip 与相关测试继续通过。
- 已完成 `T2003r3a` 的代码实现：
  - `HandleStateMachinePlan.states[*].actions` / `HandleSegment.ops` 已改为结构化 `HandleStateOp`。
  - `StateTerminator::Branch` / `HandleSegmentTerminator::Branch` 已改为结构化 `HandleBranchCondition`。
  - pretty dump 继续输出原有人类可读文本，但文本已退化为展示层，不再充当执行 contract。
- 已新增回归测试 `segment_round_trip_preserves_typed_emit_ops_and_branch_metadata`，用于锁住 direct/branch/while/finally representative sample 的结构化 contract round-trip。
- 已完成验证：
  - `cargo fmt --all`
  - `cargo test -p scoopc llvm::codegen::effect::tests:: -- --nocapture`
  - `cargo test -p scoopc`
  - `cargo clippy --workspace --all-targets -- -D warnings`
- 下一步仅剩文档状态收尾与提交，本轮完成后停止。
