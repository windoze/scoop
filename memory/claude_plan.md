# 执行计划

说明：不写入内部详细思维链路，这里仅记录可审计的执行计划、关键判断、进度与变更。

## 初始计划

1. 查看最新一次 Git 提交信息，确认是否提到了需要先修复的既有问题。
2. 阅读 `TODO.md`，定位第一个未完成任务。
3. 阅读与该任务直接相关的 `PLAN.md`、代码与测试，确认范围和依赖。
4. 如果该任务过大，则把它拆成可执行子任务，并同步更新 `TODO.md` 与 `PLAN.md`；本次只执行拆分后的第一个子任务。
5. 实现当前目标任务，保持实现符合规格，不引入临时性绕过方案。
6. 运行相关测试与必要的质量检查；若发现前置缺陷或规格不匹配，先补充到 `TODO.md`/`PLAN.md`，按依赖顺序处理。
7. 完成后更新 `TODO.md`、`PLAN.md` 与本文件进度记录。
8. 提交本次修改，提交后停止，不继续处理下一个任务。

## 进度记录

- 已创建本文件，准备开始检查最新提交与任务列表。
- 已检查最新提交：`git log -1 --format=%B` 仅包含 `[T4006R] 收口 compilation-unit 调用点 side table`，未在提交正文中额外记录需优先修复的遗留问题。
- 已定位 `TODO.md` 中第一个未完成任务为 `T4007 [TODO] 为 RTTI 补齐泛型与 eff 参数化类型支持`；其后续 review 任务为 `T4007R`。
- 已阅读 `ISSUES.md` 第 15 条与相关实现，当前初步判断：
  - `crates/scoopc/src/rtti/mod.rs` 仍在 `nominal_layout()` 中对 `args/eff` 直接报 `unsupported_generic_type`。
  - LLVM 侧 generic class 的对象分配似乎已经能够按实例化后的 mangled FQN 选择 `ClassInit` 与 type descriptor。
  - 但 `is/as/as?` 的运行期目标解析目前仍主要按 `nominal.fqn`（base FQN）取 descriptor / interface id，可能导致参数化目标类型检查错误。
- 下一步：
  1. 用最小 probe 复现 generic / `eff` 参数化类型在 RTTI 或 `is/as/as?` 路径上的当前失败模式。
  2. 依据复现结果判断 `T4007` 是否能在本轮一次性完整收口；若范围明显横跨“RTTI 导出”和“运行期 type test 语义”两条独立主线，则按要求拆分并先执行第一个子任务。
- 已完成 probe 复现并决定拆分：
  - generic class probe 先复现出 `Any` 上做 `is Holder<Int>` 会在 LLVM 侧退回 base generic class descriptor，触发 `TypeKind::Param(T)` / `class field type`。
  - `eff` probe 复现出 `Disposable<eff Raise<RuntimeError>>` 的实例在运行期做 `is Disposable<eff Pure>` 仍输出 `true`，说明 parameterized interface / `eff` target 仍只按 base interface id 判真。
  - 结合 `rtti/mod.rs` 旧 API 仍直接拒绝 args / `eff` nominal，确认 `T4007` 需要拆成 `T4007a -> T4007b -> T4007c -> T4007R`。
- 已完成 `T4007a`：
  - 修改 `crates/scoopc/src/llvm/codegen/mod.rs`，让 `codegen_ref_is_instance_of_nonnull` 对带 type args 的 class target 优先按 `nominal_layout_key` 选择具体实例化 descriptor，不再先命中 base generic class。
  - 已新增正式回归 `tests/fixtures/run-pass/type_check_cast_generic_class_instantiation_basic.scoop`，覆盖 generic class 的 `is/as/as?` 正反路径。
- 已验证：
  - `cargo run -p scoop -- run tests/fixtures/run-pass/type_check_cast_generic_class_instantiation_basic.scoop`
  - `cargo run -p scoop -- dump-rtti tests/fixtures/run-pass/type_check_cast_generic_class_instantiation_basic.scoop --type 'Holder<Int>'`
  - `cargo test --all`
  - `cargo clippy --all-targets -- -D warnings`
- 当前状态：
  - 已把任务文档更新为拆分后的 `T4007a/T4007b/T4007c/T4007R` 顺序。
  - 当前已完成并准备提交的是 `T4007a`；`T4007b` 继续跟踪 parameterized interface / `eff` target 的运行期匹配。
- 收尾：
  - 已将临时 probe 删除，仅保留正式 regression fixture 与进度记录文件。
  - 已执行 `cargo fmt`，工作区当前只剩待提交的 `T4007a` 实现、fixture 与计划文档更新。
