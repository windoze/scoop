## 本次执行计划

说明：按你的要求，我会持续更新这份文件记录执行计划、关键进展和必要调整。出于安全与协作边界，我不会写入逐字的内部推理过程，但会完整记录可审计的执行步骤、依据和结果。

1. 先读取 `TODO.md`，把它当作任务索引使用。
2. 按索引顺序检查对应的 `TODO-Px.md` 详细任务文件。
3. 识别第一个标题未标记 `[DONE]` 的详细任务，确认它才是当前执行单元。
4. 检查最近提交是否直接提到与该任务相关且未完成的问题；如果是，则将其视为当前任务的一部分，或在对应 `TODO-Px.md` 中登记为前置任务。
5. 阅读当前任务要求、约束、依赖、验收条件以及现有实现。
6. 如任务可直接完成，则实施最小正确修改，不引入规避性方案。
7. 运行相关验证：至少包括与当前改动直接相关的测试；若任务要求或影响范围需要，则补充更广的验证，并确保无警告通过 `cargo clippy --all-targets -- -D warnings`。
8. 若出现阻塞当前任务的真实缺口或规范不匹配：
   - 精确记录问题；
   - 在正确的 `TODO-Px.md` 中插入最小必要前置任务；
   - 同步 `TODO.md`；
   - 仅在阶段计划被影响时更新 `PLAN.md`；
   - 提交并停止。
9. 若任务完成：
   - 在对应 `TODO-Px.md` 中将任务标题改为 `[DONE]`；
   - 更新完成记录；
   - 如索引需要同步，则更新 `TODO.md`；
   - 仅在阶段计划变化时更新 `PLAN.md`。
10. 检查工作区中与本次任务相关的未提交修改，避免覆盖他人改动；如本次是在延续上次失败后的同一任务，则按要求一并提交当前所有未提交文件。
11. 使用清晰的 Git 提交信息提交本次结果，然后停止，不继续下一个任务。

## 进度记录

- 已写入初始执行计划。
- 已读取 `TODO.md`，确认索引中的首个未完成详细任务为 `P6-T02`。
- 已读取 `TODO-P6.md`，确认 `P6-T02` 的 authoritative 要求是把 P5 的 `Step` / frame / continuation / resume-interface 合同下沉到新的 LLVM type/layout materialization 层，并补齐单测与 build fixture 验证。
- 已检查最近一次提交：`[P6-T01b] Track refactor build ABI visibility blocker`。该提交直接相关的阻塞已在当前工作树中以 `P6-T01b` 的完成记录形式落账，并且索引 `TODO.md` 也已同步为 `[DONE]`。
- 已检查工作区，发现一组围绕 `P6-T02`/`P6-T01b` 收口的未提交修改，包含：
  - refactor build/LLVM stage 新增 ABI visibility handoff；
  - build fixture runner 透传 refactor session 选项；
  - `P6-T02` 的三个 build fixtures 改为 `pure main + 不可达 effectful helper` 验证形状；
  - `TODO-P6.md` / `TODO.md` / `memory/claude_plan.md` 的任务记录尚未最终收口。
- 当前判断：这是同一执行单元的续作，而不是新的无关改动；因此接下来先验证这些修改是否足以让 `P6-T02` 完成。
- 下一步验证矩阵：
  - `cargo test -p scoopc refactor_llvm_step_layout`
  - `cargo test -p scoopc refactor_llvm_frame_layout`
  - `cargo test -p scoopc refactor_llvm_continuation_layout`
  - `cargo test -p scoopc refactor_llvm_unit_abi`
  - `cargo run -p scoop -- --effect-pipeline refactor test --fixtures tests/fixtures/build/effect_refactor_step_enum_single_case.scoop`
  - `cargo run -p scoop -- --effect-pipeline refactor test --fixtures tests/fixtures/build/effect_refactor_dynamic_invoke_unit_payload.scoop`
  - `cargo run -p scoop -- --effect-pipeline refactor test --fixtures tests/fixtures/build/effect_refactor_continuation_interface_full_methods.scoop`
  - `cargo run -p scoop -- --effect-pipeline legacy test --fixtures tests/fixtures/build/effect_no_perform_no_handler_symbols_basic.scoop`
  - `cargo clippy -p scoopc -p scoop --all-targets -- -D warnings`
- 已完成验证：
  - `cargo test -p scoopc refactor_llvm_`
  - `cargo test -p scoop refactor_build_`
  - `cargo test -p scoop build_fixtures_propagate_refactor_session_options_to_build_command`
  - `cargo run -p scoop -- --effect-pipeline refactor test --fixtures tests/fixtures/build/effect_refactor_step_enum_single_case.scoop`
  - `cargo run -p scoop -- --effect-pipeline refactor test --fixtures tests/fixtures/build/effect_refactor_dynamic_invoke_unit_payload.scoop`
  - `cargo run -p scoop -- --effect-pipeline refactor test --fixtures tests/fixtures/build/effect_refactor_continuation_interface_full_methods.scoop`
  - `cargo run -p scoop -- --effect-pipeline legacy test --fixtures tests/fixtures/build/effect_no_perform_no_handler_symbols_basic.scoop`
  - `cargo clippy -p scoopc -p scoop --all-targets -- -D warnings`
- 结论：`P6-T02` 所需的 type/layout query API、单测、真实 refactor build fixture 验证与 legacy 抽样兼容性均已收口，可将 `P6-T02` 标记为完成，并提交当前工作区中的同任务未提交文件。
