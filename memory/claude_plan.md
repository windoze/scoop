# 本次执行计划

1. 读取 `TODO.md` 作为索引，并按引用顺序检查对应的 `TODO-Px.md` 文件，定位第一个标题未标记 `[DONE]` 的详细任务。
2. 检查最新一次提交信息，确认是否存在与该任务直接相关且尚未完成的问题；若有，则将其视为当前任务的一部分或必要前置。
3. 阅读当前任务涉及的代码、测试、规范与依赖位置，确认实现边界与验收条件。
4. 实现当前任务所需改动；若遇到阻塞当前任务且无法在本次内直接修复的问题，则按要求在对应 `TODO-Px.md` 中补充最小前置任务，并同步 `TODO.md` 后停止。
5. 运行与当前任务直接相关的测试与必要的质量检查，修复发现的问题，直到当前任务满足要求或被前置问题阻塞。
6. 更新任务文档：在对应 `TODO-Px.md` 中将当前任务标题标记为 `[DONE]` 并补充完成记录；如任务索引发生变化则同步更新 `TODO.md`；仅在阶段计划变化时更新 `PLAN.md`。
7. 检查工作区改动，按任务要求创建一次提交，然后停止，不继续处理下一个任务。

## 进度记录

- 已创建本文件并写入初始计划。
- 已读取 `TODO.md` 与 `TODO-P6.md`，确认首个未完成详细任务为 `P6-T02i`。
- 已检查最新提交 `[P6-T02i] Track synthetic carrier ABI-lowering prerequisite`，确认它只登记了当前前置任务，没有实现代码；因此本次需要直接完成 `P6-T02i`。
- 已阅读 `crates/scoopc/src/llvm/codegen/effect_refactor/{layout,types}.rs` 与相关测试，确认当前缺口是：`RefactorAbiQuery` 只发布了 entry/layout 签名，没有把 late-lowered source type 如何映射为 ABI value 的稳定查询面公开给后续 body emitter。
- 当前实现计划已细化为：
  1. 在 `effect_refactor/types.rs` 中新增按 late-lowered `TypeId` 发布的 source-type ABI value layout/query 结构，覆盖标量、`Unit` 与 tuple carrier 的 field/index 映射。
  2. 在 `effect_refactor/layout.rs` 的 ABI materializer 中收集并发布当前任务要求的 source type contract：`invoke_args_tuple_ty`、resume payload/answer、step complete/case payload、local runtime-error payload 等。
  3. 为 `Step` variant 增补 source payload type 元数据，使后续 `P6-T03` 可以只靠 ABI query 回查 payload lowering contract，而不必再猜 shape 或回塞 legacy `TypeStore`。
  4. 增加定向测试，覆盖：single-value invoke carrier、tuple resume payload/source layout，以及缺失/不可 lowering 的 source type 会在 ABI materialization 阶段显式 fail fast。
- 已完成代码改动：
  - 在 `crates/scoopc/src/llvm/codegen/effect_refactor/types.rs` 中新增 `RefactorSourceAbiLayout*` 查询结构，并把它挂到 `RefactorAbiQuery`。
  - 在 `crates/scoopc/src/llvm/codegen/effect_refactor/layout.rs` 中把 callable entry、surface resume、resume method、step variant、local runtime-error contract 等全部改为发布/消费同一套 source-type ABI contract；同时为 `Step` variant 补充 `payload_source_ty` 元数据。
  - 已新增 `refactor_llvm_layout_*` 定向测试，覆盖 direct-entry invoke carrier、unit case payload、tuple resume payload/answer，以及不可 lowering 的 synthetic invoke args fail-fast。
- 已完成验证：
  - `cargo fmt --all`
  - `cargo test -p scoopc refactor_llvm_layout`
  - `cargo run -p scoop -- --effect-pipeline refactor test --fixtures tests/fixtures/build/effect_refactor_dynamic_entry_publication_emit_llvm.scoop`
  - `cargo clippy -p scoopc -p scoop --all-targets -- -D warnings`
- 下一步只剩任务文档回写、提交并停止。
