# 执行计划记录

说明：按要求记录执行计划、关键步骤进展与必要调整。这里记录的是可审阅的执行摘要，不包含逐字内部推理。

## 初始计划

1. 检查最新一次 Git 提交，确认提交说明或关联改动里是否暴露了需要先修复的既有问题。
2. 阅读 `TODO.md`，定位第一个未完成任务。
3. 阅读 `PLAN.md`，理解现有任务顺序、依赖与预期交付物。
4. 评估该任务是否足够小且可在本轮完整交付。
5. 如果任务过大或存在前置缺口：
   - 拆分为更小子任务；
   - 更新 `PLAN.md`；
   - 调整 `TODO.md` 中的任务顺序与依赖；
   - 本轮只执行新的第一个子任务。
6. 实现当前目标任务，避免任何规避式方案；若发现规范缺口或实现边界，先把缺口转化为更前置的任务。
7. 运行相关验证：
   - 最小必要测试；
   - 相关集成/回归测试；
   - `cargo fmt`；
   - `cargo clippy --all-targets -- -D warnings`（若与当前改动相关且成本可接受）；
   - 其他该任务直接涉及的验证命令。
8. 更新文档与任务状态：
   - 在 `TODO.md` 标记当前任务完成，或在受阻时重排任务；
   - 更新 `PLAN.md` 反映现状与后续依赖；
   - 按需补充 `README.md` / 注释 / 测试。
9. 检查工作区变更，确认未误改无关内容。
10. 以清晰提交信息创建一次 Git 提交，然后停止，不继续处理下一个任务。

## 进展日志

- 已写入初始计划。
- 已检查最新提交：`6dd5953966f15862c99080f2ddcb04f1dc389fe0`，提交说明为 `[T4005R] 收口 Elvis review 裂缝`。
- 已阅读 `TODO.md` 与 `PLAN.md`，确认当前第一个未完成任务为 `T4005S`：收口 `when` / pattern binder 中函数值的可调用 lowering / codegen。
- 该任务同时也是最近一轮复审明确暴露出的既有问题，属于必须先处理的前置 blocker。
- 已复现故障：
  - `when (maybe) { Some(f) -> f(); None -> 0 }` 在 LLVM 阶段报 `call callee` unsupported；
  - `when (pair) { (g, n) -> g() + n }` 同样失败；
  - 对照验证表明，局部 `val (f, _) = pair; f()` 早已可执行，裂缝集中在 `when` binder 主线。
- 已定位根因：
  - LLVM `bind_when_pat` 把 binder local 的 `hir_ty` 一律丢成 `None`；
  - 更早一层的 typecheck 也没有把 `when_pat::infer_when_pat_bindings` 的结果写回 `inferred_binding_tys` side table，导致 typed HIR lowering 无法恢复 binder 的精确类型。
- 已完成修复：
  - typecheck 现会把 `when` pattern binder 的推断类型写回 side table；
  - HIR lowering 新增 `when_pat_binding_tys` side table，并在 source/synthetic binder 处填充；
  - LLVM `bind_when_pat` 现按当前源文件 + binder span 恢复 `hir_ty`，并同步恢复 `call_may_suspend` 的类型层信息。
- 已新增回归：
  - `tests/fixtures/run-pass/when_pattern_function_value_call_basic.scoop`
  - `tests/fixtures/run-pass/when_pattern_function_value_call_basic.stdout`
- 已完成验证：
  - `cargo run -p scoop -- run tests/fixtures/run-pass/when_pattern_function_value_call_basic.scoop`
  - `cargo run -p scoop -- test --fixtures <临时 root，仅包含 when_pattern_function_value_call_basic>`（`fixtures: ok (1)`）
  - `cargo run -p scoop -- test --fixtures tests/fixtures/typecheck`（`fixtures: ok (329)`）
  - `cargo test --all`
  - `cargo clippy --all-targets -- -D warnings`

## 当前执行细化

1. 复现 `Some(f) -> f()` 一类 pattern binder 函数值调用在 LLVM 阶段的失败。
2. 阅读与 callable-value、pattern binder、`when` lowering、LLVM call callee 选择相关的实现，定位“函数值可调用元数据”在哪一步丢失。
3. 在不新增旁路特判的前提下修复主线：
   - 让 pattern binder 引入的函数值保留与普通局部函数值一致的可调用表示；
   - 确保 `when` / pattern binder lowering 与现有 callable-value codegen 共享同一套元数据来源。
4. 新增或更新最小回归：
   - 至少覆盖 `when (x) { Some(f) -> f(); None -> ... }`；
   - 如有必要，补一个非 `when` 的 pattern binder 调用用例，证明不是单一语法形态补丁。
5. 运行定向验证，再运行全量要求的格式化 / 测试 / lint。
6. 更新 `TODO.md`、`PLAN.md`、本计划记录并提交。
