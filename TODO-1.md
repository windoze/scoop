# TODO-1：Remove current comptime

> 生成时间：2026-05-21
> 计划基线：[`PLAN.md`](./PLAN.md) §4/P0
> 设计基线：[`PIPELINE_REFACTOR.md`](./PIPELINE_REFACTOR.md)
> 索引：[`TODO.md`](./TODO.md)
> 顺序约束：严格按本文件任务顺序推进；每个实现任务后必须执行紧随其后的 review 任务。
> 本包目标：删除现有 Scoop `comptime` / `const` surface、裁剪路径、const evaluator、runtime comptime plan 和跨阶段兼容特判，为后续 stage/fact crate 边界重建清空前置条件。

## 全局约束

- 本包只处理 `PLAN.md` 的 P0，不提前重构 HIR/MIR/effect/LIR/codegen stage output。
- 不保留旧 comptime surface 的专门 reject 逻辑；删除 surface 后，相关源码应自然落入普通 parse/resolve/typecheck 失败路径。
- 不把旧 comptime 行为迁移成新 helper、feature flag、环境变量或兼容模式。
- 不删除 Rust 语言自身需要的 `const`、`const fn`、`static` 或 LLVM 常量初始化 helper；只删除 Scoop 语言层面的 `const fun`、`const val`、`comptime if/for`、package-level `comptime if` 和对应 evaluator。
- 如果某个 `const` 命中只表示普通 global data initializer、Rust 常量、测试常量或 LLVM target helper，必须保留并在完成记录中说明判定。
- 每个任务完成后，在该任务的“完成记录”下写明改动范围、核心决策、验证命令和残余风险。

## [DONE] P0-T01：删除 package-level `comptime if` item 与裁剪路径

- 参考：
  - `PLAN.md` §1.6、§4/P0
  - `PIPELINE_REFACTOR.md` 中关于移除现有 comptime surface 的约束
- 目标：
  - 删除顶层 item 级 `comptime if` 的 AST/parser surface；
  - 删除所有 package-level comptime trimming 入口和调用点；
  - 确保后续 index/resolve/typecheck/sysroot/cone export 不再依赖“先裁剪未选分支”。
- 必须检查和修改的主要位置：
  - `crates/scoopc/src/ast/mod.rs`：`Item::ComptimeIf`、`ComptimeIfItem`、`ComptimeIfItemElse`。
  - `crates/scoopc/src/parser/file.rs`：`parse_comptime_if_item*`、顶层 `Keyword::Comptime` item 分支。
  - `crates/scoopc/src/parser/tests.rs`：package-level comptime parse 用例。
  - `crates/scoopc/src/comptime/interpreter.rs`：`trim_package_level_comptime_ifs*` 系列函数。
  - `crates/scoopc/src/session/mod.rs`：`trim_package_level_comptime_ifs_in_compilation_unit(...)` 调用和对应测试。
  - `crates/scoopc/src/sysroot/mod.rs`：sysroot AST 裁剪调用，以及跳过 `ast::Item::ComptimeIf` 的扫描逻辑。
  - `crates/scoopc/src/frontend.rs`：`run_frontend(...)` 中 cone-level compilation unit 裁剪调用。
  - `crates/scoopc/src/hir/lower/main/entry.rs`：HIR lowering 前的裁剪调用。
  - `crates/scoopc/src/mir/materialize/inputs.rs`：MIR materialize input 构造中的裁剪调用。
  - `crates/scoopc/src/effect_facts/builder.rs`：effect facts builder 中的裁剪调用和 `Item::ComptimeIf` 跳过逻辑。
  - `crates/scoopc/src/cone/visibility.rs`、`crates/scoopc/src/cone/pre_specialize.rs`、`crates/scoopc/src/cone/scoopir/export.rs`：cone 分析/export 前的裁剪调用。
