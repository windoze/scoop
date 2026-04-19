# 本轮执行计划

## 说明

用户要求先写入“完整思考过程”。我会在此记录可审阅的分析摘要、关键判断依据与分步计划，但不记录逐字内部推理。

## 初始目标

本轮只完成 `TODO.md` 中第一个未完成任务，然后停止。开始任何仓库检查命令前，先创建并写入本文件。

## 执行步骤

1. 查看最新一次提交，确认提交信息里是否提到已知遗留问题。
2. 如果最新提交明确提到遗留问题，先定位并修复这些问题，再继续正常任务流程。
3. 阅读 `TODO.md`，识别第一个未完成任务。
4. 评估该任务是否足够清晰且可在一轮内完整完成。
5. 如果任务过大或存在前置依赖缺失：
   - 拆分为更小的子任务；
   - 更新 `PLAN.md`；
   - 调整 `TODO.md` 的顺序与依赖；
   - 本轮只处理拆分后的第一个子任务，或在被依赖阻塞时仅提交计划调整。
6. 在实施前阅读相关代码、测试与规范文件，确认实现边界。
7. 完成代码实现，禁止使用规避规范的临时方案。
8. 运行相关测试，并补充必要测试；同时确保无警告要求满足，包括 `cargo clippy --all-targets -- -D warnings`（若改动范围允许，应至少覆盖相关目标）。
9. 更新文档与任务状态：
   - 在 `TODO.md` 中标记当前任务完成；
   - 在 `PLAN.md` 中记录当前状态与后续变化；
   - 如实现过程中计划变化，及时回写本文件。
10. 使用清晰提交信息创建一次 git commit。
11. 停止，不继续处理下一个任务。

## 风险检查

- 如果发现规范缺口、语言特性缺失、实现边界错误或测试只能靠 workaround 通过，必须先在 `TODO.md` 中新增前置修复任务并调整顺序，然后提交这些计划变更并停止。
- 不回退或覆盖用户已有未关联修改。
- 若 `PROMPT.md` 在过程中出现变更，需要一并纳入提交。

## 进度

- 已完成：创建本计划文件。
- 已完成：检查最新提交信息；提交消息仅为 `[T4006V] 收口非局部 receiver 的链式成员访问`，未额外声明待先修复的遗留问题。
- 已完成：读取 `TODO.md` / `PLAN.md`，确认首个未完成任务为 `T4006R`（review：确认 compilation-unit 维度规则已统一）。
- 已完成：在 `lower_for_compilation_unit_multi_files` 里定位到一个高风险点：`ctor_call_sites` / `continuation_resume_call_sites` 仍按裸 `Span` 存储，因此多文件 lowering 通过“只保留入口文件 side table”回避跨文件冲突。
- 已完成：新增 cone 复现夹具 `tests/fixtures/run_pass_cone/cross_file_ctor_named_default_basic/**`，覆盖非入口文件中的普通 helper 函数、object init、class init 三条 ctor 调用路径。
- 已完成：直接执行 `cargo run -p scoop -- build tests/fixtures/run_pass_cone/cross_file_ctor_named_default_basic -o /tmp/t4006r_cross_file_ctor.out`，稳定复现失败：后端把非入口文件中的 `Box(y = ...)` 误判为 enum variant ctor，报 `enum variant ctor call without expected enum type`。
- 已完成：把 ctor / continuation 调用点 side table 改为携带 `source_path` 的 source-aware key，并移除 multi-file lowering 中对入口文件的特权过滤；同时把 reachability、LLVM codegen、known-fun suspend 分析与 unified state-machine plan 一并切换为 source-aware 查询。
- 已完成：补充 HIR 单测 `lower_for_compilation_unit_multi_files_preserves_non_entry_call_site_side_tables`，直接断言非入口文件中的 ctor 调用点与 `Continuation.resume` 调用点都会进入 lowering side table。
- 已完成：验证 `cargo test -p scoopc lower_typed_single_source_file_records_statement_position_continuation_resume_call_site -- --nocapture`、`cargo test -p scoopc lower_for_compilation_unit_multi_files_preserves_non_entry_call_site_side_tables -- --nocapture`、`cargo run -p scoop -- build tests/fixtures/run_pass_cone/cross_file_ctor_named_default_basic -o /tmp/t4006r_cross_file_ctor.out` + 运行产物、`cargo run -p scoop -- test`（`fixtures: ok (1053)`）、`cargo test --all`、`cargo clippy --all-targets -- -D warnings`。
- 进行中：更新 `TODO.md` / `PLAN.md` / 本文件，整理提交内容并创建本轮 commit；随后停止。
