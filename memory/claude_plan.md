# 本轮执行计划

## 目标

- 只处理 `TODO.md` 中第一个未完成任务 `T5000e3c`。
- 在继续实现前，先检查最新提交是否提到需要优先修复的既有问题。
- 不接受绕过方案；如果验证中发现真实缺陷，先修复缺陷或将其作为前置任务加入 `TODO.md`。

## 当前已知状态

- 工作区已经存在与 `T5000e3c` 相关的未提交修改，本轮需要在这些修改基础上继续推进，不能回退无关改动。
- 已有一批围绕 `main` 入口签名与返回类型推断的修复和 fixture 调整，但任务尚未完成。
- 关键剩余工作是继续跑完整测试，修复因为真实入口契约收紧而暴露出的旧 fixture / 实现问题，并完成任务收尾。

## 执行步骤

1. 查看最新提交信息，确认是否有必须优先修复的既有问题。
2. 查看 `TODO.md` 与 `PLAN.md`，确认 `T5000e3c` 仍是第一个未完成任务，并核对当前计划是否需要细化。
3. 检查工作区状态，明确已修改文件范围，避免覆盖现有进展。
4. 继续运行与 `T5000e3c` 直接相关的验证，优先完成 `cargo run -p scoop -- test`。
5. 如果测试失败：
   - 判断是旧 fixture 需要按新入口契约修正，还是暴露出新的真实实现缺陷。
   - 对真实缺陷直接修复；若当前无法在本轮完整解决，则按要求把前置任务写入 `TODO.md` / `PLAN.md` 后停止。
   - 对确属过时假设的 fixture，按规范更新并补充必要断言。
6. 在 fixture 套件通过后，继续运行：
   - `cargo test --all`
   - `cargo clippy --all-targets -- -D warnings`
7. 所有相关验证通过后：
   - 更新 `memory/claude_plan.md` 记录完成情况。
   - 更新 `TODO.md`，将 `T5000e3c` 标记为完成。
   - 更新 `PLAN.md`，记录本轮完成内容与后续状态。
   - 提交 git commit，然后停止，不进入下一任务。

## 当前进展（2026-04-26）

- 已检查最新提交 `c72a6370 Update plan`，未发现提交说明中要求优先处理的新历史问题。
- 已确认 `TODO.md` 中首个未完成任务仍为 `T5000e3c`。
- 已完成的直接验证：
  - `target/debug/scoop test --fixtures tests/fixtures/typecheck`：通过（395）
  - `target/debug/scoop test --fixtures tests/fixtures/runtime_gc`：通过（24）
  - `target/debug/scoop test --fixtures tests/fixtures/run-pass`：通过（394）
  - `target/debug/scoop test --fixtures tests/fixtures/spec_doctest`：通过（1）
- 本轮新增修正：
  - 更新 `tests/fixtures/typecheck/entry_point_main_param_not_array_string_is_error.scoop`，将预期错误子串从旧的 `Array<Int>` 对齐到当前实际诊断中的 `scoop.core.Array<Int>`。
- 下一步：
  - 回写完成状态并提交

## 完成情况（2026-04-26）

- 已完成全量验证：
  - `cargo run -p scoop -- test`：通过（`fixtures: ok (1201)`）
  - `cargo test --all`：通过
  - `cargo clippy --all-targets -- -D warnings`：通过
- 已更新 `TODO.md`：`T5000e3c` 现已标记为完成，并补记实现要点与验证结果。
- 已更新 `PLAN.md`：记录 `T5000e3c` 的完成情况，并将下一条待执行任务切换为 `T5000e3cR`。
- 本轮最终代码/fixture 收尾修正只有 1 处新增变更：
  - `tests/fixtures/typecheck/entry_point_main_param_not_array_string_is_error.scoop` 的期望错误子串已对齐到当前实际诊断中的完整限定类型名。
- 下一步：
  - 查看最终 diff
  - 提交本轮变更
  - 停止，不进入 `T5000e3cR`

## 判定标准

- `T5000e3c` 涉及的实现、fixture、文档与验证全部闭环完成。
- 不存在未记录的阻塞性既有问题。
- 本轮结束前必须有清晰的计划记录、任务状态更新、验证结果与 git 提交。
