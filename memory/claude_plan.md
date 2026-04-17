# 本轮执行计划

## 约束说明

- 按要求先写入本文件，再进行仓库检查与命令执行。
- 这里记录的是可公开的分析摘要、执行计划、进展和决策，不包含逐字内部推理。
- 本轮目标是：先处理最新提交中提到的既有问题（若存在），然后完成 `TODO.md` 中第一个未完成任务；若任务过大，则先拆分并更新 `PLAN.md` / `TODO.md`，随后只执行拆分后的第一个子任务。

## 初始步骤

1. 检查最新一次 git 提交的提交信息与变更，确认是否明确提到尚未解决的既有问题。
2. 读取 `TODO.md`，定位第一个未完成任务。
3. 读取 `PLAN.md`，理解当前计划、任务编号与依赖关系。
4. 判断该任务是否可在本轮完整落地：
   - 若可以，直接实现、补测试、运行验证、更新文档与任务状态、提交 git commit。
   - 若不可以，拆成更小的子任务，更新 `PLAN.md` 与 `TODO.md`，本轮只完成第一个子任务。
5. 在实现过程中如发现任何规范不匹配、缺失特性、回归或“只能靠绕过才能继续”的情况：
   - 先把该问题建成前置任务并调整 `TODO.md` 顺序；
   - 更新 `PLAN.md` 说明阻塞原因；
   - 如当前任务因此不能继续，则提交这些计划性改动后停止。
6. 完成实现后运行相关验证：
   - 至少运行与改动直接相关的测试；
   - 若改动触及公共编译/运行路径，再补 `cargo test --all` 或更小但充分的验证；
   - 运行 `cargo clippy --all-targets -- -D warnings`（若与当前任务范围相关且耗时可接受）。
7. 更新 `TODO.md`、`PLAN.md`、本文件，并创建一次清晰的 git 提交。

## 进展记录

- 已创建本文件，准备开始检查最新提交与任务列表。
- 已检查最新提交 `79485a39e3af09ae1c7d9bd6541febc84eaa597f`（`[T3009b0aR] Review outer-scope slot writeback contract`）：
  - 本次提交只更新了 `PLAN.md`、`TODO.md` 与本记录文件，没有新增生产代码修改。
  - 提交信息未明确提出一个尚未修复、且必须先于 `TODO.md` 第一项执行的额外代码问题。
- 已读取 `TODO.md` / `PLAN.md`，确认当前第一个未完成任务是 `T3009b0R`：
  - 任务类型：review。
  - 任务目标：确认 escaped continuation 的 `Continuation.resume(...)` scalar/ref dedicated lowering 在普通 call path 与 unified state-machine 路径中都已 dedicated 化，不再回落到 generic member access / generic call。
- 当前执行策略：
  1. 审查 `crates/scoopc/src/llvm/codegen/mod.rs` 中 call 分派是否以 `continuation_resume_call_sites` 为唯一 builtin 标记来源。
  2. 审查 `crates/scoopc/src/llvm/codegen/effect/mod.rs` 中 `codegen_continuation_resume_builtin` 是否只负责共享 ABI + payload transport，不夹带 escaped-continuation-only 特判。
  3. 审查 unified state-machine 相关模块是否仍通过统一 call 路径消费该 dedicated lowering，而不是在 emitter 中另起 generic fallback。
  4. 运行与 `Continuation.resume(...)` 直接相关的结构测试 / focused fixtures / 全量质量门槛，确认 review 结论成立。
- 已完成 `T3009b0R` 复审，结论如下：
  - `codegen_call` 仍以 `continuation_resume_call_sites` 为唯一 builtin 语义来源，并在普通 call lowering 最前面直接分派到 `codegen_continuation_resume_builtin`。
  - `codegen_member_access` 中不存在按成员名/receiver 类型识别 `Continuation.resume` 的旁路补丁；普通 member access 没有回流 effect-only 特判。
  - unified state-machine emitter 对相关 `HandleStateOp` 统一委托 `codegen_expr_in_expected_context`，最终仍走同一个 `codegen_call -> codegen_continuation_resume_builtin` 分派，因此 ordinary path 与 state-machine path 共用一套 dedicated lowering。
  - `codegen_continuation_resume_builtin` 仍只复用共享 continuation runtime ABI 与 `resume_word` / `resume_gc_ref` transport；composite payload 继续留给 `T3013` / `T3009b`，没有新增 continuation-only side channel。
- 本轮验证结果：
  - 通过：`cargo test -p scoopc continuation_resume_hidden_suspend_classification_requires_typechecked_call_site_marker -- --nocapture`
  - 通过：`cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/effect_escape_continuation_resume_unit.scoop`
  - 通过：`cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/effect_escape_continuation_resume_bool.scoop`
  - 通过：`cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/effect_escape_continuation_resume_string.scoop`
  - 通过：`cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/effect_resume_nested_escape_handle_tail.scoop`
  - 通过：`cargo test --all`
  - 通过：`cargo clippy --all-targets -- -D warnings`
- 下一步：
  - 更新 `TODO.md` / `PLAN.md`，将 `T3009b0R` 标记为完成，并把队列推进到 `T3010b2b1b`。
  - 提交本轮改动并停止，等待下一次调用。
