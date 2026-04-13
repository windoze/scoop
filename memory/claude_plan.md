# 执行计划与进度记录

## 说明

按要求，本文件在执行任何命令前先建立，用于记录本次迭代的高层执行思路、分步计划、关键决策与进度更新。

注意：这里记录的是可审计的任务分析、执行计划与进展摘要，不包含内部详细推理展开。

## 初始目标

本轮只完成 `TODO.md` 中第一个未完成任务，然后停止。

在开始该任务前，先完成以下前置检查：

1. 检查最新一次 git 提交，确认是否提到了已知遗留问题；若有，则这些问题优先纳入本轮范围并先修复。
2. 读取 `TODO.md`，识别第一个未完成任务。
3. 读取 `PLAN.md`，核对当前计划与任务依赖关系。
4. 如首个未完成任务过大，则先拆分任务，并同步更新 `PLAN.md` 与 `TODO.md`。

## 预期执行步骤

1. 查看最新提交信息与改动摘要。
2. 读取 `TODO.md` / `PLAN.md` / 必要的仓库说明文件。
3. 定位首个未完成任务及其相关代码、测试与规范位置。
4. 判断是否存在阻塞该任务的规范缺口、实现缺口或历史问题。
5. 若无阻塞，直接实现该任务。
6. 运行相关格式化、测试与 lint，至少包括与改动相关的最小充分测试；若改动范围较大，再运行更完整验证。
7. 更新 `TODO.md`、`PLAN.md` 与本文件中的进度记录。
8. 提交 git commit，提交信息与任务对应。
9. 停止，不继续处理后续任务。

## 风险与决策原则

1. 不以 workaround、兼容性分支或仅夹具修补的方式伪造完成。
2. 若发现规范与实现不一致，且该问题阻塞当前任务，必须先在 `TODO.md` 中显式建模为前置任务，再更新计划并停止。
3. 不回退用户已有改动；若工作树脏且与当前任务无关，则忽略。

## 进度更新

- 已创建本文件。
- 已检查最新提交：`35f0aacfe874713e008e9ab79162b728fab9bbfc`，提交信息仅为 `[T2003u4b2] Route single-arm escape-continuation through unified plan`，未在提交信息中显式提到需要先修复的遗留问题。
- 已读取 `TODO.md` / `PLAN.md`，确认首个未完成任务原为 `T2003u4c`。
- 已判断 `T2003u4c` 范围过大，决定拆分为：
  1. `T2003u4c1`：multiple immediate-resume / sibling non-resuming 路由切到统一状态机输入。
  2. `T2003u4c2`：multiple escape-continuation / sibling non-resuming 路由切到统一状态机输入。
  3. `T2003u4c3`：immediate+escape / site-matrix 路由切到统一状态机输入。
- 当前执行目标：`T2003u4c1`。
- `T2003u4c1` 已实现完成：
  1. 新增 `resolve_top_level_immediate_resume_sites_from_plan`，按 unified plan 的 suspend-site/source-path 恢复 multiple immediate 的 top-level direct sites。
  2. `codegen_handle_expr_multiple_immediate_resume_top_level` 改为从 unified plan 解析 site，不再手工扫描 handle body。
  3. `codegen_handle_expr_immediate_resume_with_nonresuming_siblings` 改为复用 unified plan 的 single-site resolver。
  4. 新增单测 `resolve_top_level_immediate_resume_sites_from_plan_keeps_source_order`，验证 site 顺序按 source order 返回、且与 arm 顺序解耦。
- 验证结果：
  - `cargo test --all`：通过
  - `cargo run -p scoop --features llvm -- test`：通过（`fixtures: ok (993)`）
  - `cargo clippy --workspace --all-targets -- -D warnings`：通过
- 下一步（本轮不执行）：`T2003u4c2`
