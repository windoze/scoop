# 本次执行计划

说明：这里记录可执行计划、关键决策与进度更新，不包含内部推理细节。

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
