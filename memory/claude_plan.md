# 执行计划

说明：我不会写入不可公开的逐字思维链，但会持续记录可审计的执行计划、关键判断依据和进度变化。

## 初始计划

1. 检查最近一次提交的提交信息与变更，确认是否提到任何已知问题；如果存在且仍未修复，先处理这些问题。
2. 阅读 `TODO.md`，定位第一个未完成任务。
3. 阅读 `PLAN.md`，核对当前计划与 `TODO.md` 是否一致。
4. 判断首个未完成任务是否足够小且可在本轮完整完成。
5. 如果任务过大或存在前置依赖缺失：
   - 在 `PLAN.md` 中细化为更小子任务；
   - 在 `TODO.md` 中按依赖顺序插入或重排这些子任务；
   - 执行第一个新的子任务，并在文档中记录阻塞关系。
6. 实现本轮要完成的任务，避免规避规范或临时性变通。
7. 运行相关测试与质量检查，至少覆盖受影响范围；如有必要，运行更广泛的回归测试。
8. 更新 `TODO.md` 与 `PLAN.md`，标记已完成内容并记录任何计划调整。
9. 提交本轮修改，提交信息应清晰描述任务编号和变更内容。
10. 停止，不继续处理下一个任务。

## 进度

- 已完成：写入初始执行计划。
- 已完成：检查最近一次提交；提交信息未声明新的待修前置问题。
- 已完成：读取 `TODO.md` 与 `PLAN.md`；确认首个未完成任务为 `T4016R`。
- 已完成：针对 `T4016R` 执行 review，未发现需要前插的新 blocker。

## Review 结论（T4016R）

1. 语法与中间表示：
   - parser 仅把 `-> resume { ... }` 作为 removed diagnostic 处理；
   - AST / HIR 的 handler arm 只保留 non-resuming 与 escape-continuation 两类。
2. continuation answer model：
   - `sysroot/core.scoop`、spec 与 runtime 文档均使用 `Continuation<Resume, Answer, eff E>`；
   - typecheck 将 `k.resume(...): Answer` 的返回类型绑定到 continuation 的 answer type；
   - runtime 通过共享 `scoop_continuation_resume_with(...)` 提供 payload + answer 通道。
3. `Task` 合同：
   - `Task` 私有 step driver 的 delimiter answer 为 `__TaskStepResult`；
   - runtime/c/scoop_task.c 通过共享 continuation helper 恢复 continuation 并读取 `__TaskStepResult`，未再依赖 task-private frame-peek。
4. 机械复核：
   - 生产代码中未发现残留的用户态 `-> resume` surface；剩余引用仅存在于文档、removed diagnostic 与回归 fixture。
   - legacy `Continuation<Resume>` / `Continuation<Resume, eff E>` 简写只剩 removed diagnostic fixture 与前端报错文本。
5. 已完成验证：
   - `cargo run -p scoop -- test`
   - `cargo test --all`
   - `cargo clippy --all-targets -- -D warnings`

## 收尾动作

1. 将 `TODO.md` 中的 `T4016R` 标记为完成，并记录 review 结论。
2. 更新 `PLAN.md`，把下一步推进到 `T4012`。
3. 检查工作区状态并提交本轮修改。

## 本轮具体执行路径（T4016R）

1. 审查 `TODO.md` / `PLAN.md` 中对 `T4016R` 的验收要求，整理 review 清单。
2. 搜索并阅读 continuation、handler arm、`Task` 相关的实现与文档位置，重点检查：
   - `-> resume` 是否只剩 removed diagnostic，而未以其它 special form 存活；
   - `k.resume(...)` 的静态与运行时返回模型是否为 delimiter answer type；
   - `Task` 是否统一走 continuation contract，而非 task-private runtime hack。
3. 检查关键回归 fixture / 单测 / 文档是否与实现一致。
4. 运行必要测试与质量检查，至少覆盖 fixture 主线、Rust 测试与 clippy。
5. 根据审查结果二选一：
   - 若发现规范不一致或实现缺口：在 `TODO.md` / `PLAN.md` 中插入前置修复任务，记录阻塞关系，提交并停止；
   - 若未发现阻塞问题：将 `T4016R` 标为完成，更新计划与记录，提交并停止。
