# 本轮执行计划

## 约束说明

- 按要求先记录计划，再执行任何仓库检查命令。
- 本轮只处理 `TODO.md` 中第一个未完成任务；完成后测试、更新文档、提交并停止。
- 若遇到阻塞、规格不匹配或前置缺失，不做绕过实现；改为更新 `TODO.md` / `PLAN.md` 反映依赖关系，然后提交并停止。

## 初始执行步骤

1. 查看最新一次提交，确认是否提到需要先修复的既有问题。
2. 阅读 `TODO.md`，定位第一个未完成任务。
3. 阅读 `PLAN.md`，核对现有计划与任务依赖。
4. 评估该任务是否足够小可直接完成：
   - 若可直接完成，进入实现。
   - 若过大，先拆分为更小子任务，并更新 `TODO.md` / `PLAN.md`，然后执行新的第一个子任务。
5. 实现任务并补充或调整测试。
6. 运行必要验证，至少覆盖：
   - 相关测试
   - `cargo fmt --check`（必要时先 `cargo fmt`）
   - `cargo clippy --all-targets -- -D warnings`
7. 更新 `TODO.md`、`PLAN.md` 和本文件，记录完成情况或阻塞原因。
8. 使用清晰提交信息提交本轮更改。
9. 停止，不继续处理下一个任务。

## 进度记录规则

- 每完成一个关键阶段，就在本文件追加“进度更新”小节，记录：
  - 已确认的信息
  - 当前执行中的步骤
  - 计划是否发生变化
- 若任务被拆分或重排，明确写出原因、依赖和新的执行顺序。

## 进度更新 2026-04-13 02:16

- 已查看最新提交：`eeff7d9 [T2003c0c2b3d4] Support while-body mixed direct/indirect escape dispatch`。
- 最新提交的 commit message 未显式声明新的既有问题或“必须先修”的遗留缺陷；目前没有发现需先于 TODO 主线处理的额外提交备注问题。
- 已定位 `TODO.md` 中首个未完成任务：`T2003c0c2c`，内容为“LLVM 多 arm handle dispatch（multiple immediate-resume arms）”。
- 当前计划调整为：
  1. 阅读 `T2003c0c2c` 相关 lowering、诊断与现有 fixtures，确认当前实现边界。
  2. 判断任务是否足够小可直接完成；若否，先拆分并更新 `TODO.md` / `PLAN.md`。
  3. 若可直接完成，则实现 multiple immediate-resume arms，补回归并运行全量验证。

## 进度更新 2026-04-13 02:33

- 已完成 `T2003c0c2c` 的实现边界审计：
  - `codegen_handle_expr_multi_arm` 仍把多个 immediate-resume arm 直接拒绝。
  - 现有 `immediate_resume.rs` / `mixed.rs` 路径都默认“单个 immediate arm + 单个 perform site”。
  - 当前任务不必继续拆分；可以在本轮直接做一个受控实现。
- 确定本轮实现范围：
  - 支持“多个 immediate-resume arm + 可选 sibling non-resuming”的 **top-level 直连 `val x = perform`** 子集。
  - 继续保持 richer 组合（例如 multiple immediate + escape-continuation、nested/while/if 内的 multiple immediate site）为稳定诊断，不在本轮绕过实现。
- 接下来执行步骤：
  1. 在 LLVM lowering 中新增 multiple-immediate top-level state machine。
  2. 更新 multi-arm 入口分流到新 lowering。
  3. 新增 run-pass fixtures：纯 multiple immediate，以及 multiple immediate + sibling non-resuming。
  4. 跑格式化、测试、LLVM fixture、clippy。

## 进度更新 2026-04-13 02:44

- 已完成实现：
  - `codegen_handle_expr_multi_arm` 现已把“多个 immediate-resume arms + 可选 sibling non-resuming”分流到新的 LLVM lowering。
  - 新 lowering 当前支持 **top-level `val = perform`** 的多个 immediate site；会按源码 site 顺序续跑 resumed tail，并按 op tag 选择对应 arm。
  - `multiple immediate + escape-continuation` 暂未混入，继续保留稳定诊断，交给下一任务 `T2003c0c2d`。
- 已新增回归：
  - `tests/fixtures/run-pass/effect_resume_multi_immediate_top_level.scoop`
  - `tests/fixtures/run-pass/effect_resume_multi_immediate_custom_nonresuming.scoop`
- 已完成验证：
  - `cargo check -p scoopc`
  - `cargo fmt --all --check`
  - `cargo test --all`
  - `cargo run -p scoop -- test`
  - `cargo run -p scoop --features llvm -- test`
  - `cargo clippy --workspace --all-targets -- -D warnings`
- 下一步文档动作：
  1. 更新 `TODO.md` / `PLAN.md` 标记 `T2003c0c2c` 完成并推进下一任务。
  2. 检查 diff 后提交本轮改动并停止。

## 进度更新 2026-04-13 02:49

- `TODO.md` / `PLAN.md` 已更新：
  - `T2003c0c2c` 已标记完成，并补充完成说明。
  - `PLAN.md` 的当前下一步已推进到 `T2003c0c2d`。
- diff 自检已完成：
  - `git diff --check` 通过。
  - 当前改动仅包含本轮代码、fixture、计划记录文件。
- 当前状态：准备提交本轮改动并停止。
