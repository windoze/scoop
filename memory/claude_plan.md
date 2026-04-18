# 执行计划

说明：不记录逐字内部思维推演；这里记录可审计的执行计划、依据、变更与进度。

## 初始计划

1. 查看最新一次 Git 提交，确认提交说明或改动中是否提到已知问题、遗留缺陷或待修复事项。
2. 如果最新提交暴露了需要先修复的问题，优先定位、修复、验证，并记录到本文件。
3. 阅读 `TODO.md`，识别第一个未完成任务。
4. 评估该任务是否足够小且可在本次调用中完整交付。
5. 如果任务过大或被前置缺陷阻塞：
   - 更新 `PLAN.md`，拆分为更小的可执行子任务或补充前置依赖；
   - 更新 `TODO.md`，把新的前置任务放到正确顺序；
   - 本次只执行排序后的第一个可执行任务，然后停止。
6. 对本次目标任务实施代码修改。
7. 运行相关格式化、检查、测试与 lint，至少覆盖受影响范围；如有失败，继续修复直到通过，或按阻塞规则回写计划。
8. 更新 `TODO.md`、`PLAN.md`、本文件，标记进度与结果。
9. 提交 Git commit，然后停止，不继续下一个任务。

## 进度

- 已创建本文件并写入初始计划。
- 已检查最新 Git 提交：`7dab3765580dbc30ade795891bce1258d4a50e62`，提交标题为 `[T3016hR] Review unified outer-slot seeding contract`，提交信息中未单独声明新的待修复遗留问题，因此无需在本轮任务前插入额外修复。
- 已读取 `TODO.md` / `PLAN.md`：
  - 第一个未完成任务为 `T3017`：回收 `T3006` 暂时 xfail fixtures，恢复 effect run-pass 基线。
  - 当前计划文件也明确 `T3016hR` 完成后下一项即为 `T3017`。
- 当前判断：`T3017` 已有明确边界，先进行 expectation 扫描与实际 runner 验证，再决定是否需要进一步拆分。
- 已完成 expectation 扫描与单文件官方 runner 复核：
  - `tests/fixtures/run-pass` 中共有 77 个残留 `T3006` 注释、72 个 `EXPECT: fail`。
  - 将 72 个 `EXPECT: fail` fixture 临时改为 `EXPECT: pass` 后逐个用 `target/debug/scoop test --fixtures <temp-root>` 验证，结果 66 个已恢复稳定通过。
  - 真正仍失败的只有 6 个：
    - 应继续保留负向/诊断语义：`effect_resume_double_resume_exit.scoop`、`exit_code_mismatch.scoop`、`stderr_mismatch_distinguishable.scoop`、`timeout_should_fail.scoop`。
    - 真实 blocker：`gc_continuation_multi_thread_concurrent_alloc_resume.scoop`（当前报 `scoop::typecheck::ambiguous_enum_variant_ctor`，已由 `T3304` 跟踪）、`not_null_assert_basic.scoop`（当前报 `scoop::llvm::unsupported_main_body` / `when arm type mismatch`，已由 `T3406` 跟踪）。
  - 另有 9 个 fixture 已经是 `EXPECT: pass`，但仍残留 stale `T3006` 头注释，需要一并清理。
- 当前决策：`T3017` 无需再拆分。直接清理 66 个 stale `EXPECT: fail`、移除 75 个 fixture 的 `T3006` 头注释，并把 2 个真实 blocker 的原因更新为 `T3304` / `T3406`。
- 已完成 fixture 头部批量修改：
  - 66 个 stale `EXPECT: fail` 已改回 `EXPECT: pass`；
  - run-pass 下全部 `T3006` 临时注释已清空；
  - `gc_continuation_multi_thread_concurrent_alloc_resume.scoop` 与 `not_null_assert_basic.scoop` 已分别改写为真实 blocker `T3304` / `T3406`。
- 已执行全量验收 `cargo run -p scoop --features llvm -- test`，结果暴露出新的更前置生产回归：
  - `tests/fixtures/run-pass/effect_handle_suspend_call_inactive_helper_basic.scoop`
  - 当前真实失败：`scoop::llvm::module_verification_failed`
  - 关键诊断：`Terminator found in the middle of a basic block! label %resume_site0`
- 已据此更新 `TODO.md` / `PLAN.md`：
  - 在 `T3017` 前新增 `T3016i` → `T3016iR`，先修复并复审 unified `SuspendCall` inactive helper verifier 回归；
  - `T3017` 保持 `[TODO]`，并显式声明等待 `T3016iR`。
- 当前状态：本轮不继续修实现，按阻塞规则在完成文档更新后提交并停止。
