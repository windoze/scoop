# 执行计划

说明：我不会记录私有推理细节，但会在这里持续维护可核查的执行计划、关键决策和进度更新。

## 初始计划

1. 检查最近一次提交信息，确认是否提到需要先修复的既有问题。
2. 阅读 `TODO.md`，定位第一个未完成任务。
3. 阅读 `PLAN.md`，确认当前任务上下文、依赖和已有拆分。
4. 如果首个未完成任务过大，先把它拆分为更小的子任务，并同步更新 `PLAN.md` 与 `TODO.md`。
5. 实现当前应执行的首个任务。
6. 运行与该任务直接相关的测试；如发现既有问题，先修复问题或把前置修复任务插入 `TODO.md` 并停止。
7. 更新 `TODO.md` 与 `PLAN.md`，标记已完成任务或记录阻塞与重排原因。
8. 按仓库约定创建一次提交，然后停止。

## 进度记录

- 已写入初始计划，下一步开始检查最近提交与任务列表。
- 已检查最近一次提交：`29593ed [T5002b1] Make direct-call wrapper token explicit`。提交信息本身未声明需要先修复的既有问题。
- 已读取 `TODO.md` 与 `PLAN.md`。当前第一个未完成任务是 `T5002b2`：把显式 `incoming_resume_token_ref` 扩到剩余 hidden effect ABI surface。
- 接下来需要先评估 `T5002b2` 是否已经足够可执行；若范围仍过大，则按要求把它进一步拆分并同步更新 `PLAN.md` / `TODO.md`。
- 已完成初步代码勘察：`T5002b2` 同时覆盖 closure / funptr / vtable / itable 调用边界、callee resume entry、state-machine step/dispatch，以及 runtime continuation 调用侧；涉及 `call/dispatch.rs`、`closure/mod.rs`、`call/resume.rs`、`effect/state_machine_emitter.rs`、`runtime_abi.rs`、`runtime/c/scoop_runtime.c` 等多个面。
- 判断：原始 `T5002b2` 过大，需要拆分后执行。
- 拟拆分方向：
  1. 先处理 ordinary indirect-call surface：让 closure / funptr / vtable / itable 相关 generated callable signature 与 call IR 显式携带 `incoming_resume_token_ref`，fresh path 显式传 `null`，boundary 在 consume outcome 后清理 TLS token scratch。
  2. 再处理 callee resume entry 与 state-machine step/dispatch，以及 runtime continuation bridge 对新 token 形状的传递。
  3. 每个实现子任务后紧跟 review 子任务，保持 TODO 顺序约束。
- 已按上述判断把 `T5002b2` 拆分为 `T5002b2a/b/c` 及对应 review，并把 `T5002b3` 依赖改到 `T5002b2cR`。
- `T5002b2a` 已实现完成：
  1. effect-capable generated callable 的 ordinary indirect-call signature 现已显式预留 `incoming_resume_token_ref`；
  2. closure / funptr / vtable / itable boundary 会在 legacy call 前 publish incoming token（当前 fresh path 为 `null`），在 consume outcome 后 clear TLS token scratch；
  3. direct non-wrapper fresh call 若目标 signature 需要 token，也会显式传 `null` 以保持 ABI 一致。
- 已完成验证：
  - `cargo test -p scoopc --lib outward_effect`
  - `cargo test -p scoopc --lib effectful_funptr_call_uses_explicit_outcome_boundary`
  - `cargo test -p scoopc --lib skips_tls_check`
  - `cargo test -p scoopc --lib production_codegen_suspendability_observes_overridden_pass_summary`
  - `cargo clippy --all-targets -- -D warnings`
- 当前任务状态：`T5002b2a` 已完成并已回写 `TODO.md` / `PLAN.md`；下一步应提交本次变更并停止，等待下次调用执行 `T5002b2aR`。
