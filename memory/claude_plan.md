执行计划（公开版）

1. 读取 `TODO.md`，只定位第一个标题未带 `[DONE]` 的任务，不进行开放式历史问题扫描。
2. 查看该任务的要求、依赖、验证方式，并按需读取相关代码和测试。
3. 如任务可直接完成，实施最小正确修改；如发现阻塞当前任务的缺失特性或规格不匹配，按要求把最小前置任务写入 `TODO.md` 并停止。
4. 针对改动运行相关测试；若失败，继续修复直到相关验证通过或确认存在必须登记的阻塞项。
5. 更新 `TODO.md`：完成时在任务标题前加 `[DONE]` 并填写完成记录；仅在阶段计划变化时更新 `PLAN.md`。
6. 运行必要的格式化和验证命令，检查工作区变更。
7. 按当前任务提交所有相关未提交文件，然后停止，不处理下一项任务。

进度记录

- 已创建初始执行计划，下一步读取 `TODO.md` 识别首个未完成任务。
- 已读取 `TODO.md`，首个标题未带 `[DONE]` 的任务是 `P5-T01：做全量稳定化、跨平台矩阵与文档收尾`。
- 下一步检查最新 Git 提交是否记录了与 `P5-T01` 直接相关的未完成事项；随后只围绕 `P5-T01` 做文档/注释收尾、验证矩阵复核与完成记录更新。
- 最新提交为 `[P5-T00] Update execution log`，未记录与 `P5-T01` 直接相关的未完成事项。
- 已审计当前工作区既有修改：`PLAN.md` / `MANAGED_ABI.md` / sysroot / runtime / runtime ABI 注释均在向 P5 最终状态收口；不回退这些变更。
- remaining public string helper special-case audit：compiler/runtime 中未发现 `scoop_string_length/to_int/hash/is_empty/replace/char_at/repeat/compare_to/trim_indent` production callsite/declaration 残留；仅 sysroot helper、overlay fixture、comptime 对齐注释与 archive 文档命中。当前仍保留的 `byteLength/getByte/unsafeSliceBytes`、`scoop_string_concat`、`scoop_string_equals`、f-string formatting helper 均属于已记录 substrate 或 v2+ cleanup。
- 本机 macOS/aarch64 P5 验证已通过：`cargo fmt --all`、`cargo test --all --all-targets`、`cargo run -p scoop -- test`、`cargo test -p scoopc llvm_tests -- --nocapture`（filter 返回 0 passed / 864 filtered out）、`cargo test -p scoopc native_callable -- --nocapture`、`cargo test -p scoopc managed_extern_ -- --nocapture`、targeted native/managed ABI fixture、`SCOOP_GC_MOVE=1 SCOOP_GC_STRESS=1` string helper fixture、`cargo clippy --all-targets -- -D warnings` 均通过。
- 注意：`cargo run -p scoop -- test --fixtures <多路径>` 曾因 CLI 只接受单个 `--fixtures` path 报 `unexpected argument`；已改为逐条运行 targeted fixture 并全部通过，不属于代码缺陷。
- 已更新 `TODO.md`：任务索引与 `P5-T01` 标题均加 `[DONE]`，完成记录已写入改动范围、核心决策、验证结果与 `PLAN.md` / `MANAGED_ABI.md` 闭合说明。

P5-T01 执行计划

1. 查看最新提交摘要，确认是否有直接相关的未完成问题需要并入本任务。
2. 读取 `PLAN.md`、`MANAGED_ABI.md`、sysroot/runtime 相关注释，核对 P5 要求的最终 contract、substrate list 与 v2+ 边界是否已完整且一致。
3. 修正文档或注释中仍过时的 body-less/runtime-route/string/native ABI 描述；不改变已完成实现的 ABI surface。
4. 运行 `P5-T01` 指定的 macOS/aarch64 本机验证命令，并引用 `P5-T00` 已固化的 Linux/amd64 CI 记录作为跨平台矩阵。
5. 更新 `TODO.md`，把 `P5-T01` 标记为 `[DONE]` 并填写改动范围、核心决策、验证结果、与 `PLAN.md` / `MANAGED_ABI.md` 的闭合。
6. 运行格式化/必要检查，查看工作区状态，按任务提交所有相关更改后停止。
