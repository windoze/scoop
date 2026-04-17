# 本轮执行计划

## 约束说明

- 按用户要求，本轮只处理 `TODO.md` 中第一个未完成任务，完成后立即停止。
- 在开始任何仓库检查命令前，先写入本计划文件。
- 计划文件会在关键步骤完成或计划调整时持续更新。
- 这里记录的是可审计的执行思路与步骤摘要，不包含与实现无关的内部推理细节。

## 预定步骤

1. 检查最新一次 Git 提交，确认提交信息或改动中是否明确提到尚未修复的问题；如果有，优先修复这些既有问题。
2. 阅读 `TODO.md`，定位第一个未完成任务。
3. 阅读 `PLAN.md`，核对该任务的上下文、依赖与现有拆分情况。
4. 如果该任务过大或存在前置依赖缺口，则先更新 `PLAN.md` 与 `TODO.md`，将任务拆成更小子任务，并只执行新的第一个子任务。
5. 实现当前目标任务，必要时阅读相关代码、测试与规范文档。
6. 运行相关测试与质量检查，至少覆盖本次改动的直接影响范围；如果任务范围允许，还应运行更完整的检查。
7. 更新 `TODO.md`、`PLAN.md` 与本文件，记录完成情况或阻塞原因。
8. 使用清晰的 Git 提交信息提交本轮变更。
9. 停止，不继续处理下一个任务。

## 当前状态

- 已创建计划文件。
- 已检查最新提交 `eea8f752af66136977e32946f33639e6e95f8155`（`[T3009b0R] Review escaped continuation resume lowering`）。
- 已确认最新提交没有额外引入一个需要先于 `TODO.md` 排序处理的独立缺陷；提交只是把 `T3009b0R` 标记完成，并把当前执行顺序推进到 `T3010b2b1b`。
- 已定位首个未完成任务：`T3010b2b1b`。

## 针对当前任务的细化计划

1. 复现 `T3010b2b1b` 描述中提到的当前最小问题路径，优先检查 `effect_resume_nested_escape_handle_tail.scoop` 以及可能相关的 nested arm / nested handle / indirect helper fixtures，确认真实失败模式是否仍是 unified expected-context / coercion 缺口。
2. 阅读 unified state-machine emitter、普通表达式 codegen、以及 value coercion / expected-context 相关实现，找出 nested/indirect arm-body 路径与 direct 路径的差异。
3. 如果缺口真实存在且任务规模可控，直接修复并补充最小化测试。
4. 如果复现后发现原任务描述已经过时，需要更小或更前置的任务，则按用户要求更新 `TODO.md` / `PLAN.md`，调整任务顺序并在本轮停止。

## 当前发现

- `T3010b2b1b` 的三个 focused fixture 当前都已通过：
  - `effect_resume_nested_escape_handle_tail.scoop`
  - `effect_resume_nested_escape_handle_tail_multi_perform_nonunit.scoop`
  - `effect_escape_continuation_nested_arm_indirect_performs_outer.scoop`
- 代码复审结果与运行结果一致：当前 unified arm-body 路径已经统一复用 `codegen_expr_in_expected_context` / `coerce_value`，没有独立的 expected-context / coercion 缺口需要修复。
- 因此本轮将 `T3010b2b1b` 视为“重新基线化后确认缺口已消失”的任务，并把它标记完成。
- 在扩大验证时，`cargo run -p scoop --features llvm -- test` 先暴露了一个未在 `TODO.md` 跟踪的问题：`tests/fixtures/mir/handle_perform.scoop` 的 MIR golden 过期。

## 计划调整

1. 更新 `TODO.md` / `PLAN.md`：将 `T3010b2b1b` 标记完成，并新增前置任务以修复 `handle_perform.mir` golden mismatch。
2. 记录该 MIR 问题为何会先于 `T3010b2b1` 阻塞更大范围验证。
3. 本轮到此停止，不继续进入新增任务的实现。
