# 当前执行计划

## 目标

完成 `TODO.md` 索引中按顺序解析出的第一个未完成详细任务；完成后更新对应任务文件与索引、运行相关验证、提交 Git 变更，并停止。

## 执行步骤

1. 读取 `TODO.md`，只把它作为任务索引使用。
2. 按索引顺序读取对应的 `TODO-Px.md` 详细任务文件，找出第一个标题未带 `[DONE]` 的任务。
3. 检查最新提交信息，确认是否存在与该任务直接相关的未完成问题；只处理会阻塞当前任务的问题。
4. 阅读当前任务的详细要求、依赖、验证条件与完成记录。
5. 检查当前实现和相关测试，确定需要修改的最小代码范围。
6. 按任务要求实现完整功能；如果发现必须先修复的具体阻塞问题，则在对应 `TODO-Px.md` 中插入最小前置任务，同步 `TODO.md`，提交后停止。
7. 添加或更新最小必要测试与 fixture，避免绕过规范要求。
8. 运行相关验证；如失败，定位并修复，直到当前任务要求的验证通过或确认存在必须记录的阻塞前置任务。
9. 将完成的详细任务标题加上 `[DONE]`，更新完成记录，并同步 `TODO.md` 中相同任务的 `[DONE]` 状态。
10. 检查工作区变更，确认不回退或覆盖非本次任务引入的无关改动。
11. 提交本次任务全部相关变更，提交信息使用任务编号和简洁说明。
12. 停止，不继续处理下一个任务。

## 进度记录

- 已写入初始执行计划。
- 已读取 `TODO.md` 与 `TODO-P7.md`，确认首个未完成详细任务是 `P7-T02Z`。
- 最新提交完成 `P7-T02Zc`，属于 `P7-T02Z` 的直接前置；未发现需要先于当前任务记录的其它最新提交阻塞。
- 下一步将先检查工作区状态，再按 `P7-T02Z` 要求继续默认 `run-pass` fixture 验证，定位并修复剩余阻塞。
- 工作区状态：仅本轮 `memory/claude_plan.md` 修改，另有未跟踪 `crates/scoop/target/` 目录，暂不纳入任务变更。
- 默认 `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass` 首个失败为 `continuation_escape_binder_resume_effect_row_runtime_basic.scoop` 退出码 1；该 fixture 属于当前 P7-T02Z 剩余 run-pass 阻塞范围。
- 已修复 `OwnerTrampolineMixed` 跨 owner wrapper projection 的 ABI materialization 校验，并新增 cross-owner owner trampoline 单测；`continuation_escape_binder_resume_effect_row_runtime_basic.scoop` 的 `run` 与 fixture harness 均已通过。
- 继续默认 run-pass 后，下一个失败为 `effect_escape_continuation_indirect_perform_closure.scoop`，需要继续定位和修复。
- 已修复无显式 wrapper projection 时 owner Step 与 surface Step schema 不同导致的 LLVM return type drift；`effect_escape_continuation_indirect_perform_closure.scoop` 的 `run` 与 fixture harness 均已通过，并新增默认 refactor CLI 覆盖。
- 已修复 known-instance closure call boundary 对 captured env invoke carrier 的 operand contract：closure env 可作为 whole tuple source 发布并通过 ABI 校验；`effect_multi_escape_indirect_callee_suspend_matrix.scoop` 的 `run` 已通过，并新增 late-lowered operand contract 单测。
- 已修复单 payload binder 绑定完整 tuple payload 的 lowering：`effect_multi_type_params_dispatch_basic.scoop` 恢复输出 `left / 7 / right / 107 / 10`，并新增默认 refactor CLI 覆盖。
- 已修复 effect-neutral assignment 对 `Never` target local 的无值写入：`effect_raise_cleanup_gc_basic.scoop` 恢复输出 `0`，并新增默认 refactor CLI 覆盖。
- 已为 refactor `Raise.raise` perform lowering 恢复 runtime trace hook 写入：`effect_raise_trace_hook_basic.scoop` 恢复输出 `16 / 5`，并新增默认 refactor CLI 覆盖。
- 已修复 receiver effect op 中 `String.length` fun-value builtin 的 facts 与 source-slice lowering：`effect_receiver_op_basic.scoop` 恢复 expected stdout 与 exit 30，并新增 facts 单测与默认 refactor CLI 覆盖。
- 继续默认 run-pass 时阻塞在 `effect_resume_finally_body_raise_after_resume.scoop`。该问题需要发布 finally pending completion 的 origin/resume-state/composed-resume contract；简单放宽现有 fail-fast 会造成 `finally` 重复执行或错误 normal completion，不能保留。
- 已在 `TODO-P7.md` 中新增前置任务 `P7-T02Zd`，并同步 `TODO.md`；`P7-T02Z` 保持未完成并依赖该前置任务。下一步将只验证本轮已落地修复的定向测试，随后提交并停止。
- 验证完成：`cargo fmt --all`；`cargo test -p scoopc --lib effect_lowered`；`cargo test -p scoopc --lib llvm::codegen::effect_refactor`；`cargo test -p scoopc --lib refactor_effect_facts_treats_builtin_string_length_fun_value_as_plain -- --nocapture`；`cargo test -p scoop --test p7_default_pipeline`；以及本轮修复的 7 个 run-pass fixture harness 均通过。
