# 本轮执行计划

## 说明

按要求先写入计划文件，再开始读取仓库信息和执行命令。这里记录的是可审计的执行计划、决策依据摘要和后续进度，不包含逐字内部推理。

## 初始目标

本轮只完成 `TODO.md` 中第一个未完成任务；如果在执行前发现最新提交中提到的遗留问题，先修复这些问题，再处理该任务。

## 计划步骤

1. 查看最新一次 Git 提交的提交信息与变更摘要，确认是否明确提到遗留问题、已知缺陷或待修复事项。
2. 如果最新提交提到需要先处理的遗留问题：
   - 定位相关代码、测试与文档；
   - 实现修复；
   - 运行针对性测试与必要的全量校验；
   - 更新 `TODO.md` / `PLAN.md` / 本文件；
   - 提交 Git commit；
   - 若这些修复本身已构成本轮唯一工作，则停止。
3. 读取 `TODO.md`，找到第一个未完成任务。
4. 判断该任务是否可在本轮完整落地：
   - 若可以，直接实现；
   - 若过大或存在前置缺口，则拆分为更小子任务，更新 `PLAN.md` 与 `TODO.md`，并只执行拆分后的第一个子任务。
5. 在实现前检查相关模块、测试、规范说明和现有实现边界，确认不存在规避式实现或与规范不一致的隐藏阻塞。
6. 完成实现后运行充分测试，至少包含：
   - 受影响模块的定向测试；
   - 必要的集成/fixture 测试；
   - `cargo fmt --check`；
   - `cargo clippy --all-targets -- -D warnings`；
   - 如改动影响范围较大，再补充 `cargo test --all` 或合适子集。
7. 更新文档与任务状态：
   - 在 `TODO.md` 中标记本轮完成的任务；
   - 在 `PLAN.md` 中更新当前状态、后续顺序与任何新增依赖；
   - 在本文件中记录关键发现、计划调整和测试结果。
8. 使用清晰的提交信息创建 Git commit，然后停止，不继续下一个任务。

## 当前状态

- 已检查最新提交：`485e568 [T3016b0] Fix when-arm resumed-body replay`。提交说明本身未声明新的遗留问题需要优先独立处理。
- 已读取 `TODO.md` / `PLAN.md`，当前首个未完成任务为 `T3016b0R`：Review `statement-position when arm` 恢复后不再重放 enclosing `when`。
- 复审过程中确认了一个真实残留：当 enclosing `when` 的结果继续被外层 consumer（如 `println(when (...))`）读取时，旧逻辑只删除 standalone `WhenExpr`，没有改写真正 consumer，恢复后仍会重放 arm。
- 已完成修复：
  - `materialize_resume_fragments()` 现在会在存在后续 consumer 时，把 consumer 中的 `when` 子表达式改写为 materialized arm-tail block，并同步删除 resume state 中已被覆盖的显式 arm-tail actions 与 standalone `WhenExpr`。
  - 无后续 consumer 时，仍只删除已被 resumed-body 覆盖的 standalone `WhenExpr`。
  - 已新增结构测试 `source_plan_rewrites_nested_when_consumer_to_materialized_arm_tail_block` 和 run-pass fixture `effect_escape_continuation_perform_in_when_arm_nested_consumer.scoop`。
- 已完成验证：
  - `cargo test -p scoopc source_plan_elides_enclosing_when_expr_after_when_arm_resume -- --nocapture`
  - `cargo test -p scoopc source_plan_rewrites_nested_when_consumer_to_materialized_arm_tail_block -- --nocapture`
  - `cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/effect_escape_continuation_perform_in_when_arm.scoop`
  - `cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/effect_escape_continuation_perform_in_when_arm_nested_consumer.scoop`
  - `cargo test --all`
  - `cargo clippy --all-targets -- -D warnings`
- 当前任务 `T3016b0R` 已完成；下一项待执行任务为 `T3016b`。
