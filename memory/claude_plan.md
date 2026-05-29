# 当前执行计划

## 范围

- 目标：严格按照 `TODO.md` 完成第一个标题未带 `[DONE]` 的任务，然后停止。
- 约束：`TODO.md` 是任务顺序和完成状态的唯一权威来源；`PLAN.md` 只在阶段级计划变化时更新。
- 说明：本文件记录可公开的执行计划、进度和关键决策，不记录隐藏推理细节。

## 步骤

1. 读取 `TODO.md`，识别第一个未完成任务，并确认其依赖、验证要求和完成记录格式。
2. 检查最近提交和当前工作区状态，确认是否有与当前任务直接相关的未完成问题或已有未提交修改。
3. 根据当前任务读取相关代码、测试和文档，只做必要范围内的上下文构建。
4. 如果任务可直接完成，实施最小正确改动；如果发现阻塞当前任务的缺失特性或规格不匹配，在 `TODO.md` 中加入最小 prerequisite 任务并停止。
5. 按要求运行格式化、lint 和相关测试；若代码变更影响面较大，依次运行 `cargo fmt`、`cargo clippy --all-targets -- -D warnings`、`cargo test --all --all-targets` 和 `python3 tools/run_fixtures.py`。
6. 若发现未被明确排期的测试或 fixture 失败，修复它，或在 `TODO.md` 中把最小修复任务排到完成当前任务之前。
7. 完成后在 `TODO.md` 中给任务标题加 `[DONE]` 并更新完成记录；仅当阶段级计划变化时更新 `PLAN.md`。
8. 提交所有与本次任务相关的修改，提交信息包含任务编号和简短说明。
9. 停止，不继续处理下一个任务。

## 当前状态

- 已读取 `TODO.md` 与 `TODO-2.md`。
- `TODO.md` 当前工作区版本中第一个未完成任务显示为 `P2-T02`，但该状态来自未提交改动。
- `TODO-2.md` 中 `P2-T01R` 仍是 `[TODO]`，且 `P2-T02` 声明依赖 `P2-T01R`。
- 最近提交为 `[P2-T01] Trigger minor GC on nursery full`，没有 `P2-T01R` 提交；工作区已有未提交 runtime/test/TODO 改动，疑似上一轮 review 未完成。
- 决策：先完成 `P2-T01R` 这个直接 prerequisite/review 任务并提交，然后停止，不进入 `P2-T02`。
- 已复核 `P2-T01` 的 nursery-full minor GC 行为、现有未提交修正和相关测试。
- 已运行并通过：`cargo test -p scoop_runtime --test gc_immix_minor_old_edges -- --test-threads=1 --nocapture`、`cargo test -p scoop_runtime --test gc_immix_nursery -- --test-threads=1 --nocapture`、`cargo test -p scoop_runtime --test gc_immix_write_barrier -- --test-threads=1 --nocapture`、`cargo test -p scoop_runtime --test gc_immix_minor_collect -- --test-threads=1 --nocapture`。
- 发现并确认的 review 风险：P2-T01 后 minor GC 依赖“无 old→nursery 边”不变式；large/fallback 对象与 nursery 引用闭包需要更保守处理。
- 已清理 promote 上下文中的未使用字段。
- 已完成正式验证：`cargo fmt`；`cargo clippy --all-targets -- -D warnings`；`cargo test --all --all-targets`；`python3 tools/spec_fixtures.py check`；`python3 tools/run_fixtures.py`。
- 已更新 `TODO-2.md`，将 `P2-T01R` 标为 `[DONE]` 并补充完成记录；`TODO.md` 索引已显示 `P2-T01R` 完成。
- 下一步检查 diff/status，提交 P2-T01R 相关修改，然后停止。
