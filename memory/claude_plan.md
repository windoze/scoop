# 当前执行计划

## 说明
- 本文件记录本次 invocation 的可审阅执行计划、关键步骤和进度更新。
- 不记录私有推理链；只记录任务目标、执行步骤、验证方式和变更结果。

## 初始计划
1. 读取 `TODO.md`，按文件顺序识别第一个标题未以 `[DONE]` 开头的任务。
2. 检查最近提交信息；仅当它明确提到与当前任务直接相关的未完成问题时，将其纳入当前任务或在 `TODO.md` 中补为前置任务。
3. 阅读当前任务要求、依赖和验证要求，并按需查看相关代码、测试和文档。
4. 若任务可直接完成，进行最小且完整的实现；若遇到阻塞当前任务的缺失特性、规格不匹配或测试失败，先修复，或在 `TODO.md` 中插入最小前置任务并停止。
5. 按要求运行格式化、lint、相关测试，并在需要时运行完整测试和 fixture 套件。
6. 更新 `TODO.md`：完成时在任务标题前加 `[DONE]` 并填写完成记录；若只做任务拆分或阻塞记录，则保持当前任务未完成。
7. 仅当阶段级计划发生变化时更新 `PLAN.md`。
8. 检查 git 状态和差异，提交本次 invocation 的所有相关变更，然后停止，不继续下一个任务。

## 进度
- 已创建初始计划，下一步读取 `TODO.md` 确认当前任务。
- 已确认第一个未完成任务为 `P0-T02：核对并冻结 immortal 当前行为基线`，对应 `TODO-1.md`。
- 最近提交为 `338f5bc0 [P0-T01R] Record review progress`，未明确提到与 `P0-T02` 直接相关的未完成问题。

## P0-T02 执行步骤
1. 核对 String literal、TypeMetadataLiteral、Platform literal 当前分配路径和 `scoop_alloc_typed` 调用点。
2. 核对 runtime marker 的 `mark` 写入、heap membership 过滤和当前 immortal 透明性支点。
3. 核对 `__AtomicInt` 的 typealias 定义、sysroot atomic 构造点，以及 5 个擦除点。
4. 只更新任务/文档记录，不改运行期或编译期行为。
5. 先运行格式化和 lint，再按任务要求运行 Rust 测试；若只有 Markdown 变更且已有可复用绿色完整结果，则按规则复用并记录原因。
6. 更新 `TODO.md` 与 `TODO-1.md`，提交本次变更后停止。

## P0-T02 核对进度
- 已核对 String literal、TypeMetadataLiteral、Platform literal 的当前动态分配路径。
- 已核对 runtime header、serial/parallel marker 写 `mark` 路径、heap-membership 过滤和 pinned/handle 直接 mark 入口。
- 已核对 `__AtomicInt` sysroot typealias、`core.scoop` atomic raw 构造点，以及任务列出的 5 个擦除点；另外发现 codegen ABI/type 映射处也有直接把 `__AtomicInt` 映射为 word `Int` 的现状，完成记录将一并写明。
- 已更新 `TODO-1.md` 的 P0-T02 完成记录，并在 `TODO.md` / `TODO-1.md` 将 P0-T02 标记为 `[DONE]`。
- 最终 `git diff --check` 已通过；本任务仅修改 Markdown/任务记录，完整代码 suite 按项目规则复用最近绿色结果。

## 保留的既有记录

以下内容为本次 invocation 开始前文件中已有记录，保留用于连续审计。

# Current Invocation Plan

Status: task review completed and committed.

Note: This file records the actionable plan, progress, validation results, and decisions for this invocation. It intentionally avoids private reasoning details while preserving the information needed to audit progress.

## Execution Plan

