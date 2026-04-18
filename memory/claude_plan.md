# 执行计划

说明：我不能提供或记录完整的私有思维链，但会在这里持续维护可核查的执行计划、关键判断依据、当前进度与变更记录。

## 当前目标

按 `TODO.md` 的顺序完成第一个未完成任务；如果发现前置阻塞或最新提交中提到的遗留问题，先处理这些问题，再继续当前任务。完成后更新计划与任务文档、运行相关测试、提交 Git commit，然后停止。

## 初始步骤

1. 检查最新一次 Git 提交，确认是否明确提到已知问题、回归、未完成修复或需要立即处理的遗留事项。
2. 读取 `TODO.md`，定位第一个未完成任务。
3. 读取 `PLAN.md`，确认现有里程碑、依赖关系和任务上下文。
4. 根据任务涉及范围检查相关实现与测试，判断是否可以直接完成，或是否需要先细分为更小的子任务。
5. 如果任务过大或存在明确前置依赖：
   - 更新 `PLAN.md`；
   - 在 `TODO.md` 中把该任务拆分/重排为更小的可执行子任务；
   - 执行新的第一个子任务，然后停止。
6. 如果任务可直接完成：
   - 实现代码；
   - 添加/调整测试；
   - 运行格式化、测试与 lint（至少包括相关测试，以及 `cargo clippy --all-targets -- -D warnings`，若范围允许则运行更完整测试）；
   - 更新 `TODO.md` 与 `PLAN.md`；
   - 提交 Git commit；
   - 停止。

## 执行原则

- 不接受变通方案、兼容性假修复或仅靠夹具掩盖问题。
- 一旦发现规范不匹配、缺失特性或实现边界，会先把问题转化为 `TODO.md` 中的显式任务，并调整依赖顺序。
- 不会回退或覆盖仓库中与当前任务无关的现有改动。
- 所有关键步骤完成后都会更新本文件，便于追踪进展。

## 进度记录

- 已创建计划文件，待检查最新提交、`TODO.md` 和 `PLAN.md`。
- 已检查最新提交 `0736fed91a333f84051053f2e34de53bdacc6de4`（`[T3009b2R] Review indirect callee resumed-body caller-tail`）；提交信息本身未显式记录新的待修遗留问题。
- 已定位 `TODO.md` 中第一个未完成任务为 `T3009b`：在现有 dedicated lowering 基础上，继续收口 escaped continuation 的 composite resume payload（tuple / struct / boxed enum / continuation ref 等）。
- 当前判断：先运行 `T3009b` 验收夹具与相关测试，确认是否已经存在更前置的真实阻塞；若有，则按要求先更新 `TODO.md` / `PLAN.md` 重排依赖并停止；若没有，再直接实施修复。
- 已完成 `T3009b` 相关定向验证：
  - 通过：`continuation_resume_tuple.scoop`
  - 通过：`continuation_resume_struct.scoop`
  - 通过：`continuation_resume_struct_with_ref.scoop`
  - 通过：`continuation_resume_continuation.scoop`
  - 通过：`continuation_resume_enum.scoop`
  - 通过：`effect_escape_continuation_indirect_perform_resume_string.scoop`
  - 通过：`effect_escape_continuation_indirect_perform_resume_struct_with_ref.scoop`
  - 通过：`cargo test --all`
  - 通过：`cargo clippy --all-targets -- -D warnings`
- 串行复跑 `cargo run -p scoop --features llvm -- test` 后，当前首个停止点不是生产代码失败，而是 `tests/fixtures/run-pass/effect_escape_continuation_async_executor_fifo.scoop` 仍保留 `EXPECT: fail`，但该 fixture 单独运行已成功。这属于 `T3017` 已显式跟踪的 stale xfail 回收，不是 `T3009b` 新增 blocker。
- 当前执行决策：不新增前置任务；直接把 `T3009b` 作为已实现且已完成定向验收的任务收口，并在 `TODO.md` / `PLAN.md` 中记录全量 suite 目前由 `T3017` 挡住的真实状态，然后提交本轮文档更新。
- 已更新 `TODO.md`：`T3009b` 标记为 `[DONE]`，并记录 composite payload transport、定向 fixture、`cargo test --all` / `clippy` 验证结果；同时在 `T3017` 中补记当前首个 stale xfail 为 `effect_escape_continuation_async_executor_fifo.scoop`。
- 已更新 `PLAN.md`：新增 2026-04-18 本轮完成记录，说明 `T3009b` 已闭环、全量 suite 当前由 `T3017` expectation cleanup 挡住，并把当前执行顺序推进到 `T3009bR`。
- 已完成最小复核：`git diff --check` 通过，文档状态一致；下一步执行 Git commit，然后停止。
