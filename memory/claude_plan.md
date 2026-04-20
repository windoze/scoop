# 本轮执行计划

说明：这里记录可公开的推理摘要与执行步骤，不写入内部完整思维链路，但会持续更新关键判断、计划变更与完成进度。

## 当前目标

按照 `TODO.md` 的顺序完成第一个未完成任务；若发现前置缺陷、规格不匹配或任务过大，则先调整 `TODO.md` / `PLAN.md` 并在本轮处理调整后的第一个可执行任务。

## 初始步骤

1. 检查最新一次 Git 提交的信息，确认是否提到任何已知问题或遗留缺陷。
2. 阅读 `TODO.md`，定位第一个未完成任务。
3. 阅读 `PLAN.md`，确认现有计划与任务顺序是否一致。
4. 判断该任务是否存在缺失前置条件、规格缺口或复杂度过高的问题。
5. 若需要，先拆分任务并更新 `TODO.md` / `PLAN.md`，然后执行拆分后的第一个子任务。
6. 实现代码修改。
7. 运行与该任务直接相关的测试，再运行必要的全量校验（至少包括无警告要求对应的检查）。
8. 更新 `memory/claude_plan.md`、`TODO.md`、`PLAN.md`。
9. 提交 Git commit，随后停止，不继续下一个任务。

## 执行原则

- 不用变通方案掩盖规格缺陷。
- 若遇到阻塞，必须将真实前置问题写回 `TODO.md` 和 `PLAN.md`，提交后停止。
- 不回退用户已有改动；若工作区有其他变更，会先识别并尽量与其共存。

## 进度记录

- [x] 已写入本轮初始计划。
- [x] 已检查最新提交。
- [x] 已定位第一个未完成任务。
- [x] 已判断是否需要拆分/重排任务。
- [x] 已完成实现。
- [x] 已完成测试。
- [x] 已更新 `TODO.md` / `PLAN.md` / 本文件。
- [ ] 已提交 commit。

## 当前判断

- 最新提交 `0dadd732d86d2dbe07cd6cc5e1376f0163fceb2b` 仅更新执行记录（`memory/claude_plan.md`），未显式引入新的已知 issue。
- `TODO.md` 中首个未完成任务为 `T4010b`：补齐值类型字段默认值与 immutable-friendly 声明人体工学。
- 已确认当前实现里至少存在两处与 `T4010b` 直接相关的旧门禁：
  - `crates/scoopc/src/typecheck/structs.rs` 仍显式报 `struct_field_default_value_not_supported`。
  - `crates/scoopc/src/typecheck/expr/infer.rs` 仍要求 struct literal 显式覆盖全部字段。
- 已确认 direct ctor call 在 `T4010b0` 后已经具备 `has_default` / `arg_mapping` 形式的默认参数候选能力，但 HIR lowering 仍未把 struct 字段默认值真正展开为可执行语义。
- 发现一个需要验证的潜在 pre-existing issue：
  - 当前 struct “直接字段”收集路径（resolver synthetic ctor、`collect_struct_field_types`）似乎把 value-type computed property 也视为构造字段；若最小 probe 证实该行为泄漏到 `StructName(...)` / `StructName { ... }`，则需要在实现 `T4010b` 时一并修正，避免默认值建立在错误的 direct field 集合上。
- 已用 `cargo check -q -p scoopc` 确认当前未提交改动的第一批编译断点：
  - `HirLoweringSetup` 新增的 `compilation_unit` / `default_arg_structs` 尚未在所有入口传递。
  - `resolve/scopes.rs` 中 `check_type_member_property` 新签名有一个调用点未补最后一个布尔参数。
- 上述接线断点已开始修复；下一步再次 `cargo check`，确认工程回到可编译状态后再继续做 `T4010b` 的 lowering 本体。
- `T4010b` 已完成：
  - struct direct field 默认值已贯通 primary ctor 参数与 body-property 字段，并统一作用于 `StructName(...)` 与 `StructName { ... }`。
  - getter-only computed property 已从 synthetic ctor / struct literal required fields / `with` / struct destructuring 的 direct field 主线里排除。
  - `SCOOP_FULL_SPEC.md` 已同步 value-type property / direct field / 默认值语义。
- 已新增/更新回归：
  - `tests/fixtures/typecheck/struct_direct_field_default_value_ok.scoop`
  - `tests/fixtures/typecheck/struct_computed_property_not_ctor_field_ok.scoop`
  - `tests/fixtures/run-pass/struct_default_field_ctor_and_literal_equivalence_basic.scoop`
  - `tests/fixtures/run-pass/struct_computed_property_not_ctor_field_basic.scoop`
  - `tests/fixtures/hir/struct_default_field_lowering.scoop`
- 已完成验证：
  - `cargo run -q -p scoop -- test --fixtures tests/fixtures/typecheck`
  - `cargo run -q -p scoop -- test --fixtures tests/fixtures/hir`
  - `cargo run -q -p scoop -- test --fixtures tests/fixtures/run-pass`
  - `cargo test -p scoop_runtime --test gc_immix_compaction -- --test-threads=1`
  - `cargo test --all -- --test-threads=1`
  - `cargo clippy --all-targets -- -D warnings`
- 验证过程中额外暴露一个独立既有问题：
  - 值类型 computed property 读取仍会在 HIR/LLVM 侧误走 direct field access，`Point(1).doubled` 类最小 build probe 会报 `scoop::llvm::unsupported_main_body: unknown struct field`。
  - 该问题已写回 `TODO.md` / `PLAN.md` 为后续任务 `T4010b1`，并放在 `T4010R` 之前。
- 备注：
  - 本机上直接跑 `cargo test --all` 会在 `gc_immix_compaction` 两个 runtime 测试并发执行时卡在 STW wait；单独串行运行 `cargo test -p scoop_runtime --test gc_immix_compaction -- --test-threads=1` 与全量 `cargo test --all -- --test-threads=1` 均通过，说明这是现存 runtime 测试并发问题，不是本轮前端改动引入的新失败。

## 下一步

1. 用最小 probe 验证“computed property 被误当作构造字段”的怀疑是否成立。
2. 若成立，把它作为 `T4010b` 内的前置修正一并处理；若不成立，继续直接实现字段默认值主线。
3. 开始修改 typecheck / HIR lowering / 规范文档。
