## 本次执行计划

1. 先读取 `TODO.md`，确认按标题是否带有 `[DONE]` 判断的首个未完成任务。
2. 检查最近一次提交是否明确提到与该任务直接相关且尚未完成的问题；若存在且会阻塞当前任务，则先把它视为当前任务的一部分或在 `TODO.md` 中补成前置任务。
3. 阅读当前任务涉及的代码、测试、规范和依赖项，确认需要修改的最小范围。
4. 实现当前任务，避免绕过已有缺陷；若遇到阻塞当前任务的真实缺口，则先在 `TODO.md` 中补最小前置任务并停止在该前置整理处。
5. 运行当前任务要求的验证，以及必要的回归测试、`cargo fmt`、`cargo test`、`cargo clippy --all-targets -- -D warnings`（若与当前任务相关且可在合理时间内完成）。
6. 更新 `TODO.md`：仅当任务真正完成时，给任务标题加上 `[DONE]` 并填写完成记录；若只是发现阻塞，则保持未完成并补充前置任务与依赖关系。
7. 仅在阶段级计划或依赖关系发生变化时更新 `PLAN.md`；否则不改。
8. 检查工作区变更，保留不属于我的现有修改；按要求创建一次清晰的 git 提交，然后停止。

## 执行原则摘要

- 以 `TODO.md` 为唯一任务顺序与完成状态来源。
- 一次只完成一个任务；除非当前任务被具体前置问题阻塞，否则不拆分。
- 不用变通方案规避规格缺口；若遇阻塞，先把缺口显式写入 `TODO.md`。
- 只做解决当前任务所需的最小正确改动。

## 进度记录

- 已创建本计划文件，下一步读取 `TODO.md` 并定位首个未完成任务。
- 已读取 `TODO.md` 任务索引，确认首个未完成任务为 `CG-T07S0a5`：修复 `list_and_mutable_list_basic` 中 `MutableList.add/push` materialized MIR 的 array transport element type 残留 unresolved generic `T`。
- 已检查最近一次提交信息：`[CG-T07S0a4] Fix progression assign local rebind and record list blocker`。其中 “record list blocker” 与 `CG-T07S0a5` 直接相关，因此后续会把该未竟项纳入当前任务上下文，先核对任务正文与相关代码，再决定是否存在必须先补写到 `TODO.md` 的新前置问题。
- 已读取 `CG-T07S0a5` 正文并单独复现：`cargo run -p scoop -- build tests/fixtures/run-pass/list_and_mutable_list_basic.scoop -o /tmp/list_and_mutable_list_basic` 稳定报 `materialized MIR 'scoop.core.push' contains unresolved generic parameter in array transport element type ...: T`。
- 当前判断：优先检查 `crates/scoopc/src/mir/lower.rs` 的 array transport metadata 生成，以及 `crates/scoopc/src/mir/materialize.rs` 的 repair/substitution 路径，确认 `scoop.core.push` 的 materialized body 为什么没有把 element type 刷新成具体类型。
- 进一步判断：`BuilderPush` metadata 在 lowering 阶段把 `array_ty` 记为 builder 的 `Any`，materialization 后现有 repair 逻辑只能在 `array_ty` 与 `element_ty` 已经 concrete 时原样保留，因此像 `this.get(i)` 这类先经 generic `get` 再进入 builder append 的路径，可能在 call result local 已被修正为 `Int` 后仍错过 `array.element_ty` 刷新。
- 计划中的最小修复：在 `repair_array_call_transport_types` 中为 `BuilderPush` 改为优先从第二个实参 operand/local 的 concrete type 回填 element type，并同步刷新 value transport contract；同时补一个 production frontend 回归测试，直接检查 `list_and_mutable_list_basic` materialized 出来的 `scoop.core.push` body 不再携带 unresolved generic `T`。
- 已完成代码修复：`crates/scoopc/src/mir/materialize.rs` 的 `repair_array_call_transport_types()` 现为 `BuilderPush` 优先从第二个调用实参的 concrete operand/local type 回填 `array.element_ty`，修复 `MutableList<Int>.add` / `MutableArray<Int>.push` 经 generic `get` 后 append 时残留 template `T` 的问题。
- 已完成回归测试：`crates/scoopc/src/llvm/tests.rs` 新增 `production_codegen_list_fixture_materializes_mutable_list_add_and_push_instances`，通过 production `materialized_pass_view()` 验证 list fixture 会保留 `add`/`push` 实例，且 `push` body 的 builder append transport element type 已具体化为 `Int`。
- 已完成定向验证：`cargo test -p scoopc production_codegen_list_fixture_materializes_mutable_list_add_and_push_instances`、`cargo run -p scoop -- build tests/fixtures/run-pass/list_and_mutable_list_basic.scoop -o /tmp/list_and_mutable_list_basic`、`cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/list_and_mutable_list_basic.scoop`、`cargo clippy --all-targets -- -D warnings` 均通过。
- 默认 full-suite 状态已更新：`cargo run -p scoop -- test` 不再停在 `list_and_mutable_list_basic.scoop`，而是继续暴露 `tests/fixtures/run-pass/literal_numeric_expected_type_absorption_basic.scoop` 的 stdout mismatch；已在 `TODO.md` 中把 `CG-T07S0a5` 标记为 `[DONE]`，并新增前置任务 `CG-T07S0a6` 记录下一处 blocker。
- 收尾计划：检查最终 diff 与 git 状态，确认只包含本次任务及 required bookkeeping，然后创建一次提交并停止。
