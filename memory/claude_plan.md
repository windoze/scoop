# 当前执行计划

说明：按安全与协作要求，这里记录可审计的执行摘要、判断依据与步骤计划，不展开内部逐词推理。

## 本轮目标

本轮需要先审计最新提交是否声明了待补问题，再完成 `TODO.md` 当前第一个未完成任务。经检查，最新提交 `[T2003c0c2a] Support pure non-resuming multi-arm handles` 没有额外声明新的遗留 bug；当前首个未完成项原为 `T2003c0c2b`。

## 当前判断

1. `T2003c0c2b` 不是单一路径门禁。无 immediate-resume 的 escape multi-arm 至少横跨三条 lowering：
   - top-level single direct escape site；
   - top-level single indirect escape site；
   - richer site-matrix（多 site / nested block / if / while / direct+indirect mixed）。
2. 现有实现里，可直接复用的基础能力主要有三类：
   - single-arm escape continuation 的 direct / indirect / nested replay；
   - `T2003c0c1*` 已完成的 escape + sibling non-resuming dispatch / detach / cleanup；
   - `T2003c0c2a` 已完成的无-immediate pure non-resuming multi-arm dispatch。
3. 如果整包推进 `T2003c0c2b`，会在一轮里同时改动 multi-arm 入口分流、continuation step trampoline 与 nested replay，风险过高，因此需要继续拆分。

## 已完成步骤

1. 检查最新提交：未发现提交信息中显式声明必须先修的遗留问题。
2. 阅读 `TODO.md` 与 `PLAN.md`，定位首个未完成项为 `T2003c0c2b`。
3. 审计 `codegen_handle_expr_multi_arm`、`codegen_handle_expr_nonresuming_multi_arm`、single-arm escape lowering，以及 `T2003c0c1*` 的 mixed-arm escape + sibling non-resuming 路径。
4. 已把 `T2003c0c2b` 拆分为 `T2003c0c2b1`～`T2003c0c2b3`，并同步更新 `TODO.md` / `PLAN.md`。
5. 已实现 `T2003c0c2b1`：
   - `codegen_handle_expr_multi_arm` 现已支持“无 immediate-resume + 单个 top-level direct escape site”的 multi-arm 分流；
   - 新增 no-immediate direct lowering，打通一个 escape arm + sibling `Raise.raise` / custom non-resuming；
   - continuation step 会在 `resume(...)` 后继续执行 tail，并共享 sibling dispatch / cleanup。
6. 已新增 run-pass 回归：
   - `effect_multi_escape_raise_direct_single_site`
   - `effect_multi_escape_custom_nonresuming_direct_single_site`
7. 已完成验收：
   - `cargo fmt --all`
   - `cargo test --all`
   - `cargo run -p scoop --features llvm -- test`
   - `cargo clippy --workspace --all-targets -- -D warnings`

## 本轮执行计划

1. 更新 `TODO.md` / `PLAN.md`：
   - 把 `T2003c0c2b` 拆成 `T2003c0c2b1`（single direct site）、`T2003c0c2b2`（single indirect site）、`T2003c0c2b3`（site-matrix）。
   - 让 `T2003c0c2b1` 成为新的首个未完成任务。
2. 实现 `T2003c0c2b1`：
   - 在 `codegen_handle_expr_multi_arm` 为“无 immediate-resume + 单个 escape arm + 0..N sibling non-resuming + top-level direct single-site”增加分流。
   - 复用现有 direct single-site escape lowering 的 captured-handler-stack / one-shot continuation 语义。
   - 让 sibling `Raise.raise` / custom non-resuming 在 escape arm body 与 continuation step 中共享 dispatch / detach / cleanup。
3. 增加最小回归：
   - escape-only、无 immediate-resume、top-level direct single-site；
   - escape + sibling non-resuming、无 immediate-resume、top-level direct single-site。
4. 运行验证：
   - 至少执行 `cargo test --all`、`cargo run -p scoop --features llvm -- test`、`cargo clippy --workspace --all-targets -- -D warnings`；
   - 如有必要补 `cargo fmt --all` 与更小范围命令。
5. 更新 `TODO.md` / `PLAN.md` / 本文件完成记录，提交本轮改动，然后停止。

## 当前状态

- 已完成：`T2003c0c2b1`。
- 下一轮起点：`T2003c0c2b2`。
