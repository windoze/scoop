# Claude Plan

## Planning Note

按你的要求，我先记录执行计划。出于安全与协作边界，我不会写出逐字的内部推理过程，但会持续维护一份完整、可检查的执行计划、关键判断、阻塞点和进度更新。

## Initial Execution Plan

1. 读取 `TODO.md`，确认第一个标题未标记 `[DONE]` 的任务。
2. 检查最近提交信息，判断是否存在与该任务直接相关且明确未完成的问题；若存在，则将其视为当前任务的一部分或在 `TODO.md` 中补成前置任务。
3. 阅读当前任务引用的相关代码、测试、规范和文档，只聚焦完成该任务所需的范围，不做开放式历史问题清扫。
4. 判断该任务是否可以按原样一次完成；只有在存在真实前置依赖缺失或阻塞问题时，才在 `TODO.md` 中最小化拆分或插入前置任务。
5. 实现当前任务所需修改，保持改动最小且符合既有代码风格。
6. 运行该任务要求的验证，以及受影响范围内必要的测试、格式化、lint 与构建检查；若失败则继续修复直到通过，或确认存在阻塞。
7. 更新 `memory/claude_plan.md`，记录关键进展、计划变化、验证结果和任何阻塞结论。
8. 若任务完成：
   - 在 `TODO.md` 中将该任务标题前缀改为 `[DONE]`
   - 更新该任务 completion record
   - 仅在阶段计划确实变化时更新 `PLAN.md`
9. 若任务无法按规范完成且存在真实阻塞：
   - 保持当前任务未完成
   - 在 `TODO.md` 中插入最小必要前置任务并明确依赖顺序
   - 仅在阶段级依赖变化时更新 `PLAN.md`
10. 按仓库提交风格创建一次 git commit，提交本次任务完成结果或阻塞调整结果，然后停止，不进入下一个任务。

## Progress Log

- 已创建本计划文件，下一步将读取 `TODO.md` 确认首个未完成任务。
- 已读取 `TODO.md`，确认首个未完成任务为 `TODO-P8.md` 中的 `P8-T04R`（`Review P8 阶段退出条件，确认仓库已真正收口到单一新主线且本轮工作结束`）。
- 已检查最新提交 `333e2a2 [P8-T04] Close final single-pipeline regression matrix`；提交信息未额外声明与 `P8-T04R` 直接相关的未完成 issue。
- 当前执行策略更新：`P8-T04R` 属于 review 任务，将重点复核 `TODO-P8.md` 中 P8-T01 ~ P8-T04 的验证要求是否仍可在“只有新主线存在”的前提下通过，并补充最小 smoke / 负向检查与 residual 搜索摘要。
- 下一步：读取 `TODO-P8.md` 中 P8-T01 ~ P8-T04 的验证条目，整理需要重跑的命令集合，然后开始执行验证。
- 额外发现：`TODO-P8.md` 中对 `PLAN.md` 的 P8/§4 第 10 条引用已失真；仓库中实际承载 effect-refactor 阶段计划的是 `PLAN-effect-refactor.md`，且其 §4 第 10/11 条正对应本任务需要复核的 full regression 与“旧主线已删除”标准。若验证通过，收尾时需最小修正该文档引用，避免 final review 继续指向错误计划文件。
- 已完成定向验证阶段：P8-T01/T01R、P8-T02/T02R、P8-T03/T03R、P8-T03aa、P8-T03a、P8-T03ab 所要求的核心测试、CLI negative check、selector/backend/docs residual 搜索、default virtual-cone 与 hidden-init LLVM 回归均已重新执行并通过。
- 执行细节说明：为避免把 `scoopc <file>` / `scoopc --obj <file>` 的默认输出写回 fixture 目录，本次 review 以等价的显式 `-o` 临时路径重跑了这两条 smoke；验证的入口与 code path 不变，仍是默认 virtual-cone artifact 路径。
- 下一步：执行 P8-T04 的完整 full regression 矩阵（含 GC env），再补跑 P8-T04R 指定的最小 smoke 和负向检查。
- 已完成 P8-T04 完整矩阵复验：`cargo test --all`、`cargo run -p scoop -- test`、`cargo run -p scoop_tools -- spec-fixtures check`、`cargo clippy --all-targets -- -D warnings`、GC stress `run-pass` 与 `runtime_gc` 均通过；结果分别为 `790 passed`、`fixtures: ok (1270)`、`spec fixtures: ok (1)`、`fixtures: ok (391)`、`fixtures: ok (25)`。
- 已完成 P8-T04R 额外最小 smoke 与负向检查：默认 `build/run/test` smoke 通过；`--effect-pipeline legacy` 负向检查稳定失败，确认 selector 没有回流。
- 已完成收尾文档更新：`TODO-P8.md` 已把失真的 `PLAN.md` 引用修正为 `PLAN-effect-refactor.md`，并把 `P8-T04R` 标记为完成；`TODO.md` 索引也已同步标记 `[DONE]`。
- 当前结论：effect-refactor 主线任务链已全部完成；下一步仅剩按任务要求提交当前结果并创建 `v0.1.0` 标签。
