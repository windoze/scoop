# 本次执行计划

## 约束说明
- 不在这里记录内部推理细节；这里只维护可审阅的执行计划、关键判断依据与进度更新。
- 本次目标是：定位并完成 `TODO-Px.md` 中按顺序出现的首个未完成详细任务；若遇到真实阻塞，则按要求补充前置任务、同步 `TODO.md`，提交后停止。

## 初始步骤
1. 读取 `TODO.md`，确认它只是索引，并找出引用的详细任务文件。
2. 按任务顺序读取相关 `TODO-Px.md` 文件，定位第一个标题未带 `[DONE]` 的详细任务。
3. 检查最近一次提交信息，确认是否存在与该任务直接相关但未完成的问题；若有，将其视为当前任务一部分或作为新的前置任务处理。
4. 阅读当前任务要求、约束、依赖、验证方式与完成记录，确认不能跳过或擅自拆分。

## 执行策略
1. 先最小化阅读相关代码与测试，建立实现边界。
2. 直接实现当前任务所需改动；若发现阻塞当前任务的真实缺陷或缺失能力，不做规避实现。
3. 运行与任务直接相关的测试、必要的更广泛测试，以及 `cargo clippy --all-targets -- -D warnings`（若适用且不会引入与当前任务无关的噪音）。
4. 更新详细任务文件：仅当任务真正完成时，在标题前加 `[DONE]` 并补全完成记录。
5. 如任务顺序、标题、依赖或新增前置任务发生变化，同步更新 `TODO.md`；仅在阶段计划真实变化时更新 `PLAN.md`。
6. 提交所有当前未提交改动（遵循当前任务或阻塞处理结果），然后停止，不继续下一个任务。

## 当前任务
- 已定位首个未完成详细任务：`P4-T05`（`TODO-P4.md`）。
- 任务目标：新增 `dump-effect-facts` CLI 与专属 fixture/snapshot 基线，并把 P4 -> P5 handoff contract 固化到代码与测试中。

## 针对当前任务的执行步骤
1. 检查 `scoop` CLI 与命令分发实现，寻找最接近的现有 `dump-*` 命令结构，优先复用统一 stage helper 与 formatter 管线。
2. 检查 `effect_facts` 子系统当前是否已有稳定 formatter API；若不足以作为 golden，则补齐稳定文本输出层，但保持改动集中在现有子系统和新命令中。
3. 在 `scoop` 中实现 `dump-effect-facts`：
   - `refactor` 路径走 canonical MIR + effect-facts stage；
   - `legacy` 路径返回稳定且可测试的不支持诊断。
4. 扩展 fixture harness，新增 `tests/fixtures/effect_facts/**` phase 与 `.effectfacts` golden 比对，确保 CLI 与 fixture 复用同一 helper/formatter。
5. 补充最小但完整的 effect-facts 样本与 golden，覆盖任务要求中的关键场景；必要时复用现有 `.scoop` 源文件内容，但不复用输出 golden。
6. 运行定向命令、fixture 测试与 lint；若有失败，先修复当前任务相关问题。
7. 更新 `TODO-P4.md` / `TODO.md` / `memory/claude_plan.md`，在确认完成后提交一次 git commit，并停止。

## 进度跟踪
- 状态：`P4-T05` 已完成，待提交。
- 已完成事项：
  1. 新增 `dump-effect-facts` CLI 与 shared render helper；legacy 路径返回稳定 unsupported 诊断。
  2. 扩展 `MaterializedEffectFacts` 稳定 formatter，显式展示 schema/callable/body/site facts，并把 continuation object type 中的绝对路径规范化为 repo-relative 文本。
  3. 新增 `effect_facts` fixture phase、7 个 `.scoop` 样本与对应 `.effectfacts` golden。
  4. 修复 declaration-only interface/class member surface contract 缺口，使 `dispatch_and_resume_call.scoop` 中的接口调用也能进入 P4 facts dump。
  5. 已完成定向测试、用户可见 CLI/fixture 验证与 `clippy -D warnings`。
