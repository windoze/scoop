# 执行计划

1. 读取 `TODO.md`，按文件顺序找到第一个标题未带 `[DONE]` 的任务。
2. 查看该任务的要求、依赖、验证方式和完成记录；必要时查看最近提交，确认是否有直接相关的未完成问题。
3. 基于当前任务检查相关代码、测试和文档，避免进行无关历史问题排查。
4. 若发现当前任务被具体缺陷或缺失特性阻塞，先在 `TODO.md` 中加入最小必要前置任务并提交后停止。
5. 若可直接完成，按任务要求实现最小正确修改，并补充或更新对应测试/fixture。
6. 先运行 `cargo fmt`，再运行 `cargo clippy --all-targets -- -D warnings`，随后运行任务要求的相关测试；需要完整验证时运行完整 Rust 测试和 fixture 套件并使用足够超时。
7. 修复验证中发现且未被显式排期的失败；若不能在当前任务内修复，则在 `TODO.md` 中加入正确顺序的前置/后续任务并停止。
8. 完成后将当前任务标题加上 `[DONE]`，更新完成记录；仅在阶段计划确实变化时更新 `PLAN.md`。
9. 检查 git 状态和差异，提交本次任务涉及的所有变更，提交信息使用任务编号和简短描述。
10. 完成一个任务后停止，不继续处理下一个任务。

## 进度

- 已写入初始执行计划；下一步读取 `TODO.md` 识别第一个未完成任务。
- 已识别第一个未完成任务：`P2-T02R`，目标是复审 `tools/run_fixture_scan.sh` / `tools/run_run_pass_gc_scan.sh` / `tools/gc_microbench.sh` 的入口切换结果并完成端到端验证。
- 当前执行范围限定为 P2-T02R，不处理后续文档更新任务。
- 已检查相关 shell 脚本和最新提交：`tools/run_fixture_scan.sh` 与 `tools/run_run_pass_gc_scan.sh` 均通过 `python3 tools/run_fixtures.py` 驱动 fixture；`tools/gc_microbench.sh` 不涉及旧 fixture runner。
- 发现工作区存在 unrelated 未提交文件，将保持不动并仅提交本任务改动。
- 验证已完成并通过：旧入口搜索无命中，`cargo fmt`、`cargo clippy --all-targets -- -D warnings`、`cargo test --all --all-targets`、`python3 tools/run_fixtures.py`、`tools/run_fixture_scan.sh` parse 子集、`tools/run_run_pass_gc_scan.sh` 全量 run-pass GC 扫描、两个 `tools/gc_microbench.sh` smoke 均通过。
- 已将 `TODO.md` 中 `P2-T02R` 标记为 `[DONE]` 并追加完成记录；下一步检查差异并提交本任务改动。
