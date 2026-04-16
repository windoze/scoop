# 本轮执行计划（可核查摘要）

## 约束与目标

- 本轮只处理 `TODO.md` 中**第一个未完成任务**，完成后停止。
- 在动手前先检查最新提交是否提到遗留问题；若有，先修复这些问题，再继续任务流程。
- 如果首个未完成任务过大，需要先拆分，并同步更新 `PLAN.md` 与 `TODO.md`。
- 需要在实现后完成测试、文档更新，并提交 Git commit。
- 过程中的关键进展或计划变更，需要继续更新本文件。

## 执行思路摘要

我会先确认当前仓库状态与最近一次提交信息，判断是否已经明确指出需要优先修复的现存问题。随后读取 `TODO.md` 和 `PLAN.md`，识别当前最高优先级的未完成事项，并判断其复杂度与依赖。

如果该任务可以在本轮完整落地，我会直接实现、补测试、运行必要的格式化/检查/测试命令，并在 `TODO.md` / `PLAN.md` 中记录完成情况，最后提交一次清晰的 commit。

如果发现阻塞项、规范缺口、已有实现与规范不符，或者任务体量过大，我会先把缺口转成明确的前置任务或子任务，调整 `TODO.md` 的顺序与依赖，并在 `PLAN.md` 和本文件中记录原因，然后提交并停止。

## 分步计划

1. 检查最新一次提交信息，确认是否包含需要先修复的遗留问题。
2. 读取 `TODO.md` 与 `PLAN.md`，定位第一个未完成任务并判断是否需要拆分。
3. 如需拆分，先更新 `PLAN.md` 与 `TODO.md`，重新确定本轮实际执行的第一个子任务。
4. 阅读相关代码、测试与规范上下文，确认实现边界与潜在依赖。
5. 实现本轮任务所需代码修改，必要时补充或重构相关测试。
6. 运行相关验证：
   - `cargo fmt`
   - `cargo test --all`
   - `cargo clippy --all-targets -- -D warnings`
   - 以及与任务直接相关的更小粒度测试（如适用）
7. 更新 `TODO.md`、`PLAN.md` 与本文件，记录完成情况或阻塞原因。
8. 提交 Git commit，然后停止，不继续下一个任务。

## 风险检查点

- 如果最新提交提到的问题尚未修复，必须先处理，不能跳过。
- 如果任务依赖缺失的语言特性、运行时能力、诊断能力或标准库支持，不能绕过，必须先在 `TODO.md` 中补前置任务。
- 如果测试暴露出既有缺陷，也属于本轮范围，需要先修复再继续。

## 进度

- 已完成：初始化本轮计划文件。
- 已完成：检查最新提交 `cd4c08d206f7025dc7dc5b3fffbcd72400b7b9f6`，提交信息为 `[T3009a1] Add immediate-resume contract prerequisite`；提交本身未携带额外正文，但它显式把 immediate-resume 合同收紧列为前置问题。
- 已完成：读取 `TODO.md` / `PLAN.md`，确认当前首个未完成任务是 `T3009a1`。
- 已完成：复现当前失败：
  - 命令：`cargo run -p scoop --features llvm -- build tests/fixtures/run-pass/effect_resume_double_resume_exit.scoop -o /tmp/t3009a1_double_resume`
  - 结果：在 codegen 阶段报 `unsupported_main_body: unknown local value`
- 已完成：定位根因：
  - `T3009a` 只为 tail-position 的 `resume(value)` 提供了 dedicated lowering；
  - immediate-resume arm 中更早出现的 `resume(...)` 仍按普通局部函数调用流入 generic 路径；
  - `typecheck` 当前把 `resume` 注入为普通函数类型，且返回类型仍是 `Unit`，与实际“不返回到 arm body”的语义不一致。
- 已完成：查证规范口径：
  - `SCOOP_FULL_SPEC.md:694` 明确要求：`resume(value)` 在 `-> resume` arm 中必须 **exactly once**；
  - continuation one-shot 语义只适用于 `k.resume(value)`（`SCOOP_FULL_SPEC.md:714`），不适用于 immediate-resume。
- 已确定的实现方案：
  1. 在 `typecheck` 中为 `-> resume` arm 增加结构校验：每条控制流路径都必须且只能在尾值位置出现一次特殊的 `resume(value)`。
  2. 将注入的 `resume` 类型从 `(T) -> Unit` 收紧为 `(T) -> Nothing`，使控制流建模与语义一致。
  3. 增加最小单测，覆盖：
     - 非 tail `resume(...)` 被拒绝；
     - 多次 `resume(...)` 被拒绝；
     - `if/else` 分支尾部各一次 `resume(...)` 仍被接受。
  4. 更新 `effect_resume_double_resume_exit.scoop` 的说明文字，并补一个 typecheck fixture 锁定稳定诊断。
- 已完成：实现 typecheck 合同校验与测试。
  - `crates/scoopc/src/typecheck/expr/infer.rs` 已新增 immediate-resume 路径校验逻辑：
    - tail `resume(value)` 合法；
    - block/if/when 的尾值路径按结构递归校验；
    - 非 tail / 多次 `resume(...)` 会在 typecheck 阶段报错，不再落到 codegen 的 generic local-call。
  - `resume` 的注入类型已从 `(T) -> Unit` 收紧为 `(T) -> Nothing`。
  - `crates/scoopc/src/typecheck/expr/error.rs` 已新增稳定诊断：
    - `scoop::typecheck::immediate_resume_arm_resume_not_tail`
    - `scoop::typecheck::immediate_resume_arm_resume_missing`
- 已完成：新增并通过最小定向测试。
  - Rust 单测：
    - `immediate_resume_arm_rejects_resume_before_tail`
    - `immediate_resume_arm_rejects_double_resume_in_same_path`
    - `immediate_resume_arm_allows_if_else_tail_resume`
  - 新增 typecheck fixture：
    - `tests/fixtures/typecheck/immediate_resume_double_resume_is_error.scoop`
  - 历史 run-pass fixture：
    - `tests/fixtures/run-pass/effect_resume_double_resume_exit.scoop` 已改为保留“失败即通过”的历史 CLI 路径说明，不再假定运行期 one-shot。
- 已完成：定向验证与质量门槛。
  - `cargo test -p scoopc immediate_resume_arm -- --nocapture`：通过
  - `cargo run -p scoop --features llvm -- build tests/fixtures/run-pass/effect_resume_double_resume_exit.scoop -o /tmp/t3009a1_double_resume`：现在稳定报 `scoop::typecheck::immediate_resume_arm_resume_not_tail`
  - `cargo test -p scoopc`：通过
  - `cargo clippy --all-targets -- -D warnings`：通过
  - `cargo fmt`：通过
  - 最小 fixture runner 验证：
    - 单独运行新增 typecheck fixture：通过
    - 单独运行历史 run-pass fixture：通过
- 已确认的非本轮阻塞：
  - 补跑全量 `cargo run -p scoop --features llvm -- test` 时，会卡在仓库已知用例 `effect_custom_nonresuming_nested_nearest_and_arm_outside_scope.scoop`；
  - `TODO.md` / `PLAN.md` 已将该挂起问题归入 `T3014/T3017`，与本轮 immediate-resume 合同收紧无直接交集。
- 已完成：更新 `TODO.md` / `PLAN.md`，将 `T3009a1` 标记为完成，并把下一步切回 `T3009aR`。
- 进行中：准备整理工作区并提交本轮 commit，然后停止。
