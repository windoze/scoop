## 当前执行计划

### 约束与目标
- 本次只处理一个详细任务：读取 `TODO.md` 作为索引，再以对应 `TODO-Px.md` 为准，完成第一个未完成的详细任务后停止。
- 若遇到阻塞当前任务的真实前置缺口，不做变通方案；在对应 `TODO-Px.md` 中补充最小前置任务、同步 `TODO.md`，提交后停止。
- 需要在完成后更新详细任务完成记录，必要时同步 `TODO.md`；仅当阶段计划本身变化时才更新 `PLAN.md`。
- 需要运行相关验证，确保无警告；如变更范围允许，优先运行最小且充分的测试，再补充 `cargo clippy --all-targets -- -D warnings` 等仓库要求的检查。
- 最后创建一次 git 提交，然后停止，不继续下一个任务。

### 执行步骤
1. 读取 `TODO.md`，确认索引结构与任务文件引用。
2. 按索引顺序读取相关 `TODO-Px.md`，定位第一个未完成的详细任务，并核对该任务的完成判定、约束、依赖和验证要求。
3. 查看最近提交信息；如果最新提交明确提到与该任务直接相关且未完成的问题，将其视为当前任务的一部分或记录为该任务前置。
4. 阅读与当前任务直接相关的代码、测试、文档与现状，确认实现边界和已有未完成部分。
5. 实施最小正确修改，避免绕过规范；如发现阻塞当前任务的缺口，先修复，或将其登记为新的最小前置任务并同步索引。
6. 运行与本任务直接相关的测试/检查；如果失败，继续修复直到通过，或在无法继续时按阻塞流程更新任务文件。
7. 更新 `memory/claude_plan.md` 记录关键进展；更新对应 `TODO-Px.md` 的完成记录；如任务索引变化则同步 `TODO.md`；仅在阶段计划变化时更新 `PLAN.md`。
8. 检查工作区状态，确认仅提交本次相关变更，使用符合仓库风格的提交信息提交。
9. 停止，等待下一次调用。

### 当前状态
- 已完成：初始计划写入；读取 `TODO.md`、`TODO-P0.md`、`TODO-P1.md`、`TODO-P2.md`；确认当前第一个未完成的详细任务是 `P2-T04`（`输出 typed HIR effect/continuation side tables，并锁定 dump-hir / typecheck 验证矩阵`）。
- 已确认：最新提交是 `P2-T03R` review，未显式留下需要先补的新前置问题，因此本次直接执行 `P2-T04`。
- 已完成：检查 refactor typed HIR stage、现有 HIR side tables、`dump-hir` 输出路径与相关 fixtures；确认 `LoweredHir` / AST typecheck side tables 已具备构造 `P2-T04` 所需原始信息。
- 实施方案：
  1. 扩展 `crates/scoopc/src/effect_refactor_pipeline/hir_stage.rs`，让 `TypedHirEffectContracts` 显式输出：函数级 effect contract、`Continuation.resume(...)` contract、`perform` site contract、`handle` site contract，以及已知调用点分类。
  2. 在同一模块中提供稳定 formatter，使用 `TypeStore` 把 side tables 渲染成可回归文本，不依赖 `HashMap` 迭代顺序。
  3. 修改 `crates/scoop/src/commands/dump_hir.rs`，使 refactor 路径在打印 HIR `File` Debug 后追加 typed contract 区块；legacy 路径保持原样。
  4. 新增/更新定向单元测试，覆盖 continuation/runtime-error dump、handle/perform dump，以及 refactor `dump-hir` CLI 输出包含新 contract 区块。
  5. 新增 `tests/fixtures/hir/continuation_runtime_error_surface_basic.scoop`（以及必要的 `.hir` golden，避免破坏现有 fixture 运行），随后执行 TODO 要求的定向测试与 clippy。
- 已完成：
  - `TypedHirEffectContracts` 现已显式输出 `function_effects`、`call_site_kinds`、`continuation_resume_sites`、`perform_sites`、`handle_sites`，并对 `Continuation.resume(...)` 明确记录 `Out + Raise<RuntimeError>` ordinary effect contract。
  - `TypedHirStageOutput::stable_dump()` 与 `TypedHirEffectContracts::stable_dump(...)` 已落地；`scoop dump-hir --effect-pipeline refactor` 现在在 HIR `File` Debug 后追加稳定 side-table 区块。
  - 已新增 snapshot/结构测试：continuation surface、runtime-error effect 传播、handle/perform contract、命令层 refactor dump 输出。
  - 已新增 `tests/fixtures/hir/continuation_runtime_error_surface_basic.{scoop,hir}`，并验证 legacy HIR fixture 仍可运行。
- 已完成验证：
  - `cargo test -p scoopc --no-default-features refactor_typed_hir`
  - `cargo test -p scoop --no-default-features dump_hir`
  - `cargo run -q -p scoop --no-default-features -- --effect-pipeline refactor test --fixtures tests/fixtures/typecheck/continuation_resume_answer_expression_ok.scoop`
  - `cargo run -q -p scoop --no-default-features -- --effect-pipeline refactor test --fixtures tests/fixtures/typecheck/continuation_resume_requires_runtime_error_effect_is_error.scoop`
  - `cargo run -q -p scoop --no-default-features -- --effect-pipeline refactor dump-hir tests/fixtures/hir/continuation_resume_surface_named_tuple_and_unit_basic.scoop`
  - `cargo run -q -p scoop --no-default-features -- --effect-pipeline refactor dump-hir tests/fixtures/hir/continuation_runtime_error_surface_basic.scoop`
  - `cargo run -q -p scoop --no-default-features -- --effect-pipeline refactor dump-hir tests/fixtures/hir/handle_perform.scoop`
  - `cargo run -q -p scoop --no-default-features -- --effect-pipeline legacy dump-hir tests/fixtures/hir/handle_perform.scoop`
  - `cargo run -q -p scoop --no-default-features -- test --fixtures tests/fixtures/hir/continuation_runtime_error_surface_basic.scoop`
  - `cargo clippy -p scoop -p scoopc --all-targets --no-default-features -- -D warnings`
- 进行中：更新 `TODO-P2.md` 完成记录、检查工作区、准备提交。
- 待处理：git 提交并停止。
