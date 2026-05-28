# 当前执行计划

> 说明：本文件记录可审计的执行计划、关键决策和进度更新；不记录不可见的内部推理细节。

## 初始计划

1. 读取 `TODO.md`，按文件顺序识别第一个标题未带 `[DONE]` 的任务，并只处理该任务。
2. 检查该任务的要求、依赖、验证方式和完成记录；必要时查看 `PLAN.md` 了解阶段背景，但不把 `PLAN.md` 当作任务账本。
3. 检查最新提交信息是否明确提到与当前任务直接相关的未完成问题；若相关，将其纳入当前任务或作为前置任务记录到 `TODO.md`。
4. 为当前任务收集最小必要上下文，定位相关代码、测试和夹具。
5. 若任务可直接实施，进行最小正确修改；若发现必须先修复的具体前置缺口，则更新 `TODO.md`，提交后停止。
6. 按要求运行格式化、lint、相关测试，以及需要时的完整测试/fixture 套件；发现未排期失败时修复或新增最小前置任务。
7. 任务完成后，在 `TODO.md` 的任务标题前加 `[DONE]` 并更新完成记录；仅当阶段级计划变化时更新 `PLAN.md`。
8. 检查工作区差异，提交本次任务涉及的所有必要变更，然后停止，不处理下一个任务。

## 进度记录

- 本轮已确认当前第一个未完成任务为 `P3-T05R：Review closure var capture 诊断`；最新提交 `59c41b3 Update plan` 未明确提出与该 review 直接相关的未完成问题。
- P3-T05R 执行计划：复核 `SPEC_FIX.md` B5、P3-T05 完成记录、closure capture/typecheck 实现与相关 fixtures；重点确认跨 closure 边界外层 `var` 会报错，同 closure 内局部 `var` 不误报，`val` capture 仍可用，诊断包含 `RefCell<T>`、`val snapshot = ...`、fold / higher-order alternatives。
- 当前 review 发现需要加强覆盖：已有 makeCounter 负例和 `val snapshot` / `RefCell` 正例，但缺少嵌套 closure 捕获外层 closure 局部 `var` 的负例，以及同 closure 内局部 `var` 使用的正例。下一步补充这两个 targeted fixtures，并把诊断 fixture 的期望文本收紧到覆盖全部 alternatives。
- 已补充 `closure_capture_var_nested_lambda_is_error.scoop` 与 `closure_local_var_inside_lambda_ok.scoop`，并收紧 makeCounter 负例的 diagnostic substring 以覆盖全部替代方案提示。下一步运行 targeted fixtures，再按要求进行格式化、lint、Rust 测试和完整 fixture suite。
- `cargo fmt`、`cargo clippy --all-targets -- -D warnings` 与 targeted closure fixtures 均已通过。下一步运行完整 Rust 测试套件和完整 fixture suite；若发现未排期失败，将先修复或在 TODO 中排入必要前置任务。
- 完整验证已通过：`cargo test --all --all-targets` 成功，`python3 tools/run_fixtures.py` 成功（`fixtures: ok (1558)`）。已将 `TODO.md` 索引和 `TODO-3.md` 中的 `P3-T05R` 标记为 `[DONE]` 并写入 completion record；`PLAN.md` 无阶段级变化，未修改。
