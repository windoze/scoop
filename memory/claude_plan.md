# 执行计划与进度记录

## 约束说明

- 按用户要求，本文件在开始执行仓库检查前先建立，并在后续关键步骤完成或计划调整时持续更新。
- 这里记录的是可审计的执行计划、检查结果、关键判断与进度，不包含冗长的内部推理展开。
- 本轮目标是：先处理最新提交中提到的既有问题（如果存在），再完成 `TODO.md` 中第一个未完成任务，完成后测试、更新文档、提交 commit，并停止。

## 初始步骤计划

1. 检查最新一次 git commit 的提交信息与变更范围，确认是否明确提到已有问题、已知缺陷或后续必须立即修复的事项。
2. 打开并阅读 `TODO.md`，定位第一个未完成任务。
3. 打开并阅读 `PLAN.md`，理解现有任务拆分、依赖关系与项目当前阶段。
4. 检查工作区状态，确认是否存在未提交修改；避免覆盖用户已有更改。
5. 评估第一个未完成任务：
   - 如果任务边界清晰且可在一轮内完成，则直接实施。
   - 如果任务过大或存在前置缺陷/缺失特性阻塞，则先在 `PLAN.md` / `TODO.md` 中进行拆分或重排，再处理新的第一个子任务。
6. 实施代码修改，并补充必要测试。
7. 运行相关验证，至少包括与改动直接相关的测试；若范围允许，再运行更完整检查（如 `cargo test`、`cargo clippy --all-targets -- -D warnings`）。
8. 更新 `TODO.md` 与 `PLAN.md`，记录完成状态或阻塞依赖。
9. 提交本轮改动，commit message 对应任务编号与实际变更内容。
10. 停止，不继续处理下一个任务。

## 进度

- 已创建本文件并写入初始计划。
- 已检查最新一次提交、`TODO.md`、`PLAN.md` 与工作区状态。
- 确认最新提交 `6ccb2e3d [T3014R] Review multi-op handler registration and unmatched propagation` 只更新计划/记录文件，没有新增独立于 `TODO.md` 当前顺序之外、必须插队处理的生产代码 blocker。
- 已定位当前第一个未完成任务为 `T3009b`：在 `T3009b0` dedicated lowering 基础上，把 escaped continuation 的 `Continuation.resume(...)` 扩展到 composite resume payload。
- 从最新提交的计划记录可确认，当前全量 LLVM suite 仍停在已跟踪的 stale `EXPECT: fail` `tests/fixtures/run-pass/effect_escape_continuation_indirect_perform_closure_tail_return_string.scoop`（`T3017`）；这属于已知 effect 主线缺口，但按当前任务顺序并未要求先于 `T3009b` 单独插队。

## 当前执行计划（T3009b）

1. 审查 `T3009b0` / `T3013` 已落地的 dedicated lowering 与共享 composite transport，实现范围重点放在 `Continuation.resume(...)` 调用侧，而不是新开一套 continuation-only 通道。
2. 运行 `T3009b` 验收 fixture 与必要的定向单测，确认当前真实失败形态。
3. 若当前任务边界仍可一轮完成，则直接在现有 dedicated lowering 上补 composite payload；若暴露新的前置缺口，再按规则调整 `TODO.md` / `PLAN.md`。
4. 完成后执行相关验证，并更新 `TODO.md`、`PLAN.md` 与本文件。

## 本轮新发现与调整

- 已实现并验证一个可以独立收口的 direct blocker：
  - `crates/scoopc/src/llvm/codegen/effect/mod.rs::codegen_continuation_resume_builtin()` 不再直接信任 `receiver.ty` / `payload_expr.ty`，而是改为使用 `resolve_expr_concrete_type(receiver)` 与 `resolve_expr_cg_ty(payload_expr)`，修复了 `val payload: Result = Ok(42); k.resume(payload)` 这类 local enum payload 报 `value coercion` 的问题。
  - `tests/fixtures/run-pass/continuation_resume_enum.scoop` 已恢复为普通 run-pass fixture，不再保留 stale `EXPECT: fail`。
- 在继续执行原 `T3009b` 的 xfail 子集验收时，确认任务边界过大，需要拆分：
  - `effect_escape_continuation_indirect_perform_tail_return_int.scoop` 与 `effect_escape_continuation_indirect_perform_closure_tail_return_string.scoop` 已通过。
  - 但 `effect_escape_continuation_indirect_perform_resume_string.scoop` 与 `effect_escape_continuation_indirect_perform_resume_struct_with_ref.scoop` 仍会跳过间接 callee 在 suspend 点之后的 resumed-body 语句；这说明当前更前置的 blocker 是“间接 callee suspend 后 resumed-body caller-tail”尚未接回，而不是 direct composite transport 本身。
- 因此已按规则把原 `T3009b` 拆成：
  1. `T3009b1`：修复 direct enum/local-VarRef payload 类型解析（本轮完成）。
  2. `T3009b1R`：review direct precise-type 解析（下一轮）。
  3. `T3009b2` / `T3009b2R`：先修间接 callee resumed-body caller-tail。
  4. `T3009b` / `T3009bR`：随后再完成剩余 composite payload 收尾与复审。

## 当前结果

- 本轮完成的实际任务是拆分后的首个子任务 `T3009b1`，并已同步更新 `TODO.md` / `PLAN.md` / 本文件。
- 已完成本轮验收：
  - `cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/continuation_resume_enum.scoop`
  - `cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/continuation_resume_tuple.scoop`
  - `cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/continuation_resume_struct_with_ref.scoop`
  - `cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/continuation_resume_continuation.scoop`
  - `cargo test --all`
  - `cargo clippy --all-targets -- -D warnings`
- 下一轮应先执行 `T3009b1R` 复审，再进入新的前置实现任务 `T3009b2`。
