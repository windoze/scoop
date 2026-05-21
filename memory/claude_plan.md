# 执行计划

## 当前状态
- 已开始本次调用。
- 下一步将先读取 `TODO.md`，按任务标题是否带有 `[DONE]` 判断第一个未完成任务。
- 本文件记录可公开的执行计划、关键决策和进度更新。

## 执行步骤
1. 读取 `TODO.md`，确认第一个未完成任务及其依赖、验证要求和完成记录规则。
2. 检查最近提交和工作区状态，只识别与当前任务直接相关的未完成问题或冲突。
3. 阅读当前任务涉及的代码、测试、规格或夹具，确定最小正确实现范围。
4. 实现当前任务；如果遇到阻塞当前任务的规格缺口或实现缺口，则在 `TODO.md` 插入最小必要前置任务并停止。
5. 运行当前任务要求的验证命令，以及必要的相关测试；修复本任务引入或暴露的相关问题。
6. 在 `TODO.md` 将完成任务标题前缀改为 `[DONE]`，并更新完成记录。
7. 如阶段级计划未改变，不更新 `PLAN.md`；仅在阶段依赖或完成标准变化时更新。
8. 检查 `git status`、`git diff`、最近提交，提交本次任务相关更改。
9. 完成一个任务后停止，不继续处理下一个任务。

## 约束
- 以 `TODO.md` 为任务排序和完成状态的唯一来源。
- 不用 workaround 规避规格不匹配；相关缺口必须修复或登记为前置任务。
- 不回滚或覆盖用户已有未提交更改。
- 使用小而聚焦的补丁修改文件。

## 进度记录
- 已确认：`TODO.md` 中第一个未完成任务是 `P4-T02`，位于 `TODO-5.md`。
- 已读取：`P4-T02` 的目标是移除 P4 对 `MirStageOutput` / `MaterializedMir` 的可变借用，把 P4 新增类型放入 effect-owned context，并证明 P4 前后 MIR snapshot/pass artifacts/TypeStore 不变。
- 已检查：最近提交是 `P4-T01R`，与当前任务顺序一致；当前工作区只有本计划文件未提交。
- 已定位：`effect_facts_stage.rs` 通过 `canonical_snapshot_mut()` 两次打开 MIR 可变快照；`MaterializedEffectFactsBuilder` 持有 `&mut MaterializedMir`，并通过 MIR `TypeStore` intern runtime-error effect、tuple carrier、continuation/object/schema 类型。
- 实施方案：新增/发布 effect-owned type context；builder 改为只读 `MaterializedMir` + 显式可变 effect-owned context；stage 创建并复用该 context 完成 two-pass solver；`EffectFactsStageOutput::types()` 改为返回 effect facts 自有 context。
- 已实施：`MaterializedEffectFacts` 现在携带 `EffectOwnedTypeContext`；builder 和 stage 已去除 P4 对 MIR snapshot 的可变借用；新增 stage 级测试对比 P4 前后 snapshot binding、pass artifacts metadata 和 MIR `TypeStore`。
- 已验证：`cargo fmt`、effect facts stage/effect facts 单测、effect facts/effect lowered 夹具、effect lowered 单测、`cargo clippy --all-targets -- -D warnings`、`git diff --check` 均通过。
- 已完成记录：`TODO.md` 与 `TODO-5.md` 已将 `P4-T02` 标记为 `[DONE]`，并填写完成记录。
- 下一步：提交本次 `P4-T02` 变更后停止，不继续处理 `P4-T02R`。
