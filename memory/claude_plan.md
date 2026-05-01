# Claude Plan

## 约束说明
- 不写入内部逐字推理；此文件记录可审阅的执行计划、关键决策、进度和阻塞。
- 本次调用只处理一个详细任务：完成第一个未完成任务，或在存在真实阻塞时补充最小前置任务后停止。

## 初始执行计划
1. 读取 `TODO.md`，把它当作索引使用。
2. 按索引顺序读取对应的 `TODO-Px.md`，定位第一个未明确记录完成的详细任务。
3. 检查最近提交是否存在与该任务直接相关且明确未完成的问题；若有，将其并入当前任务范围，或在详细 TODO 中登记为前置任务。
4. 阅读与当前任务直接相关的代码、测试、规范和任务约束，确认实现边界。
5. 实现该任务，避免规避规范或引入临时性变通方案。
6. 运行与改动直接相关的测试；若任务要求更广验证，则补充运行必要命令，包括无 warning 的检查。
7. 更新对应 `TODO-Px.md` 的完成记录；如任务索引、标题、顺序或依赖变化，同步更新 `TODO.md`；仅在阶段计划真正变化时更新 `PLAN.md`。
8. 检查工作区状态，整理本次改动并创建一次 git 提交。
9. 停止，不继续下一个任务。

## 进度记录
- 已创建初始计划，下一步开始读取 `TODO.md` 与详细任务文件。
- 已读取 `TODO.md` 与 `TODO-P0.md` / `TODO-P1.md`。
- 已确认 `TODO-P0.md` 全部完成；当前第一个未完成详细任务为 `P1-T01`：建立 refactor AST stage 专用入口与阶段输出类型。

## 当前任务：P1-T01
1. 检查最近一次提交是否明确记录了与 `P1-T01` 直接相关的未完成问题。
2. 阅读 `effect_refactor_pipeline`、`dump-ast`、`session`、parser 相关实现，确认当前 refactor `dump-ast` 仍如何落到 legacy。
3. 在 refactor 新路径中新增 AST stage 模块与稳定输出类型，并写清 P1 handoff invariants。
4. 把 refactor `dump-ast` 路径改为显式经过 AST stage，再输出 AST 文本；保持 parser 作为中立共享模块。
5. 增补/调整对应单元测试，证明 stage 可构造且 refactor `dump-ast` 已进入 AST stage。
6. 运行 `P1-T01` 要求的定向测试与 smoke；必要时修复告警或测试失败。
7. 更新 `TODO-P1.md` 的完成记录；若索引未变则不改 `TODO.md`；若阶段计划未变则不改 `PLAN.md`。
8. 提交本次改动并停止。

## 进展更新
- 最近提交检查完成：`[P0-T04R]` 仅结束 P0 review，没有附带与 `P1-T01` 直接相关的未完成事项。
- 代码实现完成：新增 `crates/scoopc/src/effect_refactor_pipeline/ast_stage.rs`，定义 `AstStageOutput<'a>`，并在注释中固定 P1 AST handoff invariants。
- 路由调整完成：`effect_refactor_pipeline::load_ast_stage_output_for_dump(...)` 在 `refactor` 模式下显式进入 AST stage；`dump-ast` 改为消费 stage 输出，而不是只拿裸 `ast::File`。
- 共享边界保持不变：AST stage 继续通过 `Session::parse(...)` 调用 parser，没有把 pipeline selector 下沉到 parser 业务逻辑。
- 验证完成：
  - `cargo test -p scoopc --no-default-features ast_stage`
  - `cargo test -p scoopc --no-default-features effect_refactor_pipeline`
  - `cargo test -p scoop --no-default-features dump_ast_command_uses_refactor_ast_dispatcher`
  - `cargo run -p scoop --no-default-features -- --effect-pipeline refactor dump-ast tests/fixtures/parse/hello.scoop`
  - `cargo run -p scoop --no-default-features -- --effect-pipeline legacy dump-ast tests/fixtures/parse/hello.scoop`
  - `cargo clippy -p scoop -p scoopc --all-targets --no-default-features -- -D warnings`
- smoke 结果：`legacy` / `refactor` 的 `dump-ast tests/fixtures/parse/hello.scoop` 输出一致。
- `TODO-P1.md` 已写入 `P1-T01` 完成记录；`TODO.md` 与 `PLAN.md` 无需同步修改。
- 剩余收尾：检查工作区、提交并停止。
