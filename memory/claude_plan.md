## 当前执行计划

1. 读取 `TODO.md`，确认第一个未标记为 `[DONE]` 的任务。
2. 检查最近一次提交是否直接提到与该任务相关且未完成的问题；若是，则将其视为当前任务范围或在 `TODO.md` 中补成前置任务。
3. 阅读当前任务及其直接相关代码、测试、规范说明，只在当前任务范围内定位实现与验证入口。
4. 实现当前任务；若遇到阻塞当前任务的真实缺陷或缺失特性，不做变通，改为先在 `TODO.md` 中插入最小必要前置任务并停止。
5. 运行当前任务要求的验证，以及必要的回归测试、`cargo fmt`、`cargo clippy --all-targets -- -D warnings`。
6. 更新 `TODO.md`：仅当任务真正完成时，为任务标题加上 `[DONE]`，并填写完成记录；仅在阶段计划发生变化时更新 `PLAN.md`。
7. 复查工作区变更，按任务要求创建一次 Git 提交，然后停止，不继续下一个任务。

## 进度记录

- 已创建执行计划文件，待读取 `TODO.md` 并确认当前任务。
- 已确认首个未完成任务为 `P0-T02`；最近一次提交仅完成 `P0-T01`，没有额外未收尾事项需要并入当前任务。
- 当前实现策略：
  1. 新增 `crates/scoopc/src/pipeline_user_visible_failure_policy.rs`，固定 production-path failure audit、分类规则与 `FrontendReject` 语义检查。
  2. 将 `typecheck/when_pat.rs` 与 `typecheck/structs.rs` 中两个前端拒绝文案改成更明确的非法输入表述，避免继续呈现“暂不支持/后端未支持”语义。
  3. 运行任务要求的测试、grep 与 `cargo clippy --all-targets -- -D warnings`。
  4. 更新 `TODO.md` 完成记录并提交一次 Git commit 后停止。
- 已完成实现：
  - 新增 `pipeline_user_visible_failure_policy` 审计模块并接入 `crates/scoopc/src/lib.rs`。
  - 为 `when or-pattern` binder reject 新增独立诊断代码 `scoop::typecheck::when_or_pattern_binder_not_allowed`。
  - 将 `struct` 可变字段 reject 文案收紧为“必须是 `val`，不允许 `var`”，并同步更新相关 fixture 的消息与错误码预期。
- 已完成验证：
  - `cargo test -p scoopc pipeline_user_visible_failure_policy -- --nocapture`
  - `cargo test -p scoopc codegen_gap_inventory`
  - `cargo test -p scoopc refactor_mir_value_primitives_reject_unsupported_function_type_cast_before_mir`
  - `cargo run -p scoop -- test --fixtures tests/fixtures/typecheck/when_or_pattern_variant_payload_binder_is_error.scoop`
  - `cargo run -p scoop -- test --fixtures tests/fixtures/typecheck/when_or_pattern_variant_payload_binder_sharing_is_error.scoop`
  - `cargo run -p scoop -- test --fixtures tests/fixtures/typecheck/struct_primary_ctor_var_is_error.scoop`
  - `cargo run -p scoop -- test --fixtures tests/fixtures/typecheck/struct_field_must_be_val_is_error.scoop`
  - `cargo clippy --all-targets -- -D warnings`
  - `rg 'UnsupportedMainBody|Unsupported[A-Za-z_]+|todo!|panic!|unreachable!' crates/scoopc/src`
- 待办：回写 `TODO.md` 完成记录并创建提交。
