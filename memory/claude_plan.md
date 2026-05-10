## 当前执行计划

1. 读取 `TODO.md`，识别第一个标题未带 `[DONE]` 的任务；必要时查看最新提交是否有与该任务直接相关的未完成说明。
2. 读取该任务涉及的说明、依赖与验证要求，并检查相关代码与测试位置。
3. 如任务可直接完成：实现代码改动，补充或调整测试，并进行必要验证。
4. 如发现阻塞当前任务的真实前置问题：先最小化更新 `TODO.md` 以加入前置任务并记录依赖，必要时更新 `PLAN.md`。
5. 在执行过程中持续更新本文件，记录当前任务、关键决策、已完成步骤与验证结果。
6. 任务完成后：更新 `TODO.md` 完成状态与完成记录，按需更新 `PLAN.md`，提交 git commit，然后停止，不进入下一个任务。

## 进度记录

- 已创建初始执行计划，下一步读取 `TODO.md` 并锁定当前任务。
- 已读取 `TODO.md` 与 `TODO-P8.md`，当前任务锁定为 `P8-T04`：在只剩新主线的前提下重跑完整回归矩阵并完成最终收口记录。
- 当前执行顺序：
  1. 检查 git 工作树状态，确认本次改动基线。
  2. 依任务要求运行完整矩阵：`cargo test --all`、`cargo run -p scoop -- test`、`cargo run -p scoop_tools -- spec-fixtures check`、`cargo clippy --all-targets -- -D warnings`、两条 GC 环境 fixture 回归。
  3. 任一步骤失败时，定位失败点，修复仅限 refactor 主线/中立共享模块/tests/docs；禁止恢复 legacy/fallback。
  4. 修复后重跑受影响步骤，并在收尾时至少再完整重跑一遍整个矩阵。
  5. 运行任务要求的 residual 搜索，分类说明剩余命中为何安全或继续清理。
  6. 更新 `TODO-P8.md`、`TODO.md` 与本文件，完成 git commit 后停止。
- 已检查工作树：存在未提交改动 `crates/scoopc/src/llvm/codegen/effect_refactor/layout.rs`、`crates/scoopc/src/mir/lower.rs`、`runtime/c/scoop_runtime.c`，当前先保留并在本任务上下文内一并处理，不做回退。
- 关键进度：`cargo test --all` 已通过。
- 关键进度：`cargo run -p scoop -- test` 已通过（`fixtures: ok (1270)`）。
- 关键进度：`cargo run -p scoop_tools -- spec-fixtures check` 已通过（`spec fixtures: ok (1)`）。
- 关键进度：`cargo clippy --all-targets -- -D warnings` 已通过。
- 关键进度：`SCOOP_GC_MOVE=1 SCOOP_GC_STRESS=1 SCOOP_GC_VERIFY_ROOTS=1 cargo run -p scoop -- test --fixtures tests/fixtures/run-pass` 已通过（`fixtures: ok (391)`）。
- 当前 blocker：`SCOOP_GC_MOVE=1 SCOOP_GC_STRESS=1 SCOOP_GC_VERIFY_ROOTS=1 cargo run -p scoop -- test --fixtures tests/fixtures/runtime_gc` 失败；`gc_stw_cross_thread_roots_basic.scoop` 在 5000ms fixture 超时内未完成。下一步复现单 fixture，区分真实死锁/卡死与超时预算不合理。
- 已定位 blocker：不是 GC 语义死锁；在整组 `runtime_gc` 压测里，`gc_stw_cross_thread_roots_basic` 的 `scoop run` 编译+执行耗时会超过 5s。已把 `gc_stw_cross_thread_roots_basic.scoop` 与同类 `gc_stw_cross_thread_in_native_roots_basic.scoop` 的 `TIMEOUT` 从 5000ms 调整到 10000ms，并补充注释说明预算原因。
- 关键进度：修复后 `SCOOP_GC_MOVE=1 SCOOP_GC_STRESS=1 SCOOP_GC_VERIFY_ROOTS=1 cargo run -p scoop -- test --fixtures tests/fixtures/runtime_gc` 已通过（`fixtures: ok (25)`）。
- 关键进度：已按 `P8-T04` 要求完整重跑一遍全矩阵，并全部通过：`cargo test --all`、`cargo run -p scoop -- test`、`cargo run -p scoop_tools -- spec-fixtures check`、`cargo clippy --all-targets -- -D warnings`、GC stress `run-pass`、GC stress `runtime_gc`。
- residual 搜索结论：未发现可执行 legacy 主线、selector 或旧 effect/continuation 主实现残留；剩余命中均为历史 TODO/PLAN/PIPELINE 文档、删除/负向测试断言、或命名中带 `legacy` 的非生产诊断/fixture 名称。搜索中的 `crates/scoop/target/fixtures/effect_refactor_no_legacy_handler_stack_calls.ll` 为仓库内静态 IR 产物名，不承载可执行 selector/legacy backend。
- 下一步：更新 `TODO-P8.md` 与 `TODO.md`，把 `P8-T04` 标记为完成并写入验证摘要，然后提交 git commit。
