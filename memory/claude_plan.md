# 执行计划记录

更新时间：2026-04-11

说明：
- 按要求先记录可公开的执行思路与步骤。
- 我不会写入内部私有推理细节，但会持续维护足够详细的计划、判断依据、进度和变更记录，便于检查执行过程。

当前目标：
1. 检查最新一次 Git 提交，确认是否提到仓库中已知但尚未修复的问题；若有，优先修复。
2. 阅读 `TODO.md`，定位第一个未完成任务。
3. 判断该任务是否足够小且可直接完成；若过大，则拆分任务并更新 `PLAN.md` / `TODO.md`。
4. 实现当前任务，并补充必要测试与文档更新。
5. 运行格式化、测试、`clippy` 等质量检查，修复发现的问题。
6. 更新 `TODO.md`、`PLAN.md`、`memory/claude_plan.md`，提交 Git commit，然后停止。

初始执行步骤：
1. 读取最新提交信息与变更摘要。
2. 读取 `TODO.md`、`PLAN.md`，理解当前任务序列与上下文。
3. 视需要读取相关源码与测试文件，明确实现范围。
4. 修改代码与测试。
5. 运行验证命令。
6. 更新任务状态并提交。

进度记录：
- [完成] 创建本计划文件并写入初始计划。
- [完成] 检查最新提交 `a7b19d2c414c5162d0665f3ab429678561adfa64` 与 `TODO.md` / `PLAN.md`。
- [结论] 最新提交信息仅说明完成 `T2003b3`（immediate-resume while-body perform），提交说明未额外挂出需要先修的历史问题；暂未发现“提交已知但未修复的问题”。
- [结论] 当前首个未完成任务为 `T2003c`：补 immediate-resume 的 mixed-arm / nested handle / GC stress 高风险回归矩阵。
- [完成] 读取现有 effect/immediate-resume 相关源码与 fixtures，判断 `T2003c` 的真实前置条件。
- [关键发现] `crates/scoopc/src/llvm/codegen/effect.rs` 中的 `codegen_handle_expr` 仍在 `handle.arms.len() != 1` 时直接返回 `UnsupportedMainBody { kind: \"handle arm count (only 1 supported)\" }`；这意味着 LLVM 后端尚不支持 mixed-arm handle。
- [结论] 原 `T2003c` 不是单纯“补回归矩阵”，而是依赖一个尚未落地的语言/后端能力：多 arm handle 的 LLVM dispatch，尤其是 single-source-handle 下的 mixed-arm immediate-resume。
- [决策] 按用户规则，不在缺少前置语言能力时强行实现回归矩阵；改为先在 `TODO.md` / `PLAN.md` 中显式插入该依赖任务，再停止。
- [完成] 已把原 `T2003c` 拆为 `T2003c0`（LLVM 多 arm handle dispatch 前置能力）+ `T2003c`（回归矩阵）。
- [完成] 已检查文档变更 diff，确认首个未完成任务现为 `T2003c0`，原 `T2003c` 已改为依赖它。
- [进行中] 准备提交本轮“依赖重排”。

针对 `T2003c` 审计后的修订计划：
1. 盘点现有 immediate-resume fixtures，确认 mixed-arm、nested handle、GC stress 各自是否已有覆盖或已知缺口。
2. 阅读 LLVM effect lowering 中与 immediate-resume / nested handle / mixed-arm 相关的代码，确认预期支持边界。
3. 若 mixed-arm 只缺回归，则直接补 fixtures；若 mixed-arm 依赖缺失的 LLVM 多 arm dispatch，则先把该能力显式建模为新的前置任务。
4. 更新 `TODO.md`、`PLAN.md` 与本文件，说明 `T2003c` 现等待 `T2003c0`。
5. 检查变更并提交 Git commit，然后停止，等待下一轮实现新的首个未完成任务。
