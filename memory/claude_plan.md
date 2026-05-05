# 执行计划

## 目标

完成 `TODO.md` 索引中对应详细任务文件里的第一个未完成任务，只完成一个任务并停止。

## 约束

- 先读取 `TODO.md`，再按索引顺序读取对应的 `TODO-Px.md`。
- 任务标题只有显式带 `[DONE]` 才算完成。
- 如 `TODO.md` 与详细任务文件不一致，以详细任务文件为准并同步索引。
- 不为方便而拆分任务；只有遇到确切前置阻塞时才新增最少的前置任务。
- 不用 workaround 规避规格或实现缺口；相关阻塞必须修复或记录成前置任务并提交。
- 完成后更新详细任务文件的 `[DONE]` 标记和完成记录，必要时同步 `TODO.md`。
- 最后提交 Git commit，然后停止。

## 步骤

1. 读取 `TODO.md`，确定详细任务文件的检查顺序。
2. 读取相关 `TODO-Px.md`，找出第一个标题未带 `[DONE]` 的详细任务。
3. 检查最新提交是否明确提到与该任务直接相关的未完成问题。
4. 阅读当前任务正文、约束、依赖和验证要求。
5. 检查相关源码、测试、fixtures 和规格，定位需要修改的位置。
6. 实现任务要求；如遇规格阻塞，新增最少前置任务、同步索引、提交并停止。
7. 运行任务要求的验证命令及必要的相关测试。
8. 修复验证中暴露的当前任务相关问题并重新验证。
9. 更新 `TODO-Px.md` 中当前任务标题为 `[DONE]`，补充完成记录；必要时同步 `TODO.md`。
10. 更新本文件记录关键进展。
11. 按仓库提交风格创建 Git commit，包含本次任务产生的所有相关变更。
12. 停止，不继续下一个任务。

## 当前状态

- 已读取 `TODO.md` 与 `TODO-P7.md`。
- 已确认第一个未完成详细任务为 `P7-T03`：在 refactor 成为默认主线后运行标准 full regression 矩阵，并修复所有默认路径回归。
- 最新提交 `6a59d342 [P7-T02R] Review default pipeline fallback guards` 未声明与 `P7-T03` 直接相关的未完成阻塞。
- `cargo test --all` 首次运行失败在 `commands::parity::build_emit_llvm_cli_parity_matches_legacy_and_refactor`。
- 根因：该旧测试仍要求 effectful `build --emit-llvm` 的 legacy/refactor IR 字节级一致；P6 后 refactor 后端已经发布 `Step`/continuation ABI，legacy 仍是旧 handler-frame 形状，二者只应守护显式入口可用和 ABI 形状正确。
- 已将该测试改为 `build_emit_llvm_cli_legacy_and_refactor_both_succeed_after_backend_split`，断言两端均成功、均有 `main`，并分别包含 legacy handler-frame 与 refactor Step ABI 形状。
- 重跑 `cargo test --all` 后，`scoop` 单元测试通过，`scoopc --lib` 暴露多项历史测试假设问题。
- 已开始修复：legacy state-machine/emitter 测试改用显式 legacy session；P6 `NoOutward` plain ABI 后的 refactor ABI 测试改查 plain callable layout / plain carrier fallback；effect facts precision stale 断言改为更精确的 `Precise`；pure loop-control late-lowering 测试改为真正 effectful callable；refactor LLVM stage 的旧 fail-fast 测试改为正向验证 effectful lowering。
- `cargo test -p scoopc --lib` 已修到通过（769 passed）。主要收敛内容：补充 call-boundary callee continuation surface-resume ABI 查询，兼容 legacy `Continuation.resume` callee/receiver 形状，更新 NoOutward/plain ABI 与 MIR pass 后处理后的测试合同。
- 最终 `cargo test --all` 已通过。
- `cargo run -p scoop -- test` 暴露默认 refactor build fixture 阻塞。已修复一批前置清理（legacy/refactor 测试合同、runtime compaction test descriptor lifetime、部分 refactor value lowering），但保留原 fixture 形状后仍需 `P7-T02S` 处理 extern/native + GC handle/interpolated string、invalid literal 在 `.toString()` / narrow integer target 下的诊断传播、以及 `Task.step()` non-`Unit` handle arm completion payload source。最终确认的当前首个 blocker：`tests/fixtures/build/extern_enter_native_no_statepoint_writeback.scoop` 因 refactor value primitive 不支持 interpolated string lowering pending 而失败。
- 按无 workaround 规则，已新增前置任务 `P7-T02S` 到 `TODO-P7.md` 并同步 `TODO.md`，使下一次执行先修复这些默认 build fixture 阻塞；`P7-T03` 保持未完成并依赖 `P7-T02S`。已恢复被临时改弱的 blocker fixtures，避免把缺口隐藏掉。
- 下一步：提交当前修复与任务同步变更，然后停止。
