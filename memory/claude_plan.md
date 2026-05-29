# 执行计划

## 范围

- 以 `TODO.md` 作为权威任务列表。
- 只处理第一个标题未带 `[DONE]` 的任务。
- 完成该任务并提交后停止，不进入下一个任务。

## 步骤

1. 读取 `TODO.md`，定位第一个未完成任务及其验证要求。
2. 只检查该任务需要的代码、文档和最近提交上下文。
3. 完成所需实现或 review 修复；若出现具体阻塞前置项，则更新 `TODO.md` 并提交后停止。
4. 按要求依次运行 `cargo fmt`、`cargo clippy --all-targets -- -D warnings`，再运行相关度量、完整测试和 fixture suite。
5. 在标记任务完成前处理所有已观察且未安排的失败。
6. 更新 `TODO.md` / `TODO-1.md`，为完成任务标题加 `[DONE]` 并填写完成记录。
7. 提交本任务相关全部改动。

## 进度

- 已在执行前初始化本计划文件。
- 已识别第一个未完成任务：`TODO-1.md` 中的 `P1-T01R`。
- 最近提交是 `[P1-T01] Implement GC pacing core`，与当前 review 直接相关；提交标题未显示需要另立前置任务的未完成事项。
- review 聚焦 `ScoopGcHeap` pacing 字段、alloc 侧 request 设置、safepoint 消费、cycle 末 `next_gc` 更新、长程序有界性，以及 pacing-off 对照要求。
- review 发现 pacing-off 对照路径缺失；已补齐真实的 `SCOOP_GC_PACING=off` 初始化路径，避免用命令或文档绕过验证。
- 已完成验证：`cargo fmt`、`cargo clippy --all-targets -- -D warnings`、默认 heap-growth 度量、`SCOOP_GC_PACING=off` heap-growth 度量、`cargo test -p scoop_runtime --test gc_immix_allocator`、`cargo test --all --all-targets`、`python3 tools/run_fixtures.py` 均通过。
- 已更新 `TODO.md` 与 `TODO-1.md`，将 `P1-T01R` 标记为 `[DONE]` 并记录 review 发现、修复和验证结果。
