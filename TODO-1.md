# TODO-1：P0-P1 基线冻结与 low-risk cleanup

> 索引：[`TODO.md`](./TODO.md)  
> 计划基线：[`PLAN.md`](./PLAN.md)  
> 覆盖阶段：P0-P1  
> 包目标：冻结旧 surface / overload bug 基线，关闭纯 spec drift，并删除 `@Inline` surface。

## P0：冻结当前偏离、建立迁移清单与最小回归矩阵

### [TODO] P0-T01：建立旧 surface / sysroot / fixture 迁移清单

- 参考：
  - [`PLAN.md`](./PLAN.md) §5 / P0
  - [`SPEC_FIX.md`](./SPEC_FIX.md) summary table
  - [`OVERLOAD_RESOLUTION.md`](./OVERLOAD_RESOLUTION.md) §1、§10
- 目标：
  - 在改语言行为前，固定所有旧 surface 命中、sysroot 依赖点、fixture 迁移点和 overload 已知 bug 样例。
  - 后续 P1-P6 agent 应能直接从本任务完成记录读取迁移清单，而不是重新全仓搜索。
- 必须检查的文件/位置：
  - `SCOOP_FULL_SPEC.md`
  - `docs/spec/language_spec-part*.md`
  - `sysroot/lib/scoop.core/src/core.scoop`
  - `sysroot/lib/scoop.unsafe/src/unsafe.scoop`
  - `tests/fixtures/**`
  - `crates/scoopc_ast/src/syntax/{token.rs,lexer.rs,string_literal.rs}`
  - `crates/scoopc_ast/src/parser/{expr.rs,decls.rs,stmt.rs,pattern.rs}`
  - `crates/scoopc_hir/src/typecheck/**`
  - `crates/scoopc_hir/src/hir/lower/expr/**`
  - `crates/scoopc_mir/src/mir/lower/**`
  - `crates/scoopc_cone/src/{scoopir/export.rs,visibility.rs}`
- 必须实现的内容：
  1. 在本条“完成记录”里列出旧 surface 命中摘要，至少覆盖：`perform`、`handle ... with`、tuple `._0` / `._1`、f-string `{...}` / `{{` / `}}`、`@Inline` / `annotation class Inline`、`AnyRef` / `AnyValue`、隐式 public sysroot/API declarations、operator-like functions lacking `operator`。
  2. 列出需要优先关注的 fixture 文件名或 glob，至少覆盖 `tests/fixtures/parse/*handle*`、`tests/fixtures/parse/*f_string*`、`tests/fixtures/parse/*with*`、`tests/fixtures/**/**/*perform*.scoop`、`tests/fixtures/**/**/*overload*.scoop`、`tests/fixtures/**/**/*vararg*.scoop`、`tests/fixtures/**/**/*inline*.scoop`、`tests/fixtures/**/**/*not_null*.scoop`、`tests/fixtures/**/**/*cast*.scoop`。
  3. 记录哪些旧语法 fixture 应机械迁移为新语法，哪些应保留为 negative fixture。
  4. 记录 overload 三个已知 bug 的最小代码样例：concrete overload `f(Int)` / `f(Bool)` lowering 不应串扰；arity overload `g(Int)` / `g(Int, Int)` 应各自 materialize callable version；generic + concrete 同名时 `h(Int)` 应按 specificity 胜出。
- 必须遵从的约束：
  - 本任务不改 compiler 语义，不新增会让全量 fixture 失败的 pass fixture。
  - 如果新增临时 inventory 文件，必须在本任务完成记录写明位置；否则直接把清单写入本条完成记录即可。
- 验证：
  1. `python3 tools/spec_fixtures.py check`
  2. `python3 tools/run_fixtures.py`
  3. 对完成记录中的 glob / 文件清单做人工抽样复核。
- 完成条件：
  - 后续任务能从本条完成记录直接知道旧 surface 和 fixture 迁移范围。
- 依赖：无
- 完成记录：
  - 改动范围：
  - 核心决策：
  - 验证结果：
  - 与 `PLAN.md` / 设计文档对应闭合：

### [TODO] P0-T01R：Review 旧 surface / sysroot / fixture 迁移清单

- 参考：
  - P0-T01 完成记录
  - [`PLAN.md`](./PLAN.md) §5 / P0
