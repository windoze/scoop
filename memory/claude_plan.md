# 本轮执行计划与进度记录

## 说明

按要求，本文件在执行仓库检查与代码命令前创建，并在后续关键步骤完成或计划调整时持续更新。

说明边界：
- 这里记录可共享的执行计划、判断依据摘要、关键发现、风险与进度。
- 不记录模型的私有内部推理逐字过程，但会尽量完整记录外显步骤，便于审阅。

## 当前目标

完成 `TODO.md` 中第一个未完成任务；若发现前置阻塞问题或最新提交中提到的遗留问题，则先修复这些问题，再决定是否继续当前任务；本轮只完成一个任务后停止。

## 执行步骤

1. 查看最新一次提交信息，确认是否明确提到已有问题、回归、临时方案或待补修复项。
2. 读取 `TODO.md`，定位第一个未完成任务。
3. 读取 `PLAN.md`，核对当前计划、任务编号、依赖关系和已有拆分。
4. 结合代码现状评估该任务是否可在本轮完整交付：
   - 若可直接完成，则进入实现。
   - 若过大或存在明显前置依赖，则在 `PLAN.md` / `TODO.md` 中拆分或重排，并以首个新子任务作为本轮目标。
5. 实现任务，同时检查是否遇到规范不一致、语言特性缺失、运行时/类型系统/诊断缺陷等真实阻塞：
   - 若遇到阻塞，不做规避实现。
   - 在 `TODO.md` 中新增前置修复任务并重排顺序。
   - 在 `PLAN.md` 中记录阻塞原因与依赖。
   - 提交后停止。
6. 对实现结果执行必要验证：
   - 至少运行与改动直接相关的测试。
   - 若范围允许，运行更广覆盖。
   - 按要求检查无警告：优先考虑 `cargo clippy --all-targets -- -D warnings`。
7. 更新文档状态：
   - 在 `TODO.md` 中标记任务完成或重排后的待办顺序。
   - 在 `PLAN.md` 中记录已完成内容、后续影响和剩余事项。
   - 若实现影响使用方式或仓库说明，检查 `README.md` 是否需要更新。
8. 检查工作树，确认只包含本轮相关改动，不回退用户已有改动。
9. 提交本轮改动，提交信息使用明确的任务描述。
10. 停止，不继续处理后续任务。

## 待记录项

- 最新提交检查结果：已完成；最新提交仅记录 `T4011b` 收口内容，未额外挂出新的先修 issue
- 第一个未完成任务：`T4011R Review：确认 or-pattern 没有偷偷放开 binder-sharing 或 bare-variant sugar`
- 是否需要任务拆分：不需要；本轮可直接作为 review 任务完成
- 实现摘要：已完成；补充 parser / typecheck 回归，锁定“禁止 binder-sharing”“禁止 bare payload variant sugar”两条边界
- 测试命令与结果：已完成；见下方“测试结果”
- 文档更新摘要：已完成；已更新 `TODO.md` / `PLAN.md`
- 最终提交信息：待执行

## 当前进展

- 已查看最新提交 `35ed218a319d4efbc62091bbe7adb942bcfe47f1`（`[T4011b] Lower when payload or-pattern matching`）。
- 最新提交说明未额外挂出新的“先修 issue”；当前仍按 `TODO.md` 顺序推进。
- 已确认 `TODO.md` 中第一个未完成任务为 `T4011R`，且 `PLAN.md` 中下一项也一致指向 `T4011R`。
- 当前执行策略：
  - 先复审 parser / typecheck / LLVM `when` payload-or-pattern 主线；
  - 再用最小 probe 与现有回归验证“禁止 binder-sharing”“禁止 bare payload variant sugar”两条边界；
  - 若无真实缺口，则把 `T4011R` 标记完成并补充 review 结论。

## 实现摘要

- 已补充 parser 单测 `parse_when_bare_variant_or_pattern_keeps_zero_arity_variants`，确认 bare `Hit | Miss` 在 parser 里仍只是两个 0 参数 variant pattern，不会被偷偷扩成 wildcard/rest payload sugar。
- 已新增 typecheck 负例：
  - `tests/fixtures/typecheck/when_or_pattern_variant_payload_binder_sharing_is_error.scoop`
  - `tests/fixtures/typecheck/when_or_pattern_variant_payload_bare_name_is_error.scoop`
- 复审结论：
  - `typecheck::when_pat::check_when_pat` 对 `ast::WhenPat::Or` 仍先做 `when_pat_contains_bind` 递归检查，因此 `A(x) | C(x)` 不会被宽松合并成共享 binder。
  - payload variant 若写成 bare `A | C`，仍会在 typecheck 命中 `when_variant_pat_arity_mismatch`，而不是被当成“忽略 payload”的合法 sugar。

## 测试结果

- `cargo test -p scoopc parse_when_ -- --nocapture`：通过。
- `cargo run -q -p scoop -- test --fixtures tests/fixtures/typecheck`：通过，`fixtures: ok (353)`。
- `cargo run -q -p scoop -- test`：通过，`fixtures: ok (1109)`。
- `cargo test --all -- --test-threads=1`：通过。
- `cargo clippy --all-targets -- -D warnings`：通过。

## 文档更新摘要

- 已将 `TODO.md` 中的 `T4011R` 标记为完成，并记录 review 结论、回归与验证命令。
- 已将 `PLAN.md` 更新到 `T4011R` 完成状态，并把下一项推进到 `T4011S`。
