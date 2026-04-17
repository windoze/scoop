# 本次执行计划

## 约束与目标

- 本轮只处理 `TODO.md` 中第一个未完成任务，完成后停止。
- 在继续任务前，先检查最新提交是否提到任何既有问题；若有，先修复这些问题。
- 任何实现都不能依赖规避方案；若遇到规范缺口或前置依赖，必须先更新 `TODO.md` / `PLAN.md`，提交后停止。
- 全程用中文记录进展，并在关键步骤完成或计划调整时更新本文件。

## 初始执行步骤

1. 查看最新一次 Git 提交信息，确认是否显式提到需要先处理的遗留问题。
2. 阅读 `TODO.md` 与 `PLAN.md`，找出第一个未完成任务，并确认其上下文与依赖。
3. 判断该任务是否可在本轮完整完成：
   - 若可完成：直接实现、测试、更新文档、提交。
   - 若过大或存在缺失前置：把任务拆分或补充前置任务，并更新 `TODO.md` / `PLAN.md` 后提交并停止。
4. 实现任务时同步检查相关规范、代码路径与测试覆盖，避免引入临时方案。
5. 运行必要验证：
   - 相关单测 / 集成测试 / fixture 测试
   - `cargo fmt --check`
   - `cargo clippy --all-targets -- -D warnings`
   - 与本次修改直接相关的额外命令
6. 完成后更新：
   - `TODO.md`
   - `PLAN.md`
   - 本文件中的进展记录
7. 生成一次 Git 提交，提交信息对应当前任务，然后停止。

## 进展记录

- 已创建本计划文件，下一步开始检查最新提交与待办列表。
- 已检查最新提交：`[T3009b0a2] Track runtime raise boundary caller-tail gap`。该提交只是在 `TODO.md` / `PLAN.md` 中前移登记 shared blocker，没有额外要求先修的遗留实现。
- 已定位本轮第一项未完成任务为 `T3009b0a2`：修正 unified `RuntimeRaiseBoundary` 的 inactive-continue / active-dispatch 合同。
- 在真正进入 `T3009b0a2` 实现前，通过最小复现 `handle { helper(false) + 1 } with { Ask.ask() -> 2 }` 发现更前置的未跟踪缺口：`SuspendCall` 的 inactive 成功路径也仍被统一 terminator 当成“无条件 suspend”，程序空输出退出，本应打印 `8`。
- 该问题比 `RuntimeRaiseBoundary` 更基础，并且会先于 `T3009b0a2` 暴露；按用户规则，本轮不再继续写生产代码，而是先更新 `TODO.md` / `PLAN.md` 的依赖顺序，提交后停止。

## 当前聚焦计划：T3009b0a2

1. 阅读 `TODO.md` / `PLAN.md` 中 `T3009b0a2`、`T3009b0`、`T3010b2b1` 的上下文，确认它是当前最前置任务。
2. 定位 `RuntimeRaiseBoundary` 在 unified state-machine plan / emitter / 普通 codegen 中的建模与发射路径。
3. 用两个任务定义里的最小复现重新确认现状：
   - `tests/fixtures/run-pass/type_check_cast_is_as_asq_basic.scoop`
   - `tests/fixtures/run-pass/effect_escape_continuation_resume_unit.scoop`
4. 根据复现结果修正共享合同：
   - inactive 路径继续执行当前 state machine caller-tail；
   - active 路径保持向 enclosing `handle` / `try` 交回 dispatch。
5. 运行定向测试与全量质量门槛，确认没有引入回归。
6. 更新 `TODO.md` / `PLAN.md` / 本文件，提交一次只覆盖本任务的 Git commit，然后停止。

## 当前阻塞与处理

- 新发现的前置 blocker：unified `SuspendCall` 的 inactive-continue / active-dispatch 合同未实现。
- 直接证据：
  - `cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/type_check_cast_is_as_asq_basic.scoop`
    当前输出停在 `x is Base: true`。
  - `cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/effect_escape_continuation_resume_unit.scoop`
    当前输出停在 `after_pause`。
  - 最小 repro `handle { helper(false) + 1 } with { Ask.ask() -> 2 }`
    当前空输出退出，本应打印 `8`。
- 本轮调整：
  - 在 `TODO.md` 中前移新增 `T3009b0a1c` / `T3009b0a1cR`，让 `T3009b0a2` 改为依赖它们。
  - 在 `PLAN.md` 中记录复现结果和新的执行顺序。
  - 完成后提交并停止，等待下一轮先处理 `SuspendCall`。
- 当前状态：
  - `TODO.md` / `PLAN.md` / 本文件已更新完成。
  - `git diff --check` 已通过。
  - 下一步只剩创建 Git 提交并停止。
