# 本轮执行计划

## 约束说明

- 按要求先记录本轮计划，再执行任何命令。
- 出于安全与工程实践考虑，此文件记录的是可审计的决策摘要、执行步骤、检查项与进度更新，不包含逐字内部推理。
- 本轮目标是：先检查最新提交是否提到需先修复的既有问题；然后识别 `TODO.md` 中第一个未完成任务；只完成这一个任务（或在必要时将其拆分并执行第一个子任务）；完成后测试、更新文档、提交并停止。

## 初始执行步骤

1. 查看最新一次 Git 提交信息，确认是否提到需要先处理的遗留问题。
2. 阅读 `TODO.md`，识别第一个未完成任务。
3. 阅读 `PLAN.md`、`README.md` 以及与该任务直接相关的代码与测试，确认上下文。
4. 判断该任务是否可在本轮完整完成：
   - 若可以，直接实现。
   - 若过大或依赖不清，先拆分为更小子任务，并同步更新 `PLAN.md` 与 `TODO.md`，然后执行拆分后的第一个子任务。
5. 实现改动时保持模块化，必要时补充注释与文档。
6. 运行针对性测试，并在需要时运行更完整的校验：
   - 至少运行与改动直接相关的测试。
   - 若改动影响面较大，补充运行 `cargo test --all`。
   - 在合理范围内运行 `cargo clippy --all-targets -- -D warnings`，确保无告警。
7. 更新进度文件：
   - 在 `TODO.md` 中标记该任务完成，或在无法直接完成时按要求重排待办顺序并记录依赖。
   - 更新 `PLAN.md` 反映当前状态。
   - 按阶段更新本文件，记录关键进展与计划变化。
8. 提交本轮变更，提交信息与任务编号保持一致，然后停止。

## 关键检查项

- 不回退或覆盖与本轮任务无关的现有修改。
- 只完成一个任务，不进入下一个待办。
- 若发现最新提交中明确提到的既有问题，优先修复后再处理待办。
- 若 `PROMPT.md` 在过程中发生变动，需要一并纳入提交。
- 若所有任务已完成，则执行最终审查、必要修正、提交并打 `v0.1.0` 标签。

## 进度更新

- 已完成：初始化计划文件。
- 已完成：检查最新提交，未发现提交说明中要求优先修复的遗留问题。
- 已完成：定位 `TODO.md` 中第一个未完成任务为 `T0150h`。
- 已完成：对 `T0150h` 做快速盘点，确认其范围过大，需拆分。
- 已完成：用最小样例复现首个明确缺口：
  - `val a: Float32 = 1.5 + 2.5` 当前报 `initializer_type_mismatch`（结果先被推导为 `Float64`）。
  - `val b: UInt8 = 1 + 2` 当前报 `initializer_type_mismatch`（结果先被推导为 `Int`）。
  - `val xs: Array<UInt8> = [1 + 2, 3]` 当前报 `array_lit_element_type_mismatch`。
- 结论：本轮将把 `T0150h` 拆为更小子任务，并执行第一个子任务：修复“数值字面量运算表达式在 expected type 语境下的 absorption”。
- 下一步：
  1. 更新 `TODO.md` / `PLAN.md`，把 `T0150h` 拆分为多个子任务并将第一个子任务置于首位。
  2. 修改 typecheck 中 expected type 向纯字面量算术表达式传播的规则。
  3. 为赋值 / return / call / array element 增加回归 fixture。
  4. 运行测试与 clippy，更新进度文件并提交。
- 已完成：把 `T0150h` 拆分为 `T0150h-1` / `T0150h-2` / `T0150h-3`，并更新 `TODO.md`、`PLAN.md`。
- 已完成：实现 `T0150h-1` 的核心修复：
  - typecheck：expected type 现在会向数值字面量的一元/二元运算表达式下传；
  - HIR lowering：在 typecheck 已吸收为窄类型时，数值字面量与一元/二元表达式会保留正确的 HIR 类型，避免 `Array<Float32>` 元素按 `Float64` 错位编码。
- 已完成：新增回归覆盖：
  - `tests/fixtures/typecheck/literal_numeric_expected_type_absorption_ok.scoop`
  - `tests/fixtures/run-pass/literal_numeric_expected_type_absorption_basic.*`
- 已验证（针对性）：
  - `literal_numeric_expected_type_absorption_basic.scoop` 可构建并输出期望结果；
  - 其中显式覆盖了先前复现的 `Array<Float32> = [1.5, 2.5f]` 首元素错误输出 `0.0` 的问题，现已修复。
- 已完成：全量验证
  - `cargo test --all`
  - `cargo run -p scoop -- test`（`fixtures: ok (874)`）
  - `cargo clippy --workspace --all-targets --message-format short -- -D warnings`
- 已完成：更新 `TODO.md` / `PLAN.md`，将 `T0150h-1` 标记为完成。
- 下一步：
  1. 复核本轮 diff 与工作区状态。
  2. 以 `T0150h-1` 为主题提交 Git commit。
  3. 停止，等待下一轮调用。