- 目标：
  - 独立复核 P0-T01 的 inventory 是否足以支撑后续任务，避免漏掉旧 surface 或关键 fixture。
- 必须检查的文件/位置：
  - P0-T01 完成记录中的所有文件和 glob
  - `TODO.md` 与 `TODO-1.md` 中 P0-T01 状态/完成记录
- 必须实现的内容：
  1. 抽样复查 P0-T01 列出的旧 surface 命中，确认分类准确。
  2. 反向检查至少一遍 spec、sysroot、fixtures、parser/typecheck/lowering 入口，确认没有明显漏项。
  3. 确认 P0-T01 没有引入会破坏全量 suite 的 positive fixture。
  4. 如发现漏项，直接补充清单或新增后续任务；若影响阶段边界，先更新 `PLAN.md`。
- 必须遵从的约束：
  - Review 不得只看文档格式；必须复核 inventory 可执行性。
  - 如果 P0-T01 未完成目标，阻塞 P0-T02。
- 验证：
  1. `python3 tools/spec_fixtures.py check`
  2. `python3 tools/run_fixtures.py`
- 完成条件：
  - 迁移清单完整、可执行，后续任务无需重新全仓搜索。
- 依赖：P0-T01
- 完成记录：
  - 改动范围：
  - 核心决策：
  - 验证结果：
  - 与 `PLAN.md` / 设计文档对应闭合：

### [TODO] P0-T02：建立 overload bug 与 diagnostics 基线样例

- 参考：
  - [`PLAN.md`](./PLAN.md) §5 / P0
  - [`OVERLOAD_RESOLUTION.md`](./OVERLOAD_RESOLUTION.md) §1、§10
- 目标：
  - 在正式改 overload resolution 前，固定当前必须修复的样例和诊断要求。
  - 避免 P5 agent 再重新从设计文档复制测试程序。
- 必须检查的文件/位置：
  - `tests/fixtures/typecheck/**`
  - `tests/fixtures/run-pass/**`
  - `tests/fixtures/build/**`
  - `tools/run_fixtures.py::{PHASE_DIRS,run_one_file,parse_expectations}`
  - `tools/audit_user_visible_failure_policy.py::FRONTEND_REJECT_FORBIDDEN_TERMS`
- 必须实现的内容：
  1. 确认 fixture runner 是否已有 expected-fail / negative 机制足以承载“当前未修复但目标明确”的 overload 样例。
  2. 若 runner 可表达当前失败，新增或更新 targeted fixtures，样例内容直接来自 `OVERLOAD_RESOLUTION.md`：`overload_concrete_bug`、`overload_arity_bug`、`overload_gvc_ok`。
  3. 若 runner 不适合承载当前失败，不要加入会破坏全量 suite 的 fixture；把完整样例与预期结果写入本条完成记录，并在 P5-T04 正式加入 run-pass。
  4. 记录 overload diagnostics 的 audit 要求：`ambiguous_overload`、`no_applicable_overload`、`conflicting_overloads` 必须列候选位置，且不含 `backend` / `LLVM` / `UnsupportedMainBody` / `codegen`。
- 必须遵从的约束：
  - 不得通过删除或弱化现有 overload fixture 来让 P0 通过。
  - 不得在 P0 修 overload 算法；这里只做基线样例和测试落点确认。
- 验证：
  1. `python3 tools/run_fixtures.py`
  2. 如果新增 fixture，定向运行对应 fixture 所在 phase。
  3. 如果未新增 fixture，在完成记录中写明原因和 P5 应新增的具体文件名建议。
- 完成条件：
  - P5 agent 不需要重新整理 overload bug reproduction；样例、预期和落点已固定。
- 依赖：P0-T01R
- 完成记录：
  - 改动范围：
  - 核心决策：
  - 验证结果：
  - 与 `PLAN.md` / 设计文档对应闭合：

### [TODO] P0-T02R：Review overload bug 与 diagnostics 基线

- 参考：
  - P0-T02 完成记录
  - [`OVERLOAD_RESOLUTION.md`](./OVERLOAD_RESOLUTION.md) §1、§10
- 目标：
  - 复核 overload baseline 是否准确覆盖已知 codegen bug 和诊断要求。
