# Claude Plan

## Constraints
- 先按 `TODO.md` 索引定位首个未完成详细任务；`TODO-Px.md` 是任务真源。
- 一次只完成一个详细任务；若遇到阻塞，只补充最小前置任务并同步 `TODO.md`。
- 不采用规避方案；实现必须符合任务与规格要求。
- 完成后必须测试、更新任务记录，并按任务号提交 git commit，然后停止。

## Initial Execution Plan
1. 读取 `TODO.md`，确认详细任务文件引用与当前顺序。
2. 按顺序读取相关 `TODO-Px.md`，定位首个标题未带 `[DONE]` 的详细任务。
3. 查看最近一次提交信息，判断是否存在与该任务直接相关且未完成的问题；若有，将其视为任务内容或必要前置。
4. 阅读当前任务要求、依赖、验收条件与完成记录，确认需要修改的代码、测试与文档位置。
5. 检查相关实现与现有测试，确定最小正确改动方案；若发现阻塞当前任务的真实缺口，先记录为前置任务并同步任务索引。
6. 使用最小必要改动实现当前任务。
7. 运行与任务直接相关的测试；随后运行必要的全面验证（至少包括相关测试，必要时包括 `cargo test`、`cargo clippy --all-targets -- -D warnings`、格式检查或任务要求中的专用命令）。
8. 若验证失败，先修复问题并重新验证，直到当前任务达成或明确形成新的前置阻塞。
9. 更新 `TODO-Px.md`：将任务标题标记为 `[DONE]`，填写/补全完成记录；若任务索引内容受影响，同步更新 `TODO.md`。
10. 仅当阶段计划或依赖结构发生变化时更新 `PLAN.md`。
11. 复查工作区，确保不回退他人改动；按任务号撰写清晰 commit message 提交当前全部相关未提交更改。
12. 停止，等待下一次调用。

## Progress Log
- 已创建初始执行计划，下一步开始读取 `TODO.md` 和详细任务文件。
- 已从 `TODO.md` / `TODO-P6.md` 确认首个未完成详细任务为 `P6-T03`："按 P5 state graph / boundary contract 完成 refactor LLVM body lowering，停止在 backend 重做 state-machine transformation"。
- 已检查最近提交 `1ab1e399a4d87468f9c14b0fab3b0a00b2815c90`（`[P5-T07a] Fix pure caller runtime-error call projection`）；该提交是现有依赖的一部分，没有额外标明新的未完 blocker。
- 下一步：阅读 `crates/scoopc/src/llvm/codegen/effect_refactor/**`、当前 legacy LLVM body lowering、以及 P5 late-lowered IR/query API，判断实现边界与可能阻塞项。
- 已确认当前 blocker：`ResumeSiteEffectFacts` 只提供 continuation schema / out-step 语义，而当前 `P6-T02` ABI query 只发布 direct/dynamic callable entry 与 internal resume-interface method，尚未发布 `Continuation.resume(...)` surface lowering 的 authoritative LLVM call target / query contract。
- 基于该 blocker，已在 `TODO-P6.md` / `TODO.md` 新增最小前置任务 `P6-T02c`，并把 `P6-T03` 改为依赖该任务；阶段顺序未变化，因此未修改 `PLAN.md`。
- 下一步：检查变更、运行必要的最小验证（任务文件同步/工作区状态），然后提交本次“新增前置任务并停止”的结果。
