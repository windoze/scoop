## 执行计划

说明：这里记录可审计的执行计划、关键决策与进度更新，不写入逐字的私有推理。

1. 检查最新一次 Git 提交，确认提交说明里是否提到需要先修复的既有问题。
2. 阅读 `TODO.md`，定位第一个未完成任务。
3. 阅读 `PLAN.md`，核对该任务的上下文、依赖和既有拆分。
4. 判断该任务是否过大：
   - 如果过大，先在 `PLAN.md` 和 `TODO.md` 中拆分成更小的前置子任务，只执行新的第一个子任务。
   - 如果可直接完成，进入实现阶段。
5. 在实现前做必要的代码与测试现状调查，识别任何阻塞该任务的既有缺陷。
6. 若发现阻塞性既有问题：
   - 先修复该问题；如果当前无法在本轮直接修复，则把它作为前置任务插入 `TODO.md` 的正确位置，更新 `PLAN.md`，提交后停止。
7. 实现当前目标任务。
8. 运行相关验证：
   - 先跑最小必要测试；
   - 再跑任务相关的完整测试；
   - 最后跑质量门禁，包括 `cargo fmt --check`、`cargo clippy --all-targets -- -D warnings` 和必要的 `cargo test`。
9. 更新文档与任务状态：
   - 在 `TODO.md` 中标记任务完成；
   - 在 `PLAN.md` 中更新进度、风险和后续顺序；
   - 视需要更新 `README.md` 或相关注释。
10. 检查工作区变更，确保不误改无关内容。
11. 使用清晰的提交信息提交本轮变更。
12. 停止，不继续处理下一个任务。

## 进度记录

- 已创建本文件，并写入初始计划。
- 已检查最新提交 `b485ed3f [T5000c3aR] Review shared state-machine analysis ownership`，提交说明未额外声明必须先修复的遗留缺陷。
- 已阅读 `TODO.md` / `PLAN.md`，当前第一条未完成任务为 `T5000c3b 收口 concrete-type / field-type / receiver exactness 共享 helper 的消费方向`。
- 已完成任务探查，当前判断这条任务可在本轮直接完成，不需要先拆分：
  - `crates/scoopc/src/effect_state_machine_analysis.rs` 里存在两套几乎平行的 concrete-type / field-type 解析 helper：一套给 planning，自由函数前缀为 `resolve_plan_*`；另一套给 direct-step summary，挂在 `SuspendCallAnalysis` 上。
  - `crates/scoopc/src/llvm/codegen/mod.rs` 里还保留第三套平行 helper：`resolve_expr_concrete_type`、`resolve_member_access_concrete_type`、`resolve_*_field_concrete_type`、`resolve_call_result_type`。
  - 这些逻辑都依赖同一批 shared facts：`ProgramFacts::{top_level_value_tys, fun_return_tys, object_property_tys, struct_field_tys, class_field_tys, class_super_keys}`。
- 本轮具体执行方案：
  1. 新增一个 backend-agnostic 的 shared resolver 模块，负责基于 `TypeStore + ProgramFacts + local type lookup` 解析表达式 concrete type / field type / call result type。
  2. 在 `ProgramFacts` 上补充最小查询 helper，把 struct/class field、object property、top-level value、function return 等事实访问集中到 shared 层。
  3. 将 `effect_state_machine_analysis.rs` 中 planning 与 direct-step summary 的 duplicated helper 改成统一调用 shared resolver。
  4. 将 `llvm/codegen/mod.rs` 中的 duplicated helper 收口为对 shared resolver 的薄包装，避免 backend generic lowering 继续维护独立实现。
  5. 运行格式化、相关测试、全量测试与 `clippy -D warnings`，若中途暴露既有缺陷，先修复再继续。
  6. 更新 `TODO.md` / `PLAN.md` / 本文件，提交本轮变更后停止。
- 已完成代码实现：
  - 新增 `crates/scoopc/src/expr_facts.rs`，把 concrete-type / field-type / call-result 解析统一收口为 shared `ExprFactResolver`；
  - 在 `crates/scoopc/src/program_facts.rs` 中补充 shared fact 查询 helper；
  - `effect_state_machine_analysis.rs` 与 `llvm/codegen/mod.rs` 中的 duplicated helper 已切换到 shared resolver，原平行实现已删除。
- 已完成验证：
  - `cargo fmt --all --check`
  - `cargo test -p scoopc llvm::tests::lowered_call_results_keep_concrete_types_for_local_bindings`
  - `cargo test -p scoopc direct_step_effect_rows_include_direct_effectful_call_after_escape_site`
  - `cargo test -p scoopc --no-default-features direct_step_effect_rows_include_direct_effectful_call_after_escape_site`
  - `cargo test -p scoopc --no-default-features`
  - `cargo test --all`
  - `cargo clippy --all-targets -- -D warnings`
  - 结果：全部通过。
- 当前收尾步骤：
  1. 检查工作区 diff，确认只包含本轮任务与计划更新。
  2. 提交变更。
  3. 停止，等待下一轮从 `T5000c3bR` 继续。
