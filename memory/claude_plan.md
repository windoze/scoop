# 执行计划

说明：本文件记录可审计的执行计划与进度更新，不记录私密逐字思考。

## 当前计划

1. 阅读 `TODO.md`，按标题是否带 `[DONE]` 判定并锁定第一个未完成任务。
2. 阅读该任务相关上下文，必要时检查 `PLAN.md`、最近提交和相关代码/测试，避免做开放式历史问题扫荡。
3. 按任务要求实现最小正确变更；如果发现阻塞当前任务的缺失特性或规格不一致，优先修复或在 `TODO.md` 中插入最小必要前置任务并停止。
4. 运行格式化、lint、相关测试，并在需要时运行完整测试/fixture 套件。
5. 更新 `TODO.md`：完成时在任务标题前加 `[DONE]` 并填写完成记录；仅在阶段计划变化时更新 `PLAN.md`。
6. 检查工作区差异，提交本次任务相关全部变更，然后停止，不进入下一个任务。

## 进度

- 已创建初始执行计划，下一步读取 `TODO.md` 识别第一个未完成任务。
- 已读取 `TODO.md`，第一个未完成任务为 `T2-04：per-callable fact 挂到 callable 节点`。
- 当前任务目标：把 `LirFacts.callables`、`source_signatures`、`intrinsic_callables` 的 value 内容迁入 `LateLoweredCallable` 或其节点旁挂结构；消费侧不再通过 FQN/string map 查找 source/intrinsic signature。
- 最近提交为 `T2-03-R` review，未发现直接声明的未完成阻塞项。
- 选定实现路径：保留 `LirFacts` 平表作为当前验证/序列化投影，但在构建 facts 后把 callable facts、source signatures、intrinsic metadata 回填到 `LateLoweredProgram.callables`；LLVM codegen 的 source signature / intrinsic / callable facts 查询改为从 program/callable 节点读取。
- 已实现核心迁移：`LateLoweredCallable` 新增 per-callable facts/source signatures/intrinsic payload，`LateLoweredProgram` 新增声明旁挂结构和查询方法；`effect_lowering_stage` 在构建 facts 后回填 payload；LLVM codegen 消费侧已切换到 program/callable 节点查询。
- `cargo clippy --all-targets -- -D warnings` 已通过。
- `cargo test --all --all-targets` 首次运行发现 `frontend::tests::dependency_frontend_cache_hit_uses_artifact_without_reading_source` 解码 `lir_program.bin` EOF；该失败与新增 LIR program 序列化字段直接相关，下一步修复缓存测试/artifact 序列化构造后重跑完整验证。
- 已修复 bincode EOF：新加的 LIR program 字段不再使用 `skip_serializing_if`，避免非自描述 bincode payload 省略字段。
- `python3 tools/run_fixtures.py` 首次运行发现 3 个 fixture 失败；根因是 codegen 只切换 active `LirFacts`，未同步切换 active `LateLoweredProgram`，迁移后导致 source-site / signature / intrinsic 查询混用。已新增 active LIR program 上下文并修复 3 个失败 fixture 的定向回归。
- 最终验证已通过：`cargo fmt`；`cargo clippy --all-targets -- -D warnings`；`cargo test --all --all-targets`；`cargo build -p scoop -p scoopc`；`python3 tools/dependency_gate.py`；`python3 tools/spec_fixtures.py check`；`python3 tools/run_fixtures.py`。
- 已将 `TODO.md` 中 `T2-04` 标记为 `[DONE]` 并填写完成记录；下一步检查 git diff/status 后提交本任务变更。
