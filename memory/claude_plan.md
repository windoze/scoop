# 当前执行计划

说明：这里记录可执行的推理摘要、检查步骤、决策依据与进度更新，不记录不可审计的原始思维草稿。

## 初始目标

按仓库要求完成 `TODO.md` 中第一个未完成任务，并在完成后停止；如果途中发现更早的既有问题，则先修复该问题或将其整理为更前置的任务。

## 执行步骤

1. 检查最新一次 Git 提交信息，确认是否明确提到需要先修复的既有问题。
2. 读取 `TODO.md` 与 `PLAN.md`，定位第一个未完成任务，并核对依赖关系。
3. 如果该任务过大，先把它拆成更小的子任务，更新 `PLAN.md` 与 `TODO.md`，然后只执行拆分后的第一个子任务。
4. 阅读相关代码、测试、规格与最近变更，确认实现边界。
5. 实现该任务；若实现或测试中暴露出既有缺陷、规格不匹配、回归或不完整边界，则立即优先处理：
   - 能直接修复就先修复；
   - 若无法在本轮直接完成，则把缺陷作为当前任务的前置任务插入 `TODO.md`，更新 `PLAN.md`，提交后停止。
6. 运行与本任务直接相关的测试。
7. 在必要时扩展到更广的验证，至少包括：
   - `cargo fmt --check`
   - `cargo clippy --all-targets -- -D warnings`
   - 与改动相关的 `cargo test` / fixture 测试
8. 更新文档状态：
   - 在 `TODO.md` 中标记当前任务完成，或在阻塞场景下重排任务顺序；
   - 在 `PLAN.md` 中记录当前状态、依赖变化与后续顺序；
   - 继续同步本文件中的关键进展。
9. 检查工作区改动，确认只包含本轮需要的修改。
10. 提交一个清晰的 Git commit，然后停止，不继续做下一个任务。

## 初始检查点

- [ ] 查看最新提交说明
- [ ] 读取 `TODO.md`
- [ ] 读取 `PLAN.md`
- [ ] 确定当前首个未完成任务
- [ ] 判断是否需要拆分
- [ ] 完成实现
- [ ] 完成验证
- [ ] 更新 `TODO.md` / `PLAN.md`
- [ ] 提交 Git commit

## 进度日志

- 已创建本计划文件，下一步检查最新提交与任务列表。
- 已检查最新提交 `68b9952893546dbfc0e45a2f0b284b1daad4dd11`，提交信息未额外点名新的既有 issue；目前未发现需要先于 `TODO.md` 顺序插入的“最新提交自述 blocker”。
- 已读取 `TODO.md` / `PLAN.md`，当前首个需要执行的具体未完成条目为 `T4016T1d2`：补齐 generic helper / method body 内的 monomorph/type-param leak。
- 已确认 `T4016T1d1` 的现有 run-pass 仅覆盖 concrete-instance 窄化验收：`task_generic_state_object_model_basic.scoop` 仍使用 `driveInt(...)` 与 main 中直接 `lock.destroy()`，尚未覆盖 `fun <T> drive(...)`、`if (x is Box<T>) x.value`、`carrier.lock.destroy()` 这三条 `T4016T1d2` 验收路径。
- 下一步：
  1. 为这三条路径设计最小 probe，先复现当前失败形态；
  2. 阅读 HIR lowering / typecheck / LLVM codegen 中与 type-param 具体化、member access、monomorphized function/member 解析相关的代码；
  3. 修复后将 probe 升格为正式 regression，并执行全量验证。
- 已新增 run-pass 回归 `tests/fixtures/run-pass/task_generic_state_generic_helper_method_basic.scoop`，在一个最小用例里同时覆盖：
  - `fun <T> drive(carrier: TaskCarrier<T>, fallback: T): T`
  - generic method body 里的 `if (x is Box<T>) x.value`
  - generic receiver 字段上的 `carrier.lock.destroy()`
- 复现到的首个真实失败是 `carrier.lock.destroy()` 在 LLVM `sync.destroy` special-case 中仍只按旧逻辑读取 receiver 类型；已改为统一走 `resolve_expr_concrete_type(...)`。
- 进一步复现确认真正的前置根因是：单态化后的 generic fun/member/getter 重新做 HIR lowering 时没有复用 typecheck side table，导致依赖 smart-cast / late member resolution 的路径（例如 monomorphized `if (x is Box<T>) x.value`）退回到 base generic class `Box`，最终在 LLVM class field lookup 上再次看到 `field_ty = T`。
- 已完成实现修复：
  1. `LoweringInputs` 现可携带 `typecheck_types`；
  2. `lower_fun_with_type_bindings` / `lower_member_fun_with_type_bindings` / `lower_value_property_getter_with_type_bindings` 会把这份 side table 传入 `HirLoweringSetup`；
  3. `collect_generic_fun_instantiations` 与 `collect_generic_member_fun_instantiations` 现会在 compilation-unit lowering 路径上传递 `Some(typecheck_types)`，而 dump / pre-specialize / monomorph dump 路径继续显式传 `None`；
  4. 新 fixture 已可成功 build，产物运行退出码为 `7`。
- 当前进入正式验证阶段：`cargo fmt --check`、相关 fixture suite、`cargo test --all`、`cargo clippy --all-targets -- -D warnings`。
- 已完成正式验证：
  - `cargo fmt --check`
  - `cargo run -p scoop -- build tests/fixtures/run-pass/task_generic_state_generic_helper_method_basic.scoop -o /tmp/task_generic_state_generic_helper_method_basic.out`
  - `/tmp/task_generic_state_generic_helper_method_basic.out`（退出码 `7`）
  - `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass`（`fixtures: ok (387)`）
  - `cargo run -p scoop -- test`（`fixtures: ok (1157)`；过程中有既有告警日志，但命令整体通过）
  - `cargo test --all`
  - `cargo clippy --all-targets -- -D warnings`
- 已更新 `TODO.md` / `PLAN.md`：`T4016T1d2` 标记为完成，当前下一任务已前移为 `T4016T2`。
- 当前剩余步骤：
  1. 检查最终工作区改动；
  2. 提交 Git commit；
  3. 本轮停止，不继续执行 `T4016T2`。
