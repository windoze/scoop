# 本次执行计划

说明：这里记录可执行计划、关键决策与进度更新，不包含内部推理细节。

## 2026-05-02 新一轮执行

1. 重新读取 `TODO.md` 索引，并按引用顺序核对 `TODO-Px.md`，确认当前仓库中的第一个未完成详细任务。
2. 检查最新提交与工作区状态，确认是否存在与该任务直接相关的未完成事项或已做未提交改动。
3. 若当前首个未完成任务仍是既有任务，则直接完成它；若已完成，则定位下一个未完成任务并以详细任务文件为准。
4. 实施任务所需代码与测试改动；如果遇到真实阻塞，则先在相应 `TODO-Px.md` 中插入最小前置任务并同步 `TODO.md`。
5. 运行当前任务要求的验证，包括相关定向测试、必要的 `cargo test` / `cargo clippy --all-targets -- -D warnings` 或更小范围等价校验。
6. 更新 `TODO-Px.md` 完成记录；仅在索引/顺序变化时同步 `TODO.md`，仅在阶段计划变化时更新 `PLAN.md`。
7. 整理本次相关改动，使用清晰的任务号提交信息创建一次 git commit，然后停止。

### 当前定位

- 已按 `TODO.md -> TODO-P0.md ... TODO-P3.md` 顺序核对完成记录。
- 当前仓库中的首个未完成详细任务是 `TODO-P3.md` 的 `P3-T04R`。
- 最新提交为 `[P3-T04] Freeze refactor MIR snapshot baselines`，未在提交信息中显式声明与 `P3-T04R` 直接相关的未完成事项。
- 当前工作区另有未提交改动：`crates/scoop/src/commands/dump_ir.rs`、`crates/scoopc/src/hir/lower/expr.rs`、`crates/scoopc/src/hir/lower/mod.rs`、`crates/scoopc/src/parser/tests.rs`、`crates/scoopc/src/typecheck/expr/call.rs`。这些文件与本次 `P3-T04R` review 目标暂不直接相关，除非后续验证发现冲突，否则不触碰。

### P3-T04R 执行步骤

1. 阅读 `P3-T04R` 指定的关键实现位置，重点复核：refactor MIR stage/formatter 是否独立存在、typed contract 是否已下沉到 MIR、CFG/cleanup/finally 是否显式化、refactor snapshot/golden 是否与 legacy baseline 分离。
2. 重新运行 `P3-T01` 到 `P3-T04` 要求的定向测试与命令，保持在 P3 范围内，不扩大到全量测试。
3. 若 review 发现 refactor MIR 仍需回看 HIR、snapshot 入口不统一、或阶段退出条件未满足，则先修复该问题；若无法在当前调用内修复，则按要求补最小前置任务并同步 `TODO.md`。
4. 若 review 通过，则在 `TODO-P3.md` 的 `P3-T04R` 条目下写入完成记录；仅在索引/阶段计划变化时再更新 `TODO.md` / `PLAN.md`。
5. 复查差异，仅提交本次 review 相关文件与 `memory/claude_plan.md`，创建一次 `git commit` 后停止。

### 进行中进展

- 已完成代码复核：`crates/scoopc/src/effect_refactor_pipeline/mir_stage.rs`、`crates/scoop/src/commands/dump_mir.rs`、`crates/scoop/src/fixtures/mod.rs`、`crates/scoopc/src/mir/mod.rs` 当前与 `P3-T04` 目标一致，且 CLI/fixture 共享同一 `stable_dump()` formatter。
- 已通过定向 Rust 测试：`refactor_direct_mir_stage`、`refactor_mir_lowering_contract`、`refactor_mir_cfg`、`refactor_mir_site_id`、`effect_refactor_pipeline`、`dump_mir`、`parity`。
- 已通过部分 CLI smoke：legacy/refactor `direct_zero_arg_call`、refactor `direct_and_fun_value_call`、legacy/refactor `dispatch_and_resume_call`、refactor `handle_perform`、refactor `continuation_resume_unit_sugar`、refactor `handle_finally_boundary`、refactor `effect_boundary_inside_expr_context`。
- 并行 `cargo run` 期间曾出现 `while_break_continue` 命令超时；单独复跑 `cargo run -q -p scoop --no-default-features -- --effect-pipeline refactor dump-mir tests/fixtures/mir/while_break_continue.scoop` 后已确认输出正常，该超时源于并行构建锁等待，不是实际挂起。
- 已完成剩余 `mir_refactor` / legacy fixture 复验与 `cargo clippy -q -p scoop -p scoopc --all-targets --no-default-features -- -D warnings`；当前 review 结论为：`P3-T04R` 可关闭，且无需在 `P4-T01` 前补入新前置任务。
- 已将 review 结果回写到 `TODO-P3.md`；下一步仅需整理本次相关差异并创建 `[P3-T04R] ...` 提交。

