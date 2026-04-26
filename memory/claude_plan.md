# 本轮执行计划

## 目标

本轮只完成 `TODO.md` 中第一个未完成任务；但在开始该任务前，必须先检查最新提交是否提到已有问题，并优先修复所有在探查、测试、实现过程中发现的既有问题。

## 约束与执行原则

- 不采用规避方案、夹层修补、仅对夹具生效的特判或任何偏离规格的实现。
- 如果发现当前任务依赖尚未实现或存在错误的前置能力，必须先把该问题整理为更前置的任务，更新 `TODO.md` / `PLAN.md`，提交后停止。
- 在执行过程中持续更新本文件，记录当前判断、关键步骤完成情况、计划变更与阻塞原因。
- 完成一个任务后必须：
  - 更新 `TODO.md`
  - 更新 `PLAN.md`
  - 运行相关验证（至少包括必要测试；如适用还要跑 `cargo fmt`、`cargo test`、`cargo clippy --all-targets -- -D warnings`）
  - 提交 git commit
  - 立即停止

## 初始判断

当前尚未读取仓库状态，因此需要先确认以下信息后才能收敛到具体实现：

1. 最新提交是否提到待修复的已知问题。
2. `TODO.md` 中第一个未完成任务是什么。
3. 当前工作树是否已有未提交改动，避免覆盖用户工作。
4. 目标任务是否足够小，若不够小则需要先拆分并改写 `TODO.md` / `PLAN.md`。

## 详细步骤

1. 查看最新提交信息与变更摘要，确认是否明确提到已有 issue、bug、regression 或 TODO。
2. 阅读 `TODO.md` 与 `PLAN.md`，定位第一个未完成任务，并理解现有计划顺序。
3. 检查工作树状态以及必要的仓库说明文件（如 `README.md`、相关模块说明），避免在脏工作树上误改。
4. 根据目标任务涉及的模块，阅读实现与测试，确认规格、现状和潜在前置问题。
5. 若任务过大：
   - 先拆分为更小子任务；
   - 更新 `TODO.md` 和 `PLAN.md`；
   - 提交一次仅包含任务拆分/重排的提交；
   - 停止。
6. 若存在阻塞当前任务的既有缺陷或缺失能力：
   - 先把该缺陷作为前置任务加入 `TODO.md` 并调整顺序；
   - 更新 `PLAN.md` 解释阻塞关系；
   - 提交后停止。
7. 若任务可直接实施：
   - 修改代码实现；
   - 补充或调整测试；
   - 运行格式化、测试与 lint；
   - 修复验证过程中暴露的既有问题。
8. 任务完成后：
   - 在 `TODO.md` 标记完成；
   - 在 `PLAN.md` 更新状态；
   - 在本文件记录结果与验证命令；
   - 提交一次描述清晰的 commit；
   - 停止。

## 进度记录

- 已创建本计划文件，并完成首轮仓库探查：
  - 最新提交为 `a78312e4 [T5000e1b0a1R] Review member lambda-eff direct-call closure`；
  - 当前工作树只有本文件变更；
  - `TODO.md` 中首个未完成条目为 `T5000e1b0aR Review：确认 extension/member direct-call 已不再在 request binding 阶段丢失 eff_args`。
- 已完成针对当前 review 的首轮代码阅读：
  - `crates/scoopc/src/hir/lower/expr.rs` 已有 `transparent_call_callee(...)`，member / extension direct-call 主路径会把 call-callee 位置的 `TypeApply` 视为透明包装；
  - `crates/scoopc/src/typecheck/expr/call.rs` 的顶层 / member / extension direct-call 单候选与多候选路径均会写回 `TopLevelFunCallBinding`，并把 `eff_args` 同步写入 monomorph 请求；
  - `crates/scoopc/src/typecheck/expr/ops.rs` 的 `collect_member_method_signatures_from_index(...)` 已保留 `eff_param`、`param_*_eff_base` 与 `param_eff_row_var_subst`。
- 在继续 probing 过程中发现新的既有阻塞问题，必须先修：
  - 复现 1：`cargo run -q -p scoop -- dump-mir /tmp/safe_member_plain.scoop` 报 `callee_not_callable: fixtures.safe.forward`；
  - 复现 2：`cargo run -q -p scoop -- dump-mir /tmp/safe_member_type_apply.scoop` 同样失败；
  - 初步根因判断：
    - `crates/scoopc/src/typecheck/expr/call.rs` 里 direct member fun 的 late resolution 只覆盖 `current_lambda_this`，没有覆盖 `safe` 场景下 `member.resolved == None` 的 nullable receiver；
    - `crates/scoopc/src/hir/lower/expr.rs` 的 `ExprKind::Call` lowering 在 safe-call 分支仍匹配原始 `callee.kind`，没有复用已透明化的 `callee_expr`，因此 `TypeApply(SafeMemberAccess(...))` 不会走 `lower_safe_call_expr(...)`。
- 接下来要做的修复：
  1. 扩展 typecheck 的 direct member late resolution，使 safe member call 也能基于 `actual_receiver_ty` 找到 `Owner.method`；
  2. 修正 HIR lowering 的 safe-call 分支，使 `TypeApply` 包裹的 safe member call 仍按 safe-call 主线降糖；
  3. 添加覆盖 safe member direct-call 与 safe member + `TypeApply` 的 typed HIR 回归；
  4. 继续完成 `T5000e1b0aR` 所需 review，并补足 extension/request-binding 覆盖后再更新 `TODO.md` / `PLAN.md` / 提交。
- 上述修复与回归已完成：
  - `crates/scoopc/src/typecheck/expr/call.rs`：direct member late resolution 现同时覆盖 `current_lambda_this` 与 `member.resolved == None`，safe member call 不再因 nullable receiver 丢失 `Owner.method`；
  - `crates/scoopc/src/hir/lower/expr.rs`：safe-call 分支现匹配透明化后的 `callee_expr`，`TypeApply(SafeMemberAccess(...))` 会继续走 `lower_safe_call_expr(...)`；
  - `crates/scoopc/src/hir/lower/mod.rs`：新增 `typed_hir_lowers_safe_member_type_apply_as_safe_direct_call`；
  - `crates/scoopc/src/mir/materialize.rs`：新增 `dump_materialization_inputs_keep_eff_args_for_extension_direct_call_binding`；
  - CLI 复现 `/tmp/safe_member_plain.scoop` 与 `/tmp/safe_member_type_apply.scoop` 的 `dump-mir` 均已恢复通过。
- 本轮验证结果：
  - `cargo fmt --all`
  - `cargo test -p scoopc typed_hir_ -- --nocapture`
  - `cargo test -p scoopc dump_materialization_inputs_keep_eff_args_for_ -- --nocapture`
  - `cargo test -p scoopc monomorph_rewrites_effect_generic_extension_call_to_concrete_instance -- --nocapture`
  - `cargo run -q -p scoop -- dump-mir /tmp/safe_member_plain.scoop`
  - `cargo run -q -p scoop -- dump-mir /tmp/safe_member_type_apply.scoop`
  - `cargo test --all`
  - `cargo clippy --all-targets -- -D warnings`
  - 全部通过。
- 当前收尾状态：
  - `T5000e1b0aR` 可以标记完成；
  - 下一条未完成任务将是 `T5000e1b0b`；
  - 接下来只剩 git 提交并停止。
