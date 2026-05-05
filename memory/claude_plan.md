# 当前执行计划

## 范围
- 本次只处理 `TODO.md` 索引中对应的第一个未完成详细任务。
- 完成该任务后更新任务记录、运行相关验证、提交 Git，然后停止。

## 步骤
1. 读取 `TODO.md`，按索引顺序定位需要检查的详细任务文件。
2. 读取对应的 `TODO-Px.md` 文件，按详细文件中的顺序找出第一个标题未标记 `[DONE]` 的任务。
3. 检查最新提交是否明确提到与该任务直接相关的未完成问题；如有，将其纳入当前任务或作为前置任务记录。
4. 阅读当前任务的要求、依赖、验证方式和完成记录，确认是否可以直接实现。
5. 如果发现必须先修复的具体阻塞项，则在对应 `TODO-Px.md` 中插入最小前置任务，同步 `TODO.md`，提交并停止。
6. 若无阻塞，按任务要求进行最小正确实现，避免绕过规格或弱化测试。
7. 运行与改动相关的测试；必要时运行更广泛的验证，修复由当前任务引入或暴露且阻塞当前任务的问题。
8. 将详细任务标题加上 `[DONE]`，更新完成记录；如索引项存在，同步 `TODO.md` 的 `[DONE]` 状态。
9. 更新本文件记录关键进展与验证结果。
10. 审查工作区变更，提交所有本次任务相关改动，并在提交后停止。

## 当前状态
- 已读取 `TODO.md` 与 `TODO-P7.md`，第一个未完成详细任务是 `P7-T03`：在默认 refactor 主线下运行标准 full regression，并修复所有默认路径回归。
- 最新提交为 `[P7-T02U] Fix async task resume payload ABI`，它是 `P7-T03` 上次记录的直接阻塞项修复；本轮继续完成 `P7-T03`，不切换到后续任务。
- 已运行 `cargo test --all`，首轮失败：`scoopc --lib` 中 3 个 effect-lowered 单测缺少 `tests/fixtures/effect_lowered_src/*.scoop` fixture，6 个 HIR lowering golden 与当前输出不一致。
- 已修正测试入口：effect-lowered 单测改为读取正式 `effect_lowered` phase fixture；HIR golden 单测改为通过 refactor typed HIR dump helper 生成当前默认路径输出，和 fixture harness 保持一致。
- 定向验证通过：`cargo test -p scoopc --lib effect_lowered && cargo test -p scoopc --lib hir_fixture`。
- 已运行 `cargo fmt --all`，并重跑 `cargo test --all` 通过。
- 已运行 `cargo run -p scoop -- test`，当前失败在 `tests/fixtures/run-pass/async_await_string_basic.scoop`，默认 run-pass 退出码为 1。
- 已定位并修复 `async_await_string_basic.scoop`：resume packing method 现在消费 authoritative continuation method reachability；unreachable packing method 只生成 unreachable shell，不再用 owner body 构造错误的 Step completion payload。
- 已单独运行 `cargo run -p scoop -- run tests/fixtures/run-pass/async_await_string_basic.scoop`，输出 `body/awaited/hello`。
- `async_await_string_basic.scoop` fixture harness 已通过；完整 `cargo run -p scoop -- test` 继续失败在 `tests/fixtures/run-pass/async_fun_task_runtime_basic.scoop`。
- 已修复 `async_fun_task_runtime_basic.scoop`：async fun body 的显式 tail `return value` 现在在 task step closure 内规范化为 ready-value tail，非 tail return 规范化为 `return __task_step_ready(value)`，避免以原始 `Int` 从 `__TaskStepResult<Int>` closure 返回。
- 已单独运行 `cargo run -p scoop -- run tests/fixtures/run-pass/async_fun_task_runtime_basic.scoop`，输出 `base/fetch/40/done/42`。
- 相关定向验证通过：`cargo test -p scoopc --lib lower_typed_single_source_file_routes_async_step_payload_through_transport_carrier` 与 `async_fun_task_runtime_basic.scoop` fixture harness。
- 完整 `cargo run -p scoop -- test` 继续失败在 `tests/fixtures/run-pass/bool_to_string_print_basic.scoop`，退出码为 1。
- 已修复 `bool_to_string_print_basic.scoop`：effect-facts surface contract 现在能处理 flattened extension direct-call 形状，value primitive 支持 direct `scoop.core.toString(...)` 与 plain source-slice `String.concat` callable carrier。
- 已单独运行 `cargo run -p scoop -- run tests/fixtures/run-pass/bool_to_string_print_basic.scoop`，输出与 golden 对齐。
- `bool_to_string_print_basic.scoop` fixture harness 已通过；完整 `cargo run -p scoop -- test` 继续失败在 `tests/fixtures/run-pass/callable_value_pattern_binder_receiver_named_args_basic.scoop`。
- 已单独定位 `callable_value_pattern_binder_receiver_named_args_basic.scoop`：它需要同时闭合 receiver function value、top-level callable value direct call、pattern binder function value、top-level `FunPtr` direct call、以及 GC-sensitive string receiver arg 求值期间的 callable carrier rooting。当前可执行程序仍会挂起，阻塞 P7-T03 full regression。
- 已按 roadblock 规则新增前置任务 `P7-T02V`，并将 `P7-T03` 依赖改为 `P7-T02V`；`TODO.md` 已同步插入索引项。
- 下一步审查工作区，提交本次阻塞记录与已完成的局部修复，然后停止。
