## 当前执行计划

1. 读取 `TODO.md`，按标题是否带有 `[DONE]` 判定第一个未完成任务。
2. 检查最近一次提交是否直接提到与该任务相关且未完成的问题；若该问题构成当前任务的直接前置，则把它视为当前任务的一部分或在 `TODO.md` 中登记为前置依赖。
3. 阅读当前任务涉及的代码、文档、测试与约束，确认实现边界、依赖关系和验证要求。
4. 在不引入规避方案的前提下完成该任务；若发现阻塞当前任务的真实缺陷或缺失能力，则先修复该阻塞，或在 `TODO.md` 中以最小新增前置任务显式记录后停止。
5. 运行与该任务直接相关的验证，并补充必要测试；若有失败，立即修复直到通过，必要时再跑更广的检查。
6. 更新文档与跟踪文件：
   - 在 `TODO.md` 中将完成的任务标题改为 `[DONE]` 前缀，并更新完成记录；
   - 仅当阶段计划/依赖发生变化时更新 `PLAN.md`；
   - 在本文件记录关键进展、计划调整和阻塞信息。
7. 检查工作区改动，按要求提交当前任务相关的全部未提交更改，提交信息使用任务编号。
8. 完成后停止，不继续下一个任务。

## 说明

- 这里记录的是可审计的执行计划与关键决策，不包含冗长的内部推理草稿。
- 如果后续发现阻塞、计划变更或关键步骤完成，会继续更新本文件。

## 进展记录

- 2026-05-09：已读取 `TODO.md` 并确认首个未完成任务为 `CG-T08R`（Review CG-T08 codegen phase exit audit）。
- 2026-05-09：已检查最新提交 `dc4251d3`（`[CG-T08] Complete codegen phase exit audit`）；提交信息未直接声明新的未完成缺陷，因此按 `CG-T08R` 既定范围执行复核。
- 2026-05-09：下一步先抽查 `CG-T08` 相关产物（codegen regression matrix、阶段退出审计与 `PIPELINE_GAPS.md` 状态更新），再重跑 `CG-T08` 要求的验证命令；若复核无缺口，则把 `CG-T08R` 标记为完成并提交。
- 2026-05-09：已完成产物抽查：`crates/scoop/tests/cg8_codegen_regression_matrix.rs` 覆盖 `CG-T01`-`CG-T07` 与 `P7-T02Z` 代表样本；`crates/scoop/src/fixtures/mod.rs` 保留 `run_all_recreates_session_between_independent_fixtures`；`crates/scoop/tests/p7_default_pipeline.rs` 持续守护 omission=refactor 与 default-vs-explicit refactor 等价。
- 2026-05-09：已重跑验证并全部通过：`cargo test --all`、`cargo run -p scoop -- test`、`SCOOP_GC_MOVE=1 SCOOP_GC_STRESS=1 SCOOP_GC_VERIFY_ROOTS=1 cargo run -p scoop -- test --fixtures tests/fixtures/runtime_gc`、`cargo clippy --all-targets -- -D warnings`。
- 2026-05-09：复核结论：未发现需要回退到 `CG-T08` 的遗漏缺口；已在 `TODO.md` 将 `CG-T08R` 标记为 `[DONE]`，下一步执行 git 提交并停止。

----

## P7-T02Z 执行记录（2026-05-09）

### 初始计划

这里只记录可审计的理由摘要、决策与执行步骤，不记录私有内部推理细节。

1. 读取 `TODO.md`，确认首个标题未带 `[DONE]` 的任务。
2. 检查最近一次提交是否直接提到与该任务相关的未完成工作。
3. 阅读对应任务条目，确认依赖、约束和验证要求。
4. 只检查当前任务相关的代码与测试。
5. 以最小正确改动完成任务，不引入 workaround。
6. 先跑定向验证，再跑更广验证。
7. 如发现新的真实前置阻塞，则更新 `TODO.md` 并停止。
8. 如任务已满足完成条件，则更新 `TODO` 记录、提交并停止。

### 进展摘要

- 已读取 `TODO.md`，确认当前首个未完成任务为 `P7-T02Z`（位于 `TODO-P7.md`）。
- 已检查最新提交消息 `Update plan`，未发现需要并入 `P7-T02Z` 的直接未完成实现说明。
- 当前任务目标：闭合剩余默认 refactor `run-pass` 阻塞，使 `P7-T03` 可以回到标准 full-regression 矩阵。

