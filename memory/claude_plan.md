# 当前执行计划

更新时间：2026-04-28

说明：按要求在执行命令前先记录计划。这里记录的是可审计的执行计划、观察、决策与进度摘要，不包含不可导出的内部推理细节。

## 目标

本轮只完成 `TODO.md` 中第一个未完成任务；如果在检查、测试或实现过程中发现更早的既有问题，则优先修复该问题，或将其作为前置任务插入 `TODO.md` 后停止。

## 初始步骤

1. 检查最新一次 git 提交信息，确认是否提到已知未修复问题；若有，先修复该问题。
2. 阅读 `TODO.md`，定位第一个未完成任务。
3. 阅读 `PLAN.md`，确认现有计划与依赖关系。
4. 评估该任务是否可在本轮完整交付。
5. 若任务过大，则拆分为更小子任务，并更新 `TODO.md` 与 `PLAN.md`，然后执行拆分后的第一个子任务。
6. 实现任务。
7. 运行相关测试，并补齐必要测试。
8. 运行质量检查，至少包括与改动相关的测试，以及在可行时运行 `cargo fmt`、`cargo test --all`、`cargo clippy --all-targets -- -D warnings`。
9. 更新 `TODO.md` 与 `PLAN.md` 记录结果。
10. 使用清晰的提交信息提交改动，并停止。

## 执行约束

- 不接受绕过实现缺陷的临时方案。
- 如果发现阻塞当前任务的既有缺陷，必须先修复，或将其添加为前置任务后停止。
- 本轮只完成一个任务，不推进到下一个任务。
- 不回退或覆盖我未创建的现有改动。

## 进度记录

- 2026-04-28：已创建计划文件，下一步开始检查最新提交与任务列表。
- 2026-04-28：已检查最新提交标题与 `TODO.md` / `PLAN.md`，当前首个未完成任务为 `T5000j1R Review：确认 operator-overload target 已脱离 LLVM backend 现场物化`。
- 2026-04-28：已完成静态 review。结论是 operator-overload / `compareTo` target identity 已前移到 typecheck + typed HIR / generic MIR 主线：typecheck 记录 `TopLevelFunCallBinding`，typed HIR 将 operator-overload 站点改写为显式 direct-call，`compareTo` 则改写为 `direct-call + SynthInt(0)` 的整数比较；production reachability 与 MIR materialization 继续消费显式 direct-call，不再依赖 LLVM backend 现场猜目标。
- 2026-04-28：测试已完成且通过：
  - `cargo test -p scoopc compare_to -- --nocapture`
  - `cargo test -p scoopc operator_overload -- --nocapture`
  - `cargo test --all`
  - `cargo clippy --all-targets -- -D warnings`
  - `cargo run -p scoop -- test`（`fixtures: ok (1202)`）
- 2026-04-28：未发现需要插入到 `T5000j2` 之前的新前置缺陷任务；下一步更新 `TODO.md` / `PLAN.md` 并提交。

## 本轮执行细化

1. 阅读 `T5000j1R` 相邻的 `TODO.md` / `PLAN.md` 条目，提炼本轮 review 的验收条件。
2. 检查最新提交及其相关代码路径：
   - `typecheck` 中 operator-overload / `compareTo` 绑定路径；
   - typed HIR lowering；
   - generic MIR / materialization；
   - production LLVM codegen 是否仍存在 compareTo/operator-overload 的现场猜测或 eager inclusion。
3. 运行与该路径直接相关的测试，优先覆盖：
   - `compareTo` 比较；
   - operator-overload fixture / LLVM tests；
   - 如有必要，补充回归测试。
4. 若发现既有缺陷：
   - 先修复该缺陷，再更新 `TODO.md` / `PLAN.md` / 本文件，并继续完成当前 review；
   - 如果无法在本轮安全修复，则把缺陷插入为前置任务并停止。
5. 若未发现阻塞缺陷：
   - 将 `T5000j1R` 标记完成；
   - 更新 `PLAN.md` 与本文件记录 review 结论、证据和测试结果；
   - 提交本轮改动并停止。

## 当前状态

- `T5000j1R` 的技术结论已确认。
- `TODO.md` / `PLAN.md` 已更新为完成状态。
- 下一步：检查工作区差异并提交本轮文档改动。