- 必须检查的文件/位置：
  - P0-T02 新增或记录的 fixture / 样例
  - `tests/fixtures/**/**/*overload*.scoop`
  - `tools/run_fixtures.py`
  - `tools/audit_user_visible_failure_policy.py`
- 必须实现的内容：
  1. 确认三个已知 bug 样例完整、预期明确、不会破坏当前全量 suite。
  2. 确认 P5-T04 可以直接使用 P0-T02 的样例作为 run-pass regression。
  3. 确认 diagnostics audit 要求足够明确，包含候选位置和 forbidden terms。
  4. 如基线样例无法执行，记录阻塞原因和替代落点。
- 必须遵从的约束：
  - 不得把当前 backend/codegen 报错接受为最终期望。
- 验证：
  1. `python3 tools/run_fixtures.py`
  2. 定向运行或人工复核 P0-T02 记录的样例。
- 完成条件：
  - overload bug baseline 可直接驱动 P5 实现和回归。
- 依赖：P0-T02
- 完成记录：
  - 改动范围：
  - 核心决策：
  - 验证结果：
  - 与 `PLAN.md` / 设计文档对应闭合：

## P1：纯 spec / low-risk cleanup 与 `@Inline` 删除

### [TODO] P1-T01：更新纯 spec 决议：`Nothing`、cone/package、value type `with`

- 参考：
  - [`PLAN.md`](./PLAN.md) §5 / P1
  - [`SPEC_FIX.md`](./SPEC_FIX.md) A1、A2、D1
- 目标：
  - 先关闭无需 compiler 语义改动的 spec drift。
- 必须修改的文件/位置：
  - `SCOOP_FULL_SPEC.md`
  - `docs/spec/language_spec-part*.md`
- 必须实现的内容：
  1. 在 type hierarchy 章节写入 `Nothing`：bottom type、subtype of every type、no inhabitants、only for non-returning functions、outside reference/value split。
  2. 在 cone/package 章节开头明确 cone 是 distribution / build unit，`.cone` 是 binary archive format，package 是 cone 内 source-level namespace。
  3. 在 value type / `with` 相关章节确认 value type immutable、struct 不引入 `var` field、`with` 保留为 update mechanism，C1 只会改 enum mismatch failure mode。
  4. 如 split spec 是手工维护，同步改 `docs/spec/language_spec-part*.md`；如有生成流程，在完成记录中写明实际同步方式。
- 必须遵从的约束：
  - 本任务不改 compiler 行为。
  - 不得在本任务顺手改 `perform`、handler `with`、f-string 等会影响 fixtures 的 spec code blocks；这些留给 P2/P6。
- 验证：
  1. `python3 tools/spec_fixtures.py check`
  2. 人工复核 `SCOOP_FULL_SPEC.md` 中 `Nothing`、cone/package、value type `with` 的新表述。
- 完成条件：
  - A1、A2、D1 在活跃 spec 中闭合。
- 依赖：P0-T02R
- 完成记录：
  - 改动范围：
  - 核心决策：
  - 验证结果：
  - 与 `PLAN.md` / 设计文档对应闭合：

### [TODO] P1-T01R：Review 纯 spec 决议更新

- 参考：
  - P1-T01 完成记录
  - [`SPEC_FIX.md`](./SPEC_FIX.md) A1、A2、D1
- 目标：
  - 复核 P1-T01 的 spec-only 更新是否准确、无无关 churn。
- 必须检查的文件/位置：
  - `SCOOP_FULL_SPEC.md`
  - `docs/spec/language_spec-part*.md`
  - P1-T01 diff
- 必须实现的内容：
  1. 确认 `Nothing` 描述符合 bottom type 决议。
  2. 确认 cone/package wording 没有与现有 source `package` 混淆。
  3. 确认 value type `with` 记录没有提前改变 C1 panic 语义或引入 struct `var`。
  4. 确认未修改 P2/P3 才应处理的 code blocks。
- 必须遵从的约束：
  - Review 发现 spec 决议偏离时必须先修正，再进入 P1-T02。
- 验证：
  1. `python3 tools/spec_fixtures.py check`
- 完成条件：
  - P1-T01 spec 更新准确且不影响 compiler/fixture baseline。
