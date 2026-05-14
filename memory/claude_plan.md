## 当前执行计划

1. 读取 `TODO.md`，确认按顺序排列的第一个未标记 `[DONE]` 的任务。
2. 检查最近提交记录，判断是否存在与该任务直接相关且明确未完成的问题；如果有，则按要求将其视为当前任务的一部分或在 `TODO.md` 中补充为前置依赖。
3. 阅读当前任务涉及的代码、文档与测试，确认实现边界、依赖关系和验证要求。
4. 如任务可直接完成，则进行最小且正确的实现；如遇到阻塞当前任务的真实缺陷或缺失特性，则先修复，或在 `TODO.md` 中新增最小前置任务并停止。
5. 运行与当前任务直接相关的测试、必要的仓库级检查，以及任务要求的验证命令；若失败则立即修复后重试。
6. 更新 `memory/claude_plan.md` 记录关键进展与计划变化。
7. 在 `TODO.md` 中将当前任务标记为 `[DONE]` 并填写完成记录；仅当阶段计划发生变化时才更新 `PLAN.md`。
8. 按仓库提交规范创建一次 git 提交，提交当前任务涉及的全部未提交更改，然后停止，不继续处理下一项任务。

## 进度记录

- 已写入初始计划。
- 已读取 `TODO.md` 并确认首个未完成任务为 `P4-T01b`：`vtable / itable 收集统一从 struct/class body method 抽，interface call ABI 收口为 metadata-driven marshal`。
- 已检查最近提交：`[P4-T01a] Enable ordinary struct/class member calls`，提交正文未额外标注与 `P4-T01b` 直接相关的未完成 issue。
- 已发现工作区存在与 `P4-T01b` 高度相关的未提交改动与新 fixture，需要先审阅这些改动，判断是否为上次未完成执行留下的续作，并在本次完成任务时一并纳入提交。
- 已确认 struct/class body method 的 vtable / itable 收集、value-box itable 直接指向 user struct body method symbol、interface caller side metadata-driven marshal、新增 run-pass/LLVM fixture 草稿都已在工作区实现，但 generic class interface dispatch 仍缺少 concrete member instance 发布。
- 已补齐 materializer 对 dispatch-only generic member instances 的显式索引：改为从 lowered 顶层函数首参识别 receiver owner，并接受 value/ref nominal；这样 generic class 的 interface dispatch 可以把 `Box.m::<Int>` / `Box.m::<String>` 编入初始实例请求并发布 concrete MIR body。
- 已新增/确认回归覆盖：
  - `tests/fixtures/run-pass/member_call_interface_dispatch_struct_value_box_basic.scoop`
  - `tests/fixtures/run-pass/member_call_interface_dispatch_class_body_method_basic.scoop`
  - `tests/fixtures/run-pass/member_call_interface_dispatch_generic_class_body_method_basic.scoop`
  - `crates/scoopc/src/llvm/tests.rs::value_box_interface_itable_points_directly_to_struct_body_method_symbol`
  - `crates/scoopc/src/mir/materialize.rs::materialize_for_dump_publishes_generic_interface_dispatch_member_instances`
- 已完成验证：
  - `cargo fmt --all`
  - `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/member_call_interface_dispatch_struct_value_box_basic.scoop`
  - `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/member_call_interface_dispatch_class_body_method_basic.scoop`
  - `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/member_call_interface_dispatch_generic_class_body_method_basic.scoop`
  - `cargo test -p scoopc value_box_interface_itable_points_directly_to_struct_body_method_symbol -- --nocapture`
  - `cargo test -p scoopc materialize_for_dump_publishes_generic_interface_dispatch_member_instances -- --nocapture`
  - `cargo test -p scoopc llvm_tests -- --nocapture`（当前 harness 仍返回 `0 passed; 850 filtered out`，因此继续以上述 owner tests 作为实际 LLVM 覆盖）
  - `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass`（当前仍只有两个既存失败：`extern_native_aggregate_return_direct_indirect_parity.scoop`、`sync_gc_release_task_like_object_basic.scoop`；与本任务代码路径无新增回归）
  - `cargo clippy --all-targets -- -D warnings`
- 当前剩余步骤：回写 `TODO.md` 完成记录并提交；`PLAN.md` 无需更新。
- 已将 `TODO.md` 中的 `P4-T01b` 标记为 `[DONE]`，并补齐改动范围、核心决策、验证结果及与 `PLAN.md` / `MANAGED_ABI.md` 的闭合说明。
- 当前仅剩：检查最终 diff、按仓库规范提交本任务并停止。

## 当前任务细化计划（P4-T01b）

1. 审阅 `itable` / `vtable` / interface call lowering / MIR value box 相关代码与当前未提交改动，确认已有实现状态与缺口。
2. 检查新增 fixture 与现有 LLVM 测试草稿，确认它们意图覆盖的行为是否正好对应 `P4-T01b` 的四项必做内容。
3. 以最小正确改动补齐以下能力：
   - struct/class body method 进入 vtable / itable 收集；
   - interface caller side 按 slot 元数据做 metadata-driven marshal；
   - value-type interface dispatch 不再依赖合成 box thunk；
   - generic class interface impl 的 monomorphized metadata 完整落地。
4. 运行任务要求的 fixture、相关 LLVM 锁定测试，以及 `cargo clippy --all-targets -- -D warnings`；若出现失败，先修复再继续。
5. 完成后更新 `TODO.md` 的 `P4-T01b` 状态与完成记录；若阶段级计划未变化，不改 `PLAN.md`。
6. 提交当前任务涉及的全部未提交更改并停止。
