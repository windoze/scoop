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

## 新一轮执行计划（2026-04-29）

说明：按你的要求，本文件继续记录可审阅的执行计划、关键决策和进度；不写入私有推理细节。

1. 检查最新一次提交说明，确认是否带有需要优先修复的既有问题。
2. 阅读 `TODO.md`，定位当前第一个未完成任务；若上一轮尚未提交，先确认当前工作树与计划状态。
3. 如首个未完成任务过大，则只做必要拆分，并同步更新 `PLAN.md` / `TODO.md`。
4. 完成该任务或其首个子任务，过程中发现任何既有问题都优先修复或前置建账。
5. 运行相关测试与 `cargo clippy --all-targets -- -D warnings`，修复暴露问题。
6. 更新 `TODO.md`、`PLAN.md` 与本文件，最后按约定创建一次提交并停止。

## 本轮目标（Root Frame Round，2026-04-29）

- 最新提交 `Update plan` 只改了计划文件，未显式带出需要先修复的既有问题。
- 当前首个未完成任务：`T5001a 建立当前 roots 主线基线，盘清 stackmap / native_roots / extra root slots / effect 路径`。
- 本轮不做实现切换；目标是把现状固化成可复验 baseline，作为 `T5001b+` 的统一前提。

## 本轮关键发现

- runtime 当前 managed frame roots 主线仍是 `stackmap + unwind ctx`：`scoop_gc_stackmap_visit_roots_from_ctx(...)` 被 major/minor mark、compaction roots update 和 `verify-roots` 直接复用。
- `InNative` 线程当前不是只靠 `native_roots`；`scoop_enter_native(...)` 还会捕获 caller `stack_walking_ctx`，因此现状是 `native_roots + caller stackmap ctx` 双路径并用。
- pinned / handles / globals / heap object trace 已天然围绕 `void** slot` visitor，不属于后续 explicit frame 要替换掉的 stack walking 路径。
- 编译器当前 ordinary safepoint 靠 `with_conservative_gc_local_root_spills(...)` + `extra_gc_root_slots` 保守维持 stack-backed roots；hidden sret、indirect aggregate spill、ordinary resume 临时槽位都属于后续 explicit frame layout 需要吸收的对象。
- effect/state-machine 的长生命周期状态主要已经在 heap-backed frame / continuation object 上；这些 traced fields 继续走 heap trace 合同，而不是 activation explicit frame。

## 本轮进度更新

- 已把 baseline 固化到 `ROOT_FRAME_REFACTOR.md`“4.4 当前实现基线（T5001a）”。
- 已同步更新 `PLAN.md`，把该 baseline 提炼为 P0 的可引用事实。
- 已把 `TODO.md` 中的 `T5001a` 标记为 `[DONE]`，并留下 baseline 文档入口。
- 已完成验证：
  - `cargo test --all`
  - `cargo clippy --all-targets -- -D warnings`
- 本轮未在验证或盘点过程中发现需要优先插回 `TODO.md` 的新的既有缺陷。
- 下一步：创建本轮提交，然后停止。
