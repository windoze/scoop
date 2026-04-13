# 执行计划

## 约束与目标

- 本次调用只完成 `TODO.md` 中第一个未完成任务，然后停止。
- 在开始任务前，先检查最新提交是否提到预存问题；若有，则这些问题优先纳入本次范围并修复。
- 任何发现的规格不匹配、缺失能力或阻塞项，都必须先反映到 `TODO.md` / `PLAN.md`，不能绕过。
- 实现后必须完成验证、更新文档、提交 Git commit，然后停止。

## 当前计划

1. 检查仓库状态，确认是否存在用户未提交改动，避免误覆盖。
2. 查看最新一次提交信息与变更，判断是否明确提到未解决问题；若提到，则先修复这些问题。
3. 阅读 `TODO.md`，定位第一个未完成任务。
4. 阅读 `PLAN.md` 以及与该任务直接相关的代码/测试，确认任务边界与依赖。
5. 判断该任务是否可以在本次调用内完整完成：
   - 若可以：直接实现。
   - 若不可以：把任务拆分为更小子任务，更新 `PLAN.md` 和 `TODO.md`，并执行拆分后的第一个子任务。
6. 在实现过程中，如果发现前置缺陷或规格缺口：
   - 先记录问题；
   - 更新 `TODO.md` 任务顺序与依赖；
   - 更新 `PLAN.md` 说明原因；
   - 只在必要的前置修复完成后继续，否则提交并停止。
7. 完成实现后运行相关格式化、检查与测试，至少覆盖：
   - `cargo fmt --check`（必要时先 `cargo fmt`）
   - `cargo clippy --all-targets -- -D warnings`
   - 与本次任务相关的测试；若影响面较广，则补充 `cargo test --all`
8. 更新进度文件：
   - 在 `TODO.md` 标记该任务完成，或在阻塞场景下调整顺序与依赖；
   - 在 `PLAN.md` 记录当前状态、执行结果与后续影响；
   - 按需要更新本文件中的计划状态。
9. 检查 `README.md`、注释与代码组织是否需要随本次任务一并调整；如果任务涉及这些方面，则一并完成。
10. 提交本次变更，commit message 使用任务号/主题的清晰描述。
11. 停止，不继续处理下一个任务。

## 执行记录

- 已创建本计划文件，待开始仓库检查。
- 已检查工作区与最新提交：
  - `HEAD` 为 `92c143a Update plan`，提交信息本身未提到需要先修复的预存 issue。
  - 工作区存在未提交改动：`crates/scoopc/src/llvm/codegen/effect/mod.rs`、`shared.rs`、`state_machine_simplify.rs`。这些改动与当前 `T2003r3d2` 方向一致，后续处理时需要保留并在其基础上补完，不可覆盖或回退。
- 已定位第一个未完成任务：`T2003r3d2 Effect：先物理删除所有 shape-based resuming legacy 与旧测试，再继续 unified 重写`。
- 已判断该任务可在本轮直接完成，无需再拆分；本轮不进入 `T2003r3d2a`。
- 当前识别到的直接清理范围：
  - 代码：`nonresuming.rs` 中仍保留的 shape-based multi-resuming route 枚举、match arm、标签与 route mismatch 文案；`multi_escape.rs` 遗留 legacy 文件；`state_machine_plan_tests.rs` 中直接依赖旧 frame / resolver / route label 的测试与 helper。
  - 测试资产：`tests/fixtures` 下以 `effect_resume_multi_immediate_*`、`effect_multi_escape_multi_arm_*`、`effect_resume_mixed_escape_*` 命名的旧 fixture / stdout。
- 下一步：
  1. 清理 effect 代码中的 legacy route 名称与遗留文件，改成更抽象的 unified multi-resuming 占位入口。
  2. 删除依赖这些 legacy 名称或旧 frame 类型的定向单测。
  3. 删除旧 fixture / stdout 资产。
  4. 跑最小验证（以 `cargo build -p scoopc` 与 `cargo clippy --workspace --all-targets -- -D warnings` 为主），随后更新 `TODO.md` / `PLAN.md` 并提交。
- 已完成的关键步骤：
  - 已把 `state_machine_simplify.rs` / `nonresuming.rs` 的 shape-based multi-resuming route 收口为统一 `MultiResuming` 入口；旧 `multi_escape.rs` 已删除。
  - 已删除 `state_machine_plan_tests.rs` 中依赖旧 frame / resolver / route label 的定向单测与 helper。
  - 已删除 `tests/fixtures` 下 93 个旧 shape-based resuming fixture / stdout 文件。
  - 已修复 hard cleanup 暴露出的 build/clippy warnings，当前最小验证结果：
    - `cargo fmt --all` 通过
    - `cargo build -p scoopc` 通过
    - `cargo clippy --workspace --all-targets -- -D warnings` 通过
- 剩余收尾：
  1. 确认 `TODO.md` / `PLAN.md` / 本文件的状态与下一步描述一致。
  2. 检查工作区 diff。
  3. 以 `T2003r3d2` 为主题提交 commit，然后停止。
