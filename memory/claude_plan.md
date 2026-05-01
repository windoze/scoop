## 当前执行计划

1. 读取 `TODO.md`，确认详细任务文件映射关系。
2. 按索引顺序读取相关 `TODO-Px.md`，定位第一个未完成的详细任务，并核对该任务的约束、依赖、验证要求与完成记录。
3. 检查最近一次提交是否包含与该任务直接相关且未完成的问题；如果存在，则将其视为当前任务的一部分，或在对应 `TODO-Px.md` 中补充为前置任务。
4. 阅读实现涉及的代码与测试，确认当前行为、缺口、以及是否存在阻塞当前任务的规范不匹配。
5. 如果没有阻塞项，最小化修改代码以完整实现当前任务；如果存在必须先解决的新前置问题，则仅新增最少必要任务，更新 `TODO-Px.md` 与 `TODO.md`，并停止继续后续任务。
6. 运行与当前任务直接相关的测试、检查与必要的验证命令；若失败则立即修复并重新验证。
7. 更新 `memory/claude_plan.md` 记录关键进展与计划变化。
8. 在对应 `TODO-Px.md` 中记录任务完成情况；若任务索引、标题、顺序或依赖变更，则同步更新 `TODO.md`；仅在阶段计划确实变化时更新 `PLAN.md`。
9. 按仓库提交风格创建一次 git 提交，提交信息包含当前任务 id，然后停止。

## 说明

- 我不会在此文件中记录逐词逐句的内部推理，而是记录可审计的执行计划、关键判断、阻塞项与进度更新。
- 若执行过程中发现当前任务无法按规范直接完成，我会先把阻塞原因和新增前置任务写回该文件，再同步更新详细 TODO 文件与索引。

## 当前任务定位

- 已按 `TODO.md -> TODO-P0.md -> TODO-P1.md` 顺序检查完成记录。
- `TODO-P0.md` 全部条目已填写完成记录。
- `TODO-P1.md` 中 `P1-T01`、`P1-T01R`、`P1-T02`、`P1-T02R`、`P1-T03` 已完成；第一个未完成详细任务是 `P1-T03R`（Review P1 阶段退出条件，确认可以进入 HIR / typecheck 新路径）。
- 最近一次提交为 `[P1-T03] Freeze AST handoff parity`；提交说明未显式记录与 `P1-T03R` 直接相关的未完成问题，因此当前按 review 任务本身继续执行。

## 针对 P1-T03R 的执行计划

1. 阅读并复核 `P1-T01` 到 `P1-T03` 直接相关的实现与测试入口，重点确认：
   - refactor AST stage 是否已成为独立阶段入口；
   - parser / AST 是否仍保持中立共享；
   - surface contract 与 P2 handoff 是否已写成稳定 contract；
   - AST parity 测试是否真实比较 legacy/refactor 路径。
2. 按 `P1-T03R` 要求重新运行 P1 的定向测试与 smoke 命令。
3. 如果复核和验证都通过，则回写 `TODO-P1.md` 的 `P1-T03R` 完成记录，并补充本文件进度。
4. 检查 `git status`，仅提交本次任务相关变更，使用包含 `P1-T03R` 的提交信息后停止。

## 当前进展

- 已完成实现与测试复核：`ast_stage.rs`、`effect_refactor_pipeline/{mod,refactor}.rs`、`dump_ast.rs`、`parser/tests.rs`、`parser/expr.rs`、`commands/parity.rs`。
- 关键结论：
  - refactor AST stage 已是独立阶段入口；
  - parser 仍经 `Session::parse(...)` 作为中立共享模块复用，且 `parser/` 内未出现 pipeline selector 命中；
  - `resume` 相关唯一特殊分支是已删除旧语法 `-> resume { ... }` 的迁移诊断，不影响普通 `k.resume(...)` / `k.resume()` member-call 解析；
  - AST parity harness 继续通过 CLI 参数比较 legacy/refactor 输出一致。
- 已通过验证：
  - `cargo test -p scoopc --no-default-features ast_stage`
  - `cargo test -p scoopc --no-default-features effect_refactor_pipeline`
  - `cargo test -p scoop --no-default-features dump_ast_command_uses_refactor_ast_dispatcher`
  - `cargo test -p scoopc --no-default-features parser::tests`
  - `cargo test -p scoop --no-default-features refactor_ast_stage_parity`
  - 三个 parse fixture 的 `legacy/refactor` 定向 `scoop test --fixtures`
  - `hello.scoop`、`continuation_resume_member_call_basic.scoop`、`unit_single_param_zero_arg_call_basic.scoop` 的 legacy/refactor `dump-ast` diff
  - `cargo clippy -p scoop -p scoopc --all-targets --no-default-features -- -D warnings`
- 未发现阻塞项；当前进入收尾：检查工作树并提交 `P1-T03R`。
