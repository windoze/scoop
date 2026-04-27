# 执行计划

## 约束说明

- 按要求先写本计划文件，再执行其余检查、构建、测试或代码修改动作。
- 本文件记录可审计的执行步骤、决策依据摘要、阻塞项与进度更新。
- 不写入不适合外露的内部推理细节；若计划调整，将直接在本文件中增量更新。

## 初始目标

本轮只完成 `TODO.md` 中“第一个未完成任务”，但在执行该任务之前，必须先检查最新提交是否提到已有问题；如果提到，则该问题优先修复。执行过程中如发现任何既有 bug、回归、规范不匹配、未完成边界或测试暴露的问题，也必须先修复，或者将其作为前置任务插入 `TODO.md` 后停止。

## 执行步骤

1. 检查最新一次 Git 提交信息与变更摘要，确认是否明确提到待修复的既有问题。
2. 阅读 `TODO.md`，定位第一个未完成任务。
3. 阅读 `PLAN.md`，确认该任务上下文、依赖与既有拆分情况。
4. 判断该任务是否过大：
   - 若过大，则先拆分任务，更新 `PLAN.md` 与 `TODO.md`，将新的首个子任务作为本轮执行对象。
   - 若可直接完成，则进入实现阶段。
5. 在实现前补充必要代码上下文调查：
   - 查找相关模块、测试、规范或现有未完成边界。
   - 如发现阻塞性的既有问题，优先修复，或将其前插到 `TODO.md` 后停止。
6. 实现当前目标任务。
7. 运行相关测试，并尽可能覆盖：
   - 最小相关测试；
   - 必要时运行更大范围测试；
   - `cargo fmt`；
   - `cargo clippy --all-targets -- -D warnings`；
   - 与任务相关的构建/回归命令。
8. 若测试暴露已有问题，立即修复，并回到测试步骤。
9. 更新文档与任务状态：
   - 在 `TODO.md` 中将本轮完成任务标记为完成；
   - 更新 `PLAN.md`；
   - 更新本文件记录结果与任何计划变更。
10. 检查工作区改动，确认未误伤无关改动。
11. 使用清晰提交信息创建 Git 提交。
12. 停止，不继续下一个任务。

## 进度记录

- [初始化] 已创建本计划文件，下一步将检查最新提交与任务列表。
- [检查] 已查看最新提交 `5db49536 [T5000h0b] Add production materialized callable view`；提交说明未直接挂出待先修的既有问题。
- [定位任务] 已确认 `TODO.md` 中首个未完成条目为 `T5000h0bR Review：确认 canonical materialized-body / summary 视图边界成立`。
- [上下文] 已复核 `TODO.md` / `PLAN.md` 中 `T5000h0a`、`T5000h0aR`、`T5000h0b` 的目标与验收，当前 review 需要重点确认：
  - 新视图是否真正围绕 materialized callable body / summary 建模，而不是继续包装 `instance_keys`；
  - production 查询面是否足够直接供后续 MIR rewrite / codegen 接线；
  - 是否仍有消费侧必须重复扫描 `MaterializedMir.file` 的边界泄漏。
- [下一步] 读取 `crates/scoopc/src/mir/callables.rs`、`crates/scoopc/src/mir/materialize.rs`、`crates/scoopc/src/hir/lower/types.rs` 及生产消费侧调用点，必要时运行定向测试与全量回归。
- [审查] 已复核 `crates/scoopc/src/mir/callables.rs`、`crates/scoopc/src/mir/materialize.rs`、`crates/scoopc/src/hir/lower/types.rs`、`crates/scoop/src/commands/build.rs`、`crates/scoopc/src/llvm/tests.rs`。
  - 结论 1：canonical 查询入口已稳定收口到 `MaterializedMir::callable_view()` / `LoweredHir::materialized_callable_view()`。
  - 结论 2：当前视图按 `InstanceKey -> family -> root body / callable bodies / summary` 组织，不再要求消费侧自行拼 `instance_keys + summaries + file.items`。
  - 结论 3：declaration-only instance 与真实 callable body family 的边界一致，`root_body()` 与 `summary.body_known` 未发现新的矛盾。
- [验证] 已完成并通过：
  - `cargo test -p scoopc callable_view_keeps_overloaded_generic_roots_distinct -- --nocapture`
  - `cargo test -p scoopc single_file_frontend_keeps_distinct_effect_row_generic_instances -- --nocapture`
  - `cargo test -p scoop build_frontend_keeps_distinct_effect_row_generic_instances -- --nocapture`
  - `cargo test --all`
  - `cargo run -p scoop -- test`（`fixtures: ok (1201)`）
  - `cargo clippy --all-targets -- -D warnings`
- [结果] 本轮未发现必须前插的新缺陷任务；`T5000h0bR` 可标记完成，下一条应切换到 `T5000h0c`。
