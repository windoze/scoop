# 执行计划记录

## 约束说明

- 按用户要求，先写入本文件，再执行仓库检查与实现工作。
- 这里记录的是可公开的执行计划、检查步骤、进度与决策依据摘要，不包含隐藏的内部推理细节。

## 初始计划

1. 检查最新一次提交信息，确认是否提到需要优先修复的现存问题。
2. 读取 `TODO.md`，定位第一个未完成任务。
3. 读取 `PLAN.md`，确认当前计划与任务依赖关系。
4. 结合代码与测试现状判断该任务是否可直接完成：
   - 若可直接完成，则实现、补充测试、运行验证，并更新文档与任务状态。
   - 若任务规模过大或存在前置阻塞，则先把任务拆分/重排到 `TODO.md` 与 `PLAN.md`，本次只处理新的首个子任务或前置任务。
5. 在执行过程中，任何发现的既有缺陷、回归、规范不匹配、未完成边界，都优先视为当前范围内问题处理。
6. 完成当前首个任务后：
   - 更新 `memory/claude_plan.md`
   - 更新 `TODO.md`
   - 更新 `PLAN.md`
   - 运行相关测试与 `cargo clippy --all-targets -- -D warnings`
   - 提交 git commit
   - 停止，不继续下一个任务

## 当前状态

- 已完成：初始化计划文件
- 已完成：检查最新提交、`TODO.md`、`PLAN.md`，确认最新提交未额外声明需先插入的新遗留修复任务
- 已完成：定位首个未完成任务为 `T5000e1b0aR Review`
- 进行中：执行 `T5000e1b0aR Review`

## 当前任务：T5000e1b0aR Review

### 目标

确认 extension/member direct-call 在 request binding 与调用点正规化阶段不再丢失 `eff_args`，并验证：

1. call-callee 位置的 `TypeApply` 仍会走 extension/member direct-call 降糖；
2. extension/member direct-call 的 `TopLevelFunCallBinding` 能稳定携带 `decl_file`、`decl_span`、`type_args`、`eff_args`；
3. 直连成员方法签名收集会保留 `eff_param` 与 effect-row substitution 事实；
4. 已知更深层阻塞 `T5000e1b0b` 之外，不存在新的更浅层既有问题。

### 执行步骤

1. 阅读 `TODO.md` / `PLAN.md` 中 `T5000e1b0a` 与 `T5000e1b0aR` 的说明。
2. 检查相关代码：
   - `crates/scoopc/src/hir/lower/expr.rs`
   - `crates/scoopc/src/typecheck/expr/call.rs`
   - `crates/scoopc/src/typecheck/expr/ops.rs`
   - `crates/scoopc/src/mir/materialize.rs`
   - 相关测试与探针
3. 运行定向测试和必要的 CLI probe，验证 review 验收项。
4. 若发现既有问题：
   - 先修复，或把前置任务插入 `TODO.md`/`PLAN.md` 后停止。
5. 若 review 通过：
   - 更新 `TODO.md` / `PLAN.md` / 本文件
   - 运行最终验证（至少覆盖相关测试与 `clippy`）
   - 提交 commit
   - 停止

## review 进展更新

- 已确认并补充回归：
  - typed HIR 中 `box.forward<eff E>()` 仍走 member direct-call 降糖，而不是退回成员值 / `FunValue`。
  - materialization inputs 中，typed receiver 下的 member direct-call request binding / monomorph key 会保留非 `Pure` 的 `eff_args`。
- review 过程中暴露出新的前置缺口：
  - `class Box() { fun <eff E = Pure> lift(f: () -> Int / E): Int / E { ... } }`
    配合
    `val box: Box = Box(); box.lift({ perform Boom.ping(); 1 })`
    仍会在 member direct-call typecheck 阶段命中 `NoMatchingOverload`。
  - 这说明带 lambda 实参的 effect-generic member direct-call 仍未真正消费已收集的
    `eff_param` / `param_*_eff_base` / subst facts。

## 当前结论

- `T5000e1b0aR` 不能直接完成。
- 需要先插入新的前置任务，修复 effect-generic member direct-call 的 lambda 实参 overload matching /
  `eff_arg` 推断闭环，再回到本 review。
- 下一步：
  1. 更新 `TODO.md` 与 `PLAN.md`，把新的前置任务放到 `T5000e1b0aR` 之前。
  2. 运行验证，确保新增回归测试通过且无告警。
  3. 提交本次“发现阻塞并重排任务”的变更，然后停止。

## 最新状态

- 已完成：`TODO.md` / `PLAN.md` 重排，新增 `T5000e1b0a1` / `T5000e1b0a1R` 作为 `T5000e1b0aR` 的前置任务。
- 已完成：新增并通过以下回归测试：
  - `typed_hir_keeps_effect_generic_member_type_apply_on_direct_call_path`
  - `dump_materialization_inputs_keep_eff_args_for_member_direct_call_binding`
- 已完成验证：
  - `cargo test -p scoopc`
  - `cargo clippy --all-targets -- -D warnings`
- 进行中：整理变更并提交 git commit，随后停止。
