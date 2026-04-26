# 执行计划

## 约束说明

- 按要求先写入本文件，再执行仓库检查、构建或测试命令。
- 我不会写入逐字展开的私有推理，但会持续记录可审计的执行计划、判断依据、关键发现与后续动作。
- 本次调用的目标是：先处理最新提交中提到的既有问题（如果有），再读取 `TODO.md`，完成其中第一个未完成任务，然后更新计划文件、任务文件并提交 Git，之后停止。

## 初始步骤

1. 查看最新一次 Git 提交的提交信息与改动说明，确认是否点名了任何已知问题、回归、临时方案或待修复项。
2. 读取 `TODO.md` 与 `PLAN.md`，识别当前排在最前面的未完成任务，并核对它是否依赖尚未修复的问题。
3. 如任务过大，则先把它拆分成更小的可执行子任务，更新 `PLAN.md` 与 `TODO.md`，并仅执行拆分后的第一个子任务。
4. 在实现过程中，若发现任何现存缺陷、规格不匹配或实现边界缺口，优先修复；如果无法在本轮直接修复，则把该问题作为前置任务插入 `TODO.md` 的正确位置，更新 `PLAN.md` 后提交并停止。
5. 对本轮目标任务做完整实现与相关测试，包括必要的格式化、静态检查、单测、集成测试或 fixture 测试。
6. 更新 `TODO.md`、`PLAN.md` 与本文件，记录完成状态、调整原因和验证结果。
7. 使用清晰的 Git 提交信息提交本轮变更，然后停止，不继续下一个任务。

## 进度记录

- 已完成：创建本计划文件并写入初始执行方案。
- 已完成：检查最新提交 `d5e1bbc6dee7dacf1930e7ee430d995ef992d637` 的提交信息；提交信息本身未点名新的既有问题或待先修复事项。
- 已完成：读取 `TODO.md` / `PLAN.md`，确认第一个未完成任务是 `T5000e2cR Review：确认 build/frontend 主路径已切到 MIR instance collection`。
- 进行中：针对 `T5000e2cR` 做实现审查与验证。

## 当前任务：T5000e2cR

### 审查目标

- 确认 `build` / 单文件 LLVM frontend 主路径是否已经稳定消费 MIR instance collection，而不是继续依赖 HIR eager materialization 主路径。
- 确认 `dump-ir` / build / 单文件 frontend 三条路径在实例收集、声明源身份、owner/nominal specialization、effect-row 实参等维度上没有重新分叉。
- 审查最新提交引入的代码路径是否存在回归、遗漏的调用面或只在测试路径成立的边界问题。

### 计划步骤

1. 查看最新提交中与 `T5000e2c` 直接相关的代码 diff 与测试变更，锁定关键文件。
2. 读取 `TODO.md` / `PLAN.md` 中 `T5000e2c` 的完成记录，核对本次 review 应覆盖的验收点。
3. 针对关键路径做代码审查：
   - `crates/scoop/src/commands/build.rs`
   - `crates/scoopc/src/llvm/frontend.rs`
   - `crates/scoopc/src/mir/materialize.rs`
   - `crates/scoopc/src/mir/lower.rs`
   - 相关测试与 HIR lowering 侧变更
4. 运行与该主题直接相关的测试；若审查或测试暴露既有问题，先修复问题，再回到 review 结论。
5. 若未发现阻塞问题，则更新 `TODO.md` / `PLAN.md` / 本文件，标记 `T5000e2cR` 完成并记录验证结果。
6. 提交本轮变更并停止。

## 本轮审查结论

- 已确认 `crates/scoop/src/commands/build.rs` 与 `crates/scoopc/src/llvm/frontend.rs` 的 build / single-file LLVM frontend 主入口均已切换到 `hir::lower_for_compilation_unit_multi_files_via_mir_instance_collection(...)`。
- 已确认 `crates/scoopc/src/hir/lower/mod.rs` / `util.rs` 中 build/frontend 走的是 `ExplicitMirInstances` 模式：monomorphic HIR fun/member 只按 MIR materializer 产出的 `InstanceKey` 集生成，HIR 不再通过 `MonomorphKey` / `TypeStore` 自行发现实例。
- 已确认 `crates/scoopc/src/mir/materialize.rs` 仍使用完整 compilation unit 建立 template catalog、site binding 与 callable-body facts，只把 request source 子集用于 request-root seeding；因此跨文件 template identity 和 subset-driven reachability 仍由 MIR 层统一主导。
- 已检查旧 API 调用点：`lower_for_compilation_unit_multi_files_with_type_env(...)` 已不再位于 build/frontend 主路径，实现代码中的残留调用只见于 `crates/scoopc/src/llvm/codegen/effect/state_machine_transform.rs` 的测试 helper。
- 本轮未发现必须在 `T5000e2cR` 之前插入的新前置缺陷任务。

## 验证结果

- 通过：`cargo test -p scoop build_frontend_keeps_distinct_effect_row_generic_instances -- --nocapture`
- 通过：`cargo test -p scoopc single_file_frontend_ -- --nocapture`
- 通过：`cargo test -p scoopc typechecked_compilation_unit_materialization_reaches_generic_calls_through_non_generic_async_closure_body -- --nocapture`
- 通过：`cargo test --all`
- 通过：`cargo clippy --all-targets -- -D warnings`

## 进度记录（更新）

- 已完成：`T5000e2cR Review：确认 build/frontend 主路径已切到 MIR instance collection`。
- 已完成：更新 `TODO.md` 与 `PLAN.md`，将 `T5000e2cR` 标记为完成，并把下一条待执行任务切换为 `T5000e2R Review：确认编译单元级 monomorphization 已脱离 HIR eager materialization`。
- 已完成：提交本轮任务记录与计划更新的 Git 提交，随后停止，不继续进入 `T5000e2R`。
