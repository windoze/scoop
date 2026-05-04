# 当前执行计划

## 执行原则

- 先读取 `TODO.md` 作为索引，再按索引指向的 `TODO-Px.md` 详细文件确定第一个标题未带 `[DONE]` 的任务。
- 只完成一个详细任务；完成后更新详细 TODO、同步索引、验证、提交，然后停止。
- 如果当前任务被具体实现缺口阻塞，不绕开、不缩小范围；在正确的详细 TODO 文件中插入最小必要前置任务，同步 `TODO.md`，提交后停止。
- `PLAN.md` 只在阶段级计划、依赖或完成标准发生变化时更新。

## 初始步骤

1. 读取 `TODO.md`，获取任务索引和详细文件顺序。
2. 读取相关 `TODO-Px.md` 文件，定位第一个未完成详细任务。
3. 检查最近提交是否明确提到与该任务直接相关的未完成问题；若相关，将其纳入当前任务或作为前置任务记录。
4. 阅读该任务的要求、约束、验证方式和完成记录。

## 实施步骤

1. 根据任务要求定位相关代码、测试和夹具。
2. 做最小正确实现，避免临时兼容层、夹具专用逻辑或规避式替代方案。
3. 添加或更新最小相关测试/fixture，覆盖任务指定行为。
4. 运行任务要求的验证命令，并按需要运行相关更广测试。
5. 如验证失败，定位并修复真实原因；若发现阻塞性规格缺口，则记录前置任务并停止。

## 收尾步骤

1. 在对应 `TODO-Px.md` 中给完成任务标题加 `[DONE]`，更新完成记录。
2. 同步 `TODO.md` 中相同任务的 `[DONE]` 状态和任何标题/顺序变化。
3. 更新本文件记录关键进展和验证结果。
4. 检查 git 状态和 diff，提交本次任务全部相关改动。
5. 停止，不继续下一个任务。

## 当前状态

- 已读取 `TODO.md` 与 `TODO-P6-part3.md`。
- 当前执行单元确定为 `P6-T03h：闭合 continuation protocol，覆盖 one-shot、double resume、wrapper projection、drop/unwind/abandon`。
- 最新提交为 `[P6-T03g] Close HandleDispatch protocol lowering`，与当前任务的直接依赖一致，未发现需要插入的新前置任务。

## P6-T03h 计划

1. 定位 refactor LLVM continuation allocation / resume / wrapper projection / abandon 相关实现与测试入口。
2. 检查当前定向验证失败点，区分真实缺口与已有实现边界。
3. 按 published layout 修正 continuation one-shot、double resume ordinary runtime-error、frame/local 恢复、wrapper complete/outward projection，以及 drop/unwind/abandon 的 contract-defined 行为或 fail-fast。
4. 补充或更新 `refactor_llvm_continuation_protocol` 与 `refactor_llvm_double_resume_runtime_error` 覆盖。
5. 运行任务指定验证与必要回归，修复失败。
6. 标记 `P6-T03h` 完成，同步 `TODO.md`，提交后停止。

## 进展记录

- 已确认现有 `cargo test -p scoopc refactor_llvm_continuation_protocol` 与 `refactor_llvm_double_resume_runtime_error` 目前没有匹配测试，需要补充。
- `effect_resume_double_resume_exit.scoop` 在 refactor run-pass 下失败；`effect_escape_continuation_resume_later_exit.scoop` 在 120s 内未结束，属于当前 continuation protocol 任务的直接阻塞。
- 已实现 same-schema resume-boundary wrapper projection，修复 surface resume owner step 与 wrapper step 返回类型漂移。
- 已将 double resume lowering 从 `unreachable` 改为 ordinary `RuntimeError.ContinuationAlreadyResumed` outward `Step`，并补充 continuation protocol / double-resume 单测。
- 已补充 resume-entry handle completion 返回路径，避免 escaped continuation resume 后重放已完成的 handle 外层 continuation；同一 arm 内已发布为 completion payload 的 resume 结果会直接完成 handle，而不是继续执行后续重复 resume。
- 指定验证已通过：`cargo test -p scoopc refactor_llvm_continuation_protocol`；`cargo test -p scoopc refactor_llvm_double_resume_runtime_error`；`cargo run -p scoop -- --effect-pipeline refactor test --fixtures tests/fixtures/run-pass/effect_resume_double_resume_exit.scoop`；`cargo run -p scoop -- --effect-pipeline refactor test --fixtures tests/fixtures/run-pass/effect_escape_continuation_resume_later_exit.scoop`。
- 相邻回归已通过：`cargo test -p scoopc refactor_effect_lowered_surface_resume_wrapper_completion`；`cargo test -p scoopc refactor_llvm_surface_resume_wrapper_completion`；`cargo test -p scoopc refactor_llvm_handle_dispatch_lowering`；`cargo run -p scoop -- --effect-pipeline refactor test --fixtures tests/fixtures/run-pass/handle_finally_boundary.scoop`；`cargo clippy --all-targets -- -D warnings`。
