# 执行计划

说明：我不会写出不可审计的内部思维细节，但会把可执行的外部计划、当前判断、进度和变更记录持续写在这里，便于检查。

## 初始计划

1. 查看最新一次提交的信息，确认是否提到需要先修复的既有问题。
2. 阅读 `TODO.md`，定位第一个未完成任务。
3. 阅读 `PLAN.md`，核对当前项目计划与任务顺序。
4. 判断首个未完成任务是否过大：
   - 如果可直接完成，就直接实现。
   - 如果过大，就先拆分任务，更新 `PLAN.md` 与 `TODO.md`，本次只执行拆分后排在最前的那个子任务。
5. 实现当前目标任务，同时在执行中留意所有既有问题、规格不匹配、回归、缺失实现边界或临时绕过；若发现这类问题，优先修复，或把它们作为前置任务插入 `TODO.md` 并停止继续向前。
6. 运行相关验证，包括至少与改动直接相关的测试；若任务范围涉及整体质量门槛，再补充 `cargo fmt`、`cargo test --all`、`cargo clippy --all-targets -- -D warnings` 等必要检查，并修复发现的问题。
7. 更新文档状态：
   - 在 `TODO.md` 中把本次完成的任务标记为已完成，或在阻塞时重排任务顺序并保留为待办。
   - 在 `PLAN.md` 中记录当前状态、依赖调整和后续影响。
   - 在本文件中补充执行结果与关键决策。
8. 按仓库既有风格创建一次 Git 提交，然后停止，不继续处理下一个任务。

## 进度记录

- 已创建初始执行计划，尚未开始仓库检查。
- 已检查最新提交：`1f7b3418bfdaea31aabaa3c0357af676f8f9d203`，提交信息仅为 `Update plan`，未显式提到需要先修复的既有问题。
- 已阅读 `TODO.md` 与 `PLAN.md`，当前首个未完成任务为 `T5002a`：完成 state-machine mutable-local flush-back 合同。
- 下一步：复现 `tests/fixtures/run-pass/effect_multi_escape_indirect_direct_while.scoop` 的当前状态，确认是否存在额外前置缺陷，以及该任务是否需要先拆分。
- 已复现并确认：
  - `tests/fixtures/run-pass/effect_multi_escape_indirect_direct_while.scoop` 默认环境通过；
  - `tests/fixtures/run-pass/effect_multi_escape_indirect_direct_while.scoop` 在 `SCOOP_GC_MOVE=1 SCOOP_GC_STRESS=1 SCOOP_GC_VERIFY_ROOTS=1` 下通过；
  - `tests/fixtures/run-pass/continuation_resume_enum.scoop` 在同一 GC 环境下通过。
- 当前判断：`T5002a` 很可能已经由先前代码改动实质完成，但 TODO/PLAN 尚未回写。继续补做边界覆盖核查与定向验证，确认可以把本轮工作收敛为“验收、记录、提交”而不是继续写代码。
- 已完成边界覆盖核查与定向验证：
  - LLVM 回归：`escaped_continuation_resume_ir_records_outer_slot_storage_and_writeback`、`state_machine_frame_slots_materialize_stable_exec_local_homes`、`cleanup_enter_ir_checks_cleanup_flag_before_reentering_finally`、`cleanup_propagate_ir_restores_propagating_state_after_shared_finally_exit` 通过；
  - run-pass：`effect_escape_continuation_outer_mutable_writeback_basic.scoop`、`effect_multi_escape_direct_indirect_while.scoop` 默认环境与三项 GC env 全开环境通过；
  - 先前复验：`effect_multi_escape_indirect_direct_while.scoop`、`continuation_resume_enum.scoop` 默认/GC 环境通过；
  - lint：`cargo clippy -p scoop -p scoopc --all-targets -- -D warnings` 通过。
- 已据此把 `TODO.md` 中 `T5002a` 标记为完成，并在 `PLAN.md` 中回写当前状态；本轮不继续推进 `T5002aR`，只准备提交本轮文档/状态更新。
