# 执行计划

## 约束说明

- 按要求先写入本计划文件，再执行仓库检查和命令。
- 本文件记录可审阅的执行步骤、发现、决策与进度更新，不记录内部私有推理。
- 本轮目标是：先处理最新提交提到的既有问题；若无，则完成 `TODO.md` 中第一个未完成任务；完成后测试、更新文档、提交 commit，然后停止。

## 初始步骤

1. 检查最新一次 git 提交信息，判断是否提到需要先修复的既有问题。
2. 读取 `TODO.md`、`PLAN.md`、必要时读取 `README.md` 与相关任务上下文。
3. 确认第一个未完成任务；如果任务过大，则先拆分任务并更新 `TODO.md`/`PLAN.md`。
4. 在实现前审查相关代码、测试、规格与最近变更，确认是否存在阻塞性的既有缺陷或规格不匹配。
5. 若发现既有问题：
   - 先修复该问题；或
   - 若当前轮次无法直接修复，则把它作为前置任务插入 `TODO.md`，更新 `PLAN.md`，提交后停止。
6. 若无阻塞，完整实现当前任务。
7. 运行相关验证：
   - 优先运行针对性测试；
   - 需要时运行更大范围测试；
   - 运行格式化/静态检查（至少覆盖本次改动范围，必要时执行 `cargo clippy --all-targets -- -D warnings`）。
8. 更新 `TODO.md` 与 `PLAN.md`，记录完成状态和后续依赖。
9. 提交 git commit，提交信息使用任务编号或明确描述。
10. 停止，不继续处理下一个任务。

## 进度记录

- 已创建初始计划文件。
- 已检查最新提交 `81555757685546fbf7b5e6bd1b06557c95e6256b`：提交内容是把 `T4016T1d` 作为 `T4016T2` 的前置 blocker 写入 `TODO.md` / `PLAN.md`，未额外挂出新的独立既有 bug 需要先于 `TODO.md` 处理。
- 已确认本轮首个未完成的可执行任务是 `T4016T1d`，不是顶层父任务 `T4016`。
- 已阅读 `TODO.md`、`PLAN.md`、`SCOOP_TASK.md` 与 `sysroot/core.scoop`，确认目标是补齐 ordinary Scoop generic task-state object model 所需的前端/LLVM 主线，而不是直接推进 `T4016T2`。
- 已定位一条高度可疑的主根因：
  - `crates/scoopc/src/hir/lower/util.rs` 中 `collect_generic_class_instantiation_inits()` 目前只对 `TypeKind::Param` 做浅层替换；
  - 它不会递归替换 `__TaskState<T>`、`Option<T>`、`Continuation<..., __TaskDriverStep<T>>` 等嵌套类型中的 `T`；
  - 这很可能同时导致 `class field type`、`Option<T> inner type`、ctor 参数类型残留 `TypeKind::Param` 等 LLVM 路径报错。

## 当前执行计划（已细化）

1. 先写一个最小 generic task-state/object-model probe，验证当前失败形态，并确认浅替换就是直接根因。
2. 把 generic class instantiation 的类型替换升级为递归替换，覆盖至少：
   - nominal type args；
   - `Option<T>` / nullable；
   - tuple；
   - function/continuation 嵌套类型；
   - effect row 中出现的类型参数。
3. 复验最小 probe；若 `class field type` / `Option<T>` / ctor 参数路径已消失，再继续检查 smart-cast + generic member access 是否仍是独立缺口。
4. 如 smart-cast 分支仍失败，则补齐相应的 typecheck 晚解析或成员具体化路径，并把它纳入同一轮任务完成范围。
5. 添加最小回归（优先 `run-pass`，必要时补 `typecheck` / `build`），锁定 ordinary Scoop generic task-state object model 已能在 LLVM 路径上 build/run。
6. 运行相关验证、更新 `TODO.md` / `PLAN.md` / 本文件、提交 commit，然后停止。
## 2026-04-22 当前执行计划

### 目标
- 按 `TODO.md` 顺序处理首个未完成任务。
- 先确认最新提交是否只是在记录 blocker，还是还遗留了必须先修的 pre-existing issue。
- 基于当前已实现的代码与已发现的失败 probe，判断 `T4016T1d` 是否可以完整收口；如果不能，则按依赖关系拆分为更小的前置子任务，并只完成新的首个子任务。

### 已知上下文
- 最新提交 `8155575` 的作用是把 `T4016T1d` 标成 `T4016T2` 的前置 blocker，目前没有从提交说明中看到额外挂出的独立 bug，但仍需再次核对提交内容与工作树状态。
- 已有代码改动已经修复了 generic class instantiation 的递归类型替换、layout/nominal lowering 的带 type args 实例化、`Any` receiver 的 member access 晚解析、smart-cast 后 member access codegen 使用 concrete receiver type，以及 HIR lowering 对 smart-cast/type side-table 的若干类型回填问题。
- 两个新 fixture 当前可通过：
  - `tests/fixtures/run-pass/task_generic_state_object_model_basic.scoop`
  - `tests/fixtures/run-pass/smart_cast_any_member_access_generic_class_basic.scoop`