1. 读取 `TODO.md` 作为索引，并按其中引用顺序检查对应的 `TODO-Px.md` 文件。
2. 确认第一个未完成的详细任务，以及它在详细任务文件中的完成判定标准、约束、依赖与验证要求。
3. 检查最新提交是否明确提到与当前任务直接相关的未完成问题；若是，则将其视为当前任务内容或前置依赖。
4. 实现当前任务，避免用变通方案绕过规范要求；若遇到阻塞，按要求在相应 `TODO-Px.md` 中补入最小前置任务，并同步 `TODO.md`。
5. 运行与当前任务直接相关的测试、格式化、`clippy`/构建校验，并修复发现的问题。
6. 更新 `TODO-Px.md` 的完成记录；如果任务索引、标题、顺序或依赖变化，则同步更新 `TODO.md`；仅在阶段计划变化时更新 `PLAN.md`。
7. 复查工作区差异，仅提交本次任务相关修改，使用清晰的任务号提交信息创建一次 git commit，然后停止。

## 进度更新

- 已写入初始计划，下一步开始读取任务索引并定位首个未完成详细任务。
- 已确认当前执行单元为 `TODO-P3.md` 中的 `P3-T04`：建立 refactor 专属 `dump-mir` snapshot / golden 矩阵，并冻结 P3 -> P4 的 MIR handoff contract。
- 当前实施计划：
  1. 检查现有 `tests/fixtures/mir_refactor/**`、`scoop test` fixture phase、`dump-mir` formatter 与 refactor MIR stage 输出注释，找出 `P3-T04` 尚未满足的缺口。
  2. 以最小改动补齐独立 refactor MIR golden 路径，并确保 CLI、fixture runner 与 Rust 测试复用同一 stage helper / formatter。
  3. 在代码注释或等价文档实体中明确 P3 -> P4 handoff contract：P4 只消费 refactor MIR stage 输出，不得回看 P2 HIR side tables。
  4. 运行 `P3-T04` 要求的定向 fixture / CLI / clippy 校验，确认 legacy baseline 不受影响。
  5. 更新 `TODO-P3.md` 的完成记录，必要时同步 `TODO.md` / `PLAN.md`，然后创建一次 git commit 并停止。
- 已完成代码与基线改动：
  - 新增 `mir_refactor` fixture phase，并强制其只能通过 `--effect-pipeline refactor` 进入。
  - 让 `mir_refactor` fixtures 与 CLI `dump-mir --effect-pipeline refactor` 统一复用 `load_direct_style_mir_stage_output_for_dump(...)` + `RefactorMirStageOutput::stable_dump()`。
  - 补齐 `tests/fixtures/mir_refactor/**` 的独立 `.scoop` / `.mir` goldens，覆盖 direct call、dispatch/resume、perform/handle、handle/finally、control-flow、boundary-in-expression，以及已有的 `continuation_resume_unit_sugar`。
  - 在 `RefactorMirStageOutput` 注释中冻结 P3 -> P4 handoff contract：P4 的 authoritative 输入是 refactor MIR stage 输出上的 callable body identity、可选 `materialized_mir`、以及 MIR 节点 `SiteId` / metadata，而不是回看 P2 HIR side tables。
  - 发现并修复一个直接阻塞验收的 legacy 基线漂移：`tests/fixtures/mir/dispatch_and_resume_call.mir` 未同步 richer `ResumeMetadata` 输出，现已回写。
- 已完成验证：
  - `cargo run -q -p scoop --no-default-features -- --effect-pipeline refactor test --fixtures tests/fixtures/mir_refactor`
  - `cargo run -q -p scoop --no-default-features -- --effect-pipeline refactor test --fixtures tests/fixtures/mir_refactor/direct_and_fun_value_call.scoop`
  - `cargo run -q -p scoop --no-default-features -- --effect-pipeline refactor test --fixtures tests/fixtures/mir_refactor/dispatch_and_resume_call.scoop`
  - `cargo run -q -p scoop --no-default-features -- --effect-pipeline refactor test --fixtures tests/fixtures/mir_refactor/handle_perform.scoop`
  - `cargo run -q -p scoop --no-default-features -- --effect-pipeline refactor test --fixtures tests/fixtures/mir_refactor/handle_finally_boundary.scoop`
  - `cargo run -q -p scoop --no-default-features -- --effect-pipeline refactor test --fixtures tests/fixtures/mir_refactor/effect_boundary_inside_expr_context.scoop`
  - `cargo run -q -p scoop --no-default-features -- --effect-pipeline refactor dump-mir tests/fixtures/mir_refactor/dispatch_and_resume_call.scoop`
  - `cargo run -q -p scoop --no-default-features -- --effect-pipeline refactor dump-mir tests/fixtures/mir_refactor/handle_finally_boundary.scoop`
  - `cargo run -q -p scoop --no-default-features -- --effect-pipeline legacy test --fixtures tests/fixtures/mir/dispatch_and_resume_call.scoop`
  - `cargo test -q -p scoop --no-default-features dump_mir`
  - `cargo test -q -p scoopc --no-default-features refactor_direct_mir_stage`
  - `cargo clippy -q -p scoop -p scoopc --all-targets --no-default-features -- -D warnings`
- 下一步：回写 `TODO-P3.md` 完成记录，随后整理 git 差异并创建本次任务提交。