- 必须实现的内容：
  1. 删除顶层 `Item::ComptimeIf` AST 变体及其专用结构。
  2. 删除 parser 对 package-level `comptime if` 的专门解析入口；顶层 `comptime` 不再是合法 item 起始。
  3. 删除所有 `trim_package_level_comptime_ifs*` 调用点；不要改成“先报一个专门不支持 comptime”的新路径。
  4. 清理当前为“裁剪后不会看到 `Item::ComptimeIf`”而保留的 `match` 空分支。
  5. 更新或删除 package-level comptime fixtures，至少覆盖：
     - `tests/fixtures/parse/package_level_comptime_if_basic.scoop`
     - `tests/fixtures/parse/package_level_comptime_if_stmt_in_block_fail.scoop`
     - `tests/fixtures/resolve/package_level_comptime_if_selects_branch_ok.scoop`
     - `tests/fixtures/resolve/package_level_comptime_if_untrimmed_is_error.scoop`
     - `tests/fixtures/resolve/package_level_comptime_if_cond_not_bool_is_error.scoop`
     - `tests/fixtures/typecheck/package_level_comptime_if_unselected_branch_type_error_ok.scoop`
     - `tests/fixtures/scoopir/package_level_comptime_if_public_api_trimmed.scoop`
- 禁止事项：
  - 禁止留下 `Item::ComptimeIf(_) => {}` 这种“静默忽略”分支。
  - 禁止把 package-level `comptime if` 改成新的显式 unsupported diagnostic。
  - 禁止保留 trimming 函数只为了测试或 sysroot overlay 兼容。
- 验证：
  1. `cargo fmt`
  2. `cargo test -p scoopc --no-default-features parser`
  3. `cargo test -p scoopc --no-default-features session`
  4. `cargo run -p scoop -- test`
  5. 搜索 `Item::ComptimeIf|ComptimeIfItem|trim_package_level_comptime`，活跃源码中不得再有命中。
- 完成条件：
  - package-level `comptime if` 不再是 AST item；
  - pipeline/sysroot/cone/HIR/MIR/effect facts 路径不再执行 package-level comptime 裁剪；
  - 旧 fixtures 不再验证“未选分支被裁掉”。
- 依赖：无
- 完成记录：
  - 改动范围：删除了 `Item::ComptimeIf`、`ItemBlock`、`ComptimeIfItem*` AST surface，删除 parser 的 package-level `comptime if` item 入口，并移除了 `trim_package_level_comptime_ifs*` 实现、导出和所有 pipeline/sysroot/cone/HIR/MIR/effect/fixture 调用点。
  - 核心决策：顶层 `comptime` 不再作为合法 item 起始，也没有新增专门 unsupported diagnostic；旧 surface 现在通过普通 parse expected 错误暴露。statement-level `comptime if/for` 与 Scoop `const` surface 仍按 P0-T02/P0-T03 保留。
  - Fixtures：旧 package-level trimming pass fixtures 已改为普通 parse-fail fixtures；专门验证跨文件/跨 cone trimming 的旧目录级 fixtures 已删除；混合 HIR/MIR fixtures 改为直接声明原被选中分支并更新 golden。
  - 额外修正：为保证 P0-T01 指定的 `--no-default-features` 验证可运行，补齐了 LLVM-gated HIR tests/API re-export gating，并清理了本次删除裁剪调用后产生的 unused warnings。
  - 验证命令：`cargo fmt`；`cargo test -p scoopc --no-default-features parser`；`cargo test -p scoopc --no-default-features session`；`cargo run -p scoop -- test`；`cargo clippy --all-targets -- -D warnings`；搜索 `Item::ComptimeIf|ComptimeIfItem|trim_package_level_comptime` 于 `crates/` 和 `tests/` 均无命中。
  - 残余风险：归档文档和当前 TODO 任务说明中仍保留历史 package-level comptime 文字；这是历史/计划记录，不是活跃实现。顶层 `comptime if` 仍作为失败 fixture 源码出现，用于断言普通 parse 失败路径。

## [TODO] P0-T01R：Review package-level comptime 删除结果

- 参考：P0-T01。
- 重点：
  - 是否还有任何顶层 `comptime if` AST/parser surface；
  - 是否还有任何 trimming 函数或调用点；
  - 是否存在静默忽略 `Item::ComptimeIf` 的兼容分支；
  - package-level comptime fixtures 是否已经改为普通 parse/resolve/typecheck 失败或被删除。