### 本轮执行计划

1. 检查 git worktree，避免误动无关修改。
2. 从 `TODO-P7.md` 记录的已知 blocker 开始复现当前剩余 `run-pass` 失败。
3. 如果失败仍可复现，则沿相关 handoff/contract 定位并修复。
4. 逐 fixture 重跑并扩展到任务要求的更广验证。
5. 若未发现新的 blocker 且验证通过，则把 `P7-T02Z` 标记为 `[DONE]` 并提交。

### 验证进展

- `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/effect_resume_finally_body_raise_after_resume.scoop`：通过。
- `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass`：通过（`fixtures: ok (391)`）。
- `cargo test --all`：通过。
- `cargo run -p scoop -- test`：通过（`fixtures: ok (1270)`）。
- `cargo clippy --all-targets -- -D warnings`：通过。

### 结论

- `P7-T02Z` 记录的剩余默认-refactor `run-pass` blocker 在当前树上已不再复现。
- 本轮验证未发现新的前置阻塞任务。
- 剩余工作仅是任务账本更新：在 `TODO-P7.md` / `TODO.md` 将 `P7-T02Z` 标记为完成，并提交相关记录。

----

## P7-T03 执行记录（2026-05-09）

### 初始计划

说明：这里只记录可公开的执行计划、进度与关键决策，不包含隐藏的内部推理细节。

1. 读取 `TODO.md`，识别第一个标题未带 `[DONE]` 的任务。
2. 检查最近一次提交是否存在与该任务直接相关且未完成的问题；若有，则按要求并入当前任务或在 `TODO.md` 中加入前置任务。
3. 阅读当前任务涉及的代码、测试、规范与依赖文件，确认实现边界与验证要求。
4. 实现该任务所需的最小正确修改；若遇到阻塞当前任务的真实缺口，先在 `TODO.md` 中加入最小前置任务并停止继续向前。
5. 运行与该任务直接相关的验证；在需要时补充或修复测试，并确保通过。
6. 更新 `memory/claude_plan.md` 记录关键进展与必要的计划调整。
7. 按要求更新 `TODO.md` 完成状态与 completion record；仅在阶段计划实际变化时更新 `PLAN.md`。
8. 检查工作区变更，使用清晰的提交信息创建一次 git 提交，然后停止。

### 进展摘要

- 已确认首个未完成任务为 `TODO-P7.md` 中的 `P7-T03`：在 refactor 成为默认主线后运行标准 full regression 矩阵，并修复所有默认路径回归。
- 已核对最近一次提交：`[P7-T02Z] Mark run-pass blockers complete`。提交信息未显式引入新的与 `P7-T03` 直接相关且尚未纳入 `TODO.md` 的 unfinished issue，因此当前直接按 `P7-T03` 执行。
- `git status --short` 确认开始执行时仅有 `memory/claude_plan.md` 变更，没有额外未提交代码需要并入恢复性提交。

### 验证进展

- `cargo test --all`：通过。
- `cargo run -p scoop -- test`：首轮失败在 `tests/fixtures/run-pass/delegated_property_lazy_thread_safety_synchronized_once.scoop` 的 3000ms 超时。
- 定向复验：`cargo run -p scoop -- run tests/fixtures/run-pass/delegated_property_lazy_thread_safety_synchronized_once.scoop`（default）通过；`cargo run -p scoop -- --effect-pipeline legacy run tests/fixtures/run-pass/delegated_property_lazy_thread_safety_synchronized_once.scoop` 通过；`target/debug/scoop test --fixtures tests/fixtures/run-pass/delegated_property_lazy_thread_safety_synchronized_once.scoop` 连续 5 次通过，单次耗时约 1.47s-1.72s。
- `cargo run -p scoop -- test`：重跑通过（`fixtures: ok (1270)`）。
- `cargo run -p scoop_tools -- spec-fixtures check`：通过（`spec fixtures: ok (1)`）。
- `cargo clippy --all-targets -- -D warnings`：通过。
- 已整套重跑标准矩阵：`cargo test --all`、`cargo run -p scoop -- test`、`cargo run -p scoop_tools -- spec-fixtures check`、`cargo clippy --all-targets -- -D warnings`，全部通过。
- 已补显式 `legacy` compare smoke：`cargo run -p scoop -- --effect-pipeline legacy test --fixtures tests/fixtures/run-pass/minimal_main.scoop` 与 `cargo run -p scoop -- --effect-pipeline legacy build --emit-llvm tests/fixtures/build/emit_llvm_basic.scoop -o /tmp/p7_post_regression_legacy.ll`，均通过。

