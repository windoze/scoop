# 执行计划

说明：这里记录的是可公开的执行计划、决策摘要与进度更新，不包含私有推理细节。

## 初始计划

1. 查看最新一次 Git 提交，确认提交说明中是否提到尚未处理的问题；若存在，先修复这些既有问题。
2. 阅读 `TODO.md`，定位第一个未完成任务。
3. 判断该任务是否过大：
   - 如果可直接完成，则进入实现阶段。
   - 如果过大，则先拆分任务，并同步更新 `PLAN.md` 与 `TODO.md`，本次只执行拆分后的第一个子任务。
4. 阅读与当前任务相关的代码、测试、规范和文档，确认实现边界与依赖。
5. 实现当前任务，必要时补充或调整测试。
6. 运行相关验证：
   - 最小相关测试集
   - 如有必要，运行更广泛的回归测试
   - `cargo fmt`
   - `cargo clippy --all-targets -- -D warnings`
7. 更新项目文档与计划文件：
   - 在 `TODO.md` 中标记当前任务完成，或在受阻时重排任务依赖
   - 在 `PLAN.md` 中记录当前状态、依赖变化与后续顺序
   - 持续更新本文件记录关键进展
8. 使用清晰的 Git 提交信息提交本次改动。
9. 停止，不继续处理下一个任务。

## 进度日志

- 已创建本计划文件，准备检查最新提交与任务列表。
- 已检查最新提交：`1b008ba8605909085a6ad2d8887c715a699b26bb`，提交标题为 `[T3010b2b0R] Review non-resuming callee propagation`。提交说明未直接新增独立 issue；其内容主要是 review 与计划推进。
- 已检查 `TODO.md` / `PLAN.md`：
  - 当前第一个未完成任务是 `T3010b2b1`：修正 handle arm body 内 non-resuming effect 的外传 / self-inactive / finally cleanup 语义。
  - 目前未发现需要再前插的新前置任务；先按 `T3010b2b1` 执行。

## 当前任务：T3010b2b1

### 已知目标

1. 复现并确认 arm body 中 non-resuming effect 的错误行为：
   - arm body 里的 `Raise.raise(...)` 后仍继续执行 `arm_unreachable`
   - sibling arm 错误自捕获
   - `finally` 在向外传播前没有恰好执行一次
2. 读取相关实现，重点检查：
   - unified state machine emitter 中 arm body / cleanup / outward propagation 路径
   - effect handler active/inactive 状态切换
   - handle dispatch loop 与 arm 执行后的控制流
3. 如果确认任务边界仍过大，再细化拆分并更新 `PLAN.md` / `TODO.md`；否则直接实现。
4. 实现后运行最小定向验证，再补充全量相关测试与 lint。
5. 更新 `TODO.md` / `PLAN.md` / 本文件并提交。

### 当前判断的根因

1. `emit_execute_arm_body` 目前把整个 arm body 当作普通表达式直接交给 `codegen_expr_in_expected_context`，但 step function 生成时显式清空了 `current_fun_return_ty` / `return_context`，导致 arm body 内的 `Raise.raise(...)` 只会写 TLS active，不会像普通函数 frame 那样立刻终止当前 step function；于是 `arm_unreachable` 继续执行。
2. `codegen_handle_expr_via_state_machine` 的 dispatch loop 在进入 arm 前只调用了 `scoop_effect_clear_active()`，没有把当前 handler frame 置为 inactive；因此 arm body 再次触发 effect 时，当前 handle 仍会把它当作自己可处理的 active 返回重新 dispatch，出现 sibling arm 自捕获。
3. 当前 dispatch loop 对“当前 handle 不该继续接住的 active”没有 outward propagation 路径：`dispatch_unmatched` 直接走 `handle_done`，而 `handle_done` 会 `scoop_effect_clear()`，导致原本应该向外传播的 effect 被吞掉，`finally` 也被绕过。

### 计划中的实现

1. 给 runtime ABI / symbol 补上 `scoop_effect_handler_stack_set_active` 声明。
2. 在 `emit_execute_arm_body` 内部，包一层仅针对 arm body 的“ordinary effect propagation”上下文，让 arm body 中的 non-resuming perform/call 在 active 时直接从 step function 返回，阻止继续执行 arm body 后续语句。
3. 重构 `codegen_handle_expr_via_state_machine` 的 dispatch/arm 路径：
   - arm 执行前把当前 handler frame 设为 inactive；
   - arm 正常完成后恢复 active 并回到 `dispatch_check`；
   - arm 执行期若返回 active，则先走当前 handle 的 cleanup/`finally`，然后弹出 handler frame，保留 TLS active + perform slot，改走 outward propagation。
4. 让 handle 的 unmatched/propagation 路径不再清空 TLS active；在 ordinary frame 中沿用现有 helper 继续向 caller 传播，在 state-machine/nested-handle 场景下返回 default value 但保留 active，供外层边界继续处理。
5. 跑三个定向 fixture、相关单测、`cargo test --all`、`cargo clippy --all-targets -- -D warnings`，确认没有新回归。

## 当前结果

### 已完成实现（对应新拆分子任务 `T3010b2b1a`）

1. 已修复 arm body direct non-resuming effect 在 unified state-machine 中继续执行后续语句的问题：
   - arm body 内 direct `Raise.raise(...)` / indirect helper call 触发 active 后会立刻结束当前 step function；
   - sibling arm 不再自捕获 arm body 内再次触发的 effect。
2. 已把 handle-level cleanup/`finally` 接回 arm return 与 outward propagation 出口，并用 frame `cleanup_flag` 防止重复执行。
3. 已修复 cleanup 重入 step function 时覆盖已有 handle result 的问题；finally 运行后仍保留正确 result。
4. 已回收并验证以下同根因 fixture：
   - `effect_resume_finally_arm_raise.scoop`
   - `effect_escape_continuation_finally_arm_raise.scoop`
   - `effect_multi_nonresuming_raise_custom_finally.scoop`
   - `effect_escape_continuation_finally_no_perform.scoop`
   - `effect_escape_continuation_zero_perform_returns_body.scoop`
   - `effect_no_perform_handle_elim_basic.scoop`
5. 已验证：
   - `cargo test --all`
   - `cargo clippy --all-targets -- -D warnings`

### 新发现的前置阻塞

1. 继续执行全量 `cargo run -p scoop --features llvm -- test` 后，新的首个真实失败点为：
   - `tests/fixtures/run-pass/effect_escape_continuation_nested_arm_indirect_performs_outer.scoop`
2. 定向复现结果：
   - 当前 unified path 在 inner escape-cont arm 中的间接调用结果整形上报 `暂不支持的 main 代码生成节点：value coercion`
3. 结论：
   - 这属于 broader expected-context / coercion 范围里的一个最小前置缺口，但它直接阻塞当前 `T3010b2b1` 链路；
   - 因此原 `T3010b2b1` 已继续细化为：`T3010b2b1a`（已完成）→ `T3010b2b1b`（下一步：前移修复这条 nested arm indirect path 的 unified value coercion / expected-context）→ `T3010b2b1`（随后回到剩余 nested/indirect outward propagation 验收）；
   - 当前不应在本轮里直接展开更广的 `T3012`，而应更新 `TODO.md` / `PLAN.md` 反映新的子任务顺序后提交并停止。
