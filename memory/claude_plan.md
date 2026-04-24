# 本轮执行计划（审计版）

说明：
- 按要求，先把本轮执行计划写入此文件，再开始任何命令检查或代码执行。
- 这里记录的是可审计的执行思路、判断依据摘要与步骤计划，不展开内部推理细节。

## 当前目标判断

- 依据上一次执行留下的交接信息，本轮主任务应为收口 `T4012R`，而不是继续推进 `T4013`。
- 上一轮已经完成了实质代码修复与测试，尚未完成的工作主要是：
  - 更新 `TODO.md`
  - 更新 `PLAN.md`
  - 更新 `ISSUES.md`
  - 更新本文件的状态
  - 提交 git commit
- 仍需先核对最新提交、当前工作树以及任务排序，确保没有最新提交里提到的既有问题需要优先修复，也确保 `T4012R` 仍然是 `TODO.md` 中第一个未完成任务。

## 执行步骤

1. 检查最新提交信息与当前工作树状态。
   - 查看最新 commit message 与必要上下文，确认是否提到尚未修复的既有问题。
   - 查看 `git status`，确认当前未提交改动范围，避免误动无关文件。

2. 核对任务与计划文档。
   - 读取 `TODO.md`，确认第一个未完成任务。
   - 读取 `PLAN.md`、`ISSUES.md`，定位需要更新的位置。
   - 若发现交接摘要与仓库现状不一致，以仓库现状为准并修正计划。

3. 如无新的更高优先级既有问题，完成 `T4012R` 的文档收口。
   - 在 `TODO.md` 中将 `T4012` / `T4012R` 标为完成，并记录 review 期间修复的 annotation class runtime 泄漏问题与已验证命令。
   - 在 `PLAN.md` 中把当前主线切换到 `T4013`，并更新 annotation 区段状态。
   - 在 `ISSUES.md` 中更新 annotation 相关 issue 的剩余范围，只保留 `@Inline` 交叉项。

4. 更新本文件进度。
   - 把已完成步骤与待完成步骤同步到本文件，保证过程可检查。

5. 做最小必要验证。
   - 若仅文档更新，优先复核已有测试结果是否足以支撑本轮结论。
   - 如需最小重验，运行与本轮改动最相关的 fixture 测试。

6. 提交并停止。
   - 使用与 `T4012R` 对应的清晰提交信息。
   - 提交后停止，不进入 `T4013`。

## 已知约束

- 全程使用中文。
- 只完成 `TODO.md` 中第一个未完成任务，然后停止。
- 不回退或覆盖无关改动，尤其是不处理无关的 `run_agent.sh` 改动。
- 若检查中发现新的既有问题且其优先级更高，必须先修复该问题，或把它作为前置任务插入 `TODO.md` 后再停止。

## 进度更新

- 已检查最新提交：
  - 最新提交为 `[T4012c] Add @Experimental feature-gate marker`，提交标题未暴露新的前置既有 issue 需要先于本轮处理。
- 已检查工作树：
  - 未提交改动与交接摘要一致，主要包含 annotation runtime misuse 修复、3 个新增 typecheck fixtures、当前文件，以及无关的 `run_agent.sh` 用户改动。
  - `run_agent.sh` 不属于本轮任务，保持不动。
- 已核对任务顺序：
  - `TODO.md` 中第一个未完成任务确认为 `T4012R`。
  - 结合当前未提交代码与交接摘要，本轮应完成 `T4012R` 收口、更新文档并提交，不进入 `T4013`。
- 已完成文档更新：
  - 已把 `TODO.md` 中 `T4012` / `T4012R` 标记为完成，并写入 review 期间修复的 annotation class runtime 泄漏问题与本轮复验命令。
  - 已把 `PLAN.md` 主线切换到 `T4013`，并补入 `T4012R` 完成说明。
  - 已把 `ISSUES.md` 第 9 条更新为：annotation declaration model 已收口，annotation class runtime nominal/type position 泄漏已修复，当前只剩 `@Inline` 交叉项。
- 已完成复验：
  - `cargo fmt --all`
  - `cargo run -p scoop -- test --fixtures tests/fixtures/typecheck` -> `fixtures: ok (392)`
  - `cargo run -p scoop -- test` -> `fixtures: ok (1194)`
  - `cargo test --all` -> 通过
  - `cargo clippy --all-targets -- -D warnings` -> 通过

## 当前状态

- [x] 先写入本计划文件
- [x] 检查最新提交与工作树
- [x] 核对 `TODO.md` / `PLAN.md` / `ISSUES.md`
- [x] 更新任务与计划文档
- [x] 更新本文件进度
- [x] 完成必要验证
- [ ] 提交改动并停止
