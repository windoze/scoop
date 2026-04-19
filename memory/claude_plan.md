# 本轮执行计划

## 约束与目标

- 目标：只完成 `TODO.md` 中第一个未完成任务，然后停止。
- 在继续任何实现前，先检查最新提交是否提到已知问题；若有，先修复这些既有问题。
- 不接受变通方案、兼容性垫片或仅为夹具通过而做的临时处理；如果发现规格不匹配，必须先把该问题前置到 `TODO.md`/`PLAN.md`。
- 所有输出、记录与后续说明统一使用中文。

## 初始步骤

1. 查看最新提交信息，确认是否显式提到待修复问题。
2. 阅读 `TODO.md`，定位第一个未完成任务。
3. 阅读 `PLAN.md`，确认现有计划与任务顺序是否一致。
4. 结合代码现状判断该任务是否可在本轮完整完成。
5. 如果任务过大，则把它拆成更小子任务，并同步更新 `PLAN.md` 与 `TODO.md`，本轮只做拆分后的第一个子任务。

## 实施步骤

1. 阅读相关代码、测试和规格说明，建立实现边界。
2. 直接实现第一个未完成任务，不绕过现有缺陷。
3. 补充或调整测试，覆盖新增行为与回归风险。
4. 运行相关验证命令，至少包括与改动直接相关的测试；如果改动涉及整体质量门槛，则运行 `cargo clippy --all-targets -- -D warnings`。
5. 更新 `TODO.md` 与 `PLAN.md`，记录完成状态、依赖变化或阻塞原因。
6. 提交一次 Git commit，提交信息应明确描述本轮任务。
7. 停止，不继续处理下一个任务。

## 进度记录规则

- 每完成一个关键步骤，就更新本文件。
- 如果执行中改变计划、发现新的前置缺陷，必须在这里补充原因、影响和后续动作。

## 当前状态

- 已创建本轮计划文件。
- 已查看最新提交 `b861cab55748c7fcf11d2d31903f6ace94f22933`，提交说明只有 `[T4006] 收口跨文件 / 跨包编译链路`，未在提交正文中额外挂出待修问题。
- 已读取 `TODO.md`、`PLAN.md`、`ISSUES.md`，确认当前第一个未完成任务为 `T4006S`：修复 delegated property `lazy(None)` 读取进入 `print/println` 时的 codegen 类型缺口。
- 已复现 `tests/fixtures/run-pass/delegated_property_lazy_thread_safety_none_single_thread_ok.scoop` 的失败：`cargo run -p scoop -- build ...` 报 `scoop::llvm::unsupported_main_body: sysroot print/println arg type`。
- 已定位根因：`lower_lazy_delegated_property_get_from_receiver` 在 `LazyThreadSafetyMode.None` 分支把 getter 降成 `when` 后，没有把外层 HIR 类型保留为真实属性类型，导致 LLVM `print/println` lowering 把读取结果误当成 `Any/Ref`。
- 已完成实现：
  - 将 lazy delegated property getter lowering 改为返回 `(ExprKind, TypeId)`。
  - `LazyThreadSafetyMode.None` 分支的 `when` / block / tail 统一携带真实属性 `TypeId`。
  - 新增 run-pass 回归 `tests/fixtures/run-pass/delegated_property_lazy_thread_safety_none_print_like_ok.scoop`，覆盖 `print` + `println` 共用路径。
- 已完成验证：
  - `delegated_property_lazy_thread_safety_none_single_thread_ok.scoop` 现可成功 build/run，stdout 为 `init / 7 / 7`。
  - 新增 `delegated_property_lazy_thread_safety_none_print_like_ok.scoop` 可成功 build/run，stdout 为 `init / 7,7`。
  - 既有 `lazy` 默认 / `Synchronized` / `Publication` 路径都已复验 build/run 正常。
  - `cargo test --all` 通过。
  - `cargo clippy --all-targets -- -D warnings` 通过。
- 全量验证补充结论：
  - `cargo run -p scoop -- test` 已不再卡在 `delegated_property_lazy_thread_safety_none_single_thread_ok.scoop`。
  - 全量套件继续向后暴露出新的既有 blocker：`tests/fixtures/run-pass/gc_continuation_cross_thread_resume_with_objects.scoop` 在 build 阶段报 `scoop::llvm::unsupported_main_body: value coercion`。
  - 已据此在 `TODO.md` / `PLAN.md` 中前插新任务 `T4006T`，用于下一轮先修复该既有 failure，再继续 `T4006R`。
- 当前进行中：检查文档更新后的工作区状态，准备提交本轮 `T4006S` 完成记录与新 blocker 排序调整。
