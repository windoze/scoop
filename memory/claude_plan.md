# Claude Plan

## 当前轮次目标
- 按 `TODO.md` 顺序找到第一个未完成任务，仅完成该任务后停止。

## 初始执行计划
1. 读取 `TODO.md`，确认第一个标题未标记为 `[DONE]` 的任务。
2. 检查最近提交是否有与该任务直接相关且明确未完成的问题；若存在且会阻塞当前任务，则先在 `TODO.md` 中记录为前置依赖并停止。
3. 阅读与当前任务直接相关的代码、测试、规范和任务说明，确认实现边界与验证要求。
4. 以最小且正确的改动完成任务实现；若遇到真实阻塞且无法在本轮内按规范完成，则在 `TODO.md` 中补充最小前置任务并保持当前任务未完成。
5. 运行任务要求的验证，包括相关测试、必要的 `cargo fmt`、`cargo test`、`cargo clippy --all-targets -- -D warnings`，并修复出现的问题。
6. 更新 `memory/claude_plan.md` 记录关键进展与计划变化。
7. 在 `TODO.md` 中将完成的任务标题加上 `[DONE]`，补全完成记录；仅在阶段计划真实变化时更新 `PLAN.md`。
8. 按仓库提交风格创建一次 git 提交，然后停止，不继续处理下一个任务。

## 说明
- 该文件记录高层执行计划、关键决策与进展，不记录冗长的内部推理。

## 进展记录
- 2026-05-08：已读取 `TODO.md` 顶部索引，确认首个未完成任务为 `CG-T07S0a16`（修复 `literal_array_expected_type_nested_basic.scoop` 中嵌套 `Array<UInt8>` element expected-type 传播退回 `Int` 的问题）。
- 2026-05-08：已检查最近提交 `014b9371 [CG-T07S0a15] Close hash map empty-table blocker`；提交内容与当前 `CG-T07S0a16` 没有直接的未完事项提示，当前按 `CG-T07S0a16` 继续执行。
- 2026-05-08：下一步读取 `CG-T07S0a16` 任务正文、失败 fixture 与 expected-type 传播相关实现，先复现问题，再定位 contract 缺口位于 typecheck、HIR lowering、MIR/materialization 还是 codegen。
- 2026-05-08：已复现 `literal_array_expected_type_nested_basic.scoop` 失败，实际输出为 `false / 2.5 / false / 0.75 / 1.5 / 3.5`；其中 `bytes.get(0) == 3` 与 `argByte == 4` 仍错误。
- 2026-05-08：对照验证发现更前置 regression：`literal_numeric_expected_type_absorption_basic.scoop` 也重新失败，build/run 末两行回退为 `false` / `false`。`dump-mir` 显示 direct `Array<UInt8>` path 中 `__scoop_array_builder_push` 仍保留 `UInt8` scalar transport，但 `scoop.core.get` / compare path 把 element surface 发布成 nominal/composite `Struct`。
- 2026-05-08：根据任务规则，当前无法直接继续 `CG-T07S0a16`；已决定在 `TODO.md` 中前插最小 prerequisite `CG-T07S0a16a`，把 direct `Array<UInt8>` regression 记录为当前任务的更前置 blocker。本轮只提交 `TODO.md` 与 `memory/claude_plan.md` 的顺序修正，不做代码实现。