### 结论

- 本轮未发现需要新增到 `TODO.md` 的稳定 blocker；首次 `run-pass` timeout 经复验未稳定复现。
- `P7-T03` 的标准 full regression 已在 omission/default=refactor 条件下通过，且显式 `legacy` 入口仍可用。
- 已更新 `TODO-P7.md` 与 `TODO.md`，把 `P7-T03` 标记为 `[DONE]`；下一步只需提交本轮记录并停止，不继续处理 `P7-T03R`。

----

## P7-T03R 执行记录（2026-05-09）

### 初始计划

说明：这里只记录可公开的执行计划、进度与关键决策，不包含隐藏的内部推理细节。

1. 读取 `TODO.md`，确认首个未完成任务为 `P7-T03R`。
2. 检查最新提交是否声明与该 review 直接相关的未完成问题；若有则先并入当前任务。
3. 阅读 `TODO-P7.md` 中 `P7-T03` / `P7-T03R` 定义与完成记录，确认需要复跑的标准矩阵、explicit legacy smoke 与 hidden-fallback 搜索范围。
4. 重新运行 `P7-T03` 的全部验证命令，确认结果仍在 omission/default=`refactor` 条件下稳定通过。
5. 搜索并抽查 `legacy` / fallback 相关命中，确认没有为了 full regression 通过而新引入 hidden fallback。
6. 若 review 通过，则更新 `TODO.md` / `TODO-P7.md` / 本文件并提交；若发现阻塞，则按要求在 `TODO.md` 增加最小前置任务后停止。

### 进展摘要

- 已读取 `TODO.md`，确认当前首个未完成任务为 `P7-T03R`。
- 已检查最新提交 `fd2f974e`（`[P7-T03] Complete default full regression matrix`）；提交信息未声明需要先处理的直接未完成问题，因此按 `P7-T03R` 的既定 review 范围继续。
- 已读取 `TODO-P7.md` 中 `P7-T03` / `P7-T03R` 条目与 `EFFECT_REFACTOR.md` P7/P8 handoff 说明，确认 review 的核心是证明默认主线的常规 full regression 已真实由 refactor 承担，而不是靠 legacy 兜底。

### 验证进展

- `cargo test --all`：通过。
- `cargo run -p scoop -- test`：通过（`fixtures: ok (1270)`）。
- `cargo run -p scoop_tools -- spec-fixtures check`：通过（`spec fixtures: ok (1)`）。
- `cargo clippy --all-targets -- -D warnings`：通过。
- `cargo run -p scoop -- --effect-pipeline legacy test --fixtures tests/fixtures/run-pass/minimal_main.scoop`：通过（`fixtures: ok (1)`）。
- `cargo run -p scoop -- --effect-pipeline legacy build --emit-llvm tests/fixtures/build/emit_llvm_basic.scoop -o /tmp/p7_post_regression_legacy.ll`：通过。
- `rg "effect[-_]pipeline.*legacy|fallback.*legacy|retry.*legacy" crates tools tests --glob '!target/**'` 等价搜索仅命中四处：
  - `crates/scoop/src/fixtures/run_pass.rs`：显式 legacy compare 测试断言；
  - `crates/scoop/src/fixtures/mod.rs`：`mir_refactor` legacy unsupported 诊断与显式 legacy compare 测试断言；
  - `crates/scoopc/src/driver_cli.rs`：CLI/help 文案，声明 omission 默认是 refactor；
  - `crates/scoop/src/commands/build.rs`：断言默认 build 不得回退到 legacy handler-stack/outcome lowering。

### 结论

- 本轮未发现需要新增到 `TODO.md` 的 blocker，也未发现为了让标准 full regression 通过而新增的 hidden fallback、retry legacy 或 omission 默认回 legacy 的实现路径。
- `P7-T03` 的标准矩阵与 explicit legacy compare smoke 在当前树上仍全部通过，`spec-fixtures check` 也保持通过；这与 `EFFECT_REFACTOR.md` 中“默认主线是 refactor、legacy 仅作短期 compare/rollback”基线一致。
- 已更新 `TODO-P7.md` 与 `TODO.md`，将 `P7-T03R` 标记为 `[DONE]`；下一步仅需提交本轮 review 记录并停止。