- 必须复查的文件/位置：
  - `crates/scoopc/src/ast/mod.rs`
  - `crates/scoopc/src/parser/file.rs`
  - `crates/scoopc/src/session/mod.rs`
  - `crates/scoopc/src/sysroot/mod.rs`
  - `crates/scoopc/src/frontend.rs`
  - `crates/scoopc/src/hir/lower/main/entry.rs`
  - `crates/scoopc/src/mir/materialize/inputs.rs`
  - `crates/scoopc/src/effect_facts/builder.rs`
  - `crates/scoopc/src/cone/visibility.rs`
  - `crates/scoopc/src/cone/pre_specialize.rs`
  - `crates/scoopc/src/cone/scoopir/export.rs`
- 验证：
  - 重新运行 P0-T01 的所有验证；
  - 额外搜索 `package-level comptime|ComptimeIfItem|trim_package_level_comptime|Item::ComptimeIf`。
- 完成条件：
  - review 结论明确写出：package-level comptime 裁剪路径已经物理消失，且没有用新 reject/兼容分支替代。
- 依赖：P0-T01
- 完成记录：
  - 待填写。

## [TODO] P0-T02：删除 statement-level `comptime if/for` 与 runtime comptime plan

- 参考：
  - `PLAN.md` §1.6、§4/P0
- 目标：
  - 删除语句/表达式块内的 `comptime if`、`comptime for` surface；
  - 删除 runtime comptime plan 及其 AST walker；
  - 清理 HIR/MIR lowering 中为 comptime splice/control-flow 保留的占位逻辑。
- 必须检查和修改的主要位置：
  - `crates/scoopc/src/ast/mod.rs`：`ComptimeIf`、`ComptimeIfElse`、`ComptimeFor`、`StmtKind::ComptimeIf`、`StmtKind::ComptimeFor`。
  - `crates/scoopc/src/parser/stmt.rs`：`parse_comptime_stmt(...)`、`parse_comptime_if_after_comptime(...)`、`parse_comptime_for_after_comptime(...)`。
  - `crates/scoopc/src/parser/tests.rs`：`parse_comptime_syntax_and_splice` 及相关断言。
  - `crates/scoopc/src/comptime/interpreter.rs`：`RuntimeComptimePlan`、`plan_comptime_if(...)`、`plan_comptime_for(...)`、runtime comptime AST walker。
  - `crates/scoopc/src/typecheck/lower.rs`：对 `StmtKind::ComptimeIf` / `StmtKind::ComptimeFor` / `Item::ComptimeIf` 的跳过或 lowering 分支。
  - `crates/scoopc/src/mir/materialize/templates.rs`：跳过 `ComptimeIf` 或 comptime splice 的路径。
  - `crates/scoopc/src/hir/lower/**`：HIR lowering 前后专门处理 comptime control-flow 的逻辑。
- 必须实现的内容：
  1. 删除 statement-level comptime AST 结构和 `StmtKind` 变体。
  2. 删除 parser 中 `comptime if/for` 的语句解析入口；`comptime` 不再是语句起始关键字。
  3. 删除 runtime comptime plan 数据结构和 walker；若 `crates/scoopc/src/comptime/` 中仍有 const eval 内容，留给 P0-T03 处理。
  4. 清理 HIR/MIR/typecheck 中“comptime statement/splice 被前置阶段处理掉”的空分支或占位节点。
  5. 更新或删除相关 fixtures，至少覆盖：
     - `tests/fixtures/parse/comptime_syntax_basic.scoop`
     - `tests/fixtures/comptime/comptime_block_and_if_basic.scoop`
     - `tests/fixtures/comptime/comptime_for_range_and_array_basic.scoop`
     - `tests/fixtures/comptime/comptime_if_condition_not_bool_is_error.scoop`
     - `tests/fixtures/hir/lowered_comptime_control_flow.scoop`
     - `tests/fixtures/mir_lowered/comptime_splice_class_with_update.scoop`
     - `tests/fixtures/umb_fix/B-24-reflection-comptime/pos_comptime_if_for.scoop`
     - `tests/fixtures/umb_fix/B-24-reflection-comptime/neg_comptime_if_for_gate.scoop`
