## 本轮执行计划

说明：按要求先记录执行计划。我不会写出内部完整思维链，但会记录可审阅的执行步骤、判断依据和后续更新。

1. 检查最新一次 git 提交信息，确认是否明确提到需要先修复的既有问题。
2. 阅读 `TODO.md`，定位第一个未完成任务。
3. 阅读 `PLAN.md`，核对当前任务背景、依赖和已有拆解。
4. 如果首个未完成任务过大或被既有问题阻塞：
   - 先识别阻塞点或缺失能力；
   - 更新 `PLAN.md`；
   - 调整 `TODO.md` 顺序并补充前置子任务；
   - 本轮只完成新的首个可执行任务，或仅提交任务重排结果后停止。
5. 如果任务可直接执行：
   - 阅读相关代码与测试；
   - 做最小且正确的实现修改；
   - 补充/调整测试；
   - 运行相关验证，再运行必要的格式化、测试和 `cargo clippy --all-targets -- -D warnings`；
   - 发现任何既有问题则先修复，或把它作为前置任务插入 `TODO.md`。
6. 完成后更新文档状态：
   - 在 `TODO.md` 中标记该任务完成；
   - 在 `PLAN.md` 中记录实际完成情况与剩余顺序；
   - 在本文件记录关键进展和计划变化。
7. 按仓库提交风格创建一次 git commit，然后停止，不进入下一个任务。

## 进展记录

- 已创建本计划文件，下一步开始检查最新提交与任务清单。
- 已检查最新提交：`git log -1 --stat --decorate` 仅显示 `Update plan`，提交信息未显式提出需要先修复的遗留问题。
- 已读取 `TODO.md` 与 `PLAN.md`。首个未完成任务为 `T5001f8`：将 state-machine 的 frame-backed locals 收口为“稳定执行期 local home + 统一 flush-back”设计。
- 判断：`T5001f8` 影响 handle/state-machine lowering、outer mutable local promotion、suspend/return/cleanup 边界与多个回归 fixture，单条任务过大；应先拆成更小的顺序子任务，并在本轮只执行拆分后的第一个子任务。
- 下一步：
  1. 阅读与 `T5001f8` 直接相关的 lowering 代码和阻塞 fixture；
  2. 确认目前哪些 local 仍直接把 heap frame field GEP 暴露为 `CgLocal.ptr`；
  3. 将 `T5001f8` 拆成显式子任务并回写 `TODO.md` / `PLAN.md`；
  4. 实现新的首个子任务；
  5. 运行定向验证；
  6. 更新计划与任务状态后提交。
- 已将 `T5001f8` 拆分为 `T5001f8a/T5001f8aR/T5001f8b/T5001f8bR/T5001f8c`，并先尝试执行 `T5001f8a`。
- 已完成的局部实现：
  - `write_back_outer_scope_frame_slots(...)` 现在在 handle 仍位于原 caller activation 时，会优先经 caller 当前稳定 backing slot 回写 outer mutable local；
  - 新增最小 fixture `tests/fixtures/run-pass/effect_escape_continuation_outer_mutable_writeback_basic.scoop`，锁定默认环境下 handle-return 后立刻读回 outer mutable local 的窗口。
- 定向验证结果：
  - `cargo run -p scoop -- run tests/fixtures/run-pass/continuation_resume_enum.scoop`：已恢复通过；
  - `cargo run -p scoop -- run tests/fixtures/run-pass/effect_escape_continuation_outer_mutable_writeback_basic.scoop`：通过；
  - `env SCOOP_GC_MOVE=1 SCOOP_GC_STRESS=1 SCOOP_GC_VERIFY_ROOTS=1 cargo run -p scoop -- run tests/fixtures/run-pass/continuation_resume_enum.scoop`：仍失败，输出 `missing1/missing2`。
- 新发现的前置 blocker：`populate_frame_slots_in_env(...)` 仍把 state-body outer mutable local 直接暴露成 heap-frame GEP env local，导致 `saved = Some(k)` 在 GC env 下继续把写入落到 stale heap-slot pointer。按规则，已把该问题前移为新任务 `T5001f8a0/T5001f8a0R`，并把原 `T5001f8a` 顺延到其后。
- 本轮收尾计划：
  1. 再做一次与当前变更直接相关的格式/编译检查；
  2. 更新 `TODO.md` / `PLAN.md` 保持任务顺序正确；
  3. 提交本轮“任务拆分 + 默认路径加固 + 新前置 blocker 记录”的结果后停止。
