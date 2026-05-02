## 执行计划

说明：我不会记录或暴露逐字的内部推理，但会在这里持续维护可审计的执行计划、关键判断依据、步骤进展与变更记录。

### 初始计划

1. 读取 `TODO.md`，把它当作任务索引使用，不把其中的摘要当作任务正文。
2. 按 `TODO.md` 引用顺序读取对应的 `TODO-Px.md`，以详细任务文件中的完成记录为准，定位第一个未完成的详细任务。
3. 检查最近一次提交信息是否直接提到与该任务相关且未完成的问题；如果是，则将其视为该任务的一部分或前置条件。
4. 阅读与当前任务直接相关的代码、测试、规范和任务约束，确认需要修改的最小范围。
5. 实现当前任务；如果遇到阻塞当前任务的真实缺口或回归，则先修复它，或在对应 `TODO-Px.md` 中插入最小必要前置任务，并同步 `TODO.md`。
6. 运行与任务相关的验证：至少包括定向测试；若任务影响面较广，再运行更广的测试、`cargo fmt`、`cargo test --all`、`cargo clippy --all-targets -- -D warnings`（在时间和影响范围允许时）。
7. 更新文档性记录：
   - 在对应 `TODO-Px.md` 中记录任务完成；
   - 若任务索引、标题、顺序或文件引用发生变化，则同步更新 `TODO.md`；
   - 仅在阶段计划或依赖关系变化时更新 `PLAN.md`。
8. 再次更新本文件，记录实际执行结果、偏差、验证情况与是否存在阻塞。
9. 按仓库约定创建一次 git 提交，提交信息使用当前任务 id，完成后停止，不继续下一个任务。

### 进度记录

- 已创建初始计划文件，下一步读取任务索引并定位第一个未完成的详细任务。
- 已读取 `TODO.md`、`TODO-P0.md`、`TODO-P1.md`、`TODO-P2.md`；当前首个未完成详细任务确认为 `P2-T03`：落地 `Continuation` typed 语义、runtime error 的普通 effect 传播，以及 compiler-owned interface 约束。

### 当前任务执行计划（P2-T03）

1. 检查最近一次 git 提交信息，确认是否有与 `P2-T03` 直接相关且未完成的问题需要并入本任务或先作为前置项记录。
2. 读取 `PLAN.md`、`EFFECT_REFACTOR.md`、`sysroot/core.scoop`、`crates/scoopc/src/effect_refactor_pipeline/hir_stage.rs`、`crates/scoopc/src/typecheck/expr/call.rs` 以及 continuation/typecheck 相关实现与现有 fixtures，建立对当前 typed contract 的基线理解。
3. 判断 `P2-T03` 是否能直接在当前任务内完成；如果存在阻塞且无法按规范实现，则在 `TODO-P2.md`/`TODO.md` 中补入最小前置任务并停止。
4. 实现 `Continuation<ResumeTuple, Answer, eff Out>` 的 typed receiver contract、`Raise<RuntimeError>` 的 ordinary effect 传播、以及禁止用户实现/构造 `Continuation` 的 typecheck 约束。
5. 把 continuation/effect contract 显式写入 refactor typed HIR 输出 side table，确保后续阶段不需要回 AST/typecheck 猜测。
6. 补充或更新 typecheck fixtures、HIR/typed 单元测试与必要的快照，覆盖：answer 类型、required effects、用户实现/构造报错，以及 refactor continuation typed contract 的结构断言。
7. 运行任务要求的定向测试与 `cargo clippy -p scoop -p scoopc --all-targets --no-default-features -- -D warnings`；若失败则继续修复直到通过。
8. 回写 `TODO-P2.md` 的完成记录；如任务顺序或依赖未变化，则不改 `TODO.md` / `PLAN.md`。
9. 更新本计划文件的执行结果与验证摘要，然后创建一次 git 提交并停止。

### 当前实现决定

- `Continuation.resume(...)` 的 typed 语义基础已存在于 `typecheck/expr/call.rs`，但 refactor typed HIR stage 仍只暴露 placeholder `TypedHirEffectContracts`，无法把 `ResumeTuple/Answer/Out` 与 runtime error effect contract 显式交给后续阶段；本次将把它升级为真实 side table。
- “用户不能实现 `Continuation`” 适合落在 `typecheck/interfaces.rs`，因为当前所有 `class/object/struct/enum/interface` 的 supertype/interface 合法性都从这里统一检查。
- “用户不能构造 `Continuation`” 适合落在 `typecheck/expr/call.rs` 的 nominal constructor 解析路径，给出显式的 typecheck 拒绝，而不是继续落成模糊的 generic callee-not-callable 结果。
- 若实现过程中发现 `dump-hir` 的 typed 路径没有覆盖到新约束，将补最小必要接线，但不把 selector/pipeline 分支下沉到旧业务模块。

### 执行结果

- 已把 `TypedHirEffectContracts` 从 placeholder 升级为显式 side table：现在可从 refactor typed HIR stage 输出中按 `CallSite` 读取 `Continuation.resume(...)` 的 `receiver_ty`、`ResumeTuple`、`Answer`、`return_ty`、`Out` effect row，以及“required effects 包含 `Raise<RuntimeError>`”这一事实。
- 已在 `typecheck/interfaces.rs` 加入 `continuation_impl_not_allowed`，拒绝用户实现/继承 `Continuation`；已在 `typecheck/expr/call.rs` + `expr/error.rs` 加入 `continuation_not_constructible`，拒绝用户态 `Continuation<...>()` 构造。
- 已新增/更新定向单测与 fixtures，并同步更新 `crates/scoop/src/commands/dump_hir.rs` 的测试断言，使其与新的显式 contract 表保持一致。
- 已回写 `TODO-P2.md` 的 `P2-T03` 完成记录；未修改 `TODO.md` / `PLAN.md`。

### 验证摘要

- 通过：`cargo test -p scoopc --no-default-features refactor_continuation_typecheck`
- 通过：`cargo test -p scoopc --no-default-features effect_refactor_pipeline`
- 通过：`cargo test -p scoopc --no-default-features continuation_resume`
- 通过：`cargo test -p scoop --no-default-features dump_hir`
- 通过：9 个 refactor typecheck fixtures（含新增 `continuation_user_impl_is_error.scoop`、`continuation_runtime_ctor_is_error.scoop`、`continuation_resume_requires_runtime_error_effect_is_error.scoop`）
- 通过：`cargo clippy -p scoop -p scoopc --all-targets --no-default-features -- -D warnings`

### 待收尾步骤

1. 查看当前 git 状态与差异，确认仅提交本任务相关改动。
2. 创建 `[P2-T03] ...` 提交。
3. 停止，不继续下一条任务。