- 禁止事项：
  - 禁止新增 `UnsupportedComptime`、`ComptimeRemoved` 之类专门错误类型。
  - 禁止保留 runtime comptime plan 作为 dead code。
  - 禁止把 comptime control-flow lowering 改挂到 HIR/MIR 的普通优化路径。
- 验证：
  1. `cargo fmt`
  2. `cargo test -p scoopc --no-default-features parser`
  3. `cargo test -p scoopc --no-default-features hir`
  4. `cargo test -p scoopc --no-default-features mir`
  5. 搜索 `StmtKind::Comptime|ComptimeFor|RuntimeComptimePlan|parse_comptime_stmt`，活跃源码中不得再有命中。
- 完成条件：
  - block/statement 位置不再存在 `comptime if/for` AST 或 lowering 概念；
  - runtime comptime plan 已删除；
  - 下游阶段不再假定 comptime statement 已被上游 splice。
- 依赖：P0-T01R
- 完成记录：
  - 待填写。

## [TODO] P0-T02R：Review statement-level comptime 删除结果

- 参考：P0-T02。
- 重点：
  - `comptime if/for` 是否已从 AST、parser、typecheck、HIR lowering、MIR materialization 中消失；
  - runtime comptime plan 是否已删除而不是被闲置；
  - fixtures 是否不再把 comptime splice/control-flow 当作可用 surface。
- 必须复查的文件/位置：
  - `crates/scoopc/src/ast/mod.rs`
  - `crates/scoopc/src/parser/stmt.rs`
  - `crates/scoopc/src/parser/tests.rs`
  - `crates/scoopc/src/comptime/interpreter.rs`
  - `crates/scoopc/src/typecheck/lower.rs`
  - `crates/scoopc/src/mir/materialize/templates.rs`
  - `crates/scoopc/src/hir/lower/`
- 验证：
  - 重新运行 P0-T02 的所有验证；
  - 额外搜索 `comptime for|comptime if|RuntimeComptimePlan|ComptimeFor|StmtKind::Comptime`。
- 完成条件：
  - review 结论明确写出：statement-level comptime surface 和 runtime plan 已全部移除，且没有迁移成其它阶段特判。
- 依赖：P0-T02
- 完成记录：
  - 待填写。

## [TODO] P0-T03：删除 Scoop `const` surface、const evaluator 与跨阶段 const hooks

- 参考：
  - `PLAN.md` §1.6、§4/P0
- 目标：
  - 删除 Scoop 语言层面的 `const fun` / `const val` / `const` modifier；
  - 删除 const evaluator 和 `ConstEvalError` 传播路径；
  - 清理 resolve/typecheck/HIR/MIR/codegen 中只服务旧 const/comptime 的字段、分支和 intrinsic 识别。
- 必须检查和修改的主要位置：
  - `crates/scoopc/src/syntax/token.rs`：`Keyword::Const`。
  - `crates/scoopc/src/syntax/lexer.rs`：`"const" => Keyword::Const` 映射。
  - `crates/scoopc/src/parser/decls.rs`：`Modifier::Const` 解析。
  - `crates/scoopc/src/parser/cursor.rs`：`Keyword::Const` 展示和 recover 集合。
  - `crates/scoopc/src/ast/mod.rs`：`Modifier::Const`。
  - `crates/scoopc/src/resolve/mod.rs`：`FunSymbol::is_const` 及 `fun.modifiers.contains(&ast::Modifier::Const)`。
  - `crates/scoopc/src/session/mod.rs`：`CompileError::Comptime(ConstEvalError)`。
  - `crates/scoopc/src/hir/lower/types.rs`：从 `ConstEvalError` 派生的错误变体。
  - `crates/scoopc/src/comptime/{mod.rs,eval.rs,interpreter.rs,value.rs,tests.rs}`：const evaluator 模块。
  - `crates/scoopc/src/lib.rs`：`pub mod comptime`。
  - `crates/scoopc/src/intrinsics.rs`、`crates/scoopc/src/typecheck/**`、`crates/scoopc/src/hir/**`、`crates/scoopc/src/mir/**`、`crates/scoopc/src/llvm/**`：搜索 `is_const`、`ConstEval`、`const fun`、`const val`、`Modifier::Const` 后确认是否只服务旧 surface。
  - `sysroot/core.scoop`：删除或改写 `const fun` reflection intrinsic surface 与 `const val` 常量 surface。
  - `tests/fixtures/**`：删除或改写显式使用 `const fun` / `const val` 的 fixtures，尤其是 `tests/fixtures/umb_fix/B-24-reflection-comptime/` 和 sysroot overlay fixtures。
  - `crates/scoopc/src/pipeline_user_visible_failure_policy.rs`：更新仍引用旧 const/comptime diagnostic 或 codegen sentinel 的审计清单。
