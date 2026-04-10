# 当前执行计划（外显推理摘要）

## 约束与目标

- 本次调用只完成 `TODO.md` 中第一个未完成任务，然后停止。
- 在处理任务前，先检查最新一次 Git 提交是否提到任何既有问题；若有，先修复这些问题。
- 任何计划变化、关键步骤完成、遇到阻塞时，都要更新本文件。
- 需要在实现后运行充分测试，并尽量满足无 warning 的质量要求，包括 `cargo clippy --all-targets -- -D warnings`。
- 完成后需要更新 `TODO.md`、`PLAN.md`，并提交 Git commit。

## 初始执行步骤

1. 检查最新一次 Git 提交的提交信息和变更内容，确认是否提到了待修复的既有问题。
2. 阅读 `TODO.md` 和 `PLAN.md`，识别第一个未完成任务，并判断任务是否需要拆分。
3. 如果任务过大：
   - 在 `PLAN.md` 中补充细化计划；
   - 在 `TODO.md` 中将该任务拆成更小子任务，并把第一个子任务作为本次执行目标。
4. 阅读与目标任务相关的代码、测试和文档，建立实现上下文。
5. 实现该任务，并在必要时补充或调整测试。
6. 运行相关验证：
   - 优先运行与改动直接相关的测试；
   - 再运行必要的全量检查，如 `cargo test --all`、`cargo clippy --all-targets -- -D warnings`（若耗时或受环境限制，会记录实际执行范围和结果）。
7. 更新文档与任务状态：
   - 在 `TODO.md` 中标记完成；
   - 在 `PLAN.md` 中反映当前状态与后续安排；
   - 若有必要，补充 `README.md` 或代码注释。
8. 检查工作区差异，确认仅包含本次相关修改。
9. 使用清晰的提交信息创建 Git commit。
10. 停止，不继续处理下一个任务。

## 当前未知项

- `T0149` 是否能在不新增大规模基础设施的前提下一次完成。
- Array 字面量在“无 expected type / return / assignment / nested array”这些路径上的具体失败点分别在哪一层。

## 记录规则

- 每完成一个关键阶段，就在本文件追加“进展更新”。
- 若计划发生变化，直接修改对应步骤，并说明原因。

## 进展更新

### 2026-04-11：任务定位完成

- 已检查最新提交 `453471c397e3934dbc6b4ea9d925d104c3bff7b4`。提交中提到的既有限制（顶层 `const val` 一般表达式 codegen）已在该提交内修复，当前没有额外遗留的“必须先修”的旧问题。
- 已确认 `TODO.md` 首个未完成任务为 `T0149 Array 字面量类型推断：移除无上下文限制`。
- 已评估复杂度：本轮先不拆分 `TODO.md` / `PLAN.md`。任务涉及 typecheck + HIR lowering + fixtures，但边界清晰，可在一轮内完成。

### 2026-04-11：现状复现与根因

- 已复现三类当前失败：
  1. `val xs = [1, 2, 3]`：typecheck 失败，报 `scoop::typecheck::unsupported_expr` / `array literal`。
  2. `return [1, 2, 3]`（函数返回 `Array<Int>`）：typecheck 已通过，但 HIR dump 显示 `ExprKind::Todo("array_lit")`，最终在 LLVM codegen 报 `unsupported_main_body: expression`。
  3. `xs = [1, 2, 3]`（`xs: Array<Int>`）：同样是 typecheck 通过、HIR lowering 未传播上下文，最终 codegen 失败。
- 已确认 HIR lowering 当前只在“显式 expected type hint”存在时处理 array literal；否则直接产出 `Todo("array_lit")`。
- 已确认仅靠 HIR 当前局部推断不足以完整覆盖任务目标：
  - `lower_ident_expr` / 普通 call lowering 大量表达式类型仍是 `Any`；
  - 因此若只用 HIR 自己猜元素类型，`[a]`、`[foo()]`、嵌套 array 等合法场景仍会残缺。

### 2026-04-11：更新后的实现方案

1. 在 AST 文件对象上增加一个**不影响 Debug/fixtures 输出**的表达式类型 side table。
2. 在 typecheck 阶段记录每个表达式的最终 `TypeId` 到该 side table。
3. 在 HIR lowering 阶段读取 side table：
   - 若 array literal 已有 typecheck 结果，则按该结果决定容器种类（`Array` / `MutableArray`）与元素类型；
   - 元素 lowering 递归传入期望类型，避免 inner array / `[a]` / `[foo()]` 再退化为 `Todo`。
4. 对未运行 typecheck 的 `dump-hir` 回退路径保留一个保守 heuristic：
   - 非空字面量可尝试从已知简单元素类型推断；
   - 空数组无上下文保持明确失败，而不是静默错误代码生成。
5. 新增/更新测试：
   - run-pass：无标注 Int/String/Char/Float 数组、函数参数、空数组带标注。
   - typecheck failure：`val x = []`、`val x = [1, "a"]`。
   - 如实现自然支持，再补一个 nested array 回归；否则给出明确诊断而非静默退化。

### 2026-04-11：接手续做计划

- 本轮只收尾 `T0149`，不推进 `TODO.md` 的后续任务。
- 当前判断：核心实现已完成，剩余工作集中在格式化、回归验证、文档状态更新与提交。
- 接下来按以下顺序执行：
  1. 运行 `cargo fmt --all`，统一改动格式。
  2. 运行与 `T0149` 直接相关的 fixture / 测试，确认 array literal 推断与错误诊断都稳定。
  3. 运行 `cargo test --all` 与 `cargo clippy --workspace --all-targets -- -D warnings`；若发现本任务引入的问题，立即修复。
  4. 若验证通过，更新 `TODO.md` 与 `PLAN.md`，把 `T0149` 标记为完成，并记录验证结果。
  5. 最后在本文件补充结果摘要，创建一次 Git commit，然后停止。
- 若验证阶段发现 `T0149` 无法在本轮完整闭环，则保持其为 TODO，按依赖顺序调整 `TODO.md` / `PLAN.md` 后提交并停止。

### 2026-04-11：验证与收尾完成

- `cargo fmt --all` 已完成。
- HIR fixture `tests/fixtures/hir/array_lit_lowering.hir` 已按新行为更新：数组字面量 block 的结果类型从旧的 `Any` 精确成真实 `Array<Int>` / `MutableArray<Int>`；同时补修了 `dump-hir` 回退路径的函数参数类型传播，避免 `MutableArray` 实参退化成普通 `Array`。
- run-pass 新增用例里发现了两处**既有** LLVM codegen 边界，不属于 `T0149` 本身：
  1. `chars.get(2).toInt()` 这类 rvalue 上直接调 Char extension 仍会失败；
  2. `nested.get(1).get(0)` 这类 rvalue nested-array 连续 `get` 仍会失败。
  为避免把无关缺口混入本任务，fixtures 已改为先绑定中间值到局部变量，再继续验证推断结果。
- 严格验证已全部通过：
  - `cargo test --all`
  - `cargo run -p scoop -- test`（`fixtures: ok (868)`）
  - `cargo clippy --workspace --all-targets --message-format short -- -D warnings`
- `clippy` 过程中还发现并顺手修掉一个现存 lint：`crates/scoopc/src/hir/lower/mod.rs` 的 `HirLowering::new(...)` 参数过多。现已收口为 `HirLoweringSetup`，不改变行为，仅用于恢复严格 clippy 基线。
- `TODO.md` / `PLAN.md` 现已更新为 `T0149` 完成状态。剩余操作只剩检查工作区、创建 `[T0149] ...` 提交并停止。
