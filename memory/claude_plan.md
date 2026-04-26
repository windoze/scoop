# 本轮执行计划

## 目标

本轮只完成 `TODO.md` 中第一个未完成任务；如果在执行前或执行过程中发现更早的既有问题、规格不匹配或实现边界缺口，则优先修复该问题，或者把它整理为阻塞当前任务的前置任务并更新 `TODO.md` / `PLAN.md` 后停止。

## 当前已完成的准备

1. 确认当前工作目录是仓库根目录 `/home/chenxu/repos/scoop-1`。
2. 确认仓库存在 `memory/` 目录，便于持续记录本轮计划与进展。
3. 已查看最新提交 `dfc9c13fdf753c992286e5ef288f433b2211e92a`，提交主题为 `[T5000e2a] Extract compilation-unit materialization API`；提交说明本身未额外声明需要先修的遗留问题。
4. 已阅读 `TODO.md` / `PLAN.md` 并定位本轮首个未完成条目为 `T5000e2aR Review：确认编译单元级 materialization API 已脱离 dump-only 包装`。

## 执行步骤

1. 查看最新提交，确认提交说明是否提到需要优先修复的既有问题。
2. 阅读 `TODO.md`，定位第一个未完成任务。
3. 阅读 `PLAN.md`，核对该任务的上下文、依赖和预期范围。
4. 判断该任务是否过大：
   - 如果过大，则把它拆成更小的前置子任务，更新 `TODO.md` 和 `PLAN.md`，本轮只执行拆分后的第一个子任务。
   - 如果规模合适，则直接进入实现。
5. 在实现前检查相关代码、测试和最近变更，确认没有被遗漏的既有问题。
6. 完成实现，并为行为补充或更新测试。
7. 运行与改动相关的验证；若改动涉及通用编译/运行路径，则至少补充执行对应子集测试，并尽量满足 `cargo fmt`、`cargo test --all`、`cargo clippy --all-targets -- -D warnings` 的要求；若全量验证成本过高，则记录实际执行范围与原因。
8. 更新 `TODO.md`、`PLAN.md` 和本文件，标记本轮任务完成情况与后续状态。
9. 使用清晰提交信息创建一次 git 提交，然后停止。

## 当前聚焦任务

`T5000e2aR Review：确认编译单元级 materialization API 已脱离 dump-only 包装`

### 当前 review 检查点

1. 复核 `materialize_compilation_unit_from_typechecked_inputs(...)` 是否直接消费既有 typechecked compilation-unit facts，而不是重新构造 dump 专用前端。
2. 复核跨文件 template identity、`eff_args`、site binding 收集是否已经位于可复用 API 边界内，而不是仍藏在 dump 包装路径。
3. 复核 build/frontend 后续接线是否已有直接入口；若发现真实边界泄漏或语义缺陷，优先修复，再继续 review。
4. 运行针对性测试与必要的全量验证，然后回写 `TODO.md` / `PLAN.md` / 本文件，并提交。

## 已发现的真实问题

1. `materialize_compilation_unit_from_typechecked_inputs(...)` 目前定义在私有 `mir::materialize` 模块内，`mir/mod.rs` 尚未把它作为 `pub(crate)` 入口重新导出；从模块边界看，后续 `llvm/frontend` 仍拿不到真正的编译单元 materialization API。
2. 该 API 当前只从 `files_to_lower` 收集 site binding，并且只为 `files_to_lower` 生成 generic HIR/MIR template；这在 `compilation_unit == files_to_lower` 的 dump 包装下能工作，但对 build/frontend 典型形状（`compilation_unit` 含 sysroot/辅助文件，`files_to_lower` 只含输入文件）会遗漏跨文件 generic template 的 MIR root 与 side table，不能作为稳定复用入口。

## 修复计划

1. 调整编译单元 materialization API 的内部数据来源，使 generic template lowering 与 site binding 收集覆盖完整 `compilation_unit`，避免跨文件模板提供者缺失 MIR root / binding。
2. 在 `mir/mod.rs` 把该 API 以 `pub(crate)` 重新导出，形成真正可被后续 frontend/build 路径调用的模块边界。
3. 新增跨文件回归测试：helper 文件定义 generic fun，main 文件只负责触发实例化，验证即使 `files_to_lower` 只包含 main，编译单元 materialization 仍能成功产出 helper 泛型实例。

## 已完成

1. 已修改 `crates/scoopc/src/mir/materialize.rs`：
   - `materialize_for_dump(...)` 现经由 `mir` 模块级包装入口进入编译单元 materialization 主线；
   - `materialize_compilation_unit_from_typechecked_inputs(...)` 现统一基于完整 `compilation_unit` 收集 site binding 并生成 generic HIR/MIR template，不再只覆盖请求源文件子集。
2. 已修改 `crates/scoopc/src/mir/mod.rs`：
   - 新增模块级 `pub(crate)` 包装入口 `materialize_compilation_unit_from_typechecked_inputs(...)`，把编译单元 materialization API 从私有 `mir::materialize` 实现边界提升到可复用模块边界。
3. 已新增回归测试 `mir::materialize::tests::typechecked_compilation_unit_materialization_keeps_cross_file_effect_roots_when_request_sources_are_subset`，覆盖“helper 文件提供 effect-generic 模板、main 文件仅贡献实例请求”的跨文件场景。
4. 已更新 `TODO.md` / `PLAN.md`，将 `T5000e2aR` 标记为完成，并记录本轮 review 实际发现与修复的边界问题。

## 验证结果

1. `cargo test -p scoopc typechecked_compilation_unit_materialization_keeps_cross_file_effect_roots_when_request_sources_are_subset -- --nocapture`
2. `cargo fmt --all`
3. `cargo test --all`
4. `cargo clippy --all-targets -- -D warnings`

以上命令均已通过。

## 本轮结论

- `T5000e2aR` 已完成。
- 下一条未完成任务已切换为 `T5000e2b 让编译单元 MIR instance collection 覆盖 owner/nominal specialization`。

## 记录原则

- 持续记录关键判断、计划变更、阻塞项和已完成步骤。
- 不用“临时绕过”推进任务；如果发现规格缺口，必须先修或先入列为前置任务。
