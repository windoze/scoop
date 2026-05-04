执行计划

1. 读取根目录 `TODO.md`，只把它作为任务索引使用，确认按顺序引用的详细任务文件。
2. 依索引顺序打开对应的 `TODO-Px.md` 文件，以详细文件中的标题 `[DONE]` 标记作为唯一完成判定，找出第一条未完成详细任务。
3. 检查最近提交是否明确提到与该任务直接相关的未完成问题；若有，将其纳入当前任务或作为必要前置任务记录。
4. 阅读当前任务的完整要求、依赖、约束、验证命令与完成记录，必要时补充本文件中的更具体执行步骤。
5. 实现当前任务要求的最小正确改动，不绕过规格、不削弱测试形状、不引入任务私有特例。
6. 运行与该任务相关的测试；若出现阻塞性规格缺口或实现边界，先修复；若无法在当前任务中正确完成，则在对应 `TODO-Px.md` 插入最小前置任务，同步 `TODO.md`，提交后停止。
7. 完成后在对应 `TODO-Px.md` 的任务标题加 `[DONE]` 并更新完成记录；若索引中有该任务，同步 `TODO.md` 的 `[DONE]` 标记。
8. 根据需要运行格式化、构建或测试验证，确保没有相关警告或失败。
9. 提交本次所有相关改动，提交信息包含任务编号与清晰说明。
10. 提交后停止，不继续处理下一条任务。

进度记录

- 已创建本执行计划；尚未读取任务索引或执行实现工作。
- 已读取 `TODO.md` 与 `TODO-P6-part3.md`，确认第一条未完成详细任务为 `P6-T05`：建立 refactor LLVM 定向 build/run-pass/runtime_gc 验证矩阵，并冻结 P6 -> P7 handoff contract。
- 最近提交为 `[P6-T04R] Review GC runtime integration`，与当前任务前置关系一致，未显示需要抢先处理的未完成 blocker。
- 当前工作树已有若干与 P6-T05 形态相关的未提交改动/新增 fixtures；后续将先审阅这些改动，判断是否为本任务的中断续作，并在完成时一并提交。
- P6-T05 指定命令与新增 build artifact 样本大多已通过；新增跨线程 runtime_gc 样本失败，实际输出只到 `after_thread/done`，说明 refactor path 仍把 `__scoop_thread_spawn_join_resume_u64` 降到 legacy continuation runtime helper，未调用 refactor surface-resume ABI。
- 计划修复：为 refactor lowering 增加专用 runtime bridge `scoop_thread_spawn_join_refactor_resume_u64(k, value, thunk)`，runtime 只负责 handle-rooting、spawn/join 与调用 compiler 生成 thunk；compiler thunk 消费 published surface-resume ABI，不回 legacy continuation helper。
- 已实现 refactor 专用跨线程 resume bridge，并重跑 `effect_cross_thread_resume_payload_refs.scoop`，moving-GC runtime_gc 样本通过。
- 已同步完成记录：`TODO-P6-part3.md` 的 `P6-T05` 已标记 `[DONE]`，`TODO.md` 索引已同步 `[DONE]`。
