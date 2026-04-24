# 本轮执行计划

## 当前任务
- 目标任务：`TODO.md` 中首个未完成任务 `T4017b`。
- 上一轮已完成主要实现与部分定向验证；本轮重点是补全全量验证、确认无前置问题、更新计划/任务状态并提交。

## 已知上下文
- 最新提交 `[T4017a] Align EffectCtx and EffectOutcome docs` 未在提交信息中声明新的待修前置问题。
- `T4017b` 当前实现方向是把“签名 effectful”与“函数体是否真的向外传播 effect”分离，避免 ordinary direct/closure/funptr 调用仅因签名 effectful 就无条件做 TLS active 检查。
- 已完成 `cargo fmt`，并通过 `cargo test -p scoopc` 及若干定向测试。
- 仍需确认 fixture / workspace / clippy 级别验证是否全部通过。

## 执行步骤
1. 检查长跑中的 `cargo run -p scoop -- test` 是否仍在执行；若会话已失效或任务未完成，则重新运行并等待结果。
2. 运行 `cargo test --all`，确认工作区测试全部通过。
3. 运行 `cargo clippy --all-targets -- -D warnings`，确认没有新增 warning 或 lint 问题。
4. 若验证阶段发现任何既有 bug、回归、spec mismatch 或实现边界问题，则立即转为优先修复；若无法当场修复，则按要求调整 `TODO.md` / `PLAN.md` 依赖顺序并停止。
5. 若全部验证通过，则更新：
   - `TODO.md`：将 `T4017b` 标记完成。
   - `PLAN.md`：记录 `T4017b` 已完成，并将后续焦点顺延到下一任务。
   - `memory/claude_plan.md`：补充验证结果、完成情况与提交信息。
6. 检查变更摘要，使用与任务匹配的提交信息提交本轮改动。
7. 提交后停止，不继续执行下一项任务。

## 关键判断标准
- 只有在实现、相关测试、工作区测试、clippy 全部通过后，才能把 `T4017b` 标记为完成。
- 不能通过缩小覆盖面、改夹具形状或保留 workaround 来宣告完成；如遇阻塞，必须把阻塞项前置进 `TODO.md`。

## 当前状态记录
- 计划已写入，下一步开始检查并完成剩余验证。
- `cargo run -p scoop -- test` 已通过，结果为 `fixtures: ok (1169)`。
- `cargo test --all` 已通过；在修复 `clippy` 收尾问题后已再次复验通过。
- `cargo clippy --all-targets -- -D warnings` 首次运行失败：`codegen_funptr_value_call` 因新增 `call_may_suspend` 参数触发 `clippy::too_many_arguments`。
- 已将 callable-value/funptr 调用元数据整理为 `CallableValueCallSpec`，消除新增参数带来的 `too_many_arguments` lint，同时不改变调用语义。
- 最终验证已完成：`cargo run -p scoop -- test`、`cargo test --all`、`cargo clippy --all-targets -- -D warnings` 全部通过。
- `TODO.md`、`PLAN.md` 已更新为：`T4017b` 完成，下一步转入 `T4017c`。
- 下一步只剩检查 diff 并以 `[T4017b] Gate ordinary TLS checks by outward-effect analysis` 提交，然后停止。
