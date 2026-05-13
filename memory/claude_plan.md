# 当前执行计划

说明：按要求先记录执行计划。这里记录的是可审阅的高层步骤与决策依据，不包含不可审阅的内部推理细节。

1. 读取 `TODO.md`，确定第一个标题未带 `[DONE]` 的任务，严格按该任务作为当前执行单元。
2. 查看最近一次提交信息，确认是否存在与该任务直接相关的未完成事项；若存在且构成当前任务前提，则将其纳入当前任务或作为 `TODO.md` 中的新前置任务处理。
3. 阅读当前任务涉及的 `TODO.md` 条目、依赖说明、验证要求，以及必要的源码/测试文件，建立最小实现范围。
4. 如任务可直接完成：实现代码、补充或更新测试、运行任务要求的验证命令，并修复出现的问题直到通过。
5. 如遇到阻塞当前任务且不能绕过的缺失/回归/规格不匹配：先精确定义问题，再只在必要时对 `TODO.md` 做最小前置任务调整；仅当阶段计划发生变化时更新 `PLAN.md`。
6. 完成后更新 `memory/claude_plan.md`、在 `TODO.md` 中将任务标题改为 `[DONE]` 并填写完成记录，随后按仓库约定提交一次 git commit，然后停止，不进入下一个任务。

当前任务：`P7-T01R：Review 全量收口结果，确认 stable-id 方案已闭合且未带来功能漂移`

补充计划：
1. 已根据 `TODO.md` 确认首个未完成任务为 `P7-T01R`。
2. 下一步检查最近一次提交与当前工作区，确认是否存在与最终 review 直接相关、且必须先纳入本任务的未完成事项。
3. 若无新的 blocker，则按 review 任务执行：复核 `PLAN.md` / `STABLE_ID.md` 的最终完成标准，对照 P7 相关实现与测试入口重跑必要验证，并检查是否存在 residual identity 泄漏或功能漂移。
4. 若 review 发现真实 blocker，则在 `TODO.md` 中插入最小前置任务并停止；若未发现 blocker，则回写 `TODO.md` 完成记录并提交。

当前验证矩阵：
1. 审计/签收定向验证：
   - `cargo test -p scoopc checkout_root -- --nocapture`
   - `cargo test -p scoopc distinct_virtual_cones -- --nocapture`
   - `cargo test -p scoopc stable_id_audit_grep_inventory_scans_repo_roots -- --nocapture`
2. 全量行为验证：
   - `cargo test -p scoopc`
   - `cargo test -p scoop_runtime`
   - `cargo run -p scoop -- test`
   - `cargo run -p scoop_tools -- spec-fixtures check`
   - `cargo test --all`
3. 质量验证：
   - `cargo clippy --all-targets -- -D warnings`
4. 若以上通过，再把 `PLAN.md` §6 的 8 条完成标准逐项写入 `TODO.md` 完成记录并提交。

阶段结果更新：
1. 最近提交 `[P7-T01C] Record completion log` 未声明与 `P7-T01R` 直接相关的额外未完事项。
2. 已完成的 review 验证：
   - `cargo test -p scoopc checkout_root -- --nocapture` 通过。
   - `cargo test -p scoopc distinct_virtual_cones -- --nocapture` 通过。
   - `cargo test -p scoopc stable_id_audit_grep_inventory_scans_repo_roots -- --nocapture` 通过。
   - `cargo test -p scoopc` 通过。
   - `cargo test -p scoop_runtime` 通过。
   - `cargo run -p scoop_tools -- spec-fixtures check` 通过。
3. 新发现 blocker：`cargo run -p scoop -- test` 失败，阻塞用例为 `tests/fixtures/run-pass/class_init_raise_cleanup_init_block_gc_basic.scoop`。
   - 直接复现 `cargo run -p scoop -- run tests/fixtures/run-pass/class_init_raise_cleanup_init_block_gc_basic.scoop` 报错：
     `LLVM codegen 前端准备失败：MIR value box LLVM type 无法构造 stable canonical type key: missing stable type parameter key for 'B'`。
   - 初步定位：`crates/scoopc/src/llvm/codegen/mod.rs` 的 `canonical_type_key_text_for_codegen` / `stable_rtti_type_id_for_codegen` 仍固定使用 `NoTypeParamResolver`，而 `mir_body.rs` 的 MIR value-box/type-desc/type-driven private naming 在某些泛型清理/展开路径中已会遇到仍含 type param 的 `TypeId`。
4. 执行动作调整：不继续做 `cargo run -p scoop -- test` 之后的全量签收；已在 `TODO.md` 中插入最小前置任务 `P7-T01D`，并把 `P7-T01R` 标记为依赖该任务；下一步提交这些任务编排变更并停止。

待执行中的检查点：
- [x] 确定当前首个未完成任务
- [x] 确认最近提交是否带来直接相关的未完事项
- [x] 完成实现或记录阻塞并调整 `TODO.md`
- [ ] 完成验证
- [ ] 更新文档与提交
