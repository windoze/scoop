# 本轮执行计划

## 说明

按要求先记录执行思路与计划。这里保留的是可审计的执行摘要、判断依据和步骤计划，不包含原始内部推理草稿。

## 初始目标

1. 读取 `TODO.md`，把它当作任务索引而不是详细任务正文。
2. 依照 `TODO.md` 引用顺序打开对应的 `TODO-Px.md` 文件。
3. 找到第一个标题未带 `[DONE]` 的详细任务，并以该详细任务为本轮唯一执行单元。
4. 检查最近提交是否直接提到与该任务相关且未完成的问题；若该问题构成当前任务前置条件，则先把它作为当前任务的一部分处理，或在对应 `TODO-Px.md` 中新增最小前置任务并同步 `TODO.md`。
5. 在不规避规范、不缩小任务范围的前提下完成该任务。
6. 运行与该任务直接相关的验证；若任务涉及整体质量门槛，则补充运行格式化、测试、以及 `cargo clippy --all-targets -- -D warnings`。
7. 更新任务记录：
   - 在对应 `TODO-Px.md` 中将该任务标题改为 `[DONE]`。
   - 补充完成记录、验证命令和必要说明。
   - 若任务标题、顺序或依赖发生变化，同步更新 `TODO.md`。
   - 仅当阶段计划确实变化时才更新 `PLAN.md`。
8. 检查工作区未提交改动，避免覆盖他人改动；若本轮是续做且存在本任务遗留未提交文件，则与本次改动一并提交。
9. 按仓库约定创建一次提交，然后停止，不继续下一个任务。

## 执行约束

1. 不做开放式历史问题清扫。
2. 不以变通方案代替规范实现。
3. 仅在存在真实新前置依赖时才拆分任务，并把最小新增任务写回详细 TODO 文件与索引。
4. 若遇到阻塞：保持当前任务未完成，新增前置任务、同步索引、必要时更新阶段计划，然后提交并停止。

## 进度更新约定

在以下节点更新本文件：

1. 定位到本轮目标任务之后。
2. 开始代码修改之前。
3. 若执行路径、依赖判断或任务范围发生变化。
4. 验证完成之后。
5. 提交前，记录最终结果与未解决风险（如有）。

## 当前已定位任务

- 本轮唯一执行单元：`P6-T02R`。
- 任务性质：review 任务，目标是确认 `P6-T02` 与 `P6-T02a` 是否已经把 refactor LLVM 的 type/layout ABI 合同真正固定下来。
- 当前已知上下文：`P6-T02R` 之前曾在完成记录里识别出一个 blocker，随后新增并完成了 `P6-T02a`；本轮需要复审该 blocker 是否已被实质修复，而不是仅靠记录关闭。

## 当前执行步骤

1. 审阅 `crates/scoopc/src/llvm/codegen/effect_refactor/{types,layout}.rs` 及相关调用点，确认 canonical `Step_F`、frame、continuation、resume-interface ABI 的 authoritative 来源与查询面。
2. 审阅是否仍存在对 legacy `EffectSignal` / `EffectOutcome` / `LegacyEffectBoundary` 合同的 ABI 依赖，尤其是在 refactor 主实现路径而非 legacy 对照路径中。
3. 运行 `P6-T02R` 要求的测试与搜索命令，验证 review 结论。
4. 若无 blocker，则把 `P6-T02R` 标记为 `[DONE]` 并更新完成记录；若发现 blocker，则按要求新增最小前置任务并同步索引。
5. 最后提交本轮所有改动并停止。

## 当前发现

- 已重跑：
  - `cargo test -p scoopc refactor_llvm_`
  - `cargo test -p scoopc refactor_resume_interface_completeness_groups_methods_by_effect_family`
  - 三个 refactor build fixtures
  - 一个 legacy build fixture 抽样
- 搜索结果表明：`crates/scoopc/src/llvm/codegen/effect_refactor/**` 中未发现 `EffectSignal` / `EffectOutcome` / `LegacyEffectBoundary` 命中；legacy 命中仍局限在 `crates/scoopc/src/llvm/codegen/effect/**`。
- 但 review 识别出新的真实 blocker：
  - `crates/scoopc/src/llvm/codegen/effect_refactor/layout.rs` 中 `materialize_resume_interface_layout(...)` 只校验了“已发布 method 自身是否匹配 step shell”；
  - 它维护了 `published_case_tags` 去检查重复，却没有把该集合与同一 effect family 在 authoritative `StepSchema` 中应有的 case 集做完整性比对；
  - 因此若 `LateLoweredResumeInterface.methods()` 少发了某个 case，LLVM ABI materializer 仍会静默接受并产出缩水的 vtable/method 布局，这违背了 `P6-T02` / `P6-T02a` 对完整 method 集和 fail-fast 的要求。

## 计划调整

1. 不把 `P6-T02R` 标记完成。
2. 在 `TODO-P6.md` 中于 `P6-T02a` 与 `P6-T02R` 之间新增一个最小前置实现任务，修复 authoritative resume-interface method completeness 校验漏洞。
3. 同步 `TODO.md` 索引顺序与标题。
4. 在 `P6-T02R` 完成记录中写明本轮新的 blocker 和证据。
5. 提交这些任务编排更新并停止，等待下一轮先完成新增前置任务。

## 当前结果

- 已完成：
  - 新增详细任务 `P6-T02b`；
  - 同步 `TODO.md` 索引；
  - 更新 `P6-T02R` 的依赖与 review 记录，明确当前不能标记完成。
- 待提交内容只包含：`TODO-P6.md`、`TODO.md`、`memory/claude_plan.md`。
