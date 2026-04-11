# 执行计划记录

## 说明

按要求，先写入本次执行的计划与过程记录。这里记录的是可审阅的执行思路摘要、步骤、检查点和后续变更，不包含不可审阅的内部完整思维链。

## 当前目标

完成 `TODO.md` 中第一个未完成任务，并在完成后停止。

## 执行顺序

1. 检查最新一次 Git 提交，确认提交信息里是否提到了已知问题、遗留问题或需要顺带修复的事项。
2. 读取 `TODO.md`，定位第一个未完成任务。
3. 读取 `PLAN.md`，理解当前项目计划与该任务上下文。
4. 判断该任务是否可以在本轮完整落地：
   - 如果可以，直接实现。
   - 如果过大，先拆分成更小子任务，并同步更新 `TODO.md` 与 `PLAN.md`，当前只执行拆分后的第一个子任务。
5. 在实现前梳理受影响模块、测试位置、相关文档（必要时包括 `README.md`）。
6. 实施代码修改，优先保持模块边界清晰，必要时做小范围重构以支撑任务完成。
7. 运行与该任务相关的验证：
   - 至少运行针对性测试；
   - 若改动影响较大，再补充更广的测试；
   - 按要求检查无警告，包括 `cargo clippy --all-targets -- -D warnings`（若依赖环境允许）。
8. 若测试或 lint 失败，立即修复直到通过，或在确认是前置依赖缺失后回退到“依赖调整流程”。
9. 更新文档与进度：
   - 在 `TODO.md` 标记当前任务完成；
   - 在 `PLAN.md` 反映当前状态与剩余计划；
   - 在本文件追加关键进展记录。
10. 查看工作区差异，确认不误伤用户已有改动。
11. 提交 Git commit，提交信息明确对应任务。
12. 停止，不继续处理下一个任务。

## 风险与处理原则

- 不回退我未创建的现有改动。
- 如果发现任务依赖尚未实现的语言特性或库能力，不会硬做；会调整 `TODO.md` / `PLAN.md` 反映依赖并提交。
- 如果最新提交中提到了需要先修复的问题，这些问题优先于当前任务。
- 若遇到任务范围过大，会先拆分并只完成拆分后的第一项。

## 进度日志

- 2026-04-11：初始化本文件，尚未开始仓库检查。
- 2026-04-11：已检查最新提交 `5ae52bcee03d0a3b95f85ce53b57b63472a52055`，提交信息未额外声明必须先处理的遗留问题；当前工作区已有对本文件的改动，后续仅在此基础上追加。
- 2026-04-11：已读取 `TODO.md` / `PLAN.md`，首个未完成任务为 `T2003c0b2b1`：mixed-arm immediate-resume + sibling escape-continuation 的 post-immediate multiple direct sites。
- 2026-04-11：已完成代码审计，确认现有 direct mixed-arm lowering 仅支持单个 top-level direct escape site；对应限制点在 `crates/scoopc/src/llvm/codegen/effect.rs` 中的 `codegen_handle_expr_immediate_resume_with_escape_sibling_direct(...)`。
- 2026-04-11：决定不再拆分任务，直接实现当前子任务。计划如下：
  1. 将 direct mixed-arm 的 escape site 收集从单个站点扩展为多个 top-level `val = perform` 站点，并保留对 pre-immediate / nested / indirect 组合的稳定诊断。
  2. 把 mixed-arm escape state 从“单 resume site”扩展成带 `pc` 的多站点状态机，让 step trampoline 能在每次 `resume(...)` 后继续跑到下一个 direct site 或 tail 结束。
  3. 复用统一的 escape capture 存储协议（word / gc_ref），避免继续维持 direct mixed-arm 的手写 `Ref/String/Bool/Int` 分支。
  4. 增加 run-pass 回归覆盖“immediate site 之后多个 direct escape sites”，并移除旧的 build-fail 预期。
  5. 运行针对性测试，再补 `cargo test --all`、`cargo run -p scoop --features llvm -- test`、`cargo clippy --workspace --all-targets -- -D warnings`。
- 2026-04-11：代码实现已完成：
  - direct mixed-arm escape site 收集已扩展为多个 top-level direct sites；
  - mixed escape state 已新增 `pc` 字段，step trampoline 现在能在每次 `resume(...)` 后恢复当前 site 的结果并继续推进后续 sibling escape site；
  - outer/body capture 已改为统一走 `EscapeCaptureStorageKind`，不再局限于 `Ref/String/Bool/Int` 手写分支。
- 2026-04-11：已删除旧的 build-fail 夹具 `tests/fixtures/build/effect_resume_mixed_escape_is_error.scoop`，新增 run-pass：
  - `tests/fixtures/run-pass/effect_resume_mixed_escape_direct_multi.scoop`
  - `tests/fixtures/run-pass/effect_resume_mixed_escape_direct_multi.stdout`
- 2026-04-11：已完成验证：
  - 定点验证：`cargo run -p scoop --features llvm -- build tests/fixtures/run-pass/effect_resume_mixed_escape_direct_multi.scoop -o /tmp/effect_resume_mixed_escape_direct_multi` 后运行产物，stdout 与预期一致。
  - 全量验证：`cargo test --all` 通过。
  - 全量 LLVM fixtures：`cargo run -p scoop --features llvm -- test` 通过，结果 `fixtures: ok (926)`。
  - lint：`cargo clippy --workspace --all-targets -- -D warnings` 通过。
- 2026-04-11：已更新 `TODO.md` / `PLAN.md`，将 `T2003c0b2b1` 标记完成，并把下一步指向 `T2003c0b2b2`。
