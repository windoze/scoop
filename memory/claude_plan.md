# 执行计划（公开摘要）

## 目标

本次调用只完成 `TODO.md` 中第一个未完成任务；若发现该任务被前置缺陷、规范不匹配或缺失能力阻塞，则先把阻塞问题整理为新的前置任务，更新 `TODO.md` 与 `PLAN.md`，提交后停止。

## 计划步骤

1. 检查最新一次 Git 提交，确认是否明确提到已有问题；若提到，则把这些问题纳入本次范围并优先处理。
2. 阅读 `TODO.md`，定位第一个未完成任务。
3. 阅读 `PLAN.md` 与相关上下文，判断该任务是否可在一次调用内完整完成。
4. 若任务过大或存在明确前置依赖：
   - 将任务拆分为更小子任务；
   - 更新 `PLAN.md`；
   - 更新 `TODO.md` 的任务顺序与依赖；
   - 本次执行拆分后的第一个子任务，或在被阻塞时仅提交计划调整。
5. 阅读并修改相关代码，严格按规范实现，不引入临时绕过方案。
6. 运行与任务相关的测试，并补充必要测试。
7. 运行质量检查，至少覆盖格式、测试和无警告要求；若某项受环境限制，需在计划和最终说明中写明。
8. 更新 `TODO.md`、`PLAN.md` 与本文件，记录完成状态或阻塞原因。
9. 提交一次清晰的 Git commit，然后停止，不继续下一个任务。

## 执行记录

- 已创建本计划文件，后续会在关键节点更新。
- 已检查最新一次 Git 提交：提交信息为 `[T3016eR] Review nested handler arm outward propagation`，正文未显式提到新的需先修遗留问题。
- 已读取 `TODO.md` / `PLAN.md`，当前第一个未完成任务为 `T3017`：回收 `T3006` 暂时 xfail fixtures，恢复 effect run-pass 基线。
- 下一步：盘点 `tests/fixtures/run-pass/**` 中剩余的 `EXPECT: fail` / `T3006` 临时标记，并用正式 runner 验证当前真实阻塞面，判断 `T3017` 能否直接完成，或是否必须按规则继续前移新的前置任务。
- 已完成盘点：
  - `cargo run -p scoop --features llvm -- test` 当前首个停止点仍是 `effect_escape_continuation_async_executor_fifo.scoop` 的 stale `EXPECT: fail`。
  - run-pass 中仍有 83 个文件残留 `EXPECT: fail` 或 `T3006` 临时标记；其中 75 个仍带 `EXPECT: fail`。
  - 对这 75 个 `EXPECT: fail` fixture 逐条单独切回 `EXPECT: pass` 后验证，结果为：62 条已经真实通过，4 条属于应继续保持失败语义的诊断/负向 fixture，9 条暴露真实问题。
- 目前确认的新真实 blocker：
  - effect 主线 blocker 1：top-level multi-site direct/indirect replay 仍会在后续 `resume(...)` 后重放已完成 prefix（`effect_multi_escape_custom_nonresuming_direct_indirect_multi.scoop`、`effect_multi_escape_custom_nonresuming_indirect_multi.scoop`、`effect_multi_escape_indirect_multi.scoop`、`effect_resume_mixed_multi_escape_direct_indirect.scoop`）。
  - effect 主线 blocker 2：`effect_resume_finally_body_raise_after_resume.scoop` 在 resumed body 再次 `Raise.raise(...)` 后没有向外传播，而是继续执行 `handle_unreachable`。
  - effect 主线 blocker 3：`std_task_async_adapters_basic.scoop` 与 `stdlib_smoke_test_and_preconditions.scoop` 仍报 `unsupported_main_body: effect frame seed outer-scope local`。
  - 非 effect 真实问题：`gc_continuation_multi_thread_concurrent_alloc_resume.scoop` 先被 `None` ctor 歧义卡住；`not_null_assert_basic.scoop` 仍报 `when arm type mismatch`。
- 已决定按阻塞规则更新 `TODO.md` / `PLAN.md`：
  - 在 `T3017` 前新增 `T3016f`→`T3016hR` 三组 effect 前置任务。
  - 将 enum ctor 歧义与 `!!` lowering 缺口分别转记到后续任务。
  - 本次调用只提交计划/任务重排，不继续实现代码修复。
- 已完成文档更新：
  - `TODO.md` 已插入 `T3016f` / `T3016g` / `T3016h` 及对应 review 任务，并将 `T3017` 顺延到这些前置任务之后。
  - `TODO.md` 也已新增后续任务 `T3304`（同名 enum variant ctor 消歧）与 `T3406`（`!!` lowering/codegen）。
  - `PLAN.md` 已同步记录本轮扫描结论，并把当前执行顺序更新为从 `T3016f` 开始。
- 当前结束条件：
  - 本次不改生产代码，只提交计划/任务重排。
  - 下一次调用应从新的首个未完成任务 `T3016f` 开始。