- 必须实现的内容：
  1. 删除 lexer/parser/AST 中的 Scoop `const` modifier surface。
  2. 删除 resolver/typecheck 中 `const fun` 可调用性、const-only context、const value binding 等规则。
  3. 删除 `crates/scoopc/src/comptime/` 模块；如果个别纯 literal parser helper 仍有通用价值，先迁到对应中立模块，再删除 comptime 命名空间。
  4. 清理 HIR/MIR/codegen 里只为旧 `const val` 或 const evaluator 提供的 lowering/codegen 支持。
  5. 改写 sysroot 和 fixture surface，使标准库不再声明 `const fun` / `const val`。
  6. 搜索并分类保留 Rust/LLVM 层面的 `const` 命中，在完成记录中列出“保留原因”。
- 禁止事项：
  - 禁止删除 Rust `const fn`、Rust 常量、LLVM constant initializer helper 这类非 Scoop surface。
  - 禁止把 `const fun` 改名为普通 intrinsic 后继续保留旧 compile-time evaluation 语义。
  - 禁止留下 `is_const` 字段但永远为 false。
- 验证：
  1. `cargo fmt`
  2. `cargo test -p scoopc --no-default-features parser`
  3. `cargo test -p scoopc --no-default-features resolve`
  4. `cargo test -p scoopc --no-default-features typecheck`
  5. `cargo test -p scoopc --no-default-features hir`
  6. `cargo test -p scoopc --no-default-features mir`
  7. 搜索 `pub mod comptime|ConstEval|Modifier::Const|Keyword::Const|is_const|const fun|const val`，活跃源码中不得再有旧 Scoop surface 命中；允许的 Rust/LLVM `const` 命中必须在完成记录中解释。
- 完成条件：
  - Scoop `const` surface 无法再被 lexer/parser/AST/resolve/typecheck 识别为语言特性；
  - const evaluator 模块已删除；
  - HIR/MIR/codegen 不再通过旧 const-eval 结果获取语义。
- 依赖：P0-T02R
- 完成记录：
  - 待填写。

## [TODO] P0-T03R：Review `const` surface 与 evaluator 删除结果

- 参考：P0-T03。
- 重点：
  - `const fun` / `const val` 是否从 lexer/parser/AST/resolve/typecheck/sysroot/fixtures 中消失；
  - `crates/scoopc/src/comptime/` 是否已删除，且没有被其它名字复活；
  - 保留的 `const` 命中是否都属于 Rust/LLVM/测试常量，而不是 Scoop 旧 surface。
- 必须复查的文件/位置：
  - `crates/scoopc/src/syntax/token.rs`
  - `crates/scoopc/src/syntax/lexer.rs`
  - `crates/scoopc/src/parser/decls.rs`
  - `crates/scoopc/src/ast/mod.rs`
  - `crates/scoopc/src/resolve/mod.rs`
  - `crates/scoopc/src/session/mod.rs`
  - `crates/scoopc/src/hir/lower/types.rs`
  - `crates/scoopc/src/lib.rs`
  - `sysroot/core.scoop`
  - `tests/fixtures/umb_fix/B-24-reflection-comptime/`
