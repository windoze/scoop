# 执行计划

这里不记录私有推理过程；本文件只记录本次调用可审计的执行计划、关键决策和进度。

## 当前计划

1. 先读取 `TODO.md` 识别第一个未完成任务，仅把标题带 `[DONE]` 的任务视为已完成。
2. 只检查最新提交是否包含与当前任务直接相关的未完成事项。
3. 针对已选任务检查必要代码和测试，不做无关历史问题扫查。
4. 完整实现当前任务；若被规范正确性问题阻塞，则只添加最小必要前置任务。
5. 运行 `TODO.md` 要求的任务验证，以及确保无 warning / regression 所需的相关 build、lint、test 命令。
6. 若任务完成，在 `TODO.md` / 对应任务文件中标记 `[DONE]` 并填写完成记录；只有阶段级计划变化才更新 `PLAN.md`。
7. 用清晰的任务标签提交本次调用的预期变更。
8. 完成一个任务或提交阻塞/前置任务更新后停止。

## 进度记录

- 已开始本次调用，并在项目检查前写入初始执行计划。
- 已识别首个未完成任务为 `P5-T04`：建立正式 LIR optimization family 与 pass pipeline。最新提交 `290c3232 [P5-T03R] Review LIR query switch` 未显示与本任务直接相关的未完成阻塞项。
- 已检查现有 `effect_lowered::opt`、`LirStageOutput`、`LirFacts` 和 effect-lowered fixture dump 形状；未做无关历史问题扫查。
- 已实现首轮代码改造：`scoopc_lir_facts` 增加 opt pipeline metadata / revision 结构、dump 输出和 verifier binding；LIR stage 现在从 `run_lir_opt_pipeline(...)` metadata 构造 `LirFacts`，不再只调用隐式 helper。
- 已增加显式 pass 顺序 metadata：local state-machine elimination、higher-order wrapper inline/devirt owner skeleton、wrapper state folding、dynamic invoke rewrite、dead state/slot cleanup、resume packing pruning、post-opt verification。
- 已泛化 control-body 优化路径，让 effect-step callable 与带本地 effect/control body 的 plain callable 共用 LIR-owned state/boundary/frame/capture rewrite 和 verifier 路径。
- 已用新的 opt pipeline metadata 重新生成并验证 `tests/fixtures/effect_lowered/*.effectlowered`。
- 已在 `TODO.md` 与 `TODO-5.md` 标记 `P5-T04` 完成并填写完成记录；验证通过 `cargo fmt`、`cargo test -p scoopc --no-default-features effect_lowered::opt`、`cargo run -p scoop -- test --fixtures tests/fixtures/effect_lowered`、`cargo test -p scoopc_lir_facts`、`cargo clippy --all-targets -- -D warnings` 和 `git diff --check`。
- 已将 post-opt verifier 拆到 `effect_lowered::opt_verify`，让 `opt.rs` 聚焦 pass driver 和 rewrite 实现。
