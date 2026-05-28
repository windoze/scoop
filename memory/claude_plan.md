# 当前执行计划

> 说明：本文件记录可审计的执行计划、关键决策和进度更新；不记录不可见的内部推理细节。

## 初始计划

1. 读取 `TODO.md`，按文件顺序识别第一个标题未带 `[DONE]` 的任务，并只处理该任务。
2. 检查该任务的要求、依赖、验证方式和完成记录；必要时查看 `PLAN.md` 了解阶段背景，但不把 `PLAN.md` 当作任务账本。
3. 检查最新提交信息是否明确提到与当前任务直接相关的未完成问题；若相关，将其纳入当前任务或作为前置任务记录到 `TODO.md`。
4. 为当前任务收集最小必要上下文，定位相关代码、测试和夹具。
5. 若任务可直接实施，进行最小正确修改；若发现必须先修复的具体前置缺口，则更新 `TODO.md`，提交后停止。
6. 按要求运行格式化、lint、相关测试，以及需要时的完整测试/fixture 套件；发现未排期失败时修复或新增最小前置任务。
7. 任务完成后，在 `TODO.md` 的任务标题前加 `[DONE]` 并更新完成记录；仅当阶段级计划变化时更新 `PLAN.md`。
8. 检查工作区差异，提交本次任务涉及的所有必要变更，然后停止，不处理下一个任务。

## 进度记录

- 已创建初始计划，下一步读取 `TODO.md` 选择第一个未完成任务。
- 已确认第一个未完成任务为 `P3-T05：禁止 closure 捕获外层 var`，任务源位于 `TODO-3.md`。最新提交为 `f6eac748 [P3-T04R] Review refutable val patterns`，未显示与 P3-T05 直接相关的未完成问题。
- 当前执行重点：在前端/typecheck 或闭包捕获分析阶段识别跨 closure 边界捕获的外层 `var`，给出清晰诊断并保留 `val` 捕获行为；补充 makeCounter 风格负例与 val snapshot 正例。
- 已读取 `SPEC_FIX.md` B5 与 `PLAN.md` P3，确认应在 sema/typecheck 侧报错，提示 `RefCell<T>`、显式 `val snapshot = ...`、fold/higher-order accumulation；不改变 SLIR/codegen closure 环境布局。
- 实施方案：新增共享的 closure capture 检查器，遍历当前 lambda body 的 AST 引用，跳过嵌套 lambda；若引用的 local decl span 属于当前 closure 外层已知 `var` binding，则返回 `scoop::typecheck::closure_var_capture_not_allowed`。在 lambda value inference 与 statement-position structural check 两条路径调用，保证普通 `val` capture 和 closure 内部局部 `var` 使用不误报。
- 针对性 fixture 已通过；完整 Rust 测试首次发现 6 个旧单元测试样例仍依赖 captured-`var` lowering / LLVM 行为，这是 P3-T05 直接替换的旧语义。下一步迁移这些测试样例到 `RefCell` 或 `val` capture，并删除不再合法的 mutable-capture lowering 断言。
- 已迁移旧 MIR/LLVM 单元测试样例：`assignment_places` 改用显式 snapshot，aggregate/LLVM composite closure 改用 `RefCell`，旧 “mutable capture lowers to per-call local” 单元测试改为确认 typecheck 在 MIR 前拒绝。`cargo test -p scoopc --lib` 已通过。
- 完整验证状态：`cargo fmt` 已运行；`cargo clippy --all-targets -- -D warnings` 通过；`cargo test --all --all-targets` 通过；首次完整 fixture 因两个 MIR golden 漂移失败，更新 `assignment_places.mir` 与 `aggregate_transport.mir` 后 targeted fixture 通过，最终 `python3 tools/run_fixtures.py` 通过（`fixtures: ok (1556)`）。
- 已将 `TODO.md` 索引和 `TODO-3.md` 中的 `P3-T05` 标记为 `[DONE]`，并写入完成记录；`PLAN.md` 无阶段级变化，未修改。
