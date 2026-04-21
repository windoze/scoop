# 本轮执行计划（T4016T1a）

## 目标

完成 `TODO.md` 中当前第一个未完成任务 `T4016T1a`，修复“枚举/结构体字段为函数类型时的布局与调用链不完整”问题，并在完成后停止，不继续处理后续任务。

## 已确认前提

- 已检查上一轮总结，当前首个未完成任务是 `T4016T1a`。
- 已识别两个必须一起修复的既有问题：
  1. HIR layout side table 会把 `TypeRef::Function` 丢成 `None`，导致 LLVM 最终无法为函数类型字段生成正确 struct field type。
  2. `receiver.f()` 中若 `f` 是函数值，typecheck 会错误报 `callee_not_callable`，阻塞字段上函数值的直接调用。
- 这两个问题都属于当前任务范围，不能拆开只修一半。

## 执行步骤

1. 检查并修复当前 Rust 编译错误，重点关注：
   - `crates/scoopc/src/hir/lower/util.rs`
   - `crates/scoopc/src/llvm/codegen/mod.rs`
2. 重新编译并运行最小定向用例，确认以下场景全部可用：
   - enum payload 为函数值
   - task state payload 为函数值
   - struct 字段为函数值且可直接调用
3. 运行相关 fixture 测试：
   - `tests/fixtures/typecheck`
   - `tests/fixtures/run-pass`
4. 运行更完整验证：
   - `cargo run -p scoop -- test`
   - `cargo test --all`
   - `cargo clippy --all-targets -- -D warnings`
5. 若中途发现新的既有 blocker：
   - 优先修复；
   - 若无法在本轮直接完成，则先更新 `TODO.md` / `PLAN.md` / 本文件，插入前置任务后停止。
6. 若当前任务完成：
   - 更新 `TODO.md`，将 `T4016T1a` 标记完成；
   - 更新 `PLAN.md` 与本文件，记录结果；
   - 提交 git commit；
   - 停止。

## 当前关注点

- `hir/lower/util.rs` 里 generic layout collector 可能仍有借用冲突，需要通过缩短 `types.kind(...)` 的借用生命周期或改写闭包为显式循环解决。
- `llvm/codegen/mod.rs` 里 callable callee 分支可能有 `self` 双重可变借用，需要把 `self.codegen_expr(callee)?` 拆成前置局部变量。
- 必须确保实现不是 workaround，而是让字段上的函数类型走正常布局和 callable-value 主线。

## 进度记录

- 已写入本轮计划。
- 已修复当前 Rust 编译错误：
  - `hir/lower/util.rs`：缩短 `TypeStore` 借用生命周期，并在 generic struct/enum layout 收集时先克隆 nominal 元数据，避免字段解析时的可变/不可变借用冲突。
  - `llvm/codegen/mod.rs`：将 callable callee 的 `codegen_expr` 与 `coerce_value` 拆成两个步骤，消除 `self` 双重可变借用。
- 已确认 `cargo check -p scoopc --features llvm` 通过。
- 重新定位并修复了两个主线漏口：
  - `typecheck/expr/call.rs`：当 member call 实际落在 value member 且该成员类型可调用时，除了沿 callable-value 主线推导返回类型，还需要把重新解析得到的 member resolution 写回 side table；否则 build 阶段 HIR 仍保留 `resolved: None`，LLVM 无法识别 `receiver.f()` 是函数值调用。
  - `llvm/codegen/mod.rs`：补上 `MemberAccess` 与 struct 构造结果的 concrete type 恢复，使 struct/class 字段函数值可在 codegen 前恢复真实 `TypeId`，命中新加的 callable-callee 分支。
- 三个最小定向用例已全部通过：
  - `struct_function_field_call_basic.scoop` 可构建，执行退出码为 `7`。
  - `task_state_function_payload_basic.scoop` 可构建，执行退出码为 `2`。
  - `enum_function_payload_basic.scoop` 可构建，执行退出码为 `16`。
- fixture 形状微调：
  - enum probe 在 `main` 中改为先绑定 `val step: Step = Ready({ 8 })` 再调用 `drive(step)`，避免把验证重点混入当前与 prelude 同名 variant 的调用点歧义。
  - task-state probe 用 `__scoop_task_step_ready(0)` 产出私有 `__TaskStepResult`，更贴近真实 task driver 路径。
- 已完成更完整验证：
  - `cargo run -p scoop -- test --fixtures tests/fixtures/typecheck`
  - `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass`
  - `cargo run -p scoop -- test`
  - `cargo test --all`
  - `cargo clippy --all-targets -- -D warnings`
- 额外修复的既有 blocker：
  - fixture runner 在 `cargo run -p scoop -- test` 中通过 `current_exe()` 获取到 `.../scoop (deleted)` 路径，导致 run-pass 自调用失败；现已修复为回退到去掉 `(deleted)` 后缀的真实路径。
- 当前状态：
  - `T4016T1a` 已完成并已更新 `TODO.md` / `PLAN.md`。
  - 下一轮应从 `T4016T1R` 开始，而不是直接进入 `T4016T2`。
