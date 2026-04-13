# 执行计划与进度记录

## 说明

按要求先写入本文件，再开始执行任何仓库检查命令。这里记录的是可审阅的执行计划、决策依据摘要和后续进度更新，不包含不可审阅的内部推理原文。

## 初始计划

1. 检查最新一次 Git 提交，确认提交说明中是否提到任何未解决问题。
2. 如果最新提交提到需先修复的问题，优先识别其影响范围并修复，随后补充测试。
3. 阅读 `TODO.md`，定位第一个未完成任务。
4. 阅读 `PLAN.md`，确认现有计划与 `TODO.md` 是否一致。
5. 判断首个未完成任务是否足够小且可在本轮完整交付。
6. 如果任务过大：
   - 在 `PLAN.md` 中拆分为更小子任务。
   - 在 `TODO.md` 中重排并插入子任务。
   - 执行拆分后的第一个子任务。
7. 实现目标任务，避免规避性实现或偏离规范。
8. 运行相关测试，并补充必要测试，确保无回归。
9. 运行格式化、`cargo clippy --all-targets -- -D warnings` 以及相关测试命令，确认质量门禁通过。
10. 更新 `TODO.md`、`PLAN.md` 和本文件，记录完成情况或阻塞原因。
11. 使用清晰提交信息创建 Git 提交。
12. 停止，不进入下一个任务。

## 执行约束

- 仅完成一个任务或一个经拆分后的首个子任务。
- 若发现规范不匹配、缺失语言特性或已有实现缺口，必须先在 `TODO.md` / `PLAN.md` 中建模为前置任务，再提交并停止。
- 不回退或覆盖非本次任务相关的现有改动。

## 进度更新

- 已创建本文件，准备开始检查最新提交与任务列表。
- 已检查最新提交：提交说明未额外标注需要先修复的遗留问题。
- 已读取 `TODO.md` / `PLAN.md`，确认首个未完成任务原为 `T2003r3b`。
- 经审计后决定先拆分 `T2003r3b`：
  - `T2003r3b1`：统一 emitter 接管 `NoSuspendSites` 主线。
  - `T2003r3b2`：统一 emitter 接管 `SingleNonResuming`。
  - `T2003r3b3`：统一 emitter 接管 `MultiNonResuming` 并退化旧 specialized entry。
- 拆分原因：
  - `NoSuspendSites` 需要先建立统一 CFG / cleanup / nested-handle 发射骨架。
  - `SingleNonResuming` / `MultiNonResuming` 还要额外处理 handler frame、dispatch 和 payload ABI。
  - 若在同一轮同时推进，风险和回归面过大。
- 接下来执行 `T2003r3b1`，本轮只完成该子任务并停止。
- `T2003r3b1` 已实现：
  - `codegen_handle_expr` 现已通过新的 `UnifiedNoContinuationEntrypoint::NoSuspendSites` 统一入口处理 no-suspend handle。
  - 旧 `codegen_handle_expr_no_perform` 已退化为共享 leaf helper，不再承担 no-suspend 主选路。
  - 统一入口会先校验 plan 中不存在 suspend subtree 与 resuming arm，再进入顺序 body/finally 发射。
- 已补测试与样例：
  - 单测：`unified_no_continuation_entrypoint_marks_nosuspend_finally_nested_handle_sample`
  - 单测：`unified_no_continuation_entrypoint_skips_single_nonresuming_sample`
  - run-pass fixture：`tests/fixtures/run-pass/effect_nosuspend_finally_nested_handle.scoop`
- 已完成验证：
  - `cargo fmt --all`
  - `cargo test -p scoopc llvm::codegen::effect::tests:: -- --nocapture`
  - `cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/effect_nosuspend_finally_nested_handle.scoop`
  - `cargo test --all`
  - `cargo clippy --workspace --all-targets -- -D warnings`
- 额外修正：
  - 确认 `scoop test` 只接受目录路径、不支持 `--filter` 名称过滤；已把当前相关 TODO 验收命令改成仓库实际支持的命令形式。
- 下一步不是继续实现，而是更新任务状态并提交本轮改动后停止。
