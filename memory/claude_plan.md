# 执行计划

1. 读取 `TODO.md`，仅把它作为任务索引使用。
2. 按 `TODO.md` 引用顺序读取对应的 `TODO-Px.md`，确认第一个未完成的详细任务；如果详细文件与索引不一致，以详细文件为准。
3. 检查最近一次提交是否直接提到与该任务相关且未完成的问题；如果这是当前任务的直接前置条件，则将其纳入当前任务范围，或在相应 `TODO-Px.md` 中补充最小必要前置任务，并同步 `TODO.md`。
4. 阅读当前任务涉及的代码、规格、测试和依赖约束，确认不能通过缩小范围或规避路径完成任务。
5. 直接实现当前详细任务；如果遇到阻塞当前任务的真实缺陷或缺失能力，则先修复它，或在对应 `TODO-Px.md` 中新增最小前置任务并同步 `TODO.md`，然后停止。
6. 运行与当前任务直接相关的验证，包括必要的单测、集成测试、夹具测试，以及适用时的 `cargo fmt`、`cargo test`、`cargo clippy --all-targets -- -D warnings`。
7. 更新 `memory/claude_plan.md` 记录关键进展或计划变更。
8. 在对应 `TODO-Px.md` 中记录当前任务完成情况；若任务编号、标题、顺序或依赖变化，同步更新 `TODO.md`；仅当阶段计划本身变化时才更新 `PLAN.md`。
9. 按仓库提交风格创建一次 git 提交，并在完成当前详细任务后停止，不继续处理下一个任务。

## 进展记录

- 已写入初始执行计划，下一步开始读取 `TODO.md` 与详细任务文件，定位首个未完成任务。
- 已读取 `TODO.md` 与 `TODO-P0.md`、`TODO-P1.md`；确认当前执行单元是 `P1-T01R`（Review AST stage 入口与 handoff 类型，确认 parser 仍是中立共享模块）。
- 当前任务的执行步骤细化为：
  1. 检查最近一次提交信息，确认是否显式记录了与 `P1-T01R` 直接相关且未完成的问题。
  2. 复核 `crates/scoopc/src/effect_refactor_pipeline/ast_stage.rs`、`crates/scoopc/src/effect_refactor_pipeline/mod.rs`、`crates/scoop/src/commands/dump_ast.rs`、`crates/scoopc/src/session/mod.rs` 的实现与 contract。
  3. 搜索 `crates/scoopc/src/parser` 与 `crates/scoopc/src/ast`，确认没有把 pipeline mode 注入 parser/AST 业务逻辑。
  4. 运行 `P1-T01R` 指定的定向测试、smoke 和必要 lint。
  5. 若 review 通过，则回写 `TODO-P1.md` 的完成记录；若发现直接阻塞 `P1-T01R` 的问题，则先修复或按要求补最小前置任务并同步索引。
  6. 提交本次变更后停止，不进入 `P1-T02`。
- 最近一次提交为 `[P1-T01] Introduce refactor AST stage output`，未显式记录与 `P1-T01R` 直接相关的未完成事项。
- 代码复核结果：
  1. `crates/scoopc/src/effect_refactor_pipeline/ast_stage.rs` 已定义 `AstStageOutput<'a>`，并把 P1 handoff invariants 固定在文档注释中。
  2. `crates/scoopc/src/effect_refactor_pipeline/refactor.rs` 通过 `ast_stage::run(...)` 提供 refactor AST stage 入口；`crates/scoop/src/commands/dump_ast.rs` 的生产路径经 `load_ast_for_dump(...)` -> `load_ast_stage_output_for_dump(...)` 进入该 stage。
  3. `crates/scoopc/src/session/mod.rs` 仍通过中立共享 API `Session::parse(&SourceFile)` 复用 parser；`crates/scoopc/src/parser/mod.rs` 仅暴露 `parse_file(...)`，没有 selector 参数。
- 搜索 `rg -n "EffectPipelineMode|effect_pipeline|legacy|refactor" crates/scoopc/src/parser crates/scoopc/src/ast` 输出为 0 命中，未发现 parser/AST 业务代码中的 pipeline 渗透。
- 定向验证已通过：`cargo test -p scoopc --no-default-features ast_stage`、`cargo test -p scoopc --no-default-features effect_refactor_pipeline`、`cargo test -p scoop --no-default-features dump_ast_command_uses_refactor_ast_dispatcher`、`diff -u <(cargo run -q -p scoop --no-default-features -- --effect-pipeline legacy dump-ast tests/fixtures/parse/hello.scoop) <(cargo run -q -p scoop --no-default-features -- --effect-pipeline refactor dump-ast tests/fixtures/parse/hello.scoop)>`、`cargo clippy -p scoop -p scoopc --all-targets --no-default-features -- -D warnings`。
- 已回写 `TODO-P1.md` 的 `P1-T01R` 完成记录；下一步执行 git 状态检查并创建本次任务提交，然后停止。
