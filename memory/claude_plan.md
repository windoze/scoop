## 当前执行计划

1. 读取 `TODO.md`，定位第一个标题未标记 `[DONE]` 的任务，并确认该任务的要求、依赖、验证方式与完成记录格式。
2. 检查最近提交是否有与该任务直接相关且明确未完成的问题；如果存在且会阻塞当前任务，则先把它视为当前任务范围内问题，或按要求在 `TODO.md` 中登记为前置任务。
3. 阅读实现该任务所需的最小相关代码、测试、规范与文档，确认当前实现状态与缺口，不做开放式无关问题排查。
4. 如当前任务可直接完成，则按最小正确改动实现完整功能；若遇到会阻塞任务的真实缺口或规格不匹配，则在 `TODO.md` 中增加最小前置任务、更新依赖关系，并停止继续后续任务。
5. 运行任务要求的验证命令，以及必要的回归测试、`cargo fmt`、`cargo clippy --all-targets -- -D warnings`；若失败则立即修复并重新验证。
6. 完成后更新 `memory/claude_plan.md` 记录进展，更新 `TODO.md` 将当前任务标题标记为 `[DONE]` 并填写完成记录；仅在阶段计划发生变化时更新 `PLAN.md`。
7. 检查工作区改动，按要求提交一次 Git 提交，提交信息使用当前任务编号，并在本次调用中停止，不继续下一个任务。

## 约束说明

- 不采用绕过规格的临时方案。
- 如果发现阻塞当前任务的真实缺口，先在 `TODO.md` 中显式建前置任务并调整顺序。
- 只完成 `TODO.md` 中当前排位的一个任务。
- 这里记录的是执行计划与外显决策，不包含内部推理细节。

## 当前进展

- 已确认首个未完成任务为 `P4-T01a`：解锁 struct/class（含 generic class）instance method 的常规 `receiver.method()` 调用。
- 已检查最近一次提交：只更新了 `PLAN.md` / `TODO.md`，未额外声明会直接阻塞 `P4-T01a` 的未完成实现问题。
- 下一步聚焦阅读 `P4-T01a` 指向的实现入口与现有注释，确认当前 member call / instance method / generic class 的实际缺口。

## 已确认的实现缺口

- `crates/scoopc/src/typecheck/expr/entry.rs` 当前只真正检查 `class` / `object` 的成员函数体；`struct` body 内 `fun` 尚未走成员函数体 typecheck。
- `crates/scoopc/src/hir/lower/expr.rs` 的成员调用降糖当前只对 `class` / `interface` / `object` 生效，`struct` 成员方法调用仍会保留成 `MemberAccess` 回退形态。
- `generic class` 成员方法调用与单态化路径已经存在既有回归，因此当前任务重点是补齐 `struct` 路径，并用新 fixture 明确锁住 `struct` / `class` / `generic class` 三类常规 `receiver.method()` surface。

## 当前编辑计划

1. 在 `typecheck/expr/entry.rs` 让 `struct` 也复用现有成员函数体检查逻辑，并保留 class 专属的 property/init/super-ctor 路径不变。
2. 在 `hir/lower/expr.rs` 放开 `struct` 的 member-call / safe-member-call 直降糖，使其和现有 class/object member-call 一样改写为 `<Type>.method(this, ...)`。
3. 新增三个 `run-pass` fixture，分别锁住 user `struct`、user `class`、generic `class` 的实例方法调用；其中至少一条同时覆盖“成员方法优先于同名扩展函数”。
4. 运行格式化、目标 fixture、`tests/fixtures/run-pass`、`cargo test -p scoopc llvm_tests -- --nocapture`、`cargo clippy --all-targets -- -D warnings`，通过后回写 `TODO.md` 并提交。

## 已完成步骤

- 已在 `crates/scoopc/src/typecheck/expr/entry.rs` 把 `struct` member fun body 接入既有成员函数体 typecheck 主线；class 专属的 property/init/super-ctor 检查保持不变。
- 已在 `crates/scoopc/src/hir/lower/expr.rs` 放开 `struct` 的 ordinary member-call / safe-member-call 直降糖，统一改写为 `<Owner>.method(receiver, ...)`。
- 已新增以下 run-pass fixture：
  - `tests/fixtures/run-pass/member_call_struct_body_method_basic.scoop`
  - `tests/fixtures/run-pass/member_call_class_body_method_basic.scoop`
  - `tests/fixtures/run-pass/member_call_generic_class_body_method_basic.scoop`

## 验证状态

- 已通过：
  - `cargo fmt --all`
  - `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/member_call_struct_body_method_basic.scoop`
  - `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/member_call_class_body_method_basic.scoop`
  - `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/member_call_generic_class_body_method_basic.scoop`
  - `cargo test -p scoopc object_member_call_uses_gc_managed_singleton_receiver -- --nocapture`
  - `cargo test -p scoopc builtin_string_member_calls_lower_to_direct_calls -- --nocapture`
  - `cargo test -p scoopc builtin_string_trim_indent_member_calls_lower_to_direct_calls -- --nocapture`
  - `cargo clippy --all-targets -- -D warnings`
- 已记录：
  - `cargo test -p scoopc llvm_tests -- --nocapture` 当前仍是空过滤：`0 passed; 848 filtered out`。
  - `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass` 的 member-call 相关 fixture 均通过，但全量当前仍暴露两个与本任务代码路径无直接交集的现存失败：
    - `tests/fixtures/run-pass/extern_native_aggregate_return_direct_indirect_parity.scoop`
    - `tests/fixtures/run-pass/sync_gc_release_task_like_object_basic.scoop`

## 收尾

- 下一步只剩检查工作区、提交本任务改动，并停止在 `P4-T01a` 处，不继续 `P4-T01b`。
