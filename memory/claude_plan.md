# 执行记录与计划

## 当前约束

- 本次调用只处理 `TODO.md` 中第一个未完成任务，完成后停止。
- 在开始任何仓库检查之前，先写入本文件作为初始计划。
- 执行过程中如果计划变化、发现阻塞、完成关键步骤，持续更新本文件。
- 需要先检查最新提交是否提到已有问题；若提到，优先修复该问题。
- 若任务过大，需要把任务拆分到 `PLAN.md` 和 `TODO.md`，并只执行拆分后的第一个子任务。
- 不允许以变通方案绕过现有缺陷；若发现阻塞缺陷，必须先修复，或把缺陷作为前置任务插入 `TODO.md` 并停止。
- 完成当前任务后，需要更新 `TODO.md`、`PLAN.md`，运行相关测试与校验，并创建一次 git 提交。

## 初始执行步骤

1. 查看最新一条 git 提交信息，确认是否提到了待修复的既有问题。
2. 查看 `TODO.md` 与 `PLAN.md`，确定第一个未完成任务，以及是否需要先拆分任务。
3. 查看当前工作区状态，确认是否存在用户未提交改动，避免误覆盖。
4. 根据第一个未完成任务定位相关代码、测试和规范位置。
5. 如遇到既有缺陷或实现边界问题，先判断是否必须作为前置问题处理，并据此更新 `TODO.md` / `PLAN.md`。
6. 实现当前任务或当前子任务。
7. 运行必要的格式化、测试和 lint/检查命令，修复出现的问题。
8. 更新 `TODO.md`、`PLAN.md` 与本文件，记录完成情况或阻塞原因。
9. 使用清晰的提交信息提交本次变更，然后停止。

## 说明

- 由于尚未读取仓库内容，上述计划是初始计划；在读取 `TODO.md`、`PLAN.md`、最新提交说明后，我会细化并更新。

## 已完成的检查

- 已查看最新提交：`42e272aa [T5000e1b0bR] Review member MIR materialization`。
- 已查看 `TODO.md`、`PLAN.md` 与当前工作区状态。
- 最新提交说明中未直接声明新的待修复遗留问题；当前工作区只有本文件处于修改状态。

## 当前锁定任务

- 当前第一个未完成任务是：`T5000e1bR Review：确认 effect-row 实参已成为 InstanceKey / materializer 的一等维度`。
- 该任务依赖 `T5000e1b0bR`，而该依赖在 `TODO.md` / `PLAN.md` 中已标记完成。

## 针对当前任务的细化计划

1. 复核 `MonomorphKey`、`TopLevelFunCallBinding`、HIR lowering、MIR materializer、`InstanceKey` 与调试输出中的 `eff_args` 传播链。
2. 复核 effect-only generic fun、相同 type args 但不同 effect row、top-level function value、member/extension/companion member 路径是否都被统一覆盖。
3. 运行与 `T5000e1b*` 直接相关的 targeted tests / CLI 复现，必要时补跑更大范围测试。
4. 若 review 暴露新的既有缺陷：
   - 先修复；若无法在本轮直接修复，则把它写成 `TODO.md` 中位于当前任务之前的前置任务，并同步更新 `PLAN.md`；
   - 然后提交并停止。
5. 若 review 通过：
   - 在 `TODO.md` 中将 `T5000e1bR` 标记完成；
   - 在 `PLAN.md` 中记录 review 结论与验证结果；
   - 更新本文件记录关键结论；
   - 提交并停止。

## 当前结果

- `T5000e1bR` 已确认可完成，不需要插入新的前置缺陷任务。
- 关键复核结论：
  - `eff_args` 已从 `MonomorphKey` / `TopLevelFunCallBinding` / `TopLevelFunValueRef` 进入 `SiteInstanceBinding`、`InstanceKey`、`instance_fqn(...)`、effect-row substitution 与 `HashMap<InstanceKey, ...>` materializer cache。
  - HIR/MIR template 仍通过 effect-row marker 保留 `<eff E>` 语义，实例化阶段才展开，不存在在 lowering 时提前塌缩成 `Pure` / `Any` 的回退。
  - effect-only generic、相同 type args 不同 effect row、top-level function value、extension/member/lambda-derived member、type-body member、companion member 等路径都已有代码复核和回归覆盖。
  - `dump-ir` CLI 复现已确认用户可见输出里会出现两个不同 effect row 的 `InstanceKey` 与 concrete callee。

## 本轮验证

- 已通过 targeted tests：
  - `monomorph_materializes_effect_only_generic_instance`
  - `monomorph_distinguishes_same_type_args_with_different_effect_rows`
  - `monomorph_rewrites_top_level_fun_value_effect_instance`
  - `dump_materialization_inputs_keep_eff_args_for_extension_direct_call_binding`
  - `dump_materialization_inputs_keep_eff_args_for_member_direct_call_binding`
  - `dump_materialization_inputs_keep_eff_args_for_member_direct_call_binding_from_lambda`
  - `materialize_for_dump_handles_type_body_generic_member_fun_roots`
  - `materialize_for_dump_distinguishes_companion_member_fun_effect_instances`
- 已通过更大范围验证：
  - `cargo test -p scoopc monomorph::lower -- --nocapture`
  - `cargo test --all`
  - `cargo clippy --all-targets -- -D warnings`

## 收尾步骤

1. 提交 `TODO.md`、`PLAN.md` 与本文件更新。
2. 停止，等待下一次调用处理 `T5000e1R`。