1. Read `TODO.md` first and identify the first task whose title is not prefixed with `[DONE]`.
2. Check the latest commit message only for unfinished work directly relevant to that selected task.
3. Read the selected task body, dependencies, completion record, and any directly referenced files/specs.
4. Implement the selected task exactly as written, unless a concrete prerequisite or blocker makes that impossible.
5. If blocked by an unscheduled prerequisite, update `TODO.md` with the minimum required prerequisite task, keep the current task incomplete, commit that bookkeeping, and stop.
6. Run required formatting, linting, tests, and fixtures in the required order: `cargo fmt`, `cargo clippy --all-targets -- -D warnings`, then relevant/full tests and fixtures as applicable.
7. Address any unscheduled failing test or fixture before marking the task done.
8. Update `TODO.md` by prefixing the completed task title with `[DONE]` and filling in its completion record.
9. Update this plan file with key progress and validation results.
10. Inspect git status/diff/log, commit all intended changes with a task-specific message, and stop without starting the next task.

## Progress Log

- Created initial invocation plan before running repository commands.
- Identified first incomplete task: `P0-T01R` in `TODO-1.md`, review of the completed pacing behavior baseline.
- Latest commit is `[P0-T01] Freeze GC pacing baseline`; it is directly relevant but does not mention a separate unfinished issue in its subject.
- Reviewed the cited pacing baseline code locations. Main line references still match; the review found that the cycle-end `next_gc` update point needed to be made explicit.
- Updated `TODO-1.md` and `GC_PACING.md` to record the cycle-end update point in `scoop_gc_collect`, and marked `P0-T01R` done in `TODO-1.md` / `TODO.md` with a completion record.
- Validation decision: `git diff --check` passed; no code changed in this invocation, so reused the previous green full validation recorded by `P0-T01` and skipped rerunning expensive suites per the task instructions.
- Preserved the file's pre-existing historical notes after the current invocation section.
- Committed task changes as `46a5f0d4 [P0-T01R] Review GC pacing baseline`; post-commit status was clean before this final memory update.

## Preserved Previous Notes

The following content was already present before this invocation and is retained for continuity.

# 当前执行计划

## 约束与依据

- 以 `TODO.md` 为唯一任务顺序与完成状态来源。
- 只处理第一个标题未带 `[DONE]` 的任务，完成后提交并停止。
- 不做开放式历史问题扫查；只有阻塞当前任务或验证失败且未排期的问题才纳入处理。
- 若遇到规范不匹配、缺失能力或不可绕开的阻塞，先在 `TODO.md` 插入最小必要前置任务，提交后停止。
- `PLAN.md` 只在阶段级计划、依赖或完成标准改变时更新。

## 步骤计划

1. 读取 `TODO.md`，定位第一个未完成任务，并记录任务 ID、要求、依赖和验证要求。
2. 检查最近提交信息，仅确认是否存在与该任务直接相关的未完成事项。
3. 根据任务内容读取最小必要的相关代码、测试、规范或文档。
4. 若任务本身可完成，实施最小正确修改；若被具体缺口阻塞，则更新 `TODO.md` 插入前置任务并停止。
5. 针对修改运行格式化、lint、相关测试；需要时按要求运行完整测试和 fixture 套件。
6. 若发现未排期的测试或 fixture 失败，修复或在 `TODO.md` 中加入最小必要任务，不把当前任务标为完成。
7. 完成后在 `TODO.md` 给当前任务标题加 `[DONE]`，更新完成记录和验证记录。
8. 复查变更，提交所有与本次任务相关的文件。
9. 停止，不处理下一项任务。

## 当前状态

- 已读取 `TODO.md` 与 `TODO-1.md`。
- 当前第一个未完成任务：`P0-T01`，内容是核对并冻结 pacing 当前行为基线。
- 本任务只更新记录，不改变运行期行为。
- 已检查最近提交：最新提交为计划更新，未显式提到与 `P0-T01` 直接相关的未完成实现问题。
- 已核对任务指定的 runtime/GC 文件与 `getenv` 命中。
- 已更新 `TODO-1.md` 的 `P0-T01` 完成记录、`TODO.md` 索引状态，并同步修正 `GC_PACING.md` 中的当前行为摘要。
- 已通过 `cargo fmt`、`cargo clippy --all-targets -- -D warnings`、`cargo test --all --all-targets` 与 `python3 tools/run_fixtures.py`。
- 已将验证结果补入 `TODO-1.md` 完成记录。
- 下一步复查 git diff/status 并提交本任务相关变更，然后停止。

