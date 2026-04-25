# 当前执行计划

说明：用户要求先写入“完整思考过程”，但我不会记录不可共享的内部推理；这里记录可审计的高层计划、关键判断和执行进度。

## 初始计划

1. 检查最新一次 Git 提交的提交信息与变更，确认是否明确提到尚未修复的问题。
2. 检查工作树状态，避免覆盖用户或既有未提交修改。
3. 阅读 `TODO.md`，定位第一个未完成任务。
4. 阅读 `PLAN.md`，确认现有计划与该任务的关系。
5. 如果任务过大，先把任务拆分为更小的子任务，并更新 `PLAN.md` 与 `TODO.md`，然后只执行新的第一个子任务。
6. 实现该任务，同时留意任何既有缺陷、回归、规范不一致或实现边界问题；若发现阻塞项，先修复，或将其作为前置任务插入 `TODO.md` 并停止。
7. 运行相关验证：
   - 最小相关测试
   - 必要时运行更广泛测试
   - `cargo fmt`
   - `cargo clippy --all-targets -- -D warnings`
8. 更新文档与计划：
   - 在 `TODO.md` 中标记任务完成，或在阻塞时重排任务顺序
   - 更新 `PLAN.md`
   - 视进展更新本文件
9. 使用清晰的提交信息提交本次变更。
10. 停止，不继续下一个任务。

## 进度

- 已完成：初始计划写入。
- 已完成：检查最新提交；未发现提交信息中直接要求先修的独立缺陷。
- 已完成：确认首个未完成任务为 `T5000aR Review`。
- 已完成：抽样核对 `MainCodegen::new`、reachability / eager inclusion、`-O0` pass pipeline、`HandlePlanContext::from_codegen` 与 `effect_step_summary.rs` 的 `include!` 耦合。
- 已完成：将 review 结论回写到 `OPTIMIZATION.md`、`PLAN.md`、`TODO.md`。
- 已完成：运行 `cargo test --all`，全部通过。
- 已完成：运行 `cargo clippy --all-targets -- -D warnings`，零 warning 通过。
- 已完成：运行 `cargo fmt --all --check`，格式检查通过。
- 进行中：检查工作树并准备提交。
