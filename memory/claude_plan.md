# 本轮执行计划

## 说明

按你的要求，我会先记录一份可审阅的执行思路摘要与操作计划，再开始读取仓库状态、最新提交和任务列表。这里记录的是高层次执行方案、检查顺序、决策准则和进度更新点，不包含不可审阅的内部推理细节。

## 目标

本轮只完成 `TODO.md` 中的第一个未完成任务；如果在执行前或执行过程中发现已存在问题阻塞该任务，则先修复该问题，或将其作为前置任务插入 `TODO.md` 并停止。

## 执行步骤

1. 检查最新一次 Git 提交：
   - 查看提交标题与正文是否提到已知问题、回归、临时方案或待补修复。
   - 若最新提交明确提到未解决问题，则优先修复该问题。

2. 读取任务与计划文件：
   - 读取 `TODO.md`，找出第一个未完成任务。
   - 读取 `PLAN.md`，理解当前项目阶段、依赖关系与任务上下文。

3. 判断任务粒度：
   - 如果第一个未完成任务过大或依赖未满足，则将其拆分为更小的前置子任务。
   - 更新 `PLAN.md` 与 `TODO.md`，保证任务顺序反映真实依赖关系。
   - 如果只是拆分任务而无法继续实现，则提交这些计划调整并停止。

4. 实施任务：
   - 修改代码、测试、文档或任务文件，以完整实现当前目标。
   - 在过程中主动检查是否暴露出已有缺陷、规格不匹配、未完成边界或依赖缺失。
   - 若发现这类问题，会优先修复；如无法在本轮直接修复，则将其插入 `TODO.md` 作为前置任务并停止。

5. 验证质量：
   - 运行与改动直接相关的测试。
   - 如有必要，运行更高覆盖度的检查，例如 `cargo test --all`、`cargo clippy --all-targets -- -D warnings`、特定夹具测试或格式化检查。
   - 若测试失败，先修复再继续。

6. 更新文档与任务状态：
   - 在 `TODO.md` 中将本轮完成的任务标记为完成。
   - 在 `PLAN.md` 中更新当前状态、后续影响和剩余工作。
   - 按需回写本文件，记录关键步骤完成情况和计划变化。

7. 提交并停止：
   - 使用清晰的 Git 提交信息提交本轮所有改动。
   - 完成一个任务后立即停止，不继续处理下一个任务。

## 关键检查点

- 如果最新提交提到待修问题：先修它。
- 如果第一个未完成任务存在缺失前置能力：先把前置任务写进 `TODO.md`，调整顺序后停止。
- 不接受规避式实现、夹具特判、窄化规格或替代表示来绕过真正缺陷。
- 若执行过程中计划变化，本文件会同步更新。

## 当前状态

- 已完成：创建本计划文件。
- 已完成：检查最新 Git 提交、`TODO.md`、`PLAN.md`、`ISSUES.md`。
- 结论：
  - 最新提交 `bfab87ee3cf47d5a060c541360c6918d28e847bb` 标题为 `[T1220b] Reuse typechecked bindings for package-level comptime if`，未在提交标题/正文中直接声明待补修复项。
  - `TODO.md` 中顺序上的首个未完成总任务是 `T4015`，其剩余未完成具体子任务为 `T4015R`。
  - `T4015R` 的目标是 review const/comptime 主线，确认不再残留“同文件 + 名字/参数个数 + 字面量求值”的旧旁路。
- 复审中发现并确认的既有问题：
  - 多个入口仍直接调用旧的 `trim_package_level_comptime_ifs(...)`，导致 package-level `comptime if` 在这些路径上继续丢失 compilation-unit 的 typechecked 调用绑定。
  - 具体暴露点包括：`typecheck_multi` fixture runner、多源 cone 导出/分析路径、`Session::build_top_level_index`、若干单源但带 sysroot 可见性的 dump/RTTI 路径，以及 sysroot 自身加载时的 package-level trim。
  - 已用新增回归 `tests/fixtures/typecheck_multi/package_level_comptime_if_cross_file_const_fun/` 复现旧问题；旧代码会稳定报 `scoop::comptime::unsupported_const_fun_signature`（`explicit type args`）。
- 已完成的修复：
  - 扩展 comptime trim API，使其可在带 visible-unit / cone id 的上下文中刷新 package-level `comptime if` 条件的 typechecked 绑定。
  - 将 sysroot、自顶向下索引、cone 导出/注解/visibility/pre-specialize、fixture runner、HIR/MIR dump、RTTI 与相关测试入口统一切到新的 compilation-unit trim 主线。
  - 新增回归：
    - `tests/fixtures/typecheck_multi/package_level_comptime_if_cross_file_const_fun/`
    - `tests/fixtures/typecheck_cone/package_level_comptime_if_cross_cone_const_fun/`
    - `session::tests::build_top_level_index_trims_package_level_comptime_if_across_source_set`
    - `cone::scoopir::tests::export_public_api_for_cone_sources_trims_package_level_comptime_if_across_files`
- 已完成的定向验证：
  - `cargo fmt --all`
  - `cargo run -p scoop -- test --fixtures tests/fixtures/typecheck_multi/package_level_comptime_if_cross_file_const_fun`
  - `cargo test -p scoopc build_top_level_index_trims_package_level_comptime_if_across_source_set -- --nocapture`
  - `cargo test -p scoopc export_public_api_for_cone_sources_trims_package_level_comptime_if_across_files -- --nocapture`
  - `cargo test -p scoopc package_level_comptime_if_ -- --nocapture`
  - `cargo run -p scoop -- test --fixtures tests/fixtures/run_pass_cone/package_level_comptime_if_cross_file_const_fun`
- 下一步：运行全量验证（至少 `cargo run -p scoop -- test`、`cargo test --all`、`cargo clippy --all-targets -- -D warnings`，必要时补 `spec-fixtures check`），若全部通过则回写 `TODO.md` / `PLAN.md` 并提交。
