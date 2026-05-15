## 当前执行计划

注意：这里记录的是可审计的执行计划、决策依据摘要与进度，不包含内部推理细节。

1. 在不做开放式问题排查的前提下，先读取 `TODO.md`，定位第一个标题未带 `[DONE]` 的任务。
2. 读取与该任务直接相关的上下文文件（必要时包括 `PLAN.md`、相关源码、测试、最新提交信息），确认需求、依赖和验收标准。
3. 检查最新提交是否明确提到与当前任务直接相关且未完成的问题；如果是，则将其视为当前任务的一部分，或在 `TODO.md` 中补充为前置任务。
4. 实现当前任务要求的最小正确改动；如果遇到阻塞当前任务的真实缺口或缺陷，不做规避，而是在 `TODO.md` 中补充最小必要前置任务并停止继续后续任务。
5. 运行当前任务要求的验证，以及受影响范围内的测试、格式化、lint/编译检查；若发现问题，立即修复并重新验证。
6. 完成后更新 `TODO.md`：将当前任务标题标记为 `[DONE]`，并补充完成记录；仅在阶段计划确实变化时更新 `PLAN.md`。
7. 检查工作区改动，按要求创建一次 git 提交，提交信息应反映当前任务编号与内容，然后停止，不进入下一个任务。

## 进度记录

- 已创建本计划文件。
- 已读取 `TODO.md`；发现索引表与正文标题存在状态不一致，当前以正文标题是否带 `[DONE]` 作为完成判定依据。
- 已检查最新提交：`[P4-T01e] Lower Array intrinsics directly in IR`，说明最近提交与 `P4-T01e` 直接相关，因此需要结合正文标题继续确认首个真正未完成任务，而不能仅凭索引表判断。
- 已确认首个正文标题未完成任务是 `P4-T01`。
- 已读取 `sysroot/core.scoop`、`sysroot/print.scoop`、`resolve/typecheck/HIR/LLVM` 中所有 `toString` 相关旁路，并确认 `print/println` 也仍然保留独立 builtin lowering。
- 已识别出阻塞 `P4-T01` 的真实前置缺口：source-level native `@Extern` 通过 `check_extern_fun_signature_matches_native_abi()` + `TypeLowering::is_native_abi_value_type()` 明确拒绝 `String` / 其它 managed ref 穿越 native ABI，因此任务描述中的 sysroot wrapper 当前没有合法源码落点；继续借用旧 `toString` by-name intercept 来实现 wrapper 则会直接违背 `P4-T01` 的删除目标。
- 已回滚一处用于验证思路的试探性 sysroot 声明修改，避免在 blocker 未建模完成前留下半成品代码。
- 已按最小原则把该缺口回写为新的前置任务 `P4-T01f`，并同步更新 `TODO.md` 顺序、`P4-T01` 依赖以及 `PLAN.md` 的阶段依赖说明。
- 下一步：检查工作区仅包含 blocker 文档更新后，按要求提交并停止，本轮不继续实现 `P4-T01`。
