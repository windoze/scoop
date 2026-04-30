## 当前计划

说明：按安全与协作要求，这里记录可执行计划、关键决策与进展摘要，不记录私有推理细节。

1. 检查最新一次提交，确认是否提到需要先修复的既有问题。
2. 阅读 `TODO.md`，定位第一个未完成任务。
3. 评估该任务是否需要拆分；若需要，则更新 `PLAN.md` 与 `TODO.md`，并只执行拆分后的第一个子任务。
4. 实现当前任务所需改动，期间若发现既有缺陷、规格不匹配或前置能力缺失，优先修复；若无法在本轮直接修复，则把前置修复任务插入 `TODO.md` 到依赖位置，并停止。
5. 运行与改动直接相关的测试；如有必要，再补充更广的验证，直到当前任务范围内结果稳定。
6. 更新 `TODO.md` 与 `PLAN.md`，记录完成状态、依赖调整和验证结果。
7. 按仓库约定创建一次 git 提交，然后停止，不继续下一个任务。

## 进展

- 已写入初始计划，下一步开始检查最新提交与待办顺序。
- 已检查最新提交：`aa45320fef6d947801a3bbb189a7fbbffdb0ec62`，提交说明为 `[T5002b2a1] Fix pass-MIR closure boundary gaps`，未在提交说明中显式记录新的待优先修复既有问题。
- 已读取 `TODO.md` 与 `PLAN.md`，当前首个未完成任务为 `T5002b2aR`：review ordinary indirect-call surface 是否已统一改走显式 `incoming_resume_token_ref`。
- 当前执行计划细化为：
  1. 复核 closure / funptr / vtable / itable 相关 signature、boundary helper 与 caller IR 生成路径。
  2. 运行与 `T5002b2aR` 直接相关的 LLVM 回归与至少一组对应 fixture，必要时在 GC 环境下复验。
  3. 若 review 暴露既有缺口，先修复该缺口；若无法在本轮直接闭合，则按依赖前插 `TODO.md`/`PLAN.md` 并停止。
  4. 若 review 通过，则更新 `TODO.md`、`PLAN.md`、本文件，提交一次只覆盖本任务的变更。
- 已完成代码复核：`codegen/mod.rs`、`closure/mod.rs`、`mir_body.rs` 与 `call/dispatch.rs` 中，generated callable 的 hidden 参数顺序已经统一为“hidden sret（如有）后接 incoming token，再进入 env/receiver/普通参数”；ordinary indirect-call boundary 与 pass-MIR closure caller 也都在 `consume outcome` 后显式 `clear` token。
- 已完成验证：
  - `cargo test -p scoopc explicit_outcome_boundary -- --nocapture`
  - `cargo test -p scoopc production_pass_mir_closure_call_reloads_closure_after_effect_boundary -- --nocapture`
  - `cargo test -p scoopc production_codegen_lowers_raw_mir_effectful_closure_body_direct_perform -- --nocapture`
  - `cargo test -p scoopc production_pass_mir_effectful_closure_body_direct_perform_lowering -- --nocapture`
  - `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/effect_indirect_perform_nonresuming_function_value_local.scoop`
  - `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/effect_indirect_perform_materialized_mir_closure_basic.scoop`
  - `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/effect_handle_hidden_suspend_virtual_helper_basic.scoop`
  - `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/effect_handle_hidden_suspend_interface_helper_basic.scoop`
  - 上述 4 个 fixture 也已在 `SCOOP_GC_MOVE=1 SCOOP_GC_STRESS=1 SCOOP_GC_VERIFY_ROOTS=1` 下复验通过。
  - `cargo clippy --all-targets -- -D warnings`
- 当前结论：`T5002b2aR` 已写回 `TODO.md` / `PLAN.md`，下一步只提交本轮 review 记录并停止。
