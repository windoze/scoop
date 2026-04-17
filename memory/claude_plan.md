## 当前执行说明

按照要求，我会先在本文件记录一份**可公开的执行依据摘要**与**分步执行计划**，随后再开始读取提交记录、`TODO.md`、`PLAN.md` 和代码库内容。

说明：我不会在文件中记录原样的内部私有推理内容，但会持续维护足够详细的任务判断依据、计划、进度、阻塞项和结论，便于审查执行过程。

## 初始目标

本轮只完成一件事：

1. 检查最新提交是否提到已有问题，如有则先修复。
2. 读取 `TODO.md`，定位第一个未完成任务。
3. 判断该任务是否需要拆分；若需要，则更新 `PLAN.md` 与 `TODO.md`。
4. 实现当前应执行的首个任务。
5. 运行相关测试与质量检查，修复发现的问题。
6. 更新 `TODO.md`、`PLAN.md`、本文件。
7. 提交 git commit，然后停止，不继续下一个任务。

## 预定执行步骤

1. 查看最近一次提交信息与变更摘要，确认是否显式提到待修复问题。
2. 读取 `TODO.md` 与 `PLAN.md`，识别第一个未完成任务及其上下文。
3. 如任务过大或存在前置缺陷，先整理依赖关系并改写计划。
4. 定位相关代码、测试与规范位置，确认实现边界。
5. 修改代码并补充/调整测试。
6. 运行至少与本任务直接相关的测试；若改动影响较广，则扩大测试范围。
7. 运行格式化、lint 与必要的全量/增量检查，目标是无 warning。
8. 更新任务文档状态并记录结果。
9. 使用清晰的提交信息提交本轮改动。

## 进度记录

- 已完成：创建本计划文件并写入初始执行计划。
- 已完成：检查最新提交 `f7ab537922a45684d837718384e6ed1b62761382`（`[T3009b0a] Write back outer-scope handle slots`），提交信息本身未额外声明独立于任务列表之外的新 pre-existing issue。
- 已完成：读取 `TODO.md` 与 `PLAN.md`，确认当前第一个未完成任务为 `T3009b0aR`。
- 当前任务：`T3009b0aR` Review，目标是确认 outer-scope slot 的收集与写回属于 unified handle 合同的一部分，而不是只对 escaped continuation / 特定 fixture 生效的局部补丁。

## 当前任务执行计划（T3009b0aR）

1. 阅读 `TODO.md` / `PLAN.md` 中 `T3009b0a` 与 `T3009b0aR` 的上下文，明确 review 验收标准。
2. 审查相关生产代码，重点关注：
   - outer-scope slot 收集范围是否覆盖整个 `handle`（body、arms、finally），而非单一路径；
   - seeded slot writeback 是否在统一的 handle 退出路径上执行，而非仅在 escaped continuation 或单 fixture 路径触发；
   - 是否存在按 effect 形状、特定 op、特定 fixture 追加的分支。
3. 如发现实现缺口，直接修复生产代码并补测试。
4. 运行与该 review 直接相关的验证；至少包括定向测试，必要时补跑 `cargo test --all` 与 `cargo clippy --all-targets -- -D warnings`。
5. 更新 `TODO.md`、`PLAN.md` 与本文件，记录 review 结论与任何修复。
6. 提交本轮 commit，然后停止。

## 本轮审查结果与计划调整

- 已审查的生产路径：
  - `crates/scoopc/src/llvm/codegen/effect/state_machine_plan.rs`
    - `collect_outer_scope_slots(...)` 现已覆盖整个 `handle`（body、arms、finally），并显式排除 arm binder / resume / continuation locals / handle 内局部。
  - `crates/scoopc/src/llvm/codegen/effect/state_machine_emitter.rs`
    - `seed_outer_scope_frame_slots(...)` 与 `write_back_outer_scope_frame_slots(...)` 都由 `FrameSlot.seed_from_outer_scope` + mutability metadata 驱动；
    - 初次 handle 退出时的写回统一挂在 `handle_propagate` 与 `handle_done`，不是按 arm kind/fixture 名称分流。
  - `crates/scoopc/src/llvm/codegen/effect/mod.rs`
    - `codegen_continuation_resume_builtin(...)` 当前只负责 payload 写入 continuation + 调用 runtime `scoop_continuation_resume(...)` + ordinary effect propagation check，本身没有 outer-slot writeback 逻辑。
  - `runtime/c/scoop_runtime.c`
    - `scoop_continuation_resume_common(...)` 当前只恢复 handler stack / resume_state_tag 并直接调用 `step_fn`，没有任何 outer-local writeback 钩子。

- 新发现的未跟踪问题：
  - outer-scope slot 写回目前只覆盖“第一次离开 handle”的 `handle_done` / `handle_propagate`。
  - escaped continuation 在 handle 返回后通过 `k.resume(...)` 继续执行 body / finally 时，当前 resume 完成路径没有统一的 frame-metadata 驱动 writeback 出口。
  - 用临时最小 repro 运行 `cargo run -p scoop --features llvm -- run memory/t3009b0aR_resume_writeback_repro.scoop`，观察到输出：
    - `body_before`
    - `arm_saved`
    - `after_handle`
    - `before`
    - `body_after`
  - 这说明 resumed body 已继续执行，但当前 post-resume completion path 既没有形成可复审的统一 outer-local 同步合同，也仍受下一任务 `T3009b0` 的 caller-tail 缺口影响。

- 结论：
  - `T3009b0aR` 不能在当前状态下完成。
  - 已按要求把新前置缺口整理为 `T3009b0a1`，并在 `TODO.md` / `PLAN.md` 中把 `T3009b0aR` 顺延到它之后。

## 当前状态

- 已完成：
  - 检查最新提交、读取 `TODO.md` / `PLAN.md`。
  - 定位首个未完成任务 `T3009b0aR`。
  - 完成该 review 的生产代码审查，并识别出新的未跟踪前置缺口。
  - 更新 `TODO.md` / `PLAN.md`，新增 `T3009b0a1`，重排后续顺序。
- 当前收尾：
  - 清理临时复现文件。
  - 检查改动并提交本轮 commit。
