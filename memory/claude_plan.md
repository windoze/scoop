执行计划

1. 读取 `TODO.md`，按文件顺序识别第一个标题未带 `[DONE]` 前缀的任务，并检查该任务的依赖、验证要求和完成记录。
2. 查看最近提交信息，确认是否明确提到与当前任务直接相关的未完成问题；如有，将其纳入当前任务或作为前置任务记录。
3. 只围绕第一个未完成任务收集必要上下文，避免开放式历史问题排查。
4. 如果任务可直接完成，实施最小且完整的代码、测试或文档改动；如遇阻塞当前任务的缺失特性或规格不匹配，则在 `TODO.md` 插入最小前置任务并停止。
5. 运行当前任务要求的验证命令及必要的相关测试；如果验证失败，定位并修复与当前任务相关的问题。
6. 更新 `TODO.md`：将完成任务标题加上 `[DONE]`，补全完成记录；仅在阶段级计划确实变化时更新 `PLAN.md`。
7. 在关键步骤完成或计划变化时更新本文件，保持进度可见。
8. 提交本次任务涉及的全部相关改动，提交信息使用任务编号和简明描述。
9. 完成一个任务后立即停止，不继续处理后续任务。

当前状态

- 已读取 `TODO.md` 并确认首个未完成任务是 `P6-T04`：P6 全包清场、文档同步与依赖审计。
- 已检查最新提交 `b1e7ab34 Update plan`；其变更聚焦 overload 文档和 P7-T04 的 TypeStore/stable wire format 补充，未发现直接阻塞 `P6-T04` 的未完成事项。
- 已执行 P6 目标残余审计：top-level `val` 普通访问只做 initialized check 后读 eager backing storage；per-cone init routine 从 `LirFacts.global_init.final_entry_order` / `cone_init_routines` 生成并执行 eager roots；runtime `scoop_once_begin/end` 调用点只服务 object singleton；`scoopc_lir_facts` 已发布 global init/storage/final-entry 合同并由 verifier 检查。
- 文档同步决策：更新 `README.md` 与 `PIPELINE-CLEANUP.md`，把 P6 global init/storage/entry order 从剩余 residual 中移出，并明确 P7 仍需清理 HIR scaffold、raw MIR/pass-view、reachability、physical ABI/layout 与 TypeStore bridge。`PIPELINE_REFACTOR.md` 不更新，因为阶段设计边界没有变化。
- 验证发现 P6 相关阻塞：`cargo run -p scoop -- test --fixtures tests/fixtures/run-pass` 有多项线程相关失败；单独运行 `std_thread_basic.scoop` 并带 `SCOOP_SYSROOT_DEPS=scoop.thread` 后定位为链接错误，`scoop.thread` native object 引用 `scoop_thread_init_current`，但当前 LLVM 只在存在 TLS init routines 时定义该 hook。
- 修复计划更新：先把 `scoop_thread_init_current` 固定为总是由 LLVM 模块定义；没有 TLS roots 时生成 no-op body，有 TLS roots 时仍执行当前线程 TLS roots。然后重跑 P6-T04 验证。
- 已完成 hook 修复：`ensure_thread_init_current_function_defined` 现在始终定义 `scoop_thread_init_current`，无 TLS roots 时返回 no-op body。`std_thread_basic`、`delegated_property_lazy_thread_safety_synchronized_once`、`gc_continuation_multi_thread_concurrent_alloc_resume` 单独重跑通过。
- 已重跑 `P6-T04` 验证：`cargo fmt`、`cargo test -p scoopc_lir_facts`、global init fixtures、`cargo test -p scoopc --no-default-features storage_policy`、dependency gate、clippy 和 `git diff --check` 通过；完整 run-pass 重跑后 P6/thread-init 相关失败已清零，仅剩 7 个既有非 P6 baseline 失败。
- 已更新 `TODO.md` / `TODO-6.md`，将 `P6-T04` 标记为 `[DONE]` 并填写完成记录。
- 下一步：检查工作区 diff/status，提交本任务改动后停止。
