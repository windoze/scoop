# 本轮执行计划（摘要版）

## 约束说明

- 按要求先记录计划，再执行仓库检查与实现工作。
- 出于安全约束，这里记录的是“可审计的步骤计划、决策依据摘要、进度更新与结果”，不记录原始逐字思维链。
- 本文件会在关键步骤完成或计划调整时持续更新。

## 初始执行步骤

1. 检查最新一次 Git 提交，确认是否提到需要先修复的既有问题。
2. 阅读 `TODO.md`，识别第一个未完成任务。
3. 阅读 `PLAN.md`，确认当前计划与该任务的关系。
4. 如果首个未完成任务过大，则将其拆分为更小子任务，并同步更新 `PLAN.md` 与 `TODO.md`；本轮只执行拆分后的第一个子任务。
5. 实现本轮目标任务，过程中如果发现任何规范不匹配、缺失特性或既有缺陷，先把它们作为前置任务写入 `TODO.md` / `PLAN.md`，然后停止在合适边界。
6. 运行相关验证：
   - 目标相关测试
   - 必要时运行 `cargo test --all`
   - 必要时运行 `cargo clippy --all-targets -- -D warnings`
7. 完成后更新 `TODO.md`、`PLAN.md` 和本文件。
8. 提交 Git commit，只完成一个任务后停止。

## 待确认事项

- 最新提交是否声明了必须优先修复的问题。
- 当前首个未完成任务是否可在本轮完整落地。
- 是否存在阻塞该任务的规范缺口或实现缺陷。

## 进度日志

- 已创建本文件，准备开始仓库检查。
- 已检查最新提交 `5b139b75e1a86e9c4f7ce482e6e42691f7fde6d8`；提交说明未声明需要优先修复的遗留问题。
- 已读取 `TODO.md` 与 `PLAN.md`；当前首个未完成任务为 `T2003u5d`：mixed-arm immediate+escape 的 while richer matrix replay。
- 下一步：
  1. 检查 `mixed.rs` / `matrix.rs` 中与 `while` mixed replay 相关的现有门禁。
  2. 定位对应 build-fail fixtures 与已有相邻 run-pass 覆盖，判断 `T2003u5d` 是否仍需继续拆分。
  3. 若可控，则直接实现并转正相关回归；若仍过大，则先更新 `TODO.md` / `PLAN.md` 做进一步拆分。
- 已完成复杂度审计：`T2003u5d` 同时耦合了
  1. immediate+escape 的 while separate-stmt mixed 分类，
  2. `while -> block/if -> ...` 的 deeper nested replay，
  3. `while -> while` 的 nested-while dedicated lowering。
- 已将 `T2003u5d` 拆分为 `T2003u5d1`～`T2003u5d3`，并同步更新 `TODO.md` / `PLAN.md`。
- 本轮执行目标已切换为首个子任务 `T2003u5d1`：
  - 收口 immediate+escape mixed-arm 的 while separate-stmt direct/indirect mixed replay。
  - 转正 `effect_resume_mixed_escape_while_direct_indirect_separate_stmt_is_error`，并补 ordering 回归。
- 已完成 `T2003u5d1` 实现：
  - `matrix.rs` 的 immediate+escape site-matrix while 分类现已支持 top-level separate-stmt `direct -> indirect` / `indirect -> direct`。
  - direct->indirect 的 capture 收集已补齐；reverse-order 的 future-iteration re-entry 已接到 `while_tail_after_mixed_direct_site`。
  - earliest while indirect re-entry 现仅恢复 lexical scopes，不再错误重放当前 while 前缀。
- 已新增 / 更新回归：
  - 新增 run-pass：
    - `tests/fixtures/run-pass/effect_resume_mixed_escape_post_immediate_while_direct_indirect_separate_stmt.scoop`
    - `tests/fixtures/run-pass/effect_resume_mixed_escape_post_immediate_while_indirect_direct_separate_stmt.scoop`
  - 删除 build-fail：
    - `tests/fixtures/build/effect_resume_mixed_escape_while_direct_indirect_separate_stmt_is_error.scoop`
  - 更新既有 golden：
    - `tests/fixtures/run-pass/effect_resume_mixed_escape_pre_immediate_while_indirect_direct.stdout`
- 最终验证结果：
  - `cargo test --all` 通过。
  - `cargo run -p scoop -- test` 通过（`fixtures: ok (997)`）。
  - `cargo run -p scoop --features llvm -- test` 通过（`fixtures: ok (997)`）。
  - `cargo clippy --workspace --all-targets -- -D warnings` 通过。
