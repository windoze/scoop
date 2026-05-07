# 当前执行计划

## 目标

- 以 `TODO.md` 为唯一任务顺序来源，完成第一个标题未带 `[DONE]` 的任务。
- 完成后更新 `TODO.md` 的任务标题和完成记录，运行相关验证，提交一次 Git commit，然后停止。

## 步骤

1. 读取 `TODO.md`，定位第一个未完成任务，并记录任务要求、依赖和验证命令。
2. 检查最近提交信息，仅判断是否有直接关联当前任务的未完成事项。
3. 阅读当前任务相关代码、测试和规格，避免无关历史问题扫描。
4. 按任务要求做最小正确实现；若发现阻塞当前任务的缺失特性或规格不匹配，则更新 `TODO.md` 插入最小前置任务并停止。
5. 运行当前任务要求的验证；若失败，修复当前任务范围内的问题并重跑。
6. 更新 `TODO.md`：将完成任务标题加 `[DONE]`，补充完成记录；仅在阶段级计划变化时更新 `PLAN.md`。
7. 复查工作树，提交所有本次任务相关变更，提交信息包含任务编号。

## 进度记录

- 已创建本计划文件，下一步读取 `TODO.md`。
- 已读取 `TODO.md`，第一个未完成任务为 `CG-T04R：Review CG-T04a-CG-T04f composite transport lowering`。
- 下一步检查最近提交是否有直接关联该复审任务的未完成事项，然后复审 composite transport 的代码路径与验证样本。
- 最近提交是 `CG-T04f`，未发现提交信息中的额外未完成项。
- 复审发现当前任务范围内的真实缺口：materialized MIR 未验证 cross-thread resume payload transport；runtime composite transport 在数组和跨线程 native roots 中未统一使用 descriptor trace surface，数组对象回收也未 drop composite elements。
- 下一步修复这些缺口，新增/加强 runtime 回归测试，然后运行 CG-T04R 要求的定向验证。
- 已修复上述缺口：materialized call transport 现在验证 `thread_resume_payload`；runtime composite array 写入/构建使用 descriptor trace surface 枚举 ref slots 并经写屏障写入，数组 release 会 drop composite 元素；跨线程 resume native roots 也改为通过 `scoop_composite_trace` 收集 payload roots。
- 已新增 runtime 回归：composite array release drop 测试与 Immix composite array write-barrier promote 测试。
- 已通过 CG-T04 精确 cargo 验证、代表 fixture、GC stress 跨线程 payload 样本和 `cargo clippy --all-targets -- -D warnings`。宽泛额外命令 `cargo test -p scoopc refactor_llvm_` 发现非 CG-T04 的 `refactor_llvm_runtime_type_primitives` 既有失败，未纳入当前任务完成条件。
- 已更新 `TODO.md`，将 `CG-T04R` 标记为 `[DONE]` 并写入复审/修复/验证记录。下一步复查 git diff 并提交。
- 复查 diff 时进一步收紧 array build 写屏障时序：composite 元素现在先准备副本、扩展 `len` 使当前元素可被 trace，再逐 slot 通过写屏障提交；相关 runtime 测试、array fixture 与 clippy 已重跑通过。
