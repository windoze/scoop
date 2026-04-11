# 本轮执行计划

## 说明

按要求在动手前先记录计划。这里保留可审阅的执行摘要、步骤和关键决策，不写入不适合外露的原始内部思维链。

## 目标

本轮只完成 `TODO.md` 中第一个未完成任务；如果发现前置问题、规格不匹配或任务过大，则先调整 `TODO.md` / `PLAN.md`，提交后停止。

## 预定步骤

1. 检查最新一次 Git 提交，确认提交说明中是否提到遗留问题或已知缺陷。
2. 如最新提交提到需要先修复的问题，先定位、实现、测试并修复这些问题。
3. 阅读 `TODO.md`，确定第一个未完成任务。
4. 阅读 `PLAN.md`、相关模块和测试，判断该任务是否可以在本轮完整完成。
5. 如果任务过大：
   - 在 `PLAN.md` 中拆分为更小的子任务；
   - 在 `TODO.md` 中按依赖顺序加入子任务；
   - 执行第一个子任务并在完成后停止。
6. 实现当前目标任务，必要时补充或调整测试。
7. 运行相关验证，至少覆盖：
   - 针对改动的测试；
   - 必要的工作区测试；
   - `cargo fmt`；
   - `cargo clippy --all-targets -- -D warnings`（如果时间与改动范围允许则完整执行；若受阻，会在此文件说明原因）。
8. 更新 `TODO.md`、`PLAN.md` 和本文件，记录完成情况或阻塞原因。
9. 提交 Git commit，提交信息与任务编号或实际修复内容对应。
10. 停止，不继续下一个任务。

## 关键检查点

- 不接受变通方案、夹层兼容或仅为 fixture 通过而做的规避。
- 若遇到规范缺口或实现边界，必须先把问题显式写入 `TODO.md` 并调整依赖顺序。
- 不回退与当前任务无关的用户改动。

## 执行日志

- 已创建本文件，待开始仓库检查。
- 已检查最新提交 `dca01f5b2c73ffee7f4a39340d2d8c9b59334951`，提交说明未额外声明需要先修的遗留问题。
- 已读取 `TODO.md` / `PLAN.md`，当前第一个未完成任务是 `T2003c0b2c3b`：mixed-arm sibling escape-continuation 在 `if` branch 中的 indirect call site。
- 已完成范围审计：当前任务可以直接实现，不需要再拆子任务。
- 当前实现缺口已定位为三部分：
  1. `scan_mixed_escape_indirect_sites` 仍把 `if` branch 中的 indirect call site 统一报错。
  2. mixed-arm site matrix 只为 `if` 中的 direct site 建了分派路径，缺少 `if` indirect 的分类与执行入口。
  3. indirect continuation step 仅支持 nested block tail replay，缺少 if branch tail replay / after-if 合流。
- 下一步实现计划：
  1. 扩展 indirect site 扫描与校验，允许 `if` then/else branch 中的 statement-position val-bound indirect site。
  2. 为 mixed-arm site matrix 增加 `if` indirect 的分类与 lowering，覆盖初次执行、resume step、以及 immediate site 前后两侧路径。
  3. 补 if-indirect run-pass fixtures（pre/post immediate）与 while-nested-indirect build-fail fixture。
  4. 运行 `cargo fmt --all`、相关测试、`cargo clippy --workspace --all-targets -- -D warnings`。
  5. 更新 `TODO.md` / `PLAN.md` / 本文件并提交。
- 已完成代码实现：
  - indirect scanner 现已接受 if then/else branch 中的 statement-position val-bound indirect call site；
  - mixed-arm site matrix 新增 if-indirect 分类与 lowering；
  - continuation step / state0 / state1 现已共享 if branch prefix / tail replay helper。
- 已完成回归补充：
  - 新增 run-pass：`effect_resume_mixed_escape_pre_immediate_if_indirect`、`effect_resume_mixed_escape_post_immediate_if_indirect`。
  - 旧的 if 负例已替换为 while 边界负例：`effect_resume_mixed_escape_while_indirect_is_error`。
- 已执行验证：
  - `cargo fmt --all`
  - `cargo test --all`
  - `cargo run -p scoop -- test`
  - `cargo run -p scoop --features llvm -- test`
  - `cargo clippy --workspace --all-targets -- -D warnings`
- 已更新 `TODO.md` / `PLAN.md`：`T2003c0b2c3b` 标记为完成，下一步切换为 `T2003c0b2c3c`。
