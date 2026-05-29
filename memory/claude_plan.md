# 当前执行计划

## 约束

- 只处理 `TODO.md` 中第一个标题未带 `[DONE]` 的任务，完成后停止。
- `TODO.md` 是任务顺序、依赖、验证要求和完成记录的权威来源。
- 不做开放式历史问题清扫；只处理当前任务及其直接阻塞问题。
- 若遇到阻塞当前任务的缺失特性、规格不符或未安排的测试失败，先修复；若无法在本次完成，则在 `TODO.md` 中加入最小前置任务并提交后停止。
- 完成任务后必须更新 `TODO.md` 标题为 `[DONE]` 并补充完成记录，然后提交 Git。

## 步骤

1. 读取 `TODO.md`，识别第一个未完成任务及其验证要求。
2. 检查最近提交是否明确提到与该任务直接相关的未完成事项。
3. 阅读当前任务涉及的代码、测试、规格或夹具，确认实现边界。
4. 按任务要求做最小正确实现；如发现直接阻塞问题，更新本计划并按规则处理。
5. 运行格式化、lint、相关测试；需要时运行完整测试和 fixture 套件。
6. 修复验证中暴露的与当前任务相关的问题；未安排的失败按规则修复或排入 `TODO.md`。
7. 更新 `TODO.md` 完成记录并为任务标题加 `[DONE]`。
8. 检查 Git diff/status，提交本次任务相关全部改动。
9. 停止，不处理下一个任务。

## 当前进度

- 已读取 `TODO.md` 和 `TODO-1.md`，第一个未完成任务是 `P1-T01：实现 pacing 核心 next_gc + request_collect + safepoint + 阈值`。
- 最近提交为 `P0-T03R` review，不包含与 `P1-T01` 直接相关的未完成事项。
- 下一步聚焦 `runtime/c/scoop_gc.h`、`runtime/c/scoop_runtime.c`、`runtime/c/scoop_gc_backend_immix.c` 的 pacing 字段、alloc 计数点、safepoint 消费点和 GC cycle 末更新点。
- 已确认现有 `bytes_allocated` / `bytes_freed` 是累计调试计数；为保持该 ABI/测试语义，`next_gc` 将保存累计分配水位线：`bytes_freed + max(4MB, live * 1.5)`。这等价于目标 live heap 模型，同时让 alloc hot path 只做 relaxed `bytes_allocated >= next_gc` 比较。
- 下一步执行小 patch：先扩展 `ScoopGcHeap` 字段并初始化，再接入 Immix pacing helper、safepoint 消费与 GC cycle 末更新。
- 已扩展 `ScoopGcHeap`，所有 backend init 均初始化 `next_gc/request_collect`；Immix 已在分配登记后做 relaxed 阈值比较并置 request flag，`scoop_gc_safepoint_poll` 已消费 request 并在分配前 collect，full GC cycle 末已更新下一次水位线。
- 已在 Immix allocator 集成测试中新增默认 pacing 阈值回归，验证超过 4MiB 后无需显式 GC 也会释放无根对象且 live heap 保持接近默认 floor。
- 下一步运行 `cargo fmt`，随后执行定向测试；若通过再进入 clippy、完整 Rust 测试和 fixture 验证。
- `cargo fmt`、`cargo test -p scoop_runtime --test gc_immix_allocator`、`cargo clippy --all-targets -- -D warnings` 均已通过。
- P0-T03 10M heap-growth 度量已通过：默认 Immix pacing 下 `peak_live=3725312`、`peak_reserved=4456448`，不再随 320MB 累计分配无界增长。
- 完整 `cargo test --all --all-targets` 已通过。
- 完整 `python3 tools/run_fixtures.py` 已通过，汇总 `fixtures: ok (1608)`。
- 已更新 `TODO.md` 与 `TODO-1.md`，将 `P1-T01` 标记为 `[DONE]` 并写入实现、度量和验证记录。
- 在 `runtime/c/scoop_runtime.c::scoop_alloc` 的 alloc 前 safepoint 注释中补充了 pacing request 消费语义；这是注释/记录更新，不改变编译输出，复用此前完整绿色验证结果。
- 下一步重新检查 `git status`、`git diff`、最近提交，并提交本任务全部改动。
