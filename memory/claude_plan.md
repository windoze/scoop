## 当前执行计划

1. 先读取 `TODO.md` 作为索引，再按索引顺序检查对应的 `TODO-Px.md` 明细文件。
2. 确认“第一个未完成的详细任务”，并检查最近一次提交是否提到与该任务直接相关但未完成的问题。
3. 阅读该任务要求、约束、依赖与完成记录，定位需要修改的代码、测试或文档位置。
4. 实现该任务；若发现无法按规范完成的真实阻塞项，则只引入最小必要前置任务，并同步 `TODO.md` 与相关 `TODO-Px.md`。
5. 运行与该任务直接相关的验证；如有必要，补充或修复测试，直到结果稳定通过。
6. 更新任务记录：在对应 `TODO-Px.md` 中记录完成情况；若任务索引、标题、顺序或依赖发生变化，则同步更新 `TODO.md`；仅在阶段计划变化时更新 `PLAN.md`。
7. 复查工作区变更，使用符合仓库风格的提交信息创建一次 git 提交，然后停止，不继续下一个任务。

## 当前任务

- 已按 `TODO.md` -> `TODO-P0.md` / `TODO-P1.md` / `TODO-P2.md` / `TODO-P3.md` 顺序定位到首个未完成详细任务：`P3-T03R`（Review CFG / cleanup / `SiteId` invariants，确认 refactor MIR 已经语义闭包）。
- 最近一次提交为 `[P3-T02R] Confirm refactor MIR typed contracts`，未在提交标题中显式声明与 `P3-T03R` 直接相关的未完成项。
- 当前工作区已有未提交改动；其中 `TODO-P3.md`、`crates/scoopc/src/mir/{lower,mod,inline}.rs`、`crates/scoopc/src/effect_refactor_pipeline/mir_stage.rs` 与新增 `tests/fixtures/mir_refactor/*` 直接关联 `P3-T03/P3-T03R`，需要在不回退现有改动的前提下审阅并继续完成。

## 进度记录

- 已创建初始计划。
- 已确认当前应执行的任务为 `P3-T03R`。
- 下一步：检查 `P3-T03` 相关代码改动、定向搜索遗留占位字符串，并运行 `P3-T03R` 要求的验证命令。
- 已完成代码与任务记录复核：`mir/lower.rs`、`mir/mod.rs`、`mir/inline.rs`、`mir/materialize.rs`、`effect_refactor_pipeline/mir_stage.rs` 与新增 `tests/fixtures/mir_refactor/*` 已审阅。
- 已完成搜索检查：`handle result/body/arm/finally pending` 与 `perform unwind pending` 只剩 `mir/mod.rs` 的 verifier 禁用列表，以及 `mir/escape.rs` 中面向旧形状的兼容 helper；refactor stage / lowering 主线不再依赖这些占位。
- 已完成定向验证并通过：
  - `cargo test -p scoopc --no-default-features refactor_mir_cfg`
  - `cargo test -p scoopc --no-default-features refactor_mir_site_id`
  - `cargo test -p scoopc --no-default-features refactor_direct_mir_stage`
  - `cargo test -p scoop --no-default-features dump_mir`
  - `cargo run -q -p scoop --no-default-features -- --effect-pipeline refactor dump-mir tests/fixtures/mir/while_break_continue.scoop`
  - `cargo run -q -p scoop --no-default-features -- --effect-pipeline refactor dump-mir tests/fixtures/mir_refactor/handle_finally_boundary.scoop`
  - `cargo run -q -p scoop --no-default-features -- --effect-pipeline refactor dump-mir tests/fixtures/mir_refactor/effect_boundary_inside_expr_context.scoop`
  - `cargo run -q -p scoop --no-default-features -- --effect-pipeline refactor dump-mir tests/fixtures/run-pass/effect_handle_return_from_function_finally.scoop`
  - `cargo clippy -p scoop -p scoopc --all-targets --no-default-features -- -D warnings`
- 结论：当前未发现会阻塞 `P3-T03R` 关闭的新缺陷；已回写 `TODO-P3.md` 的 review 完成记录。下一步整理提交并停止，不继续 `P3-T04`。
