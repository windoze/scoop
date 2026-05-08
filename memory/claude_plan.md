# 执行计划

## 约束
- 以 `TODO.md` 为唯一任务排序和完成状态来源。
- 只处理第一个标题未带 `[DONE]` 的任务，完成后停止。
- 不用规避、降级或替代表示来绕过实现缺口；若遇到阻塞缺口，先把最小必要前置任务写入 `TODO.md` 并提交。
- `PLAN.md` 只在阶段级计划、依赖或完成标准改变时更新。
- 完成当前任务后必须更新 `TODO.md` 的标题 `[DONE]` 和完成记录，并创建 git commit。

## 步骤
1. 读取 `TODO.md`，定位第一个未完成任务，并检查该任务的依赖、验证要求和完成记录格式。
2. 查看最新提交信息，确认是否存在与当前任务直接相关的未完成事项；仅在其阻塞当前任务时纳入范围或写成前置任务。
3. 根据当前任务范围检查相关代码、测试和文档，避免扩大到无关历史问题。
4. 若任务可直接完成，实施最小正确修改；若发现必须先修复的具体缺口，更新 `TODO.md` 插入最小前置任务并停止。
5. 运行当前任务要求的验证命令，以及必要的相关测试；若失败，定位并修复与当前任务相关的问题。
6. 更新 `TODO.md`：给已完成任务标题加 `[DONE]`，补充完成记录、验证命令和结果。
7. 仅在阶段计划实际变化时更新 `PLAN.md`。
8. 检查 git 状态和差异，提交本次任务涉及的全部未提交变更。
9. 停止，不继续处理下一个任务。

## 当前进度
- 已读取 `TODO.md`。`CG-T07S0a21` 的任务正文标题已标记 `[DONE]` 且最新提交为 `[CG-T07S0a21] Close callable ABI blockers`，但任务索引行仍缺少 `[DONE]`，属于 TODO bookkeeping 漂移。
- 当前将以第一个未完成任务正文 `CG-T07S0a22` 为执行目标，并在本次 TODO 更新中同步修正 `CG-T07S0a21` 索引行。
- 下一步读取 `CG-T07S0a22` 涉及的 fixture、失败清单和相关 compilation-unit / package binding 代码，先复现两个指定失败，再做最小修复。
- 已复现两个失败：`top_level_val_pattern_runtime_basic.scoop` build 报 main wrapper 缺入口 step schema layout；cone package fixture 在 effect facts 阶段报跨文件 `enabled` import 未解析。
- 当前实现方向：让 refactor LLVM/effect-facts 阶段消费 build/source-map 提供的真实 compilation-unit 源集，修复 package-level `comptime if` 跨文件绑定；随后继续处理顶层 pattern once-init wrapper 的 entry schema 发布问题。
- 已完成实现：effect-facts stage 支持 compilation-unit source set，refactor LLVM stage 从 source map 传入非 sysroot 源；main wrapper 使用 direct-entry ABI layout 的 entry step schema 解读返回 Step。
- 已通过定向验证和 clippy；已更新 `TODO.md` 与 `FAILED_FIXTURES.md`。下一步检查 git diff 并提交本任务变更。
