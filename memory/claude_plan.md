# Claude Plan

## 执行约束
- 先以 `TODO.md` 为唯一任务真源，定位第一个未完成任务。
- 在未确认当前任务前，不做开放式问题排查。
- 如遇到阻塞当前任务的真实缺陷或缺失能力，不绕过；最小化更新 `TODO.md` / 必要时更新 `PLAN.md`，提交后停止。

## 初始计划
1. 读取 `TODO.md`，定位第一个标题未带 `[DONE]` 的任务。
2. 检查最近提交是否直接提到与该任务相关且未完成的问题；若是，则将其视为任务一部分或在 `TODO.md` 中补成前置任务。
3. 阅读当前任务涉及的代码、测试、规范和依赖约束，确认实现边界。
4. 实现当前任务所需改动，只做满足任务要求的最小正确修改。
5. 运行任务要求的验证、相关测试，以及必要的 `cargo fmt` / `cargo clippy --all-targets -- -D warnings`。
6. 更新 `memory/claude_plan.md` 记录进展；将任务在 `TODO.md` 中标记为 `[DONE]` 并补全完成记录；仅在阶段计划变化时更新 `PLAN.md`。
7. 按仓库约定创建一次 git 提交，然后停止，不继续下一个任务。

## 当前任务
- 已定位第一个未完成任务：`CG-T07S0`。
- 任务目标：修复 receiver callable value / `FunPtr` 在 receiver + named args 组合下的 lowering 槽位映射回归，并恢复默认 full-suite 继续前进。
- 最近提交 `[CG-T07S0a] Complete effect-handle top-level val blocker` 只完成了已记录前置任务 `CG-T07S0a`，未引入新的同任务未完成条目。

## 当前进展
- 已读取 `CG-T07S0` / `CG-T07S` 条目，确认当前 invocation 只处理 `CG-T07S0`。
- 已定位根因：最新相关提交 `12185b94` 删除了任务要求的 run-pass fixture，并把 function value / `FunPtr` direct call 的 named args 改成 typecheck 直接拒绝；这与 `CG-T07S0` 目标冲突。
- 已确认历史正确行为：旧实现通过 synthetic param names `receiver` / `a0` / `a1` 做 callable value / `FunPtr` named-arg 绑定，且存在配套 run-pass fixture `callable_value_pattern_binder_receiver_named_args_basic.scoop`。
- 已恢复 callable value / `FunPtr` direct-call named-arg typecheck 绑定，补回正向 run-pass fixture，并删除与当前任务目标冲突的两个 typecheck 负例。
- 已修复 HIR generic call fallback：当 callable surface 有 authoritative `arg_binding` 但没有 top-level/ctor `plan` 时，保留 `CallArg::Named` 交给 MIR canonicalize，避免 `when` binder / 顶层 `FunPtr` 重新退回源码顺序 positional 实参。
- 已新增 `callable_value_and_top_level_funptr_named_args_keep_binding_order_in_mir` 单测，锁定 `FunValue` 与顶层 `FunPtr` 的 receiver-first MIR 实参顺序。
- 验证完成：定向单测、单 fixture build/test、默认 `cargo run -p scoop -- test`（`fixtures: ok (1270)`）与 `cargo clippy --all-targets -- -D warnings` 全部通过。
- 下一步：检查工作树，提交本次 `CG-T07S0` 改动并停止。

## 说明
- 这里记录的是可审阅的执行计划与进展摘要，不包含私有推理细节。
