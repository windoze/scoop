## 当前轮次执行计划（初始）

说明：这里记录可公开的执行思路摘要与步骤计划，不写入内部隐式推理细节。计划会在读取仓库状态后继续细化和更新。

1. 检查最近一次提交，确认是否提到已知遗留问题；如果有，先修复这些问题。
2. 阅读 `TODO.md`，定位第一个未完成任务。
3. 阅读 `PLAN.md`，确认现有规划与任务依赖关系。
4. 评估该任务是否可以在本轮完整实现：
   - 如果可以，直接实现、补测试、运行验证。
   - 如果过大或被前置问题阻塞，则拆分任务并更新 `TODO.md` / `PLAN.md`。
5. 在实现过程中持续更新本文件，记录关键结论、阻塞、计划变化与完成状态。
6. 完成后更新 `TODO.md` 和 `PLAN.md`，提交 git commit，然后停止，不进入下一任务。

## 当前已知约束

- 需要先处理最新提交中提到的遗留问题。
- 每轮只完成一个任务。
- 不接受规避规范的临时方案；若发现规范缺口，必须先在 `TODO.md` 中建前置任务。
- 需要补充测试并尽量保证 `cargo clippy --all-targets -- -D warnings` 无警告。

## 已完成的上下文检查

1. 已检查最新提交 `9cea869afbd928bce523bb30ad0d224dbe5977c3`：
   - 提交信息仅为 `Update plan`。
   - 未在提交信息中发现明确声明的遗留 bug，因此无需先按“提交说明里的已知问题”插队修复。
2. 已阅读 `TODO.md` 与 `PLAN.md`：
   - 当前第一个未完成任务是 `T2003r1`：从零开始实现统一 segmenting，产出完整 `HandleSegmentList`。
3. 已审阅现有实现：
   - 现有 `state_machine_plan.rs` 已有统一 plan builder、pretty dump、representative 单测。
   - 现有 builder 已显式建模 state / suspend site / cleanup / nested handle，可作为抽出 segment 层的基础。

## 任务复杂度判断

`T2003r1` 当前范围过大，单轮同时完成“segment IR 设计 + 全部合法组合的统一分段 + 全量回归”风险过高，因此需要先拆分，再执行第一个子任务。

## 拟采用的拆分方案

计划把 `T2003r1` 拆为以下子任务，并在 `TODO.md` / `PLAN.md` 中落地：

1. `T2003r1a`
   - 定义统一 `HandleSegmentList` / `HandleSegment` / `HandleSegmentEdge` 数据结构；
   - 从现有统一 plan walker 抽出第一版 segment dump；
   - 覆盖 direct/indirect、`if`/`while`、nested handle、`finally` 的代表性单测。
2. `T2003r1b`
   - 补齐 multi-arm dispatch entry/exit、arm body、cleanup stack / dispatch context 等 richer segment metadata。
3. `T2003r1c`
   - 收口 nested-while / richer mixed representative samples，并明确 builder 下一步只消费 segment list。

## 本轮准备执行的实际任务

执行 `T2003r1a`：

1. 更新 `TODO.md` / `PLAN.md`，把 `T2003r1` 拆成 `T2003r1a`~`T2003r1c`。
2. 在 LLVM effect 代码中新增 segmenting 数据结构与 pretty dump。
3. 让测试可以直接构建 segment dump，并补代表性断言。
4. 运行本子任务相关测试与静态检查。
5. 更新 `TODO.md` / `PLAN.md` / 本文件，提交 commit，然后停止。

## 当前执行结果

1. 已完成任务拆分：
   - `TODO.md` / `PLAN.md` 已把原 `T2003r1` 拆为 `T2003r1a` / `T2003r1b` / `T2003r1c`。
   - 当前实际执行并完成的是 `T2003r1a`。
2. 已完成代码实现：
   - 新增 `crates/scoopc/src/llvm/codegen/effect/state_machine_segments.rs`。
   - 定义了统一 `HandleSegmentList` / `HandleSegment` / `HandleSegmentEdge` / `HandleSegmentTerminator`。
   - 允许从现有 `HandleStateMachinePlan` 投影出第一版 segment list，并提供 segment pretty dump。
   - `MainCodegen::build_handle_state_machine_plan` 现会同步计算 segment projection 的结构签名，保证阶段 1 输出在正常构建中被持续触达。
3. 已完成测试与验证：
   - `cargo test -p scoopc segment_dump_`
   - `cargo test -p scoopc plan_dump_`
   - `cargo clippy --workspace --all-targets -- -D warnings`
   - 上述命令均已通过。

## 剩余收尾步骤

1. 检查最终 diff 与 git 状态。
2. 提交 commit，建议信息：`[T2003r1a] Add initial unified handle segment dump`
3. 停止，不进入 `T2003r1b`。
