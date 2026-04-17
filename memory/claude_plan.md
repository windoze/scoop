# 本轮执行计划

## 目标

本轮只完成 `TODO.md` 中第一个未完成任务，并在完成后停止。

## 约束与执行原则

- 先检查最新提交是否提到已有问题；若有，先修复这些问题。
- 之后读取 `TODO.md`，定位第一个未完成任务。
- 若该任务过大，先拆分任务，并同步更新 `PLAN.md` 与 `TODO.md`，本轮只执行拆分后的第一个子任务。
- 实现时不得使用规避方案、临时补丁、仅夹具生效的 hack，必须符合规范。
- 若发现规范缺口、实现边界或前置依赖缺失，需要先把问题转化为新的 `TODO.md` 任务，调整顺序，更新 `PLAN.md`，提交并停止。
- 代码修改后需要运行相关测试，并尽量保证 `cargo clippy --all-targets -- -D warnings` 无警告。
- 完成后需要更新 `TODO.md`、`PLAN.md`，并创建一次 git 提交，然后停止。

## 初始步骤计划

1. 查看最新一次 git 提交的提交信息与改动，确认是否显式提到待修复问题。
2. 读取 `TODO.md` 与 `PLAN.md`，识别第一个未完成任务及其上下文。
3. 评估该任务是否可以在一轮内完整完成；若不能，设计最小可执行子任务并更新计划文件。
4. 阅读相关代码、测试、规范或文档，确认实现边界与潜在依赖。
5. 实现本轮目标任务。
6. 运行与改动直接相关的测试；如改动范围较广，再补充更全面的测试与 lint。
7. 更新 `memory/claude_plan.md`、`TODO.md`、`PLAN.md`，记录完成情况或阻塞原因。
8. 提交本轮改动，提交信息引用对应任务编号；随后停止，不进入下一个任务。

## 记录方式

- 我会在关键节点更新本文件，记录当前判断、计划变化、已完成步骤、测试结果以及是否存在阻塞。
- 若发现最新提交中提到的问题属于本轮前置修复项，会先记录问题，再优先处理。

## 当前状态

- 已创建本计划文件。
- 已检查最新提交 `c6c7bc9f5a21b16865bdbcf37dc6683de272acee`：提交信息为 `[T3009b0] Mark continuation resume scalar/ref lowering done`，未显式声明新的待修复遗留问题。
- 已读取 `TODO.md` / `PLAN.md`，当前第一个未完成任务为 `T3010b2b1b`：补齐 nested arm indirect outward propagation 所需的 unified value coercion / expected-context 前置。
- 进一步核对后，先前对首个未完成任务的识别有误：`TODO.md` 中更早的未完成项实际是 `T3009b0a1b`，其后才是 `T3009b0aR`、`T3009b0R` 与 `T3010b2b1b`。
- `T3009b0a1b` 的目标是在 `T3009b0` 已接回 caller-tail 后，新增 focused fixture，直接观察 escaped continuation `resume()` 返回点可见的 outer-slot 写回。
- 已检查现有相关 fixture：
  - `effect_escape_continuation_outer_var_writeback_basic.scoop` 只覆盖第一次离开 handle 时 body/arm/finally 对 outer slot 的写回。
  - `effect_escape_continuation_resume_unit.scoop` 等 fixture 覆盖 resume caller-tail 已接回，但没有直接观察 outer local 在 `resume()` 返回点的可见值。
- 当前判断：`T3009b0a1b` 可以在本轮完整完成，预计不需要新的生产代码修改，重点是补 focused fixture 并完成定向验证。
- 下一步：
  1. 新增一个 focused run-pass fixture，要求输出明确覆盖“初次离开 handle 时旧值仍在；`resume()` 返回后新值可见”。
  2. 运行该 fixture，并补跑 `cargo test --all` 与 `cargo clippy --all-targets -- -D warnings`。
  3. 更新 `TODO.md` / `PLAN.md` / 本文件并提交，仅完成这一项后停止。

## 当前进展

- 已新增 fixture：
  - `tests/fixtures/run-pass/effect_escape_continuation_resume_outer_var_writeback.scoop`
  - `tests/fixtures/run-pass/effect_escape_continuation_resume_outer_var_writeback.stdout`
- fixture 语义：
  - `outerValue` 仅在 `Suspend.pause()` 返回后改写，因此 `handle` 初次离开时调用点仍可见旧值 `5`。
  - `k.resume(41)` 返回后再次在调用点打印 `outerValue`，应可见新值 `42`。
- 已完成验证：
  - `cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/effect_escape_continuation_resume_outer_var_writeback.scoop`
  - `cargo test --all`
  - `cargo clippy --all-targets -- -D warnings`
- 验证结果：
  - 定向 fixture 输出为 `after_handle / 5 / after_resume / 42 / done`，符合任务要求。
  - `cargo test --all` 通过。
  - `cargo clippy --all-targets -- -D warnings` 通过。
- 已更新 `TODO.md`：`T3009b0a1b` 标记为完成。
- 已更新 `PLAN.md`：新增本轮完成记录，并将当前执行顺序前移到 `T3009b0aR`。

## 剩余收尾步骤

1. 检查工作区 diff，确认本轮只包含 fixture、计划与任务状态更新。
2. 创建 git 提交，提交信息使用任务号 `T3009b0a1b`。
3. 提交后停止，不进入下一个任务。
