# 执行计划

1. 先读取 `TODO.md`，按标题是否带有 `[DONE]` 来确定第一个未完成任务；不做开放式问题排查。
2. 检查最近一次提交信息是否直接提到与该任务相关的未完成事项；如果是，则将其视为当前任务的一部分，或在 `TODO.md` 中补充为前置任务。
3. 阅读当前任务条目中的要求、依赖、验证方式与完成记录；必要时再查看 `PLAN.md` 了解阶段背景，但不把 `PLAN.md` 当作日常执行日志。
4. 基于任务要求检查相关代码、测试与文档，确认现状与缺口。
5. 以最小且正确的改动实现当前任务；如遇阻塞当前任务的真实缺陷或缺失能力，不做变通，而是在 `TODO.md` 中补充最小前置任务并停止。
6. 运行与当前任务直接相关的验证，包括要求中的测试，以及必要的格式化、`cargo test`、`cargo clippy --all-targets -- -D warnings` 等，直到结果满足任务要求或明确暴露阻塞问题。
7. 及时更新本文件，记录任务识别结果、关键决策、已完成步骤、测试结果，以及计划变更。
8. 若任务完成，则在 `TODO.md` 中将该任务标题显式改为 `[DONE]`，补全完成记录；仅当阶段计划本身变化时才更新 `PLAN.md`。
9. 按仓库约定创建一次 Git 提交，提交本次任务涉及的全部未提交修改，然后停止，不继续下一个任务。

## 当前状态

- 已读取 `TODO.md`，首个未完成任务为 `CG-T07S`：修复 full-suite cross-fixture transport metadata drift，解除 `CG-T08` 默认回归阻塞。
- 最近一次提交为 `[CG-T07S0] Restore callable direct named-arg lowering`，与 `CG-T07S` 直接相关，且 `TODO.md` 已把它记录为前置 prerequisite `CG-T07S0`，目前该 prerequisite 已标记 `[DONE]`。
- `CG-T07S` 当前需要完成的工作是重新执行任务要求中的验证，确认 `tests/fixtures/mir_refactor/aggregate_transport.scoop` 与默认 `cargo run -p scoop -- test` 在 full-suite/单跑下不再发生 transport metadata drift；若验证稳定通过，则把 `CG-T07S` 标记为 `[DONE]` 并提交。

## 当前执行步骤

1. 查看当前工作区状态，确认是否存在未提交修改，以及是否需要在本次提交中一并纳入。
2. 运行 `CG-T07S` 要求的定向验证：
   - `cargo test -p scoop run_all_recreates_session_between_independent_fixtures`
   - `cargo test -p scoopc refactor_mir_stable_dump_canonicalizes_type_ids_by_structure`
   - `cargo run -p scoop -- test --fixtures tests/fixtures/mir_refactor/aggregate_transport.scoop`
   - `cargo run -p scoop -- test`
3. 若验证中出现新的且直接阻塞 `CG-T07S` 的问题，先精确定位根因，并按规则仅在 `TODO.md` 中加入最小前置任务后停止。
4. 若验证全部通过，更新 `memory/claude_plan.md` 记录结果，并把 `TODO.md` 中 `CG-T07S` 标题改为 `[DONE]`，补全完成记录。
5. 运行 `cargo clippy --all-targets -- -D warnings` 作为质量闸门。
6. 生成一次 Git 提交，提交本次任务涉及的全部当前未提交文件，然后停止。

## 执行进展

- 已完成工作区检查：当前未提交修改仅有 `memory/claude_plan.md`。
- 已完成 `CG-T07S` 规定验证，结果如下：
  - `cargo test -p scoop run_all_recreates_session_between_independent_fixtures`：通过。
  - `cargo test -p scoopc refactor_mir_stable_dump_canonicalizes_type_ids_by_structure`：通过。
  - `cargo run -p scoop -- test --fixtures tests/fixtures/mir_refactor/aggregate_transport.scoop`：通过，`fixtures: ok (1)`。
  - `cargo run -p scoop -- test`：通过，`fixtures: ok (1270)`。
- 已将 `TODO.md` 中的索引与任务标题更新为 `[DONE] CG-T07S`，并补充 2026-05-09 的完成记录。
- `cargo clippy --all-targets -- -D warnings`：通过。
- `PLAN.md` 无需更新：本次没有阶段级依赖或完成标准变化，只是完成当前任务的收尾验证与状态登记。
- 下一步：把 `TODO.md` 与 `memory/claude_plan.md` 一并提交，提交后停止。