- 仍存在未修 blocker：
  - generic helper / generic method body 内仍会泄漏 `TypeKind::Param(T)` 到 LLVM codegen
  - 代表性失败样例：`/tmp/t4016t1d_smart_cast_generic_probe.scoop`
  - 典型现象：`cg_ty_of: TypeKind::Param(T) encountered in codegen (monomorph miss)` 与 `unsupported_main_body: class field type`
  - `carrier.lock.destroy()` 路径也仍未完全打通，当前 fixture 仅以更窄路径 `lock.destroy()` 验证已修范围

### 当前判断
- 现状更像是 `T4016T1d` 已完成一部分，但“generic helper/method body 的 monomorph/type-param leak”仍是明确 blocker。
- 如果该 blocker 仍存在，则不能把原始 `T4016T1d` 直接标记完成；应将其拆分为更小子任务，并把剩余 blocker 作为后续前置任务显式写入 `TODO.md` / `PLAN.md`。
- 本轮优先目标倾向于：
  1. 验证当前代码确实稳定覆盖较窄范围；
  2. 若验证通过，则把 `T4016T1d` 拆成至少两个子任务；
  3. 将已完成部分作为新的第一个子任务收口；
  4. 保留 generic helper/method body blocker 为紧随其后的未完成任务。

### 执行步骤
1. 检查工作树、`TODO.md`、`PLAN.md` 与最新提交内容，确认当前任务顺序与 blocker 记录是否一致。
2. 复现并确认剩余 blocker 仍然存在，避免误把未完成问题当成已完成。
3. 运行与当前改动直接相关的测试：
   - 先跑新增/相关 fixture；
   - 再跑 `cargo run -p scoop -- test`；
   - 再跑 `cargo test --all`；
   - 最后跑 `cargo clippy --all-targets -- -D warnings`。
4. 若 blocker 仍存在但不影响已修子范围：
   - 更新 `TODO.md`，把 `T4016T1d` 拆为更小子任务，并把未修 blocker 作为后续前置项；
   - 更新 `PLAN.md`，说明拆分原因、已完成范围、剩余问题与依赖关系。
5. 若当前较窄子任务已具备代码、测试和文档支撑：
   - 将该子任务标记完成；
   - 保持后续 blocker 子任务为下一个未完成项。
6. 查看 `memory/claude_plan.md` 并在关键节点补充进展记录，确保能追踪执行状态。
7. 提交本轮改动，提交信息明确说明是完成哪个具体子任务，随后停止。

### 风险与约束
- 不能把 `driveInt(...)` / `lock.destroy()` 这种较窄 fixture 验证误写成原始 `T4016T1d` 全量完成。
- 如完整测试暴露额外 pre-existing issue，则该问题也必须先进入当前处理范围，必要时进一步前置拆分任务。
- 不可通过修改 fixture 形状来绕过 generic helper/method body 的真实缺口；该缺口必须保留为显式 TODO 依赖。

## 进展记录
- 2026-04-22：基于上一轮遗留状态重新建立本轮计划，待开始核对任务文件、测试与 blocker 现状。
- 2026-04-22：已重新核对 `TODO.md` / `PLAN.md` / 最新提交，确认没有额外挂出的 pre-existing issue；当前确实是 `T4016T1d` 的收口问题。
- 2026-04-22：已复现剩余 blocker。`cargo run -p scoop -- build /tmp/t4016t1d_smart_cast_generic_probe.scoop -o /tmp/t4016t1d_smart_cast_generic_probe.out` 仍失败，报 `cg_ty_of: TypeKind::Param(T) encountered in codegen (monomorph miss)` 与 `unsupported_main_body: class field type`，说明 generic helper / method body 路径尚未完成。
- 2026-04-22：已验证当前较窄范围稳定通过：
  - `task_generic_state_object_model_basic.scoop` build + run 退出码 `7`
  - `smart_cast_any_member_access_generic_class_basic.scoop` build + run 退出码 `7`
- 2026-04-22：已完成全量验证：
  - `cargo run -p scoop -- test` 通过，结果 `fixtures: ok (1156)`
  - `cargo test --all` 通过
  - `cargo clippy --all-targets -- -D warnings` 初次因 `expr.rs` 中对 `self.types` 的多余借用失败；修正为 `substitute_type_params(self.types, ...)` 后已复跑通过
- 2026-04-22：基于上述结果，决定把原始 `T4016T1d` 拆成 `T4016T1d1` 与 `T4016T1d2`；本轮收口 `T4016T1d1`，把 generic helper / method body 的 monomorph leak 保留为紧随其后的前置任务。
- 2026-04-22：`cargo fmt --check` 额外暴露出仓库中的 Rust 格式漂移（包含本轮修改文件与两处既有源码）；已执行 `cargo fmt` 收口，并复验：
  - `cargo fmt --check` 通过
  - `cargo test --all` 通过
  - `cargo clippy --all-targets -- -D warnings` 通过
