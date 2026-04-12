# 本轮执行计划（外显版）

## 目标

完成并收尾首个未完成任务 `T2003c0c2b3d4`，确保代码、测试、任务文档、计划文档和进度记录一致，然后提交一次 git commit，提交后立即停止，不继续处理后续任务。

## 已知前置上下文

- 交接信息表明：`T2003c0c2b3d4` 的代码实现、fixture 更新、`TODO.md` / `PLAN.md` / `memory/claude_plan.md` 的部分更新，以及全量测试都已完成。
- 交接信息同时表明：尚未做最终收尾确认和 git commit。
- 本轮优先级因此是“验证收尾状态并提交”，而不是继续实现新功能。

## 执行步骤

1. 检查最新一次提交说明，确认是否提到需要先修复的既有问题。
2. 检查工作区状态，确认当前改动是否只包含本轮任务相关文件。
3. 打开并核对 `TODO.md`、`PLAN.md`、`memory/claude_plan.md`：
   - `TODO.md` 中 `T2003c0c2b3d4` 是否已标记完成。
   - `PLAN.md` 是否已记录本轮完成情况，并把下一步指向后续任务。
   - `memory/claude_plan.md` 是否需要补上最终完成与验证结果。
4. 若上述文档有缺口，补齐并保存。
5. 复查 `git diff --stat` / `git status --short`，确认无异常改动。
6. 如有必要，补充一次轻量确认；若交接中的测试结果已充分且工作区未再变化，则不重复跑全量重测试。
7. 提交本轮改动，提交信息使用：
   - `[T2003c0c2b3d4] Support while-body mixed direct/indirect escape dispatch`
8. 提交后停止，不继续处理下一个任务。

## 判断原则

- 只完成一个任务：本轮只收尾并提交 `T2003c0c2b3d4`。
- 不回滚不属于本轮的用户改动。
- 若发现文档状态与交接不一致，先修正文档，再提交。
- 若发现最新提交明确提到未修复问题，先修该问题；如果没有，则继续本轮任务收尾。

## 过程中需要持续更新的内容

- 如果发现文档缺口或执行步骤变化，更新本文件。
- 在确认任务完成、测试状态明确、提交完成后，把最终状态补写到本文件中。

## 当前检查结果

- 最新提交 `a812b07` 为 `[T2003c0c2b3d3] Support if-branch mixed escape dispatch`，提交说明中未发现需要先于本轮任务处理的既有问题。
- 工作区改动与交接一致，集中在：
  - `mixed.rs` 的 while mixed lowering 修复；
  - `TODO.md` / `PLAN.md` / 本文件的收尾更新；
  - while mixed 新增 run-pass fixture；
  - block / if mixed 既有 fixture 的 golden 同步更新；
  - while mixed build 负例文本更新。
- `TODO.md` 已把 `T2003c0c2b3d4` 标记为 `[DONE]`，并附完成说明与验证命令结果。
- `PLAN.md` 已记录本轮完成情况，当前下一步为 `T2003c0c2c`；顺序列表中的 `T2003c0c2b3d4` 状态也已同步为“已完成”。
- 交接信息表明以下验证已完成且当前工作区未再出现超出本轮范围的新改动，因此本轮不重复跑全量测试：
  - `cargo fmt --all`
  - `cargo test --all`
  - `cargo run -p scoop -- test`
  - `cargo run -p scoop --features llvm -- test`
  - `cargo clippy --workspace --all-targets -- -D warnings`
- 当前待完成事项只剩：
  - 最终复查 `git status --short` / `git diff --stat`
  - 提交 git commit
