# 执行计划与进度记录

## 约束说明

- 按要求先记录可公开的执行计划与关键决策，再开始仓库检查与实现工作。
- 这里记录的是面向协作的计划、假设、发现和进度更新，不包含逐字内部思维过程。
- 本轮目标：只完成 `TODO.md` 中第一个未完成任务，然后提交并停止。

## 初始执行计划

1. 检查最新一次 Git 提交，确认提交信息或改动中是否明确提到仍未修复的既有问题。
2. 阅读 `TODO.md`，定位第一个未完成任务。
3. 阅读 `PLAN.md`，确认当前计划与任务是否一致。
4. 如果首个未完成任务过大，先拆分为更小子任务，并同步更新 `PLAN.md` 与 `TODO.md`。
5. 实现当前要处理的首个任务或子任务。
6. 运行相关测试、格式化、`clippy`，修复发现的问题，直到相关检查通过。
7. 更新文档与任务状态：修改 `TODO.md`、`PLAN.md`，必要时补充 `README.md` 或代码注释。
8. 提交本轮变更，提交信息清晰描述本轮完成内容，然后停止。

## 进度

- 已创建本文件并写入初始计划。
- 已检查最新提交：`579eff66002c280ec266e5bf24f177d322aa60c6 [T2003c0c2c] Support multiple immediate-resume arms`。提交标题未显式留下“仍未修复的问题”说明。
- 已读取 `TODO.md` / `PLAN.md`，确认首个未完成任务是 `T2003c0c2d`。

## 现状判断

- `T2003c0c2d` 当前范围同时覆盖：
  1. 无 `immediate-resume` 时的 multiple escape-continuation arms。
  2. `immediate-resume` / multiple-immediate 与 multiple escape-continuation 的 richer mixed-arm。
- 代码入口也印证这是两条不同主线：
  - `crates/scoopc/src/llvm/codegen/effect/nonresuming.rs` 的 `codegen_handle_expr_multi_arm` 仍对 multiple escape arms 直接报 `handle mixed escape-continuation arms (only 1 supported)`。
  - 无 immediate 的 escape 路径走 `mixed.rs` 中的 `codegen_handle_expr_escape_with_nonresuming_siblings*`。
  - immediate + escape 路径走 `mixed.rs` 中的 `codegen_handle_expr_immediate_resume_with_escape_sibling*` 与 `codegen_handle_expr_multiple_immediate_resume_top_level`。
- 判断：该任务需要先拆分，否则本轮会把两套状态机与站点扫描规则耦合到一次提交里，风险过高，不符合当前仓库既有的任务拆分粒度。

## 更新后的执行计划

1. 先把 `T2003c0c2d` 拆成更小子任务，并同步更新 `TODO.md` / `PLAN.md`。
2. 执行拆分后的第一个子任务，优先选择最小且真实可运行子集。
3. 为该子任务补充或调整 fixtures。
4. 跑格式化、测试、LLVM fixture、`clippy`。
5. 回写 `TODO.md` / `PLAN.md` / 本文件，提交并停止。

## 已完成的计划调整

- 已把 `T2003c0c2d` 拆成 `T2003c0c2d1`～`T2003c0c2d4`，并更新 `TODO.md` 与 `PLAN.md`。
- 当前执行目标已变为 `T2003c0c2d1`：
  - 范围：无 `immediate-resume`、无 sibling non-resuming、无 `finally` cleanup、纯 escape-only、top-level `val = perform` direct single-site 的多个 escape-continuation arms。
  - 本轮先实现这个最小子集，再用新回归锁住剩余 richer 组合边界。

## 本轮实现结果

- 已新增 `multiple escape-continuation arms` 的 pure-direct lowering，并接入 multi-arm 入口：
  - 仅在无 `immediate-resume`、无 sibling non-resuming、无 `finally` 时启用。
  - 支持多个 top-level direct site 按 body 顺序依次命中不同 escape arm。
  - 支持不同 site 各自按真实恢复值类型 decode `resume(...)` payload。
- 已新增回归：
  - run-pass：`effect_multi_escape_multi_arm_top_level_direct`
  - build：`effect_multi_escape_multi_arm_with_nonresuming_is_error`
- 已完成验证：
  - `cargo fmt --all --check`
  - `cargo test --all`
  - `cargo run -p scoop -- test`
  - `cargo run -p scoop --features llvm -- test`
  - `cargo clippy --workspace --all-targets -- -D warnings`
