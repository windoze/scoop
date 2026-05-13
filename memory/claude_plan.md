## 当前执行计划

1. 先读取 `TODO.md`，严格按标题是否带有 `[DONE]` 判断完成状态，定位第一条未完成任务。
2. 查看最近提交信息，确认是否有与该任务直接相关且明确未完成的事项；若存在且会影响当前任务，则将其视为当前任务范围或在 `TODO.md` 中补充为前置依赖。
3. 阅读当前任务所需的最小相关上下文，包括实现代码、测试、任务依赖说明与完成记录要求；避免做开放式历史问题排查。
4. 判断是否存在阻塞当前任务的真实缺口、回归或规格不匹配：
   - 若不存在阻塞，直接完整实现当前任务。
   - 若存在阻塞且无法在本次一并修复，则仅新增最小必要前置任务到 `TODO.md`，保持当前任务未完成，并在需要时更新 `PLAN.md`。
5. 对实现进行验证，优先运行任务要求的测试，并补充必要的相关检查；如果当前任务落地涉及通用质量门槛，则运行 `cargo fmt`、相关测试，以及 `cargo clippy --all-targets -- -D warnings`（若范围合理且与变更相关）。
6. 完成后更新文档：
   - 在 `TODO.md` 中将当前任务标题显式改为带 `[DONE]`。
   - 更新该任务 completion record。
   - 仅在阶段计划或依赖结构变化时更新 `PLAN.md`。
7. 将本次任务相关的所有未提交变更一并提交，提交信息使用当前任务 ID，随后停止，不继续下一项任务。

## 计划更新规则

- 如果在执行中发现阻塞、需要新增前置任务、或验证步骤发生变化，我会及时更新本文件。
- 如果完成关键里程碑（定位任务、完成实现、完成验证、完成文档更新、完成提交），我会在本文件中补充当前状态。

## 当前状态更新

- 已读取 `TODO.md`，第一条未完成任务为 `P7-T01C：收口剩余 RTTI / runtime-match type_id 对 pretty text / sanitize 的依赖`。
- 最新提交为 `[P7-T01R] Add RTTI type-id prerequisite`，与当前任务直接相关；说明 `P7-T01C` 是在最终 review 前新增的必要前置任务，应优先完成。
- 下一步将只阅读该任务直接涉及的实现入口：
  - `crates/scoopc/src/rtti/mod.rs`
  - `crates/scoopc/src/llvm/codegen/mod.rs`
  - `crates/scoopc/src/llvm/codegen/mir_body.rs`
  - `crates/scoopc/src/llvm/codegen/gc.rs`
  - `crates/scoopc/src/stable_id.rs`
  - 以及相关测试/审计入口。

- 实现进展：已完成代码改造，当前改动包括：
  - 在 `stable_id.rs` 新增 RTTI canonical type key / derived descriptor key helper。
  - RTTI 查询、itable precise runtime-match、MIR value-box itable 已改为基于 canonical type key 生成 `type_id`。
  - `TypeDescriptorSpec` 已显式改为 `type_id_key`，boxed enum / MIR capture-box / MIR value-box descriptor 已不再把 display/sanitize 文本直接喂给 `stable_rtti_type_id(...)`。
  - 已补充 source inventory 与 RTTI dump 回归测试，覆盖“可读名不再直接等于 type-id hash 输入”。
- 下一步：运行 `cargo fmt`，再按任务要求执行定向测试、全量测试与 `clippy -D warnings`，若发现失败则继续修复后再更新 `TODO.md`/提交。

- 验证进展：`cargo fmt`、4 组定向测试、`cargo test -p scoopc`、`cargo clippy -p scoopc --all-targets -- -D warnings` 均已通过。
- 文档进展：已在 `TODO.md` 将 `P7-T01C` 标记为 `[DONE]` 并补齐完成记录；`PLAN.md` 无需更新。
- 剩余步骤：检查提交面、创建 `[P7-T01C] ...` 提交、记录提交完成并停止。
