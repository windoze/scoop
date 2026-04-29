## 当前计划

1. 查看最新一次 git 提交信息，确认是否提到任何已知问题；若有，先复现、修复并验证。
2. 阅读 `TODO.md`，确定第一个未完成任务。
3. 如该任务过大，则把它拆解为更小的可执行子任务，并同步更新 `PLAN.md` 与 `TODO.md`，本次只执行第一个子任务。
4. 在动手修改前阅读相关代码与测试，确认实现边界，并检查是否存在会阻塞当前任务的既有问题。
5. 实现当前任务所需的最小正确改动；若发现既有缺陷或规格不匹配，先修复，或将其作为前置任务插入 `TODO.md` 后停止。
6. 运行相关验证，包括针对性测试，以及必要时运行更广泛的检查（如 `cargo test` / `cargo clippy --all-targets -- -D warnings`）。
7. 更新文档进度：勾掉 `TODO.md` 中已完成任务，并同步更新 `PLAN.md`。
8. 按仓库提交风格创建一次 git commit，然后停止，不继续下一个任务。

## 说明

- 我会在关键节点更新本文件，记录计划调整、阻塞项与完成状态。
- 这里记录的是执行摘要与步骤，不包含冗长的内部推理。

## 当前任务定位（更新）

- 最新提交是 `T5000j3bR` 的 review 记录，没有单独点名新的待修缺陷。
- `TODO.md` 中首个未完成条目是 `T5000j3R Review：确认 higher-order / init 场景扩张没有把分析责任倒灌回 backend`。
- 本轮将重点复核以下边界：
  1. `llvm/codegen/mir_body.rs` 是否只消费 materialized MIR / pass artifacts 做 lowering；
  2. `llvm/reachability.rs` 与 `llvm/emit.rs` 是否只做 canonical body 选择与可达性收集，而不是现场重建 target-set；
  3. `object-init` / `top-level-init` / closure / fun-value 扩张是否仍依赖 shared facts、side tables 与 pass view；
  4. 若 review 暴露既有问题，先修复或将其插入 `TODO.md` 作为前置任务后停止。

## 执行结果（更新）

- 已完成 `T5000j3R Review` 的代码审计，未发现需要前插到 `T5000j4` 之前的新既有缺陷任务。
- 审计结论摘要：
  1. `llvm/emit.rs` / `llvm/reachability.rs` / `llvm/codegen/mod.rs` 继续只负责 canonical body 选择、可达性扫描与 lowering 入口，不在 backend 现场重建 higher-order / init target-set；
  2. `llvm/codegen/mir_body.rs` / `object_init.rs` 继续只消费 materialized MIR、pass view、类型映射与现有 init side tables，负责 lowering / ABI / outcome boundary，而不是重算 shared facts；
  3. effect / suspendability / continuation-escape 输入继续来自 `ProgramFacts`、`EffectAnalysisCtx`、pass summary 与 escape facts。
- 已完成验证：
  - `cargo test -p scoopc production_codegen_lowers_raw_mir_top_level_immutable_init_access -- --nocapture`
  - `cargo test -p scoopc production_codegen_lowers_raw_mir_object_value_init_access -- --nocapture`
  - `cargo test -p scoopc production_reachability_emits_object_init_helper_dependency_for_raw_mir_top_level_ref -- --nocapture`
  - `cargo test -p scoopc production_codegen_lowers_raw_mir_non_capturing_closure_body -- --nocapture`
  - `cargo test -p scoopc production_codegen_lowers_raw_mir_immutable_capture_closure_body -- --nocapture`
  - `cargo test -p scoopc production_codegen_lowers_raw_mir_fun_value_call_body -- --nocapture`
  - `cargo test -p scoopc production_codegen_uses_closure_definition_source_for_cross_file_raw_mir_body -- --nocapture`
  - `cargo test --all`
  - `cargo clippy --all-targets -- -D warnings`
  - `cargo run -p scoop -- test`
- 文档状态：`TODO.md` 与 `PLAN.md` 已更新为本任务完成，下一条待执行任务已切换为 `T5000j4`。