- 验证：
  - 重新运行 P0-T03 的所有验证；
  - 额外搜索 `comptime|ConstEval|const fun|const val|Modifier::Const|Keyword::Const|is_const`，逐项确认活跃命中是否允许。
- 完成条件：
  - review 结论明确写出：旧 Scoop `const` surface、const evaluator 和跨阶段 hooks 已删除，保留命中均非旧 surface。
- 依赖：P0-T03
- 完成记录：
  - 待填写。

## [TODO] P0-T04：P0 全仓清场与文档同步

- 参考：
  - `PLAN.md` §4/P0、§6
  - `PIPELINE_REFACTOR.md`
- 目标：
  - 对 P0 的全部删除结果做全仓一致性清场；
  - 同步文档、fixtures 和审计策略；
  - 为 `TODO-2.md` 的 base crates + cone unit model 留出干净起点。
- 必须实现的内容：
  1. 全仓搜索并分类处理旧 comptime/const surface：
     - `comptime`
     - `Comptime`
     - `ConstEval`
     - `const fun`
     - `const val`
     - `Modifier::Const`
     - `Keyword::Const`
     - `is_const`
  2. 更新 `PLAN.md` 顶部当前状态或完成记录；如果执行中改变了 P0 目标，先更新 `PIPELINE_REFACTOR.md`。
  3. 更新 `PIPELINE-CLEANUP.md` 或相关审计文档中已经解决的 P0 问题标记；如果该文档只作为历史审计，不改内容，则在完成记录中说明。
  4. 清理 `tests/fixtures/comptime/` 目录；若目录变空，删除目录或在 fixture runner 允许的情况下保留说明文件。
  5. 检查 `sysroot/`、`stdlib/`、`tests/fixtures/build/**.sysroot/` 中旧 surface 是否已经同步。
  6. 确认 `PIPELINE_REFACTOR.md` 的主线阶段图不再依赖任何 comptime 特例。
- 禁止事项：
  - 禁止为了让旧 fixtures 继续通过而恢复旧 surface。
  - 禁止把 P0 未清完的问题留给 HIR/MIR/LIR 包处理。
- 验证：
  1. `cargo fmt`
  2. `cargo test --all --all-targets --no-default-features`
  3. `cargo run -p scoop -- test`
  4. `cargo run -p scoop_tools -- spec-fixtures check`
  5. 全仓搜索旧 surface；完成记录中必须附允许命中的分类摘要。
- 完成条件：
  - 现有 comptime/const surface 和实现已经从正式 pipeline 清除；
  - 没有为了兼容旧 comptime 保留的专门逻辑；
  - 后续 `TODO-2.md` 可以在不考虑旧 comptime 特例的前提下建立基础 crate 和 cone compilation unit model。
- 依赖：P0-T03R
- 完成记录：
  - 待填写。

## [TODO] P0-T04R：Review P0 全包完成度

- 参考：P0-T01 到 P0-T04 的所有完成记录。
- 重点：
  - P0 是否真的满足 `PLAN.md` §4/P0 的完成标准；
  - 是否有任何旧 comptime/const surface、evaluator、裁剪路径或跨阶段兼容特判残留；
  - 文档、fixtures、sysroot 和审计策略是否与实现一致；
  - 是否可以安全进入 `TODO-2.md`。
- 必须复查的范围：
  - `crates/scoopc/src/`
  - `sysroot/`
  - `stdlib/`
  - `tests/fixtures/`
  - `PLAN.md`
  - `PIPELINE_REFACTOR.md`
  - `PIPELINE-CLEANUP.md`
- 验证：
  - 重新运行 P0-T04 的所有验证；
  - 对旧 surface 搜索结果做人工分类复核；
  - 抽查至少一个原 comptime fixture 和一个原 `const fun` sysroot/overlay 命中，确认不是被静默兼容。
- 完成条件：
  - review 结论明确写出：P0 完成，可以进入 `TODO-2.md`；或列出阻塞项并在本 review 内修复。
- 依赖：P0-T04
- 完成记录：
  - 待填写。
