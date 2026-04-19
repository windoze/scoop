# 执行计划记录

## 约束说明

- 本文件记录可共享的执行计划、观察、关键决策、完成状态与变更原因。
- 不写入逐字内部思维；改为提供足够审计执行过程的摘要、依据与步骤。

## 初始步骤

1. 检查最新一次提交，确认是否提到了需要先处理的既有问题。
2. 阅读 `TODO.md`，定位第一个未完成任务。
3. 评估该任务是否过大：
   - 若可直接完成，则开始实现。
   - 若过大，则先拆分任务，更新 `PLAN.md` 与 `TODO.md`，并只执行拆分后的第一个子任务。
4. 在实现过程中同步更新本文件，记录：
   - 当前目标
   - 发现的问题
   - 是否触发任务重排/新增前置任务
   - 已完成的关键步骤
5. 完成后执行相关测试与质量检查，至少覆盖任务相关验证；若范围允许，补充工作区要求的通用检查。
6. 更新 `TODO.md` 与 `PLAN.md`。
7. 以清晰的提交信息提交本次改动。
8. 停止，不进入下一个任务。

## 执行中的判断准则

- 如遇规格不匹配、缺失语言特性、已有 bug 或依赖前置能力不足，不采用规避方案。
- 若当前任务被前置问题阻塞：
  - 在 `TODO.md` 中新增或重排前置任务；
  - 在 `PLAN.md` 中记录阻塞原因；
  - 提交这些计划性改动后停止。
- 不回退或覆盖与当前任务无关的用户现有改动。

## 当前状态

- 已检查最新提交：提交说明未额外声明必须先修的遗留问题。
- 已读取 `TODO.md` / `PLAN.md`，当前首个未完成任务原为 `T4008b`。
- 已完成初步可行性评估，结论如下：
  1. `T4008b` 不能按原描述直接安全推进，需要先拆分。
  2. 证据：
     - 当前 `crates/scoopc/src/typecheck/expr/infer.rs` 在 `, k ->` 分支中仍直接注入默认 `Continuation<T>`，没有为 `E` 建模。
     - 当前 `crates/scoopc/src/typecheck/lower.rs` 只有函数体 / handle body 级别的 `performed_effects` 收集，没有“从某个 escape site 恢复后，到下一次 suspension / return / 正常完成”为止”的 step-level effect summary。
     - probe `/tmp/t4008b-probe2/continuation_escape_binder_effect_row_is_not_pure.scoop` 已复现错误：`Ask.current(), k -> { requirePure(k); requireBoom(k) }` 当前“期望失败，但执行成功”，说明 binder 仍被错误地视为 `Continuation<Int, eff Pure>`。
  3. 判断：
     - 若直接把整段 handle body 的 effects 塞给 `Continuation<T, eff E>`，会把 prefix effects 与 fresh continuation 之后的 tail 一起误算进当前 `resume` step，属于新的规范偏差。
     - 因此先在 `TODO.md` / `PLAN.md` 中把 `T4008b` 拆成前置分析任务与后续语义收口任务，再停止本轮，等待下一次调用实现新的首个子任务。
- 已更新：
  - `TODO.md`：将 `T4008b` 拆分为 `T4008b1`（resumed-step effect-row 分析）与 `T4008b2`（基于该分析收口 binder 与 `resume`）。
  - `PLAN.md`：记录 probe 结论、阻塞原因与新的执行顺序。
