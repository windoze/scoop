# 当前执行计划

## 约束

- 先读取 `TODO.md` 作为索引，再按索引打开对应的 `TODO-Px.md` 详细任务文件。
- 只处理第一个标题未带 `[DONE]` 的详细任务，完成后停止。
- 若当前任务遇到阻塞且不能按规范实现，则在对应详细 TODO 文件中插入最小必要前置任务，同步 `TODO.md`，提交后停止。
- 不用 workaround、弱化测试或改变任务意图来绕过缺失功能。
- 任务完成后同步 `[DONE]` 标记、补充完成记录、运行相关验证，并提交所有本次任务相关改动。

## 步骤

1. 检查工作区状态与最近提交，确认是否存在与当前任务直接相关的未完成事项或已有未提交改动。
2. 读取 `TODO.md`，按索引顺序读取相关 `TODO-Px.md`，确定第一个未完成详细任务。
3. 阅读该任务的完整要求、依赖、验证命令与完成记录，确定最小实现范围。
4. 探索相关代码与测试位置，实施任务要求的最小正确改动。
5. 运行任务指定验证和必要的回归测试；若失败，定位并修复，直到通过或发现必须新增前置任务的真实阻塞。
6. 更新对应 `TODO-Px.md` 标题为 `[DONE]` 并补充完成记录；如索引受影响，同步 `TODO.md`。
7. 运行最终检查，查看 git diff，提交本次任务所有相关改动。
8. 停止，不继续处理下一个任务。

## 进度

- 已创建本计划文件，下一步开始读取任务索引并定位第一个未完成详细任务。
- 已读取 `TODO.md` 与最新提交；第一个未完成索引项是 `P7-T03`，最新提交 `P7-T02V` 是其直接前置修复上下文。
- 已读取 `TODO-P7.md` 的 `P7-T03` 详情；当前任务要求按默认 refactor 路径完成 `cargo test --all`、`cargo run -p scoop -- test`、`spec-fixtures check`、`clippy -D warnings`，并保留显式 legacy smoke。
- `cargo test --all` 首轮失败：`commands::build::tests::build_refactor_task_atomic_fixture_lowers_o0_without_legacy_mutex` 在 `scoop.core.__task_drive_waiting::<(Int, Any)>` 的 `HandleDispatch` contract 发布时，non-Unit handle arm completion payload source state `st11` 缺少 completion payload source；下一步修复 late-lowering 的 completion payload source 发现/传播。
- 已修复 `Nothing` 表达式后继续生成 completion Goto 的问题；定向重跑越过 completion-source 错误后，继续阻塞在 `scoop.core.panic` call site 5：facts 仍发布 `DynamicFallback`，但 canonical MIR 是 `Direct { callee_fqn: "scoop.core.panic" }`，需要对齐 direct panic 的 call-site contract。
- 已将 `scoop.core.panic` 归为 refactor-owned plain compiler intrinsic，并在 MIR lowering 中让 `Nothing` 表达式后立即终止 CFG；`build_refactor_task_atomic_fixture_lowers_o0_without_legacy_mutex` 定向单测已通过。
- 发现 broad `Nothing` 终止会破坏普通 `Raise.raise` effect fixtures，已收窄为仅 `scoop.core.panic` fatal intrinsic 终止 CFG；`cargo test --all` 已通过。
- `cargo run -p scoop -- test` 首轮失败在 `tests/fixtures/run-pass/char_runtime_textual_basic.scoop`，退出码 1；下一步单独复现并修复默认 refactor run-pass 中的 char runtime/textual 回归。
- 已补齐 refactor `scoop.core.hash` intrinsic lowering（Char/Int/String/Float）并让 refactor `print`/`toString` 按 source type 识别 `Char` 调用 char runtime；`char_runtime_textual_basic` 与 `stdlib_hash_basic` 定向 run-pass 已通过。
- 完整 `scoop test` 继续失败在 `tests/fixtures/run-pass/class_ctor_arg_eval_scope_shadow_free_basic.scoop`，下一步单独复现并修复 class ctor argument evaluation / scope shadow 相关默认 refactor run-pass 回归。
- 已修复 MIR string interpolation 使用 stale part `ty` 的问题，改为消费 operand local/const 的 authoritative source type；`class_ctor_arg_eval_scope_shadow_free_basic` 定向 run-pass 已通过。
- 完整 `scoop test` 继续失败在 `tests/fixtures/run-pass/class_ctor_named_default_and_delegation_basic.scoop`；下一步单独复现并修复 class ctor named/default/delegation 默认 refactor run-pass 回归。
- 已补齐 refactor MIR class ctor 对 named/default/delegation 的参数映射、默认值求值和初始化执行，并让 class ctor HIR 初始化表达式中不在 refactor pass-view 内的纯 helper 可按需生成普通 HIR body；`class_ctor_named_default_and_delegation_basic`、`class_secondary_ctor_delegation_this_and_super_basic`、`class_init_super_ctor_args_eval_order_basic` 定向 run-pass 已通过。
- `class_init_hidden_raise_helper_try_catch_basic` 暴露 concrete blocker：class ctor / object init hidden `Raise<RuntimeError>` 未发布到 refactor facts / boundary lowering，导致 helper/main 被误判为 `NoOutward` plain callable。已在 `TODO-P7.md` 插入前置任务 `P7-T02W`，并同步 `TODO.md`；本轮将提交已完成修复与阻塞记录后停止。
