# 执行计划

## 约束

- 本轮只处理 `TODO.md` 中第一个未完成任务，完成后立即停止。
- 在继续实现前，先检查最新提交是否提到任何遗留问题；若有，则这些问题优先进入本轮范围。
- 若当前首个未完成任务过大，需要先拆分任务，并同步更新 `PLAN.md` 与 `TODO.md`。
- 执行过程中如发现任何与规范不一致、依赖缺失、实现边界不完整的问题，不能绕过，必须先把问题整理成新的前置任务并更新计划文件。

## 初始步骤

1. 检查最新一次 Git 提交信息，确认是否提到了已知问题、后续修复项或未完成事项。
2. 阅读 `TODO.md`，定位第一个未完成任务。
3. 阅读 `PLAN.md` 与任务相关上下文文件，确认该任务是否已经有既定拆分或依赖。
4. 如任务过大，先拆分并更新 `PLAN.md` / `TODO.md`，然后只执行拆分后的第一个子任务。

## 实施步骤

1. 定位相关代码、测试、规范或文档。
2. 实现任务所需改动，避免引入权宜方案。
3. 为改动补充或调整测试。
4. 运行必要验证，至少覆盖与本任务直接相关的测试；若改动影响范围较大，再补充更广的检查。
5. 处理验证中发现的问题，直到结果稳定。

## 收尾步骤

1. 更新 `TODO.md`，将本轮完成的任务标记为已完成；若任务被拆分或重排，也同步维护顺序与依赖。
2. 更新 `PLAN.md`，记录本轮进展、剩余工作与任何计划调整。
3. 视执行进展更新本文件，记录关键判断、当前状态与后续动作。
4. 检查工作区状态，确认提交内容仅包含本轮相关变更。
5. 使用清晰提交信息创建 Git 提交，然后停止。

## 当前状态

- 已写入初始计划。
- 已检查最新提交 `5addbc2 [T3009b2b] Restore ordinary indirect callee resumed-body replay`；提交说明未额外列出遗留问题。
- 已读取 `TODO.md` 与 `PLAN.md`；当前第一个未完成任务是 `T3009b2bR`：Review ordinary indirect callee 的 resumed-body restore 是否已统一接回。
- 已完成对 `T3009b2bR` 的首轮生产代码复审，结论是当前不能直接判定 review 完成。
- 下一步：核对计划文件改动、整理本轮阻塞结论并提交。

## T3009b2bR 细化计划

1. 阅读 `TODO.md` 中 `T3009b2bR`、`T3009b2b`、`T3009b2aR` 相邻段落，提炼本次 review 的显式验收点。
2. 审查最近提交触及的关键生产文件：
   - `crates/scoopc/src/llvm/codegen/mod.rs`
   - `crates/scoopc/src/llvm/codegen/effect/mod.rs`
   - `crates/scoopc/src/llvm/codegen/effect/state_machine_emitter.rs`
   - `runtime/c/scoop_runtime.c`
   - `runtime/c/scoop_runtime_api.h`
3. 重点确认以下风险面：
   - fresh path 与 resume path 是否共用统一合同，而不是按源码形状或 callee 名称分流。
   - ordinary indirect callee 的 locals/captures restore 是否覆盖 top-level helper 与 closure 两类路径。
   - continuation 捕获 / 恢复后的 TLS 生命周期与 pin/unpin 是否对称，是否会留下悬垂状态或重复消费。
   - `ResumeAfterSite(Call)` 是否只在确有 callee suspend state 时 replay 原 call，inactive path 是否仍保持原合同。
4. 若发现生产代码问题，在本轮 review 内直接修复，并补充必要测试。
5. 运行定向测试；若影响面要求更高，再补充全量回归与 lint。
6. 更新 `TODO.md`、`PLAN.md`、本文件并创建提交，然后停止。

## T3009b2bR 复审结果

- 在 `crates/scoopc/src/llvm/codegen/mod.rs` 发现新的源码形状前提：
  - `CalleeSuspendPlan` 注释明确声明当前只覆盖“block 中单个 direct-perform \`val\` 绑定”的稳定子集。
  - `build_block_callee_suspend_plan()` 会扫描该 block 形状，并据此决定是否生成 ordinary callee 的 fresh/resume 双入口。
- 该实现与 `TODO.md` 顶部约束冲突：
  - 生产 effect codegen 禁止根据源码 / 代码形状分流。
  - LLVM lowering 的单一输入应为 state machine，而不应再回看源码 block 形状。
- 因此本轮不能把 `T3009b2bR` 标记完成，已按依赖关系新增前置任务 `T3009b2b1`，要求先移除 block-shape 扫描前提，再继续复审 `T3009b2bR`。
- 本轮已更新：
  - `TODO.md`：插入 `T3009b2b1`，并把 `T3009b2bR` 改为依赖该任务。
  - `PLAN.md`：记录当前轮复审阻塞原因，并把执行顺序改为先做 `T3009b2b1`。
