# 执行计划记录

## 任务目标

本轮只处理 `TODO.md` 中第一个未完成任务，并在完成后停止。

## 约束与执行原则

- 先检查最新提交，若提交信息或随附变更暴露出既有问题，优先修复。
- 先读取 `TODO.md`，定位第一个未完成任务；如任务过大，先拆分并同步更新 `TODO.md` 与 `PLAN.md`。
- 实现必须符合规范，不能以绕过、兼容层、仅修夹具等方式“过关”。
- 实现后必须运行相关验证，至少覆盖格式化、测试、`clippy -D warnings`，并修复发现的问题。
- 需要持续更新本文件，记录关键进展、计划变更、阻塞原因与处理结论。
- 完成后更新 `TODO.md`、`PLAN.md`，提交一次清晰的 git commit，然后停止。

## 初始执行步骤

1. 检查最新一次 git 提交，确认是否提到已知问题或暴露未修复缺陷。
2. 读取 `TODO.md` 与 `PLAN.md`，定位当前应处理的首个未完成任务。
3. 判断任务是否过大；若过大，拆分为更小子任务并调整任务顺序。
4. 阅读相关代码与测试，确定实现边界、依赖与潜在规范缺口。
5. 实现任务，必要时补充或重构测试与文档。
6. 运行格式化、测试与 `clippy`，修复所有相关问题。
7. 更新 `TODO.md`、`PLAN.md` 与本文件中的结果记录。
8. 提交 git commit，并结束本轮。

## 进度记录

- 初始计划已写入，下一步开始检查最新提交与任务列表。
- 已检查最新提交 `c06147c`，提交主题仅为 `[T3016iR] Review unified SuspendCall inactive helper IR fix`，未额外暴露出需要先于当前任务处理的新问题。
- 已确认 `TODO.md` 的首个未完成任务为 `T3017`：回收 `T3006` 暂时 xfail fixtures，恢复 effect run-pass 基线。
- 已核对当前 `tests/fixtures/run-pass`：
  - 已无 `T3006: 暂时标记为 fail` 残留。
  - 仅剩 6 条 `EXPECT: fail`，其中 4 条是本来就应失败的 runner/诊断基线，2 条已明确转记到非 effect 任务（`T3304`、`T3406`）。
- 已运行 `cargo run -p scoop --features llvm -- test` 做最终基线复核；suite 未通过，新的首个失败点是 `tests/fixtures/run-pass/effect_indirect_perform_nonresuming_closure.scoop`。
- 已进一步复现并收窄问题边界：
  - `cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/effect_indirect_perform_nonresuming_closure.scoop` 当前稳定报 `scoop::llvm::unsupported_main_body: return value`。
  - 对照的 `cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/effect_indirect_perform_nonresuming_call_chain.scoop` 仍可通过，说明 shared ordinary helper call-chain 已闭环，当前缺口收窄为 closure/function-value callee 变体。
- 按阻塞规则，本轮不继续实现 `T3017`。已在 `TODO.md` 前插新的 blocker 任务 `T3016j` / `T3016jR`，并把 `T3017` 顺延到其后；`PLAN.md` 的当前执行顺序也已同步更新。
- 下一步只做本轮收尾：检查 diff、提交包含 `TODO.md` / `PLAN.md` / `memory/claude_plan.md` 的 git commit，然后停止，等待下一轮从 `T3016j` 开始。