- 依赖：P1-T01
- 完成记录：
  - 改动范围：
  - 核心决策：
  - 验证结果：
  - 与 `PLAN.md` / 设计文档对应闭合：

### [TODO] P1-T02：删除 `@Inline` annotation surface

- 参考：
  - [`PLAN.md`](./PLAN.md) §5 / P1
  - [`SPEC_FIX.md`](./SPEC_FIX.md) B1
- 目标：
  - 删除无语义保证的 `@Inline`，避免后续 spec / sysroot / typecheck 继续维护 dead surface。
- 必须修改的文件/位置：
  - `SCOOP_FULL_SPEC.md`
  - `docs/spec/language_spec-part*.md`
  - `sysroot/lib/scoop.core/src/core.scoop`
  - `crates/scoopc_hir/src/typecheck/builtin_annotations.rs::{BuiltinAnnotationKind,builtin_annotation_kind}`
  - `crates/scoopc_hir/src/typecheck/annotations.rs::{check_inline_annotation_uses,check_builtin_inline_annotation}`
  - `crates/scoopc_ast/src/parser/decls.rs::parse_decl_prefix`
  - `tests/fixtures/**/**/*inline*.scoop`
- 必须实现的内容：
  1. 从 spec 的 annotation 章节和 built-in annotation 表删除 `@Inline`。
  2. 从 sysroot 删除 `annotation class Inline`。
  3. 从 `BuiltinAnnotationKind` 和 `builtin_annotation_kind` 删除 `Inline` 分支。
  4. 删除或改写 `check_inline_annotation_uses` / `check_builtin_inline_annotation` 的专用逻辑；如果函数只为 Inline 存在，应删除调用点。
  5. 更新 `@Inline` 正例 fixtures 为 negative fixture 或删除过时用例；保留 `inline` keyword removed diagnostic 的 parser/typecheck 覆盖。
- 必须遵从的约束：
  - 不得把 `inline` 改成新的 optimization hint。
  - 不得影响 compiler 自身 inliner heuristic。
- 验证：
  1. `python3 tools/spec_fixtures.py sync`
  2. `python3 tools/spec_fixtures.py check`
  3. `python3 tools/run_fixtures.py`
  4. targeted search：活跃 spec、sysroot、typecheck 不再出现 `annotation class Inline` / `BuiltinAnnotationKind::Inline`。
- 完成条件：
  - `@Inline` 不再是语言 surface；只有旧 keyword negative diagnostic 可保留。
- 依赖：P1-T01R
- 完成记录：
  - 改动范围：
  - 核心决策：
  - 验证结果：
  - 与 `PLAN.md` / 设计文档对应闭合：

### [TODO] P1-T02R：Review `@Inline` 删除结果

- 参考：
  - P1-T02 完成记录
  - [`SPEC_FIX.md`](./SPEC_FIX.md) B1
- 目标：
  - 确认 `@Inline` 已从 language surface、sysroot、typecheck、fixtures 和 spec 中真正删除。
- 必须检查的文件/位置：
  - `SCOOP_FULL_SPEC.md`
  - `docs/spec/language_spec-part*.md`
  - `sysroot/lib/scoop.core/src/core.scoop`
  - `crates/scoopc_hir/src/typecheck/builtin_annotations.rs`
  - `crates/scoopc_hir/src/typecheck/annotations.rs`
  - `tests/fixtures/**/**/*inline*.scoop`
- 必须实现的内容：
  1. 搜索并分类所有 `Inline` / `@Inline` / `annotation class Inline` 命中。
  2. 确认 `inline` keyword removed diagnostic 仍然覆盖旧 keyword，但不再表示 annotation surface。
  3. 确认 spec fixture sync 后无 stale generated fixture。
  4. 确认 inliner heuristic 或 optimization pass 没有被误删。
- 必须遵从的约束：
  - 不得以保留 hidden annotation alias 的方式通过 review。
- 验证：
  1. `python3 tools/spec_fixtures.py check`
  2. `python3 tools/run_fixtures.py`
- 完成条件：
  - B1 完整闭合，进入 P2 前无 `@Inline` active positive surface。
- 依赖：P1-T02
- 完成记录：
  - 改动范围：
  - 核心决策：
  - 验证结果：
  - 与 `PLAN.md` / 设计文档对应闭合：
