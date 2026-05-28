# 当前执行计划

## 约束与判断摘要

- `TODO.md` 是任务顺序和完成状态的唯一来源；只处理第一个标题未带 `[DONE]` 的任务。
- 开始实现前先确认当前任务、依赖、验证要求和最近提交是否包含与该任务直接相关的未完成问题。
- 不做开放式历史问题清扫；只有阻塞当前任务或由当前任务引入的失败才进入本轮范围。
- 如遇到不可绕过的缺失特性、规格不匹配或测试失败，优先修复；若无法在当前任务内正确完成，则在 `TODO.md` 中插入最小必要前置任务，提交后停止。
- 不在 `PLAN.md` 记录例行执行日志；仅当阶段级计划、依赖或完成标准发生变化时更新。
- 完成本轮任务后必须更新 `TODO.md` 标题为 `[DONE]`，记录验证结果，提交 Git，然后停止。

## 步骤计划

1. 读取 `TODO.md`，定位第一个标题未带 `[DONE]` 的任务，并记录任务编号、要求、依赖与验证项。
2. 查看最近提交摘要，确认是否有与该任务直接相关的未完成问题需要纳入当前任务或作为前置任务。
3. 根据任务范围检查相关代码、测试、规格或文档，避免假设实现细节。
4. 若任务可以直接完成，则做最小正确实现，并同步补充或更新必要测试/fixture。
5. 运行格式化与 lint：先 `cargo fmt`，再 `cargo clippy --all-targets -- -D warnings`。
6. 在 lint 通过后运行任务要求的相关测试；若代码语义发生变化，再运行完整 Rust 测试和 fixture 套件，分别使用足够长的超时。
7. 对发现的失败按测试失败策略处理：修复或在 `TODO.md` 中安排明确前置/后续任务，不能无记录忽略。
8. 更新 `TODO.md`：将当前任务标题加 `[DONE]`，补全完成记录、测试命令与结果；必要时更新 `PLAN.md`。
9. 检查工作区差异，确保未误改无关文件；提交所有本轮相关变更，提交信息包含任务编号。
10. 停止，不处理下一个任务。

## 进度记录

- 已写入初始计划，下一步读取 `TODO.md` 确认首个未完成任务。
- 已确认首个未完成任务：`P3-T04`，目标是允许 refutable `val` pattern，并在运行期 mismatch 时调用 `panic("pattern mismatch")`。
- 已检查最近提交：最新提交为 `P3-T03R`，未发现直接指向 `P3-T04` 的未完成项。
- 工作区存在非本轮改动：`run_agent.sh`、`GC_IMMORTAL_FIX.md`；本轮不触碰、不提交这些文件。
- 下一步检查 `SPEC_FIX.md` C3、parser/typecheck/lowering 现状和相关 fixtures。
- 已完成首轮实现：`val_pat` 不再拒绝 variant pattern，改为校验 enum/variant/arity 并注入 payload binder 类型；pattern lowering 的 mismatch fallback 改为 `panic("pattern mismatch")`；新增/更新 typecheck 与 run-pass fixtures 覆盖匹配、mismatch panic、嵌套 tuple pattern 和顶层 pattern。
- 下一步运行格式化、构建/targeted fixtures，并根据失败修正。
- 已通过 `cargo fmt`、`cargo clippy --all-targets -- -D warnings`、`cargo build -p scoop -p scoopc` 与 P3-T04 targeted fixtures。
- 下一步运行完整 `cargo test --all --all-targets` 和完整 `python3 tools/run_fixtures.py`。
- 已通过完整 `cargo test --all --all-targets` 与完整 `python3 tools/run_fixtures.py`（`fixtures: ok (1558)`）。
- 已将 `TODO.md` / `TODO-3.md` 中 `P3-T04` 标记为 `[DONE]` 并填写完成记录。
- 下一步检查 diff / status / log，确认只提交本轮相关文件，然后提交。
