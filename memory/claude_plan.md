## 执行思路摘要

说明：这里记录可公开的高层推理摘要与执行计划，不记录逐字内部思维过程。

### 当前状态

1. 已检查最新提交：`45c288c [T4010b1b] Reject mutable struct ctor params`。
2. 最新提交正文没有额外列出“必须先修”的既有问题；当前工作树只有本文件尚未提交。
3. 已读取 `TODO.md` / `PLAN.md` / `README.md` / `ISSUES.md`。
4. 当前真正需要执行的首个未完成条目是 `T4010R`，不是总括条目 `T4010` 本身。

### 当前任务

`T4010R`：Review：确认值类型仍保持整体不可变。

复审重点：
1. 不接受把 `with` 扩展成字段级写回式 `var`。
2. 不允许借“默认值人体工学”重新引入可变值类型叙事。

### 已确认的实现基线

1. `struct` 主构造参数显式 `var` 已在 `typecheck::structs::check_one_struct_fields` 中被 `StructFieldMustBeVal` 拒绝。
2. `struct` / `enum` body property 统一走 `check_one_value_type_property`，会拒绝：
   - `var` property
   - setter
   - delegated property
   - computed property initializer
3. 赋值语句 `lhs = rhs` 对 member access 统一读取 `member_mutabilities`；`struct` 成员目前统一记录为 immutable，因此 `p.x = 7` 会在 typecheck 报 `assignment_target_not_mutable`。
4. `with` 的 typecheck / lowering 主线当前只接受 value aggregate（`struct` / `tuple` / `enum`）并返回基值类型，语义是 copy-update，不是原位写回。

### 进行中的验证

1. 已完成对值类型不可变约束遗漏入口的复查，重点覆盖：
   - `enum` 上的 `var` property / setter
   - 现有 fixture 是否已覆盖 value-type property 约束
   - `with` 与默认值相关路径是否会回流出可变语义
2. 本轮未发现新的规范裂缝；因此按 review 任务预期，补了最小 regression 覆盖缺失入口。
3. 当前正在同步更新 `TODO.md` / `PLAN.md` / 本文件，并准备提交本轮任务。

### 本轮已做修改

1. 新增 typecheck 回归：
   - `tests/fixtures/typecheck/enum_property_must_be_val_is_error.scoop`
   - `tests/fixtures/typecheck/enum_property_setter_not_allowed_is_error.scoop`
2. 更新 `TODO.md`：将 `T4010` / `T4010R` 标记为完成，并记录 review 结论与验证命令。
3. 更新 `PLAN.md`：把 P7 当前状态推进到 `T4011`。

### 复审结论

1. 值类型不可变约束目前走统一主线：
   - `struct` 主构造参数与 body property 静态拒绝 `var`
   - `struct` / `enum` value-type property 统一拒绝 `var`、setter、delegate 与 computed-property initializer
   - 赋值语句通过 `member_mutabilities` 一致地拒绝值类型字段写回
2. `with` 仍是 copy-update；字段默认值只影响构造入口，没有重新引入可变 backing field / setter 语义。
3. 本轮没有发现需要前插到 `TODO.md` 的新 blocker，因此可正常完成 `T4010R`。

### 已完成验证

1. `cargo run -q -p scoop -- test --fixtures tests/fixtures/typecheck` -> `fixtures: ok (350)`
2. `cargo run -q -p scoop -- test` -> `fixtures: ok (1102)`
3. `cargo test --all -- --test-threads=1` -> 通过
4. `cargo clippy --all-targets -- -D warnings` -> 通过

### 执行约束

- 如果发现任何仍会让 value type 重新表现为“可变字段”的路径，必须先把问题前插到 `TODO.md`，不能带着问题完成 `T4010R`。
- 不回退用户已有改动；仅在当前任务相关范围内增量修改。
- 输出、计划记录与结论统一使用中文。
