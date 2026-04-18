# 执行计划记录

说明：用户要求记录“完整思考过程”。我不会写出逐字内部推理，但会记录可审计的行动计划、关键判断依据、执行进展与变更原因，便于随时检查当前状态。

## 初始计划

1. 检查最新一次 Git 提交的信息，确认是否提到需要先修复的既有问题。
2. 阅读 `TODO.md`，定位第一个未完成任务。
3. 如该任务过大，拆分为更小子任务，并同步更新 `PLAN.md` 与 `TODO.md`。
4. 实施当前要处理的首个任务。
5. 运行相关测试，并补齐必要测试直到任务验证通过。
6. 更新 `TODO.md`、`PLAN.md` 与本文件，记录完成情况或阻塞原因。
7. 提交本次变更，随后停止，不继续处理下一个任务。

## 进展日志

- 已创建计划文件，下一步将检查最新提交并读取任务列表。
- 已检查最新提交 `3ac3bdd468f23fd3c7d9119ab77596a1bc242c70`，提交信息本身未声明新的待先修复问题；提交中记录的既有遗留点是 `T3017`：`run-pass` 里仍残留一批 stale `EXPECT: fail`。
- 已定位 `TODO.md` 中首个未完成任务为 `T3017`「回收 `T3006` 暂时 xfail fixtures，恢复 effect run-pass 基线」；其后续 review 任务为 `T3017R`，当前轮不处理。
- 已盘点 `tests/fixtures/run-pass/**` 中残留的大批 `T3006` 临时 `EXPECT: fail` 标记。下一步将批量验证这些 fixture 当前是否已经实际通过：
  1. 先确认 `scoop test` 的单 fixture / 批量运行方式。
  2. 对残留 `T3006` xfail 批量执行，区分“已实际通过的 stale expectation”和“仍有真实失败的 fixture”。
  3. 若全部只是 stale expectation，则直接回收标记并跑全量验证。
  4. 若发现真实失败，则按用户要求把对应实现缺口前置成新任务，更新 `TODO.md` / `PLAN.md` / 本文件后提交并停止。
- 已确认单 fixture 验证方法可行：将目标 fixture 临时复制到隔离目录下的 `run-pass/`，把头部 `EXPECT: fail` 改为 `EXPECT: pass` 后，直接用正式 runner `./target/debug/scoop test --fixtures <tmp-root>` 校验。
- 批量扫描途中已得到关键结论：
  - 不是所有 `T3006` xfail 都只是 stale expectation。
  - 当前至少已有 13 条 fixture 在去掉 xfail 后出现真实偏差，其中大多数是 `stdout` 与 golden 不一致，少数直接以非零退出码失败。
  - 另有 `gc_continuation_escape_deep_object_graph.scoop` 在批量扫描中长时间运行，说明还存在至少一个高成本或疑似卡住的真实问题。
- 已中止无总超时的批量扫描。下一步将针对已暴露的失败样本做最小复现，判断这些问题是“只需更新 golden 的语义漂移”，还是“仍有生产代码缺口，需要把新修复任务前置到 `T3017` 之前”。
- 已完成最小复现并确认这不是单纯 golden 漂移：
  - `effect_escape_continuation_finally_multi_perform.scoop`、`effect_resume_mixed_escape_direct_finally.scoop`、`effect_resume_mixed_source_path_matrix.scoop` 都在 resumed completion 后多跑了一次 `finally/cleanup`。
  - `effect_nosuspend_finally_nested_handle.scoop` 输出 `0/0`，说明 no-suspend nested handle 的结果槽恢复也有缺口。
  - `effect_escape_continuation_perform_in_when_arm.scoop` 与 `effect_multi_escape_custom_nonresuming_direct_indirect_block_multi.scoop` 证明 resumed-body tail replay 在 `when` / block mixed suspend-site 中会重复 prefix 或跳过应有语句。
  - `effect_escape_continuation_finally_normal.scoop` 报 `unsupported_main_body: call callee`；`effect_escape_continuation_nested_outer_resume_inner_multi.scoop` 则直接空输出退出，表明 outer-body `Continuation.resume(...)` 仍有未接回的主路径 lowering。
  - `effect_escape_continuation_gc_stress_multi_string.scoop` 在 `SCOOP_GC_STRESS=1` 下打印 `missing1/missing2/missing3`，`gc_continuation_escape_deep_object_graph.scoop` 在同环境下超时，说明 GC stress continuation/object-graph 可达性合同仍未闭环。
- 已按阻塞规则更新 `TODO.md` 与 `PLAN.md`：
  1. 新增 `T3016a`→`T3016dR` 四组前置修复/复审任务。
  2. 将 `T3017` 顺延到这些前置任务之后，并让其显式依赖 `T3016dR`。
  3. 当前新的首个未完成任务已变为 `T3016a`，本轮将按要求只提交“阻塞重排”而不继续实现后续任务。
