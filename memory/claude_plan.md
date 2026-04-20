# 执行计划

## 约束与目标

- 本轮只处理 `TODO.md` 中第一个未完成任务，完成后立即停止。
- 在开始实际检查与实现前，先记录高层执行计划；后续发现的新信息、关键决策、阻塞原因、已完成步骤会持续补充到本文件。
- 若发现最新提交提到的遗留问题，需先修复这些问题，再进入 `TODO.md` 任务。
- 若首个未完成任务过大，需要先拆分并更新 `PLAN.md` 与 `TODO.md`，本轮只执行拆分后的第一个子任务。

## 初始步骤计划

1. 查看最新一次提交信息，确认是否提到需要优先修复的遗留问题。
2. 读取 `TODO.md`、`PLAN.md`、必要的仓库说明文件，定位第一个未完成任务及其上下文。
3. 判断该任务是否足够具体、是否存在前置依赖、是否需要拆分。
4. 如需拆分，先更新 `PLAN.md` 和 `TODO.md`，再执行第一个子任务；如不需拆分，直接实现该任务。
5. 实现过程中检查是否出现规范不匹配、语言特性缺失或现有 bug；若出现，则按要求把问题前置到 `TODO.md` 并更新 `PLAN.md`。
6. 对改动执行充分验证，至少覆盖与本任务直接相关的测试；若有必要，运行更广泛的检查如 `cargo test`、`cargo clippy --all-targets -- -D warnings`。
7. 完成后更新 `TODO.md`、`PLAN.md` 与本文件，提交 git commit，然后停止。

## 进度记录

- 已创建本计划文件。
- 已查看最新提交 `68f25919da4766e31e24ecb503bd2ee65283c91a`（`[T4016b1] Remove user-facing -> resume syntax`），提交信息本身未额外点名需要先处理的遗留问题，因此无需在 `TODO.md` 主线前插入新的“修最新提交遗留问题”任务。
- 已读取 `TODO.md` / `PLAN.md`，当前首个可执行未完成任务是 `T4016b2`：把 continuation answer type 接入 binder 静态模型与显式 `Continuation<Resume, Answer, eff E>` surface。
- 初步盘点结果：
  - `sysroot/core.scoop` 仍声明为 `Continuation<T, eff E = Pure>`。
  - `TypeLowering::ty_continuation(...)`、escape continuation binder 注入、`Continuation.resume` 的内建 typecheck 与 LLVM lowering 仍默认 continuation 只有一个普通类型参数（resume payload）。
  - 仓库中存在大量显式 `Continuation<...>` 注解；若直接把语法面切到两个普通类型参数，会连带影响大量现有 fixture/source。
- 计划细化：
  1. 先把 `sysroot` 与内部 `NominalType` 使用点切到 `Continuation<Resume, Answer, eff E>`。
  2. 修改 handle escape binder 注入逻辑，让 `k` 的静态类型显式携带 delimiter answer type；优先用 handle body 结果类型，必要时回退到 expected type / 内部占位类型以维持 typecheck 流程。
  3. 修改 `Continuation.resume` 的接收者解析与 LLVM 读取逻辑，使其从新 surface 中读取 payload 类型，但暂不改变 `resume` 当前返回 `Unit` 的过渡实现（真正 answer-returning `resume` 留待 `T4016b3`）。
  4. 为避免本轮把大量旧 fixture 一次性迁移成阻塞项，给“省略 answer type 的旧式 `Continuation<Resume, eff E>` / `Continuation<Resume>` 注解”保留过渡兼容 lowering；同时新增/更新定向测试，确保：
     - 显式 `Continuation<Resume, Answer, eff E>` 注解可用；
     - 推导出的 escape binder 类型会显示 answer type；
     - answer type mismatch 能通过 typecheck 暴露。
  5. 通过相关测试与 lint，随后更新 `TODO.md` / `PLAN.md` 并提交。
- 已完成核心实现：
  - `sysroot/core.scoop` 已切到 `Continuation<Resume, Answer, eff E = Pure>` surface，并把 task step continuation 的 answer type 显式写成 `__TaskStepResult`。
  - `TypeLowering::ty_continuation(...)`、escape continuation binder 推导、`Continuation.resume` 接收者解析、LLVM payload 读取路径都已改成消费 `Resume + Answer + eff` 三元模型。
  - 对旧式 `Continuation<Resume, eff E>` / `Continuation<Resume>` 注解保留了前端过渡 lowering：内部 answer-hole 允许现有 shorthand 继续通过 typecheck / HIR / run-pass，但推导出的 `k` binder 类型和新的显式注解都会显示真实 answer type。
  - 已补充 answer type mismatch、binder answer/effect 推导与 HIR 参数类型显示的定向回归。
- 已完成验证：
  - `cargo run -p scoop -- test --fixtures tests/fixtures/typecheck`
  - `cargo run -p scoop -- test --fixtures tests/fixtures/hir`
  - `cargo run -p scoop -- run tests/fixtures/run-pass/effect_resume_yield_int_basic.scoop`
  - `cargo run -p scoop -- run tests/fixtures/run-pass/continuation_resume_surface_named_tuple_and_unit_basic.scoop`
  - `cargo run -p scoop -- run tests/fixtures/run-pass/continuation_escape_binder_resume_effect_row_runtime_basic.scoop`
  - `cargo test --all`
  - `cargo clippy --all-targets -- -D warnings`
- 待执行：
  - 检查最终 diff，确认 `TODO.md` / `PLAN.md` 状态一致后提交本轮改动。