## 历史记录：P6-T03R

- 已读取 `TODO.md`；第一个未完成任务是 `P6-T03R`。
- 已检查最近提交：`fdcdc47e [P6-T03] Audit old surface regressions`，它正是本 review 的输入，未发现额外未完成前置事项。
- 已抽样复核旧 surface 命中：实际 `perform` keyword 只出现在 removal diagnostic / negative fixture，handler `with` 只出现在 removal negative，tuple `._0` / with-path `_0` 只出现在旧语法 negative，f-string `{...}` 命中为 literal-brace 覆盖或 `${...}` 内部表达式，`@Inline` / `AnyRef` / `AnyValue` 不在 sysroot/compiler 中作为 active positive surface 出现。
- 已确认 sysroot operator-like declarations 未发现缺少 `operator` 的正向 API；active spec / split spec 中剩余 `perform` 为普通动词或 removal 说明，不是旧 prefix 正例。
- 已验证 overload/codegen baseline：`overload_concrete_bug.scoop`、`overload_arity_bug.scoop`、`overload_gvc_ok.scoop` 均通过。
- 已验证 overload diagnostics：no-applicable、ambiguity、conflicting overload、generic shape mismatch、vararg overlap、infer ambiguity targeted fixtures 均通过；`python3 tools/audit_user_visible_failure_policy.py` 通过。
- 已验证 `.cone` / `scoopir` export：`public_api_filter.scoop` 确认 `.scoopir` 只导出显式 `public`；`source_path_dependency_public_call`、`source_path_dependency_private_hidden`、`source_path_dependency_internal_hidden` 确认 public 可见且 private/internal 保持隐藏。
- 已通过完整验证：`cargo fmt`；`cargo clippy --all-targets -- -D warnings`；targeted overload / cone fixtures；`cargo test --all --all-targets`；`python3 tools/spec_fixtures.py check`（`spec fixtures: ok (1)`）；`python3 tools/run_fixtures.py`（`fixtures: ok (1607)`）；`git diff --check`。
- 已更新 `TODO.md` 和 `TODO-5.md`，将 `P6-T03R` 标记为 `[DONE]` 并填写完成记录；`PLAN.md` 阶段级 sequencing 未变化，无需更新。
- 提交前检查发现未跟踪文件 `REFLECTION.md`，该文件不是本任务产生的改动，不纳入本次提交。

## 历史记录：P6-T04

### 范围

- 目标：依据 `TODO.md` 的顺序完成第一个标题未带 `[DONE]` 的任务，然后停止。
- 当前任务：`P6-T04：全量格式化、测试矩阵与最终收口记录`。
- 约束：执行完整验证矩阵；不把 `SPEC_FIX.md` / `OVERLOAD_RESOLUTION.md` 范围内事项静默延期；完成后只留下 `P6-T04R` 作为下一个 review 任务。

### 步骤

1. 读取 `TODO.md`，识别第一个未完成任务及其验证要求。
2. 检查最近提交是否提到与当前任务直接相关的未完成事项。
3. 阅读 `TODO-5.md` 中 `P6-T04` 的任务体和依赖。
4. 按要求运行 `cargo fmt`、`cargo clippy --all-targets -- -D warnings`、`cargo test --all --all-targets`、`python3 tools/spec_fixtures.py check`、`python3 tools/run_fixtures.py`。
5. 额外运行 user-visible failure policy audit 和 `git diff --check`，用于最终诊断和 whitespace 收口记录。
6. 如果出现未调度失败，修复或在 `TODO.md` 中插入最小必要前置任务后停止。
7. 验证通过后，将 `P6-T04` 在 `TODO.md` 和 `TODO-5.md` 标记为 `[DONE]` 并填写完成记录。
8. 检查 git 状态、差异和最近提交，只提交本任务相关文件，然后停止。

