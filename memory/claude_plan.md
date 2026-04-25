# 执行计划记录

更新时间：2026-04-26

说明：按要求先记录可公开的执行计划与检查步骤；在实际执行中如发现阻塞、既有问题、任务拆分或关键步骤完成，会继续更新此文件。

## 当前目标

完成 `TODO.md` 中第一个未完成任务；如果最新提交或执行过程中暴露出既有问题，则先修复该问题，或将其整理为当前任务之前的前置任务后停止。

当前已定位的首个未完成任务：`T5000c3bR Review：确认 concrete-type / receiver exactness helper 的依赖方向已拉直`。

## 初始执行顺序

1. 查看最新一次 Git 提交信息，确认是否明确提到需要先处理的既有问题。
2. 阅读 `TODO.md`，定位第一个未完成任务。
3. 阅读 `PLAN.md`，理解该任务的上下文、依赖和现有计划。
4. 评估该任务是否足够小且可直接完成。
5. 如果任务过大：
   - 将其拆分为更小的子任务；
   - 更新 `PLAN.md`；
   - 更新 `TODO.md`，把子任务插入正确顺序；
   - 执行其中第一个子任务。
6. 在实现过程中，若测试、代码审查或运行暴露既有缺陷、规格不匹配、实现边界缺失或回归：
   - 先判断能否在当前轮直接修复；
   - 若能修复，则先修复再继续；
   - 若不能直接修复，则把它作为前置任务插入 `TODO.md` 当前任务之前，更新 `PLAN.md` 后提交并停止。
7. 实现当前目标任务。
8. 运行相关验证，包括至少：
   - 针对改动范围的测试；
   - 必要时运行更高层级测试；
   - `cargo fmt`；
   - `cargo clippy --all-targets -- -D warnings`（若与本次改动相关且仓库当前状态允许）。
9. 更新文档与任务状态：
   - 在 `TODO.md` 标记任务完成，或在阻塞场景下调整顺序；
   - 更新 `PLAN.md`；
   - 继续更新本文件记录结果。
10. 检查工作区改动，确认未误回退用户已有修改。
11. 提交本轮变更，提交信息明确描述完成的任务或处理的前置问题。
12. 停止，不继续执行下一个任务。

## 当前轮审计重点（已根据 `TODO.md` / `PLAN.md` 更新）

1. 检查 `expr_facts.rs`、`program_facts.rs`、`effect_state_machine_analysis.rs`、`llvm/codegen/mod.rs` 及其相关调用点。
2. 确认 concrete-type / field-type / receiver exactness helper 是否已收口到 shared facts / analysis 层。
3. 搜索是否仍残留 planning / summary 与 backend generic lowering 各自维护的平行 helper。
4. 若审计中发现既有问题：
   - 先修复并补测；
   - 或将问题前置写入 `TODO.md` / `PLAN.md` 后停止。
5. 若未发现阻塞问题，则补跑与该 review 相关的验证，更新 `TODO.md` / `PLAN.md` / 本文件并提交。

## 当前进度

- 已确认最新提交说明未额外引入需优先处理的既有问题。
- 已定位并完成当前轮目标：`T5000c3bR Review：确认 concrete-type / receiver exactness helper 的依赖方向已拉直`。
- 审计结论：
  - `ExprFactResolver` 已成为 concrete-type / field-type / call-result 解析的唯一主体实现；
  - `ProgramFacts` 已承接 shared resolver 所需的 top-level/object/field/return 共同输入；
  - effect planning 与 backend generic lowering 中剩余相关入口仅是注入 local type lookup 的薄包装；
  - 未发现必须插入到 `T5000c3R` 之前的新前置缺陷任务。
- 本轮已完成验证：
  - `cargo fmt --all --check`
  - `cargo test -p scoopc llvm::tests::lowered_call_results_keep_concrete_types_for_local_bindings`
  - `cargo test -p scoopc direct_step_effect_rows_include_direct_effectful_call_after_escape_site`
  - `cargo test -p scoopc --no-default-features direct_step_effect_rows_include_direct_effectful_call_after_escape_site`
  - `cargo test --all`
  - `cargo clippy --all-targets -- -D warnings`
- 剩余动作：
  1. 检查工作区变更。
  2. 以 `T5000c3bR` 为主题提交本轮文档更新。
  3. 停止，等待下一次调用处理 `T5000c3R`。

## 当前已知约束

- 不接受规避性实现、夹具特判或偏离规格的临时方案。
- 如果遇到缺失语言特性、编译器/运行时缺陷或标准库缺口，必须把问题前置处理，而不是绕过。
- 需要注意仓库可能存在未提交修改，不能破坏与本轮任务无关的变更。
