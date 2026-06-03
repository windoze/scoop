# Claude Plan

## 执行原则

- 先读取 `TODO.md`，按文件顺序识别第一个标题未带 `[DONE]` 的任务。
- 只完成第一个未完成任务；完成、验证、记录并提交后停止。
- 不做开放式历史问题扫描；仅处理当前任务相关阻塞或验证中暴露且未排期的失败。
- 不写入私有思维链；本文件记录可审查的执行计划、关键决策、进度和验证结果。

## 初始执行计划

1. 读取 `TODO.md`，确定第一个未完成任务及其要求、依赖、验证条件和完成记录格式。
2. 查看最近提交，判断是否明确提到与该任务直接相关的未完成问题。
3. 根据任务范围读取相关源文件、测试和规范材料，避免无关 triage。
4. 如存在阻塞当前任务的缺失功能、规格不匹配或未排期失败，最小化更新 `TODO.md` 记录前置任务并提交后停止。
5. 如可直接执行，则做最小正确实现，补充或调整相关测试/fixture。
6. 按要求运行格式化、lint、相关测试；必要时运行完整测试与 fixture 套件。
7. 更新 `TODO.md`：在任务标题前加 `[DONE]`，填写完成记录；仅在阶段级计划变化时更新 `PLAN.md`。
8. 检查 git 状态与 diff，提交本次任务相关的全部变更，然后停止。

## 当前状态

- 已读取 `TODO.md` 与 `TODO-3.md`：第一个未完成任务是 `T3-04R：Review T3-04`。
- 最新提交为 `[T3-04J] Close source class ctor fallbacks`，与当前 review 直接相关，纳入本次审查范围。
- 工作树中已有未跟踪 `FACT_REFACTOR.md`，当前计划不读取、不修改、不提交该无关文件。
- 已完成第十一次审查复核：确认 `T3-04J` 后仍有阻塞 `T3-04R` 的 residual fallback/verifier/gate 缺口。
- 已在 `TODO.md` / `TODO-3.md` 新增前置任务 `T3-04K`，并将 `T3-04R` 的依赖改为 `T3-04K`。

## T3-04R 审查计划

1. 复核 `T3-04`、`T3-04A` 至 `T3-04J` 的完成条件与阻塞记录，整理本次 review 必查边界。
2. 搜索并阅读 P4/P5/P6/verifier/dependency gate 中与 source side table、FQN/string fallback、ABI/source-signature synthesis、class ctor/reflection/intrinsic/dispatch fallback 相关的实现。
3. 运行或至少审查 dependency gate，确认守卫覆盖当前 helper 名称和等价路径。
4. 如发现仍阻塞 `T3-04` 完成条件的缺口，新增最小前置任务（预计 `T3-04K`）并让 `T3-04R` 依赖它，然后提交并停止。
5. 如未发现缺口，运行 `python3 tools/run_fixtures.py`，将 `T3-04R` 标记 `[DONE]`，同步 `TODO.md` 子计划索引状态，并提交后停止。

## T3-04R 审查结论

- 当前 review 未能标记完成；必须先执行新增的 `T3-04K`。
- 阻塞范围包括 source-payload class ctor 合成/path-span lookup、reflection source-span bridge、named intrinsic FQN fallback、HIR direct generic source-signature scan、MIR backend source signature/ABI/named intrinsic fact 合成、value-box/member 文本拼装、unknown target 静默降级、`BodylessDirect` / `DynamicFallback` verifier escape 与 dependency gate 漏口。
- 本次只变更任务记录与计划文件；未改编译产物。后续提交前不需要运行完整测试套件，可说明因仅文档/任务清单变更而复用最近 `T3-04J` 的全量绿色验证记录。
- 已运行 `git diff --check -- TODO.md TODO-3.md memory/claude_plan.md`，通过。
