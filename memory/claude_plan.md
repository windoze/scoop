## 当前分析摘要

- 本次目标：按 `TODO.md` 顺序只完成第一个未完成任务，然后停止。
- 最新提交 `9055245e3e472477a53eec9b729d21202db19ac2` 的提交说明是“`[T3017] Reorder xfail cleanup behind new blockers`”；提交信息本身没有单独新增“必须先修”的未跟踪遗留问题，当前应按已重排后的 `TODO.md` 顺序处理。
- 已确认 `TODO.md` 的首个未完成任务是 `T3016a`：修正 escaped continuation 完成态的 cleanup/finally replay 与 no-suspend handle result 回归。
- 约束：如果当前任务过大，需要拆分，并同步更新 `PLAN.md` 与 `TODO.md`。
- 约束：实现后必须补测试、更新文档状态、提交 git commit，然后停止，不继续做下一个任务。
- 约束：如果遇到规范缺口、实现边界或任何不能按规范完成的问题，不能绕过，必须先把阻塞问题写入 `TODO.md`/`PLAN.md`，提交后停止。

## 初始执行计划

1. 检查最新一次 git 提交信息，确认是否提到需要先修复的既有问题。
2. 阅读 `TODO.md`，定位第一个未完成任务。
3. 评估 `T3016a` 的复杂度：
   - 若可直接完成，则进入实现。
   - 若过大，则先拆分任务，并更新 `PLAN.md` / `TODO.md`。
4. 阅读 `T3016a` 相关代码、失败 fixture、现有 cleanup/completion 路径与已有 `completion_tag` 设计，建立实现上下文。
5. 复现 `T3016a` 提到的回归，确认是 cleanup/finally replay、terminal completion 恢复还是 handle result 槽位被冲掉。
6. 在统一 state-machine / dispatch / cleanup 生产路径中完成修复，避免引入按源码形状分流。
7. 运行定向 fixture、相关单测、全量测试与 lint；若失败则继续修复再重测。
8. 更新 `TODO.md` 和 `PLAN.md`，记录 `T3016a` 的完成状态与验证结果；若发现阻塞则按依赖顺序重排。
9. 提交本次修改，提交信息对应 `T3016a`。
10. 停止，不继续处理后续任务。

## 进度记录

- 已完成：创建计划文件并记录初始执行计划。
- 已完成：检查最新提交，未发现独立于 `TODO.md` 的新增遗留修复项。
- 已完成：定位首个未完成任务为 `T3016a`。
- 已完成：阅读 `T3016a` 相关代码并复现两类回归。
- 当前发现：
  - `effect_escape_continuation_finally_multi_perform.scoop` 现状会在最终 resume 完成后再次打印一次 `finally`，说明 resumed completion 仍会命中 `CleanupEnter -> cleanup entry`，没有识别“cleanup 其实已在 escaped-handle 退出时执行过”。
  - `effect_nosuspend_finally_nested_handle.scoop` 现状输出 `0/0`，怀疑 outer handle 的 dispatch/done 路径在 terminal completion 已成立时仍可能受 TLS active 影响，误走 outward-propagate 或错误完成分支。
  - 两个症状都集中在 `crates/scoopc/src/llvm/codegen/effect/state_machine_emitter.rs` 的 cleanup/done 协议，而不是 fixture 自身。
- 下一步实现：
  1. 在 `CleanupEnter` lowering 中引入“cleanup 已执行则直接跳到 cleanup exit”的分支，避免 escaped continuation 恢复完成时重跑 finally/cleanup。
  2. 在 dispatch loop 的 `dispatch_check` 中优先识别 terminal `state_tag`（`HANDLE_RETURNED` / `FUNCTION_RETURNED`），避免完成态被 TLS active 误判成 outward propagation。
  3. 补 emitter/IR 级回归测试，并跑 `T3016a` 指定的定向 fixture、`cargo test --all`、`cargo clippy --all-targets -- -D warnings`。
- 已完成：修改 `state_machine_emitter.rs` 的 cleanup/done 协议并补 2 条 emitter/IR 级回归测试。
- 已验证：
  - `effect_escape_continuation_finally_multi_perform.scoop` 通过，重复 `finally` 已消失。
  - `effect_resume_mixed_escape_direct_finally.scoop` 通过。
  - `effect_resume_mixed_source_path_matrix.scoop` 通过。
  - `effect_nosuspend_finally_nested_handle.scoop` 通过，输出恢复为 `16/26`。
  - `cargo test --all` 通过。
  - `cargo clippy --all-targets -- -D warnings` 通过。
- 新发现的更基础 blocker：
  - 新增最小复现 `tests/fixtures/run-pass/effect_handle_tail_if_result.scoop` 后确认，`handle { if (flag) { 13 } else { 15 } }` 当前仍输出 `0/0`。
  - 这说明统一 state-machine 对 no-suspend handle 的 tail control-flow merge result transport 仍未闭环；该问题不应被 finally-specific 修复掩盖。
  - 已按规则把该缺口前插为新任务 `T3016a0` → `T3016a0R`，并把 `T3016a` 顺延到其后。
- 当前结论：
  - 本轮不再将 `T3016a` 标记为完成。
  - 本轮输出应以“新增 blocker 任务、更新 `TODO.md` / `PLAN.md`、提交并停止”为结束点。
