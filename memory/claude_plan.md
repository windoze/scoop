# 执行计划与进度记录

## 说明

按要求先记录本次执行计划与后续进度更新。这里记录的是可审阅的任务分析、执行步骤、决策依据与状态变化，不包含内部推理原文。

## 初始目标

本轮只处理 `TODO.md` 中第一个未完成任务，并在完成后停止。

## 初始执行步骤

1. 检查最新一次提交，确认提交信息或相关变更中是否提到已有问题；如果发现属于当前仓库的既有问题，先修复这些问题。
2. 阅读 `TODO.md`，定位第一个未完成任务。
3. 阅读 `PLAN.md`，理解现有计划与任务依赖。
4. 查看当前工作区状态，避免覆盖用户已有改动。
5. 判断第一个未完成任务是否足够小且可直接完成：
   - 如果可直接完成：实现、补测试、运行相关验证。
   - 如果过大或被前置缺陷阻塞：在 `PLAN.md`/`TODO.md` 中拆分或重排任务，并只处理新的第一个子任务或阻塞整理。
6. 更新文档：
   - 在 `TODO.md` 中标记完成或调整顺序。
   - 在 `PLAN.md` 中记录当前状态、依赖和后续安排。
   - 在本文件中同步记录关键进展与计划变更。
7. 运行必要的格式化、测试与 `clippy`，确保无警告。
8. 提交本轮改动，提交后停止，不继续下一个任务。

## 预期检查项

- 最新提交是否暴露已有问题
- 当前第一个未完成任务的真实依赖
- 是否存在与规范不一致、不能靠变通绕过的问题
- 相关测试是否齐全并通过
- `cargo clippy --all-targets -- -D warnings` 是否通过

## 当前任务确认

- 当前第一个未完成任务：`T4005T`，目标是收口顶层 callable value（含顶层 pattern binder / `FunPtr`）的调用语义。
- 最近一次提交没有实现代码，而是把该问题显式登记为新的前置 blocker，因此本轮应直接从 `T4005T` 开始。

## 目前已知问题

- 顶层命名函数值 `val topF: () -> Int = { 11 }; topF()` 在 typecheck 阶段报 `callee_not_callable`。
- 顶层 pattern binder 产出的函数值 `val (topF, topN): (() -> Int, Int) = ...; topF()` 同样在 typecheck 阶段报 `callee_not_callable`。
- 顶层 `FunPtr` direct call 可通过 typecheck，但 LLVM codegen 阶段报 `call callee type`。

## 本轮细化计划

1. 复现上述三类失败，保留最小 probe，确认问题边界。
2. 阅读 typecheck 调用推断与 LLVM `codegen_call` 主线，确认顶层 callable value 的类型与 codegen 元数据在哪一步丢失。
3. 在不新增顶层专用旁路语义的前提下补齐主线：
   - 让顶层命名 `val` / 顶层 pattern binder 上的函数值像局部函数值一样通过调用检查；
   - 让顶层 `FunPtr` direct call 与其它 callable top-level value 共享同一套 lowering / codegen 路径。
4. 新增最小 run-pass 回归，至少覆盖：
   - 顶层命名函数值调用；
   - 顶层 pattern binder 函数值调用；
   - 顶层 `FunPtr` direct call。
5. 运行定向验证、`cargo test --all`、`cargo clippy --all-targets -- -D warnings`。
6. 更新 `TODO.md`、`PLAN.md` 与本文件，提交本轮改动后停止。

## 进度记录

- 已创建本计划文件。
- 已检查最新提交、`TODO.md`、`PLAN.md` 和工作区状态。
- 已确认本轮执行任务为 `T4005T`。
- 已复现三类起始问题：
  - `/tmp/t4005t_top_level_named_function_value.scoop`：`callee_not_callable`
  - `/tmp/t4005t_top_level_pattern_function_value.scoop`：`callee_not_callable`
  - `/tmp/t4005t_top_level_funptr_direct_call.scoop`：`call callee type`
- 已完成实现：
  - typecheck 现会在顶层调用未命中 `top_level_funs` 时回查 `top_level_types`，让顶层命名函数值与顶层 pattern binder 函数值复用既有函数值 direct-call 检查；
  - LLVM `codegen_call` 现会把顶层 callable value 按精确类型分流到“函数值间接调用”或“FunPtr 间接调用”主线，并统一经由 `codegen_top_level_value_ref` 读取顶层值；
  - 顺带修复了 tuple literal 元素缺少 expected-context 的既有裂缝，closure literal 作为 tuple 元素时不再落入 `expression kind` unsupported。
- 已新增回归：
  - `tests/fixtures/run-pass/top_level_callable_value_call_basic.scoop`
  - `tests/fixtures/run-pass/top_level_callable_value_call_basic.stdout`
- 已完成定向验证：
  - `cargo run -p scoop -- run tests/fixtures/run-pass/top_level_callable_value_call_basic.scoop`
  - `cargo run -p scoop -- test --fixtures /tmp/t4005t-fixtures`（`fixtures: ok (1)`）
- 已完成全量验证：
  - `cargo fmt --all`
  - `cargo test --all`
  - `cargo clippy --all-targets -- -D warnings`
- 已更新 `TODO.md` 与 `PLAN.md`，将 `T4005T` 标记为完成，并把下一项切换为 `T4005SR`。
- 当前待办：
  - 提交本轮改动，然后停止。
