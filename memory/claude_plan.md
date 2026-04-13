# 执行摘要与计划

## 约束说明

- 按用户要求，本次只处理 `TODO.md` 中第一个未完成任务，完成后即停止。
- 在继续任务前，先检查最新提交是否提到需要先修复的既有问题；若有，则这些问题优先。
- 若当前首个未完成任务过大或被前置缺陷阻塞，需要先更新 `PLAN.md` 与 `TODO.md`，把问题拆分或重排后再执行当前最前面的可执行子任务。
- 代码修改后必须运行充分测试，并确保无新的编译 / clippy 警告。
- 本文件将持续记录关键决策、执行步骤与进度变化。

## 初始执行计划

1. 查看最新一次 git 提交，确认提交说明里是否提到仍需先处理的既有问题。
2. 读取 `TODO.md`，定位第一个未完成任务。
3. 读取 `PLAN.md` 与相关上下文，确认任务背景、依赖和验收标准。
4. 判断该任务是否可以直接完成：
   - 若可以，直接实现；
   - 若过大或被阻塞，则先拆分 / 重排 `TODO.md` 与 `PLAN.md`，并把新的最前子任务作为本次目标。
5. 阅读相关代码与测试，定位需要修改的模块、现有行为和潜在风险。
6. 实施代码修改，并在必要时补充或调整测试。
7. 运行相关验证：
   - 最小相关测试；
   - 必要时运行更广的测试集；
   - `cargo clippy --all-targets -- -D warnings`（若工作量和影响面需要）。
8. 更新文档状态：
   - 在 `TODO.md` 中标记完成，或在阻塞时重排任务；
   - 在 `PLAN.md` 中记录当前状态和后续计划；
   - 回写本文件，记录关键步骤和结果。
9. 使用清晰的提交信息提交本次变更，然后停止。

## 进度记录

- 已创建本文件，准备开始检查最新提交与任务列表。
- 已检查最新提交：最近一次提交为 `[T2003r3d2b] Reconnect unified single-resuming leaves`，提交说明中未额外标出需要优先修复的既有 issue。
- 已读取 `TODO.md` / `PLAN.md`，当前最前未完成任务为 `T2003r3d2c`。
- 经过代码审计，确认 `T2003r3d2c` 当前实际同时覆盖三类 multi-resuming leaf 接线工作：`stack-reentry-only`、`heap-continuation-only`、以及 `1 immediate + 1 escape` 当前 legal mixed。三者依赖的 emitter / dispatch / sibling non-resuming 复用面不同，单轮继续整包推进风险过高。
- 决策：先把 `T2003r3d2c` 细分为三个子任务，并在本轮执行第一个子任务：
  1. `T2003r3d2c1`：接回 unified multi-resuming leaf 的 `stack-reentry-only` 基线，覆盖多个 immediate-resume arms 以及 sibling non-resuming / `finally` 的 representative sample。
  2. `T2003r3d2c2`：接回 unified multi-resuming leaf 的 `heap-continuation-only` 基线，覆盖多个 escape-continuation arms 以及 sibling non-resuming / `finally` 的 representative sample。
  3. `T2003r3d2c3`：接回 unified multi-resuming leaf 的当前 legal `1 immediate + 1 escape` mixed 基线，并为后续 `T2003r3d3` / `T2003r3d4` 保留 arm-count 扩展空间。
- 已完成 `T2003r3d2c1` 代码接线：
  - 新增 `crates/scoopc/src/llvm/codegen/effect/multi_resuming.rs`，把 unified `stack-reentry-only` multi-resuming leaf 独立出来；
  - 在 `mod.rs` / `shared.rs` 中恢复 generic sibling non-resuming dispatch metadata 与 block helper；
  - `nonresuming.rs` 的 `MultiResuming` 分支已对 `stack-reentry-only` route 走新 leaf，对剩余 pure escape-only 与 `1 immediate + 1 escape` mixed route 改成稳定显式诊断。
- 已补测试与 representative fixture：
  - LLVM 定向单测：一个正向 stack-reentry-only sample，两个 pending-route 诊断 sample；
  - run-pass fixture：`tests/fixtures/run-pass/effect_handle_yield_and_step_finally.scoop`。
- 已完成验证：
  - `cargo fmt --all`
  - `cargo test -p scoopc unified_multi_resuming_codegen_ -- --nocapture`
  - `cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/effect_handle_yield_and_step_finally.scoop`
  - `cargo clippy --workspace --all-targets -- -D warnings`
- 下一步：回写 `TODO.md` / `PLAN.md` 的完成状态，检查最终 diff，然后提交本轮 `T2003r3d2c1` 实现并停止。
