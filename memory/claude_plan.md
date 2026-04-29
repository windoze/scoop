# Claude Plan

## Constraints
- 先检查最新提交是否提到需要先修复的既有问题；若有，优先处理。
- 只处理 `TODO.md` 中第一个未完成任务；若该任务过大，则先拆分并更新 `PLAN.md`/`TODO.md`。
- 不采用变通方案；若遇到阻塞性的既有缺陷或缺失特性，需要先修复，或把前置任务插入到 `TODO.md` 当前任务之前后停止。
- 在实现后运行相关测试，并尽量验证无警告要求是否满足。
- 完成后更新 `TODO.md`、`PLAN.md`、本文件，并按仓库风格提交一次，然后停止。

## Initial Execution Plan
1. 检查最新一次 git 提交的信息与改动，确认是否显式提到需要先修复的既有问题。
2. 阅读 `TODO.md` 与 `PLAN.md`，定位第一个未完成任务，并判断是否需要拆分。
3. 若需要拆分，更新 `PLAN.md`、`TODO.md`，并将当前迭代任务收敛为可独立完成的首个子任务。
4. 阅读与当前任务直接相关的代码、测试与规范上下文，确认实现边界与潜在前置缺陷。
5. 先修复发现的任何阻塞性既有问题；否则直接实现当前任务。
6. 运行与改动相关的测试；若任务范围合理，再补充格式化、clippy 或更广验证。
7. 更新 `TODO.md`、`PLAN.md` 与本文件的进度记录。
8. 按仓库提交风格创建一次 git commit，然后停止。

## Progress Log
- 已创建计划文件，待开始仓库检查。
- 已检查最新提交 `241a8e0 [T5001d2] Emit explicit root frame activation lifecycle`；提交说明未显式提到需要优先修复的既有问题。
- 已阅读 `TODO.md` 与 `PLAN.md`；当前第一个未完成任务为 `T5001d2R Review：确认 frame 生命周期与 NULL discipline 成立`。
- 当前执行策略：围绕 explicit frame lifecycle 做针对性代码审查与必要验证，重点检查 entry-block alloca、所有退出路径 pop，以及 dead/inactive slot 清零；若发现缺陷，先修复缺陷再决定是否能将 `T5001d2R` 标记完成。
- Review 发现一项需先修复的问题：`emit_explicit_root_frame_return_pops(...)` 仅覆盖 `ret` 终结点，但共享返回块在 `CgTy::Never` 下会发射 `unreachable`，从而漏掉 explicit frame 的 slot 清零与 TLS pop。这属于 `T5001d2` 现有生命周期实现的 correctness 缺口，必须先修复并补回归，再继续完成 `T5001d2R`。
- 已修复上述缺口：teardown 现同时覆盖 `ret` 与 `unreachable` 终结点，并补充 LLVM 回归 `never_returning_managed_function_pops_explicit_root_frame_before_unreachable`。
- 已完成验证：
  - `cargo test -p scoopc --lib managed_function_emits_explicit_root_frame_tls_lifecycle_and_slot_clear`
  - `cargo test -p scoopc --lib never_returning_managed_function_pops_explicit_root_frame_before_unreachable`
  - `cargo test -p scoopc --lib`
  - `cargo run -p scoop -- test --fixtures tests/fixtures/build`
  - `cargo clippy --all-targets -- -D warnings`
- 已回写 `TODO.md` 与 `PLAN.md`，将 `T5001d2R` 标记完成并记录 review 结论。
