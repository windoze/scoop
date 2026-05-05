# 当前执行计划

## 目标

- 按 `TODO.md` 索引和对应 `TODO-Px.md` 详细任务文件，识别第一个未完成的详细任务。
- 完整实现该任务、验证、更新任务记录，并提交一次 Git commit。
- 本次只处理一个详细任务，完成后停止。

## 步骤

1. 读取 `TODO.md`，确认任务索引和详细任务文件顺序。
2. 按索引顺序读取对应 `TODO-Px.md`，以标题是否带 `[DONE]` 判断第一个未完成详细任务。
3. 检查最新提交信息是否明确提到与该任务直接相关的未完成事项；如存在，将其纳入本任务或作为前置任务记录。
4. 阅读当前任务的详细要求、依赖、约束和验证要求。
5. 定位相关代码和测试，做最小且符合规范的实现变更。
6. 添加或更新必要测试与 fixture。
7. 运行相关验证；如失败，修复后重跑，直到当前任务要求得到满足。
8. 更新对应 `TODO-Px.md`：在任务标题前加 `[DONE]`，并填写完成记录。
9. 如任务索引需要同步，更新 `TODO.md` 中相同任务的 `[DONE]` 状态；仅在阶段计划真实变化时更新 `PLAN.md`。
10. 运行最终相关检查，查看 Git diff/status，提交包含本任务所有相关变更的 commit。
11. 停止，不继续下一个任务。

## 进度记录

- 已创建本执行计划。
- 已读取 `TODO.md` 与 `TODO-P7.md`，确认第一个未完成详细任务是 `P7-T03`。
- 已检查最新提交：`2f62f431 [P7-T02W] Fix class init hidden effect handoff`，它是 `P7-T03` 的直接前置修复；当前继续执行 `P7-T03`，不新增前置任务。
- 下一步按 `P7-T03` 要求运行标准 full regression 矩阵，先执行 `cargo test --all`，遇到默认 refactor 路径回归即修复并重跑。
- `cargo test --all` 已通过；下一步执行 `cargo run -p scoop -- test`。
- `cargo run -p scoop -- test` 首次失败在 `tests/fixtures/build/effect_refactor_dynamic_entry_publication_emit_llvm.scoop`，原因是生成 IR 命中禁止子串 `%scoop.refactor.Step__`。下一步检查该 fixture 期望、生成 IR 和相关 backend 输出，区分实现回归与过期断言。
- 已修复 effect-facts solver 的 plain local-control 判定：`ClassCtor` site 只有在 `emitted_cases` 非空时才保留本地 control step schema，避免纯 `NoOutward` class ctor 生成 complete-only Step shell。
- 定向验证通过：`cargo test -p scoopc --lib refactor_llvm_layout_binds_pure_direct_entries_without_legacy_typestore`；`cargo run -p scoop -- test --fixtures tests/fixtures/build/effect_refactor_dynamic_entry_publication_emit_llvm.scoop`。下一步重跑完整 `cargo run -p scoop -- test`。
- 完整 `scoop test` 第二次失败推进到 `tests/fixtures/run-pass/class_init_raise_cleanup_init_block_gc_basic.scoop`，现象为 run-pass 子进程 exit 1。下一步直接运行该 fixture 并检查期望。
- 已补齐 refactor facts/value lowering 对 `__scoop_gc_collect`、`__scoop_gc_debug_heap_object_count`、`__scoop_gc_debug_alloc_garbage`、`__scoop_stackmap_statepoint_smoke` 的 plain compiler intrinsic 处理；该 fixture 已能编译运行。
- 当前同一 fixture 输出 `1` 而期望 `0`，说明 class ctor hidden raise 被捕获后失败构造对象仍被临时 root 保留。下一步修复 refactor class ctor boundary 的失败路径 root 清理。
- 已修复 refactor class ctor lowering 在 ordinary propagation 被 boundary 接管时的 active/inactive 分支：active hidden-effect 路径会清理 ctor deferred root 并返回 null，inactive 路径保留对象。定向通过：`cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/class_init_raise_cleanup_init_block_gc_basic.scoop`。
- 完整 `scoop test` 第三次失败推进到 `tests/fixtures/run-pass/continuation_escape_binder_resume_effect_row_runtime_basic.scoop`，现象为 run-pass 子进程 exit 1。下一步直接运行并检查该 fixture。
- 已将剩余 blocker 明确为 `P7-T02X`：cross-call escaped continuation member provenance 与 resume-boundary continuation composition 未闭合。已在 `TODO-P7.md` 插入该前置任务、把 `P7-T03` 依赖改为 `P7-T02X`，并同步 `TODO.md` 索引。本次不标记 `P7-T03` 完成。
