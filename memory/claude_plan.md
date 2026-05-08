## 当前执行计划

说明：这里记录可公开的执行计划、决策摘要与进度更新，不包含内部完整思维链。

1. 先读取 `TODO.md`，识别第一个标题未标记 `[DONE]` 的任务。
2. 检查最近一次提交是否明确提到与该任务直接相关且尚未完成的问题；若有，将其视为当前任务的一部分，或按要求在 `TODO.md` 中登记为前置依赖。
3. 阅读当前任务涉及的代码、测试、规范说明与相关文件，确认实现边界、依赖和验证要求。
4. 直接完成该任务；如果遇到阻塞当前任务且不能规避的真实缺口或回归，则先在 `TODO.md` 中以最小必要粒度补充前置任务并停止。
5. 运行与该任务相关的验证；至少覆盖任务指定验证，并补充必要回归测试；保证 `cargo clippy --all-targets -- -D warnings` 无告警（若适用且在任务范围内）。
6. 更新文档与记录：将当前任务在 `TODO.md` 中标记为 `[DONE]` 并填写完成记录；仅在阶段计划发生变化时更新 `PLAN.md`。
7. 提交本次变更，提交信息使用当前任务编号，随后停止，不进入下一个任务。

## 进度日志

- 已写入初始执行计划，下一步读取 `TODO.md` 并识别当前应执行任务。
- 已识别第一个未完成任务为 `CG-T07S0a16`。
- 已检查最近一次提交：`[CG-T07S0a16a] Restore direct UInt8 scalar transport`，其内容与 `TODO.md` 记录一致，属于当前任务已登记的前置 blocker 修复，无需再新增前置任务。
- 已完成当前任务的验证结论确认：`literal_array_expected_type_nested_basic.scoop` 在当前代码上已恢复通过，且默认 full-suite 不再停在该 fixture。
- 已新增最小 MIR 回归 `dump_mir_nested_uint8_array_literals_keep_expected_element_type`，覆盖嵌套 `Array<UInt8>` 在 `if` / `when` / 函数参数三条路径上的 expected-type/transport contract。
- 已补充验证并记录结果：
  1. `cargo test -p scoopc dump_mir_nested_uint8_array_literals_keep_expected_element_type`
  2. `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/literal_array_expected_type_nested_basic.scoop`
  3. `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/literal_numeric_expected_type_absorption_basic.scoop`
  4. `cargo run -p scoop -- test`
  5. `cargo clippy --all-targets -- -D warnings`
- 已更新 `TODO.md`：将 `CG-T07S0a16` 标记为 `[DONE]`，并把新暴露的 `star_projection_array_read_view.scoop` blocker 记录为新的前置任务 `CG-T07S0a17`。
- 当前剩余动作：检查最终 diff、提交本次变更，随后停止，不进入下一任务。