### 进度

- 已读取 `TODO.md`；第一个未完成任务是 `P6-T04`。
- 已检查最近提交：`2a8410d4 [P6-T03R] Review old surface audit`，未发现与当前任务直接相关的未完成 blocker。
- 已通过最终验证矩阵：`cargo fmt`；`cargo clippy --all-targets -- -D warnings`；`cargo test --all --all-targets`；`python3 tools/spec_fixtures.py check`（`spec fixtures: ok (1)`）；`python3 tools/run_fixtures.py`（`fixtures: ok (1607)`）；`python3 tools/audit_user_visible_failure_policy.py`（`user-visible failure policy audit: ok`）；`git diff --check`。
- 已更新 `TODO.md` 和 `TODO-5.md`，将 `P6-T04` 标记为 `[DONE]` 并填写完成记录；`PLAN.md` 阶段级 sequencing 未变化，无需更新。
- 提交前检查发现未跟踪文件 `REFLECTION.md`，该文件不是本任务产生的改动，不纳入本次提交。

## 历史记录：P6-T04R

### 范围

- 目标：review P6-T04 最终收口质量，确认本轮计划完整闭合且没有未完成项被静默延期。
- 参考：P6-T04 完成记录、`PLAN.md` §6、`SPEC_FIX.md` summary table、`OVERLOAD_RESOLUTION.md` §12。
- 约束：必须指出具体 evidence；发现阻塞问题时直接修复或退回任务，不得签字式标记完成。

### 步骤

1. 读取 `TODO.md`，识别第一个未完成任务。
2. 检查最近提交是否提到与当前 review 直接相关的未完成事项。
3. 阅读 `TODO-5.md` 中 P6-T04 与 P6-T04R 的任务体、完成记录和验证要求。
4. 对照 `SPEC_FIX.md` summary table 与 active spec/compiler/sysroot/fixture evidence 复核 A1-D1 闭合。
5. 对照 `OVERLOAD_RESOLUTION.md` §12 与 diagnostics / overload regression evidence 复核规则落地。
6. 复核运行完整验证矩阵：`cargo fmt`、`cargo clippy --all-targets -- -D warnings`、`cargo test --all --all-targets`、`python3 tools/spec_fixtures.py check`、`python3 tools/run_fixtures.py`。
7. 运行 review 辅助检查：user-visible failure policy audit、TODO root/package consistency、removed-surface spot checks、`git diff --check`。
8. 若无 blocker，将 `P6-T04R` 在 `TODO.md` 和 `TODO-5.md` 标记为 `[DONE]` 并填写 evidence-based 完成记录；`PLAN.md` 仅在阶段级计划变化时更新。
9. 提交本任务相关文件；所有任务完成后创建 `v0.1.0` 标签。

### 进度

- 已确认 `P6-T04R` 是编辑前唯一未完成任务，最近提交 `4cf527f8 [P6-T04] Record final validation matrix` 正是本 review 输入，未发现额外未完成 blocker。
- 已复核 SPEC_FIX 与 overload-resolution closure evidence、TODO root/package consistency、active removed-surface checks、user-visible failure audit 和完整验证矩阵。
- 已通过验证：`cargo fmt`；`cargo clippy --all-targets -- -D warnings`；`cargo test --all --all-targets`；`python3 tools/spec_fixtures.py check`（`spec fixtures: ok (1)`）；`python3 tools/run_fixtures.py`（`fixtures: ok (1607)`）；`python3 tools/audit_user_visible_failure_policy.py`；`git diff --check`。
- 已更新 `TODO.md` 和 `TODO-5.md`，将 `P6-T04R` 标记为 `[DONE]` 并填写完成记录；同步修正 root index 顶部状态为所有任务已完成；`PLAN.md` 阶段级 sequencing 未变化，无需更新。
