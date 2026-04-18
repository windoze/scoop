# 执行计划记录

## 说明

- 本文件用于记录本次执行的可操作计划、检查点、关键决策与进度更新。
- 出于安全与协作边界考虑，这里不记录逐字的私有思维链，而是记录足以审计和复现执行过程的计划与决策摘要。
- 在后续执行过程中，如果计划调整、发现阻塞、完成关键步骤、或修改任务拆分，我会持续更新本文件。

## 初始目标

本轮只完成 `TODO.md` 中第一个未完成任务，然后停止。

## 初始执行步骤

1. 检查最近一次 Git 提交信息，确认是否明确提到现存问题、已知缺陷或需要先处理的遗留事项。
2. 阅读 `TODO.md`、`PLAN.md`，识别第一个未完成任务，并核对现有计划是否与任务顺序一致。
3. 判断该任务是否过大或依赖缺失：
   - 若可直接完成，则进入实现。
   - 若过大或存在前置缺口，则把任务拆分为更小子任务，更新 `PLAN.md` 与 `TODO.md`，并以第一个子任务作为本轮目标。
4. 实施任务修改，遵循项目约定，不采用规避性实现，不偏离规范。
5. 运行与该任务相关的验证：
   - 最小相关测试
   - 必要时运行更广泛测试
   - `cargo fmt`
   - `cargo clippy --all-targets -- -D warnings`
6. 若测试或验证暴露规范不匹配、缺失特性或已有缺陷：
   - 先修复最近提交中明确提到的遗留问题（若存在且在范围内）
   - 或按要求把缺陷前置为 `TODO.md` 中的新任务 / 依赖，并更新 `PLAN.md`
7. 完成后更新文档：
   - 在 `TODO.md` 标记完成或调整顺序
   - 在 `PLAN.md` 记录当前状态
   - 在本文件记录已完成步骤与结论
8. 使用清晰的 Git 提交信息提交本轮改动。
9. 停止，不继续处理下一个任务。

## 当前已知约束

- 必须优先处理最近一次提交中提到的现存问题（若有）。
- 只做一个任务。
- 不接受 workaround、fixture-only hack、或与规范不符的临时实现。
- 如果遇到缺失能力或阻塞，必须先把问题显式写入 `TODO.md` / `PLAN.md` 并提交，然后停止。

## 待确认信息

- 最近一次提交是否提到需要先修复的问题。
- `TODO.md` 的第一个未完成任务具体内容。
- 是否已有未提交工作树改动需要避让。

## 进度更新

- 已创建本文件并写入初始执行计划，下一步将读取最近一次提交与任务清单。
- 已检查最近一次提交 `6215ee6236d130653b3e9945e9c2dc700fd83fdf`（`[T3017R] Review effect passing baseline`）：
  - 提交信息与变更内容未额外声明“必须先修复的遗留 bug / issue”；
  - 当前可直接进入 `TODO.md` 的首个未完成任务。
- 已定位 `TODO.md` 首个未完成任务为 `T3103`：effect nested-block fixtures 切到 plain `do` block。
- 已初步确认本任务暂不需要再拆分到 `TODO.md` / `PLAN.md`：
  - 代码层 `do { ... }` 与 tail-value 语义已在 `T3101` / `T3102` 接通；
  - 当前工作重心主要是回归迁移与补充边界测试。
- 已盘点当前 effect 相关的 `@Safe { ... }` workaround fixture，待迁移样本共 7 个：
  - `tests/fixtures/run-pass/effect_resume_nested_block_single_perform.scoop`
  - `tests/fixtures/run-pass/effect_resume_while_nested_block_perform.scoop`
  - `tests/fixtures/run-pass/effect_resume_mixed_source_path_matrix.scoop`
  - `tests/fixtures/run-pass/effect_multi_escape_direct_source_path_matrix.scoop`
  - `tests/fixtures/run-pass/effect_multi_escape_custom_nonresuming_direct_block_multi.scoop`
  - `tests/fixtures/run-pass/effect_multi_escape_custom_nonresuming_direct_indirect_block_multi.scoop`
  - `tests/fixtures/run-pass/effect_multi_escape_custom_nonresuming_indirect_block_single_site.scoop`
- 已确认本任务还需要额外补一组回归，覆盖：
  - multiple trailing lambdas 后的下一条 `do { ... }` 作为独立 statement / block；
  - `do { ... }` 作为普通调用实参时，不与 trailing lambda 竞争；
  - parser / typecheck / HIR / run-pass 四个层面的稳定行为。
- 已完成主要修改：
  - 7 个 effect nested-block run-pass fixtures 已从 `@Safe { ... }` 迁移为 plain `do { ... }`；
  - 已新增 parser / typecheck / HIR / run-pass 边界回归文件：
    - `tests/fixtures/parse/do_block_multiple_trailing_lambda_boundary.scoop`
    - `tests/fixtures/typecheck/do_block_multiple_trailing_lambda_boundary_ok.scoop`
    - `tests/fixtures/hir/do_block_multiple_trailing_lambda_boundary.scoop`
    - `tests/fixtures/run-pass/do_block_multiple_trailing_lambda_boundary.scoop`
  - 已生成对应 golden：
    - `tests/fixtures/parse/do_block_multiple_trailing_lambda_boundary.ast`
    - `tests/fixtures/hir/do_block_multiple_trailing_lambda_boundary.hir`
- 已完成定向验证：
  - `diff -u tests/fixtures/parse/do_block_multiple_trailing_lambda_boundary.ast <(cargo run -p scoop -- dump-ast tests/fixtures/parse/do_block_multiple_trailing_lambda_boundary.scoop)` 通过；
  - `diff -u tests/fixtures/hir/do_block_multiple_trailing_lambda_boundary.hir <(cargo run -p scoop -- dump-hir tests/fixtures/hir/do_block_multiple_trailing_lambda_boundary.scoop)` 通过；
  - `cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/do_block_multiple_trailing_lambda_boundary.scoop` 通过；
  - `cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/effect_resume_nested_block_single_perform.scoop` 通过；
  - `cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/effect_resume_mixed_source_path_matrix.scoop` 通过。
- 下一步：执行全量质量门槛验证（`cargo test --all`、`cargo run -p scoop -- test`、`cargo run -p scoop --features llvm -- test`、`cargo clippy --all-targets -- -D warnings`），若全通过则更新 `TODO.md` / `PLAN.md` 并提交。
- 已完成全量质量门槛验证：
  - `cargo test --all` 通过；
  - `cargo run -p scoop -- test` 通过（`fixtures: ok (996)`）；
  - `cargo run -p scoop --features llvm -- test` 通过（`fixtures: ok (996)`）；
  - `cargo clippy --all-targets -- -D warnings` 通过。
- 已完成文档状态回写：
  - `TODO.md` 中 `T3103` 已标记为 `[DONE]`，并补充本轮迁移范围、边界回归与验证结果；
  - `PLAN.md` 已记录本轮完成更新，并把当前执行顺序推进到 `T3104`。
- 剩余收尾步骤：
  1. 检查工作树差异是否只包含本轮预期文件；
  2. 以清晰提交信息提交；
  3. 停止，不继续处理 `T3104`。
