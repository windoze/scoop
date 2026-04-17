# 执行计划与进度记录

## 说明

按要求先创建本文件，并在后续执行过程中持续更新。这里记录的是可审查的执行计划、关键判断依据、步骤进展与结果，不包含逐字逐句的内部推理草稿。

## 初始计划

1. 检查最新一次提交，确认提交说明中是否提到需要先修复的遗留问题。
2. 阅读 `TODO.md`，定位第一个未完成任务。
3. 阅读 `PLAN.md`，对照现有计划与任务顺序。
4. 如首个未完成任务过大，拆分为更小子任务，并同步更新 `PLAN.md` 与 `TODO.md`。
5. 实施当前要做的第一个任务或子任务。
6. 运行相关测试，并补齐必要测试，确保结果可靠。
7. 更新文档与任务状态：
   - 在 `TODO.md` 标记完成，或在阻塞时按要求调整顺序。
   - 在 `PLAN.md` 反映当前状态与依赖变化。
   - 在本文件记录关键进展与结论。
8. 提交变更到 Git，提交信息明确描述本轮完成的任务。
9. 停止，不继续处理下一个任务。

## 当前状态

- 已完成：创建计划记录文件。
- 已完成：检查最新提交信息；最新提交 `a6a5060 [T3009b2] Close shared resumed-body caller-tail matrix task` 的标题/正文未额外声明需要优先修复的新遗留问题。
- 已完成：读取 `TODO.md` / `PLAN.md`，确认当前第一个未完成任务为 `T3009b2R`。
- 已完成：对 `T3009b2R` 做首轮生产代码复审，定位到新的更前置生产缺口。
- 已完成：按阻塞规则把新缺口前移到 `TODO.md` / `PLAN.md`，并更新本文件记录。
- 进行中：准备提交本轮“阻塞重排”结果，然后停止。

## 当前任务

- 任务编号：`T3009b2R`
- 任务名称：Review：确认间接 callee resumed-body caller-tail 已统一接回
- 任务性质：复审任务；若发现生产代码旁路或语义缺口，必须在本任务内直接修复并重新复审。

## 本轮复审执行计划

1. 审阅 `TODO.md` 中 `T3009b2` / `T3009b2R` 的描述，明确要验证的合同边界。
2. 定向检查与 indirect callee resumed-body caller-tail 相关的生产代码，重点覆盖：
   - `crates/scoopc/src/llvm/codegen/effect/state_machine_plan.rs`
   - `crates/scoopc/src/llvm/codegen/effect/state_machine_emitter.rs`
   - `crates/scoopc/src/llvm/codegen/effect/mod.rs`
   - 必要时补查 `crates/scoopc/src/llvm/codegen/mod.rs` 与 runtime 侧 continuation 入口
3. 检索是否存在按 callee/source/fixture 形状分流的可疑逻辑，确认 tail-return 与 non-tail 路径共享合同。
4. 复跑 `T3009b2R` 相关验收测试与质量门槛。
5. 若未发现问题：
   - 将 `T3009b2R` 标记完成；
   - 更新 `PLAN.md` 与本文件，记录审查结论；
   - 提交 Git。
6. 若发现问题：
   - 先修复；
   - 重新复审与测试；
   - 再更新 `TODO.md` / `PLAN.md` / 本文件并提交 Git。

## 关键发现

- 已确认当前 `T3009b2R` 不能直接完成。
- 生产代码中的真实缺口是：`crates/scoopc/src/llvm/codegen/effect/state_machine_plan.rs` 的 `build_ordinary_callee_suspend_plan_from_unified_contract()` 只在 `builder.suspend_sites.len() == 1` 时建立 `CalleeSuspendPlan`。
- 这意味着：同一个 ordinary indirect callee 如果有多个 suspend site，即使运行时只走其中一个 site，fresh path 也不会保存 callee suspend state；随后 outer `ResumeAfterSite(Call)` 会直接把 `resume` payload 当作整次调用结果，跳过 callee 自己的 resumed body。

## 阻塞复现

- 复现方式：基于 `tests/fixtures/run-pass/effect_escape_continuation_indirect_perform_statement_container_matrix.scoop` 构造临时变体，把 `viaIf` 的 then / else 两个分支都改为 `Ask.ask(...)`。
- 实际表现：`resume("if")` 后输出中缺失 `if_resume` / `if_after` / `I:if`，却直接出现 outer caller 的 `after_if` 与值 `if`。
- 结论：multi-site ordinary indirect callee 仍未统一接回 resumed-body caller-tail。

## 计划调整

- 已在 `TODO.md` 中新增前置任务：
  - `T3009b2c`：收口 ordinary indirect callee 多 suspend-site 的 resumed-body caller-tail 合同
  - `T3009b2cR`：复审 multi-site ordinary indirect callee 路径
- 已把 `T3009b2R` 顺延到 `T3009b2cR` 之后，并在 `PLAN.md` 更新当前执行顺序。
- 本轮输出目标改为：记录阻塞、前置任务、提交计划重排；不继续修改生产代码。

## 关键约束

- 本轮只完成 `TODO.md` 中当前应执行的第一个未完成任务。
- 若遇到规格不匹配、语言特性缺失、实现边界不完整，不能绕过，必须先在 `TODO.md` / `PLAN.md` 中显式建依赖并调整顺序。
- 在我修改计划、完成关键步骤、发现阻塞时，都会更新本文件。
