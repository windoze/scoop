# 当前执行计划

## 约束与目标

- 本次只处理 `TODO.md` 中第一个未完成任务，完成后立即停止。
- 在开始具体实现前，先检查最新提交是否提到了需要先修复的既有问题；若有，优先处理这些问题。
- 执行过程中若发现当前任务依赖缺失、实现边界不完整或与规范不一致，必须先把问题写回 `TODO.md` / `PLAN.md`，调整依赖顺序，再停止本轮。
- 所有过程记录、计划调整、关键完成节点都要同步更新到本文件。

## 初始步骤

1. 查看最新一次提交的信息，确认是否有明确提到尚未解决的遗留问题。
2. 查看当前工作区状态，避免误覆盖已有修改，并确认是否存在需要纳入本次提交的意外变更。
3. 阅读 `TODO.md`，定位第一个未完成任务。
4. 阅读 `PLAN.md`，确认该任务的背景、依赖、预期完成标准。
5. 判断该任务是否可以在本轮完整完成：
   - 若可以，直接实现、补测试、验证、更新文档与任务状态。
   - 若不可以，拆分为更小的子任务，并同步更新 `TODO.md` 与 `PLAN.md`。

## 实施与验证策略

- 优先通过精确搜索定位相关模块、测试与规范描述，避免盲目修改。
- 代码改动后至少执行与本任务直接相关的测试；若改动影响面较大，再扩大到对应 crate 或工作区级别检查。
- 在最终提交前，按要求执行格式化、相关测试，必要时执行 `cargo clippy --all-targets -- -D warnings` 以确保无警告。

## 待确认信息

- 最新提交是否显式留下了必须先修复的问题。
- `TODO.md` 当前第一个未完成任务的编号、依赖与验收标准。
- 当前工作区是否存在用户未提交修改，需要在编辑时规避或一并纳入。

## 进度记录

- 已创建本文件。
- 已检查最新提交 `48e53c4fa087063421744ba01be8404742bb4032`（提交信息：`Update CI pipeline`）；提交信息本身未提到需要先处理的遗留功能/语义问题。
- 已检查工作区状态：当前仅有本文件修改，暂无其它用户未提交改动需要规避。
- 已读取 `TODO.md` / `PLAN.md` 的当前主线状态，定位到第一个未完成任务为 `T4010b1b`：`禁止 struct 主构造参数 var 回流为可变字段语义`。
- 当前判断：`T4010b1b` 是 `T4010R` 之前的明确前置 blocker，需要先确认 spec/issue 约束、现有实现缺口以及相关模块，再决定是否可直接在本轮完整收口。

## 下一步

1. 在 `crates/scoopc/src/typecheck/structs.rs` 中为 `struct` 主构造参数补上 `var` 静态拒绝，复用现有 `StructFieldMustBeVal` 诊断。
2. 在 `crates/scoopc/src/typecheck/expr/collect.rs` 中把 `struct` 成员 mutability 收口为统一不可变，防止 ctor `var` 继续被记录进 `member_mutabilities`。
3. 新增 typecheck fixture，覆盖：
   - `struct Point(var x: Int)` 在 typecheck 阶段直接失败；
   - 合法 `struct` 字段 `p.x = 7` 在 typecheck 阶段报 `assignment_target_not_mutable`。
4. 跑定向验证，再跑 `cargo test --all` 与 `cargo clippy --all-targets -- -D warnings`。
5. 若验证通过，更新 `TODO.md` / `PLAN.md` / 本文件并提交。

## 已确认的实现细节

- `parser/decls.rs` 已把主构造参数上的 `val/var` 写入 `ast::Param.kind`，因此问题不在 parser。
- `typecheck/structs.rs::check_one_struct_fields` 当前只对 type body property 的 `var` 报 `StructFieldMustBeVal`，完全漏掉主构造参数。
- `typecheck/expr/collect.rs::collect_member_mutabilities_in_type_decl` 当前会把 `struct` 主构造参数 `var` 记录为 mutable，直接导致 `p.x = 7` 通过 typecheck，并拖到 LLVM 才在 `assignment lhs` unsupported 处失败。
- 已复现两个最小现象：
  - `struct Point(var x: Int, val y: Int)` 当前可成功 build。
  - `var p = Point(1, 2); p.x = 7` 当前直到 LLVM 才失败；而合法 `struct Point(val x: Int, ...)` 的同类赋值已能在 typecheck 报 `assignment_target_not_mutable`，可作为稳定回归目标。

## 实际改动

1. 已在 `crates/scoopc/src/typecheck/structs.rs` 中为 `struct` 主构造参数补上 `var` 静态拒绝，复用现有 `StructFieldMustBeVal` 诊断。
2. 已在 `crates/scoopc/src/typecheck/expr/collect.rs` 中把 `struct` 成员 mutability 收口为统一不可变，不再把 ctor 参数上的 `var` 记成 mutable。
3. 已新增两条 typecheck fixture：
   - `tests/fixtures/typecheck/struct_primary_ctor_var_is_error.scoop`
   - `tests/fixtures/typecheck/struct_value_field_assign_is_error.scoop`
4. 已同步更新 `TODO.md` 与 `PLAN.md`，将 `T4010b1b` 标记为完成，并把下一项推进点切到 `T4010R`。

## 验证结果

- `cargo fmt --all`
- `cargo run -q -p scoop -- build /tmp/t4010b1b_probe.scoop -o /tmp/t4010b1b_probe.out`
  - 结果：按预期在 typecheck 阶段报 `scoop::typecheck::struct_field_must_be_val`
- `cargo run -q -p scoop -- test --fixtures tests/fixtures/typecheck`
  - 结果：`fixtures: ok (348)`
- `cargo run -q -p scoop -- test`
  - 结果：`fixtures: ok (1100)`
- `cargo test --all -- --test-threads=1`
- `cargo clippy --all-targets -- -D warnings`

## 当前状态

- `T4010b1b` 已完成。
- 当前工作区待提交内容包括源码修复、两条新 fixture，以及计划/任务追踪文件更新。
- 下一项应从 `T4010R` 开始，复审值类型整体不可变语义是否还存在其它裂缝。
