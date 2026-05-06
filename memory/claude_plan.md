# 执行计划

## 当前约束
- 先按 `TODO.md` 索引定位第一个未完成的详细任务，再以对应 `TODO-Px.md` 为准执行。
- 每次只完成一个详细任务，完成后停止。
- 不通过变通、弱化测试或改变既定建模方式绕过缺失功能或规格不符。
- 如遇阻塞，添加最小必要前置任务，同步 `TODO.md`，提交后停止。
- 完成任务后同步 `[DONE]` 标记、完成记录和必要索引，并提交所有相关变更。

## 初始步骤
1. 读取 `TODO.md`，按索引顺序定位引用的详细任务文件。
2. 读取对应 `TODO-Px.md` 文件，寻找第一个标题未带 `[DONE]` 的详细任务。
3. 检查最近提交信息是否显式提到与该任务直接相关的未完成问题。
4. 阅读当前任务要求、约束、验证命令和完成记录格式。

## 实施步骤
1. 基于当前任务要求检查相关代码、测试和 fixtures。
2. 做最小正确实现，避免无关重构。
3. 添加或更新与任务直接相关的测试/fixtures。
4. 运行任务要求的验证命令，并根据失败结果修复实现。
5. 必要时运行更广泛的相关验证，确保没有警告或回归。

## 收尾步骤
1. 在对应 `TODO-Px.md` 标题前加 `[DONE]` 并更新完成记录。
2. 同步 `TODO.md` 中对应索引项的 `[DONE]` 状态。
3. 仅在阶段计划实际变化时更新 `PLAN.md`。
4. 检查工作区变更，确认不误改无关内容。
5. 使用清晰任务编号提交变更。
6. 停止，不继续下一项任务。

## 进度记录
- 已写入初始计划，下一步读取任务索引并定位第一个未完成详细任务。
- 已读取 `TODO.md` 与 `TODO-P7.md`，确认第一个未完成详细任务为 `P7-T02Zc`。
- 已检查最新提交 `c4e5eea1 [P7-T02Z] Fix run-pass blockers and add multi-owner prerequisite`，其新增的 multi-owner prerequisite 与当前任务直接相关，纳入本轮范围。
- 下一步聚焦 late-lowered surface-resume dispatch / wrapper projection / ABI materializer 中 schema 单 owner 假设，修复后运行 `P7-T02Zc` 指定 fixture 与相关单测。
- 已修改 late-lowered inventory，使同一 continuation schema 可携带多个 owner-specific wrapper projection。
- 已修改 refactor LLVM ABI materializer 与 shared surface-resume 入口，允许多 owner trampoline 并按 continuation object runtime descriptor 分派。
- 下一步运行定向 `cargo check` / fixture 验证，修正编译错误或剩余语义问题。
- 已修复 composed call-boundary replay 的过度重放：当同一 source tail 后续还有会捕获 continuation 的 resuming boundary 时，不再重放已执行过的 caller prefix；仍保留 nested block mixed replay 需要的 prefix 重放。
- 已通过定向验证：new multi-owner ABI 单测、`effect_multi_escape_custom_nonresuming_direct_indirect_multi.scoop`、`effect_multi_escape_custom_nonresuming_direct_indirect_block_multi.scoop`、`effect_escape_continuation_indirect_perform_multi_site_callee_branch.scoop`、`effect_lowered`、`llvm::codegen::effect_refactor`、`llvm::tests`、`cargo clippy --all-targets -- -D warnings`。
- 下一步更新 `TODO-P7.md` / `TODO.md` 完成记录，随后检查 git diff 并提交。
