## 当前执行计划

说明：我不会写入不可公开的内部推理细节，但会持续在此记录可执行计划、关键发现与进度更新。

1. 检查最新一次提交说明，确认是否提到了需要先修复的既有问题。
2. 阅读 `TODO.md`，定位第一个未完成任务。
3. 如任务过大，先把任务拆分并同步更新 `PLAN.md` 与 `TODO.md`，本次只执行拆分后的第一个子任务。
4. 实现当前目标任务，必要时先修复执行过程中发现的既有问题。
5. 运行相关测试与质量检查，修复发现的问题直到通过。
6. 更新 `TODO.md`、`PLAN.md` 与本文件，记录完成情况或阻塞依赖。
7. 按仓库提交规范创建一次 git 提交，然后停止。

## 本轮目标（2026-04-29）

 - 当前首个未完成任务：`T5000jR Review：确认优化主线已形成可持续扩展的中端体系`。
 - 最新提交 `[T5000j4R] Review safepoint baseline reuse` 未显式引入待先修复的既有缺陷条目；仍将在本轮 review / 验证中继续留意是否有真实问题暴露。
 - 当前工作树存在非本轮改动：`run_agent.sh` 已被修改；本轮不碰该文件。

## 当前执行步骤

1. 复核 `OPTIMIZATION.md`、`TODO.md`、`PLAN.md` 与关键实现入口，确认优化主线仍以 backend-agnostic MIR / summary / shared facts 为中心，而不是重新回退到 LLVM backend 现场推断。
2. 抽样检查 shared 中端模块与 LLVM backend 的依赖方向，重点看 `crates/scoopc/src/mir/**`、`crates/scoopc/src/effect/**`、`crates/scoopc/src/program_facts.rs`、`crates/scoopc/src/llvm/**` 的接缝。
3. 跑关键验证命令，至少覆盖全量测试与无 warning 构建；若 review 过程中发现既有缺陷，先修复再继续。
4. 回写 `TODO.md` / `PLAN.md` / 本文件并提交本轮 review 结论。

## 进度更新

 - 已确认首个未完成任务为 `T5000jR Review`，不需要再拆子任务。
 - 已抽样复核 `ProgramFacts`、`EffectAnalysisCtx`、shared `effect/state_machine`、`mir` 与 LLVM bridge/codegen 入口；当前未发现 shared 中端层反向依赖 `crate::llvm` 或 `inkwell`。
 - 已完成关键验证：
   - `cargo test --all`
   - `cargo test -p scoopc --no-default-features`
   - `cargo run -p scoop -- test`（`fixtures: ok (1204)`）
   - `cargo clippy --all-targets -- -D warnings`
 - 已回写 `TODO.md` 与 `PLAN.md`，将 `T5000jR` 标记完成，并记录“优化主线已稳定收口为 backend-agnostic 中端 + backend lowering 分层”的 review 结论。
 - 下一步只剩检查 diff、创建本轮提交，然后停止。
