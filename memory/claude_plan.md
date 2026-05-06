# 当前执行计划

## 范围

- 本次只处理 `TODO.md` 中按顺序出现的第一个标题未带 `[DONE]` 的任务。
- `TODO.md` 是任务顺序、依赖、验收和完成记录的唯一权威来源。
- `PLAN.md` 只在阶段级计划、依赖或完成标准发生变化时更新。

## 步骤

1. 读取 `TODO.md`，定位第一个未完成任务，并检查其要求、依赖和验证方式。
2. 检查最近提交信息；仅当它明确提到与当前任务直接相关的未完成问题时，将其纳入当前任务或作为前置任务记录。
3. 根据当前任务读取最小必要代码和测试上下文，不做开放式历史问题扫描。
4. 如果存在阻塞当前任务的缺失语言特性、规范不匹配或实现边界，优先在 `TODO.md` 中加入最小必要前置任务，保留当前任务未完成，然后提交并停止。
5. 如果无阻塞，按任务要求实现最小正确改动，不引入绕路、夹具专用逻辑或弱化规格的实现。
6. 添加或更新相关测试/fixture，运行任务要求的验证命令及必要的回归测试。
7. 若验证失败，定位并修复与当前任务直接相关的问题，重复验证。
8. 验证通过后，在 `TODO.md` 中把当前任务标题前缀改为 `[DONE]`，并更新完成记录。
9. 更新本文件记录关键进展和实际执行结果。
10. 检查工作区变更，提交所有本次任务相关变更；如果本次是在恢复未完成任务，则按要求把当前未提交文件一起纳入提交。
11. 提交后停止，不继续处理下一个任务。

## 进度

- 已创建初始执行计划。
- 已读取 `TODO.md`，第一个未完成任务是 `HIR-T08：收口 class literal 与 reflection/platform intrinsic HIR contract`。
- 已检查最近提交：`522779ad [HIR-T07] Publish refactor HIR call contracts`，该提交与当前任务顺序直接相关，未发现需要先插入的新前置任务。
- 当前任务执行重点：消除 `ExprKind::Todo("class_lit")`，为 class literal 与 `nameOf<T>()`、`sizeOf<T>()`、`getPlatform()` 发布/展示明确 HIR contract，并按任务要求补测试与 fixture。
- 已实现 HIR `ClassLiteral` value primitive，v0 结果类型为 `String`，同时保留 source type、source FQN 与 metadata kind。
- 已让普通表达式 typecheck 接受 `Type::class` 并验证其类型引用，避免 runtime class literal 触达 HIR Todo。
- 已扩展 typed HIR intrinsic contract dump，明确 intrinsic allowed context 与 runtime fallback；新增定向单测覆盖 `nameOf<T>()`、`sizeOf<T>()`、`getPlatform()`。
- 已运行并通过：`cargo test -p scoopc --no-default-features refactor_hir_class_literal`。
- 已完成全部验证：placeholder inventory、no-Todo verifier、call contracts、dump-hir tests、三个指定 typecheck fixtures、reflection fixture dump-hir 与 clippy 均通过。
- 已将 `TODO.md` 中 `HIR-T08` 标记为 `[DONE]` 并补充完成记录。
