## 当前执行计划

1. 已确认首个未完成任务为 `CG-T07S0a6`：修复 `literal_numeric_expected_type_absorption_basic.scoop` 中 `Array<UInt8>` element expected-type absorption 失效。
2. 最近一次提交 `[CG-T07S0a5] Fix list builder transport concretization` 直接说明默认 full-suite 已推进到该 fixture；未发现需要额外补录的新前置任务，当前任务按 `TODO.md` 直接执行。
3. 复现当前失败：执行单 fixture 测试，必要时直接 build/run 观察实际输出与期望差异。
4. 阅读与 numeric literal expected-type absorption、array literal element typing、HIR/MIR/materialization 相关的实现与现有回归测试，定位 authoritative contract 在何处丢失 `UInt8` 语义。
5. 以最小改动修复 expected-type 传递/吸收主线，确保不通过 backend truncation、fixture/golden 调整或其他变通方式规避问题。
6. 补充或更新最小回归验证，至少覆盖当前 fixture；若默认 full-suite 继续暴露下一个 blocker，则按 `TODO.md` 规则处理。
7. 更新 `TODO.md` 完成记录并将 `CG-T07S0a6` 标记为 `[DONE]`；仅在阶段计划变化时更新 `PLAN.md`。
8. 提交本次变更，然后停止。

## 约束提醒

- 只处理 `TODO.md` 中当前排序下的第一个未完成任务。
- 不以变通方式绕过语言/运行时/规范缺口。
- 若存在阻塞，新增最小前置任务并提交后停止。

## 当前进展

- 已复现 `literal_numeric_expected_type_absorption_basic.scoop`：最后两行实际输出仍是 `false` / `false`。
- 直接 build/run 证实仅 `Array<UInt8>` 的 `bytes.get(0/1)` 观测异常；局部变量、return、call 路径上的窄类型吸收正常。
- 初步检查 `dump-mir` 发现 `bytes.get(...)` 的 array element transport/result 走成了 `Struct` 形状，而非前面 `Float32` 路径的 `Scalar`，说明问题更可能出在数组元素 authoritative type 发布/规范化链，而不是最终 compare/golden。
- 已完成修复：array literal HIR lowering 现在仅对“纯数值字面量算术/移位表达式”注入 element expected-binding，`Array<UInt8>` builder push transport metadata 恢复为 `UInt8` scalar surface；`literal_numeric_expected_type_absorption_basic.scoop`、`array_lit_lowering.scoop`、新增 `production_codegen_uint8_array_numeric_elements_keep_scalar_transport_metadata` 回归以及 `clippy` 均通过。
- 默认 `cargo run -p scoop -- test` 已越过 `literal_numeric_expected_type_absorption_basic.scoop`，下一处失败转为 `literal_ops_compare_direct_matrix_basic.scoop`；已按顺序约束在 `TODO.md` 新增前置任务 `CG-T07S0a7` 并将 `CG-T07S0a6` 标记为 `[DONE]`。
