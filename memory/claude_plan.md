## 本轮目标

- 按 `TODO.md` / `PLAN.md` 当前拆分结果，确认本轮只完成并提交首个已实现完毕的任务 `T3010b2a`，提交后停止，不进入后续任务。

## 已知上下文

- 上一轮已把原始首个未完成任务 `T3010b2` 拆分为更小任务，并已在 `TODO.md` / `PLAN.md` 中调整顺序。
- `T3010b2a` 的代码实现、测试验证、文档更新已经完成，但还没有 git commit。
- 当前端到端失败点已经收敛为后续任务 `T3009a` 范围内的 immediate-resume arm lowering 问题，不属于 `T3010b2a` 未完成。
- 工作区存在用户自己的无关改动 `run_agent.sh`，提交时必须排除。

## 执行计划

1. 检查最新提交信息，确认是否提到需要先修复的遗留问题；若有，则按要求先处理。
2. 检查工作区状态，确认本轮待提交文件集合与上一轮实现摘要一致，没有混入无关改动。
3. 复核 `TODO.md`、`PLAN.md`、`memory/claude_plan.md` 是否已经准确反映：
   - `T3010b2a` 已完成；
   - 下一未完成任务是 `T3010b2aR`；
   - `T3009a` 是 `T3010b2b` 的前置依赖。
4. 如有必要，对 `memory/claude_plan.md` 追加进度说明。
5. 仅暂存 `T3010b2a` 对应代码和计划文件，不包含 `run_agent.sh` 或其他无关文件。
6. 以 `[T3010b2a] 基于 resume_path 引入 synthetic resume slot` 创建提交。
7. 提交后再次检查状态，确认工作树仅剩未纳入本次提交的无关改动，然后停止。

## 进度记录

- 已完成：写入本轮计划文件。
- 已完成：检查最新提交 `[T3010b1] 冻结跨 suspend resume_path 合同`，提交说明中没有额外遗留问题需要先行处理。
- 已完成：核对工作区，确认本轮待提交文件为 `PLAN.md`、`TODO.md`、`crates/scoopc/src/hir/mod.rs`、`crates/scoopc/src/llvm/codegen/effect/state_machine_emitter.rs`、`crates/scoopc/src/llvm/codegen/effect/state_machine_plan.rs`、`crates/scoopc/src/llvm/codegen/effect/state_machine_segments.rs`、`memory/claude_plan.md`；`run_agent.sh` 为无关改动，必须排除。
- 待完成：暂存上述文件、创建 `[T3010b2a]` 提交、复核工作区并停止。
