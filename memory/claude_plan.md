# 执行计划

1. 读取 `TODO.md`，定位第一个未标记为 `[DONE]` 的任务，并确认其依赖、验证要求与完成记录。
2. 检查最近一次提交是否直接提到与该任务相关但未完成的问题；如果该问题构成前置依赖，则先在 `TODO.md` 中补充并调整顺序。
3. 阅读完成该任务所需的最小相关代码与文档，确认当前实现状态、规格约束与测试入口。
4. 直接实现当前任务；如果遇到会阻塞任务达成的真实缺口或规格不匹配，先修复该缺口，或在 `TODO.md` 中加入最小前置任务并停止。
5. 运行与该任务相关的验证，包括任务要求的测试，以及必要的格式化、lint 或构建检查。
6. 根据执行进展更新本文件，记录关键步骤、计划变更、阻塞信息与验证结果。
7. 完成后在 `TODO.md` 中将该任务标题显式标记为 `[DONE]`，补全完成记录；仅在阶段计划发生变化时更新 `PLAN.md`。
8. 按仓库提交风格创建一次提交，提交本次任务涉及的全部未提交改动，然后停止，不继续下一个任务。

## 进度记录

- 已读取 `TODO.md` 并确认首个未完成任务为 `P6-T02`：同步 `FrontendReject` surface（or-pattern binder / function-type cast / use-site effect row / struct mutable field）。
- 已检查最近一次提交信息：`[P6-T01] Close runtime cast and GC token partial surfaces`，提交说明未显式声明需要先处理的 `P6-T02` 前置未完事项。
- 已完成当前任务的事实核对：
  - `or-pattern binder` 由 `typecheck/when_pat.rs` 在 typecheck 阶段稳定拒绝，MIR/backend 不再消费该形状。
  - function-type `as/as?` 由 `typecheck/expr/infer.rs` 在 MIR 前拒绝，`pipeline/mir_stage.rs` 已有回归锁定。
  - `struct mutable field` 由 `typecheck/structs.rs` 与 `typecheck/expr/collect.rs` 共同维持为前端限制；class `var` property 仍保持支持。
  - `use-site effect row` 已有实际支持面，不只是 parse：现有 `typecheck` / `infer` / `run-pass` fixtures 已覆盖 `Type<eff Row>`、effect-row 推断，以及 parameterized interface 的 runtime `is/as/as?`。因此 `PIPELINE_GAPS.md §7.3` 的“整体 FrontendReject”描述已经过时。
- 编辑计划：
  1. 收紧四类 surface 的诊断文案，使其统一表达“当前语言 contract 下非法输入/不接受该源码形状”。
  2. 将 `PIPELINE_GAPS.md §7.3` 与 `PLAN.md` 对应条目改写为真实支持状态，并同步修正 `pipeline_user_visible_failure_policy.rs` 中对 `§7.3` 的过时 FrontendReject 绑定。
  3. 为“对无 effect-row 形参的类型误用 `Type<eff ...>`”补一个最小 typecheck 负例 fixture，作为 `use_site_eff_arg_not_allowed` 的常驻验证。
  4. 跑完 `P6-T02` 要求的 fixture/test/clippy，再回写 `TODO.md` 完成记录并提交。
- 已完成实现：
  - 统一了 `or-pattern binder`、function-type `as/as?`、非法 use-site effect row target、`struct var` 的用户可见诊断文案。
  - 新增 `tests/fixtures/typecheck/use_site_eff_arg_target_without_eff_param_is_error.scoop`，并更新相关旧 fixtures 预期文案。
  - 将 `PIPELINE_GAPS.md §7.3` 与 `PLAN.md` 对应条目改写为“已支持的 nominal `Type<eff Row>` surface”，保留非法 target 的 typecheck reject；同步修正 `pipeline_user_visible_failure_policy.rs` 和 `cone/pre_specialize.rs` 的过时说明。
- 已完成验证：
  - `cargo test -p scoopc pipeline_user_visible_failure_policy -- --nocapture`
  - `cargo test -p scoopc refactor_mir_value_primitives_reject_unsupported_function_type_cast_before_mir`
  - `cargo run -p scoop -- test --fixtures tests/fixtures/typecheck/when_or_pattern_variant_payload_binder_is_error.scoop`
  - `cargo run -p scoop -- test --fixtures tests/fixtures/typecheck/when_or_pattern_variant_payload_binder_sharing_is_error.scoop`
  - `cargo run -p scoop -- test --fixtures tests/fixtures/typecheck/fn_type_cast_effectful_asq_is_error.scoop`
  - `cargo run -p scoop -- test --fixtures tests/fixtures/typecheck/fn_type_cast_effectful_as_is_error.scoop`
  - `cargo run -p scoop -- test --fixtures tests/fixtures/typecheck/fn_type_cast_closed_pure_asq_is_error.scoop`
  - `cargo run -p scoop -- test --fixtures tests/fixtures/parse/type_args_eff_use_site_order_fail.scoop`
  - `cargo run -p scoop -- test --fixtures tests/fixtures/typecheck/use_site_eff_arg_target_without_eff_param_is_error.scoop`
  - `cargo run -p scoop -- test --fixtures tests/fixtures/typecheck/eff_row_param_infer_from_nominal_ok.scoop`
  - `cargo run -p scoop -- test --fixtures tests/fixtures/typecheck/struct_property_setter_not_allowed_is_error.scoop`
  - `cargo run -p scoop -- test --fixtures tests/fixtures/typecheck/struct_primary_ctor_var_is_error.scoop`
  - `cargo run -p scoop -- test --fixtures tests/fixtures/typecheck/struct_field_must_be_val_is_error.scoop`
  - `cargo run -p scoop -- test --fixtures tests/fixtures/typecheck/class_var_property_reassign_ok.scoop`
  - `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/type_check_cast_parameterized_interface_runtime_match_basic.scoop`
  - `cargo clippy --all-targets -- -D warnings`
- 待执行：检查工作区差异、创建 `P6-T02` 提交，然后停止。
