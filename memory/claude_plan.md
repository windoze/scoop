# 本轮执行计划

说明：按要求在这里持续记录“可执行计划、关键决策、阻塞原因与进度更新”。出于安全限制，不记录逐字内部推理，但会完整记录我将执行的步骤与结论。

## 初始计划

1. 检查最新一次 Git 提交的信息，确认是否提到任何已知遗留问题。
2. 如果最新提交提到需要先修复的遗留问题，先定位并修复这些问题，再继续后续步骤。
3. 阅读 `TODO.md`，识别第一个未完成任务。
4. 判断该任务是否足够明确且可在本轮完整完成：
   - 若可以，直接实现。
   - 若过大或依赖未满足，则细化为更小子任务，更新 `PLAN.md` 与 `TODO.md`，并执行拆分后的第一个子任务。
5. 在实现前阅读相关代码与测试，明确影响范围与现有约束。
6. 实现任务，必要时补充或整理注释、模块结构与文档。
7. 运行相关验证：
   - 至少运行与改动直接相关的测试；
   - 若改动影响面较广，补充运行更高层级测试；
   - 按要求运行 `cargo clippy --all-targets -- -D warnings`，若时间或环境阻塞则在此记录。
8. 更新文档与任务状态：
   - 在 `TODO.md` 中将本轮完成任务标记为已完成；
   - 在 `PLAN.md` 中同步当前状态、后续依赖或拆分结果；
   - 持续更新本文件记录关键进展。
9. 检查工作区改动，确保不误改无关内容。
10. 提交一个清晰的 Git commit，只完成这一项后停止。

## 进度记录

- 已创建本文件，准备开始检查最新提交与任务列表。
- 已检查最新提交：`f86a951f8d02f967dde834e575135b6c7d2ad897`，提交信息为“`[T3009b0a1] Track resume-path outer-slot writeback gap`”。该提交主要更新 `TODO.md` / `PLAN.md` / 本记录文件，用于前移并跟踪一个新的前置缺口，没有包含额外已实现代码修复。
- 已读取 `TODO.md` / `PLAN.md`，当前第一个未完成任务为 `T3009b0a1`：为 escaped continuation 的恢复完成路径补齐 outer-scope seeded slot 写回合同。
- 当前判断：先直接围绕 `Continuation.resume(...)` 恢复后的 completion path 做代码与测试定位；若发现任务范围仍过大或存在新的更前置缺口，再按要求细化 `TODO.md` / `PLAN.md`。
- 已完成代码定位并实现结构层修复：
  - `FrameLayout` 现会为 seeded outer mutable slot 记录 authoritative storage pointer metadata；
  - `seed_outer_scope_frame_slots` 会把 storage pointer 一并写入 frame；
  - `write_back_outer_scope_frame_slots` 已改为只依赖 frame metadata；
  - step function 的 `ReturnHandle` / `ReturnFromFunction` / `Suspend` / `Arm*` 返回出口与外层 `handle_done` / `handle_propagate` 共用同一 writeback helper。
- 已新增 LLVM IR 单测 `escaped_continuation_resume_ir_records_outer_slot_storage_and_writeback`，用于锁定 frame metadata + step-return writeback 基础设施。
- 在尝试把“`resume()` 返回点可见的 outer-local 同步”做成 run-pass fixture 时，直接复现得到输出停在 `body_resumed`，确认 caller-tail 仍未继续。这说明原 `T3009b0a1` 同时混合了“结构合同实现”和“依赖 `T3009b0` 的可观测验收”两部分。
- 已按此结论更新 `TODO.md` / `PLAN.md`：将原 `T3009b0a1` 拆为已完成的 `T3009b0a1a`（结构合同）和后续 `T3009b0a1b`（待 `T3009b0` 后执行的 focused fixture 验收），并把下一前置顺序调整为 `T3009b0`。
- 验证已完成：
  - `cargo test -p scoopc escaped_continuation_resume_ir_records_outer_slot_storage_and_writeback -- --nocapture`
  - `cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/effect_escape_continuation_outer_var_writeback_basic.scoop`
  - `cargo test --all`
  - `cargo clippy --all-targets -- -D warnings`
- 当前状态：本轮只完成 `T3009b0a1a`，未进入 `T3009b0`；下一次调用应从 `TODO.md` 中新的首个未完成任务 `T3009b0` 继续。
