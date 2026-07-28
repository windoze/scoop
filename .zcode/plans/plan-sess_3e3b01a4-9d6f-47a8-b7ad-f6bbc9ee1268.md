## 目标重述

现有 `scoop2_*` 前端（`scoop2_base`/`scoop2_syntax`/`scoop2_hir`/`scoop2c`）已是约 34k LOC 的重写实现：parser 基本完成（144 个 fixture 全部正确解析），typecheck 558 个 fixture 中 473 通过 / 85 真实失败。本次会话推进**全部方向分批**，优先消除目标硬性禁止的占位符 + 让 parse 阶段变绿，再开始 typecheck 修复。

采用**新缩进树格式**的 dump-hir（与 dump-ast 一致），新建 `tests/fixtures_ng/` 存放新 goldens，不污染旧目录。

## 现状调查结论（已完成）

1. **Parser 完全正确**：101 个 parse 失败 100% 是 stale golden（75 个 `.ast` 旧 Rust-Debug 格式 + 26 个旧错误码/措辞指令）。`run_fixtures.py` 无 `--accept`/regen 机制。
2. **`main.rs` 有禁止占位符**：`not_wired()`（infer/lower）+ `run_dump_hir()` 打印「尚未接通」。`no_placeholder.rs` 守卫只查 `todo!/unimplemented!/unreachable!`，没覆盖这些。
3. **typecheck 是 validate-only**：`run_typecheck -> ()`，所有类型数据（`TypeEnv`/`Index`/`Resolution`）函数返回时丢弃；`ExprChecker` 无 `NodeId→TypeId` 侧表（`expr.rs:3737` 有死代码占位 `_node_id_used` 说明一直计划做）；4 处重复私有 `fmt_type*` 渲染器需合并。
4. **`lib.rs` 文档引用了 `hir`/`completeness` 模块但未声明**——正是计划中待落地部分。

## 分批计划

### Batch 1（本次会话，优先级最高）

#### 1A. 接通 dump-hir（新缩进树格式，消除占位符）

- **新模块 `crates/scoop2_hir/src/hir/`**：
  - `mod.rs`：`pub struct TypedHir`（拥有 `Index`、`TypeStore`、声明级签名表、per-NodeId 表达式类型表 `expr_types: NodeIdTable<TypeId>`、per-file 解析信息）。
  - `render.rs`：稳定缩进树渲染器，复用 `dump-ast` 的 `Dumper` 约定（`Kind span key=value`，2 空格缩进）。每个 `Expr` 节点追加 `ty=<渲染后类型>`；声明头渲染签名/成员类型。**穷尽式 match**（新增 AST 变体编译报错，强制同步）。
- **公共类型渲染器**：在 `crates/scoop2_hir/src/ty.rs` 新增 `pub fn render_type(store, interner, id) -> String`，合并现有 4 处重复 `fmt_type*`（`mod.rs:1105/1118`、`release_hook.rs:343`、`extern_fn.rs:586`）为单一 pub 实现，覆盖 Option/Tuple/Function/Param/Nominal/标量/Nothing/StarProjection。
- **持久化 per-expr 类型**：把 `ExprChecker::walk_expr`（`expr.rs:876`，72 个调用点）重命名为 `walk_expr_inner`，新 `walk_expr` 包装：调 inner、得 `TypeId`、写 `self.expr_types.set(expr.id, ty)` 后返回。`ExprChecker` 新增 `expr_types: NodeIdTable<TypeId>` 字段，由调用方传入并保留。
- **`run_typecheck` 返回 `TypedHir`**：改为 `pub fn run_typecheck(...) -> TypedHir`。构造 `TypedHir` 持有 `index`（move）、`env.store`（move）、签名/成员表、per-file `expr_types`。`check-source` 调用方（`main.rs:302`）适配（忽略返回值即可）。
- **`completeness` 闸门模块**：`crates/scoop2_hir/src/completeness.rs`，`pub fn verify(hir) -> Vec<Diagnostic>`——拒绝任何 `expr_types` 缺失节点（兜底，确保 dump 不会缺类型）。
- **wire `run_dump_hir`**（`main.rs:346`）：调 `run_typecheck`，若有错误先报错退出；否则 `print!("{}", hir.render())`。
- **`check-source --phase infer/lower`**：移除 `not_wired`。infer/lower 超出「parser/AST/HIR」范围，但占位符被禁。处理：把 infer/lower 映射到 typecheck（HIR 是本阶段终点，spec 明确「只关心 parser/AST/HIR 错误」），或返回明确「本前端 HIR 为终点」的成功结果。选后者更诚实：infer/lower 打印说明并成功退出（非占位符，是设计边界声明）。

#### 1B. 让 parse 阶段变绿（重新生成 goldens）

- **`tools/run_fixtures.py` 增加 `--accept`**：对 `dump-ast` / `dump-hir` golden 比较失败时，`--accept` 把实际 stdout 写回 `.ast`/`.hir` golden。
- **运行**：`SCOOPC_BIN=target/debug/scoop2c python3 tools/run_fixtures.py --fixtures tests/fixtures/parse --accept` 重新生成 75 个 `.ast`（新缩进格式）。
- **更新 26 个错误码/措辞指令**：逐个核对 `.scoop` 内 `EXPECT-ERROR-CODE`/`EXPECT-ERROR` 与新细粒度 taxonomy（`expected_token`/`expected_ident`/`expected_type`/`expected_expression`/`expected_pattern`）一致；措辞差异按新前端为准更新。
- **`tests/no_placeholder.rs` 扩展守卫**：新增检测 `not_wired`、`尚未接通`、`前端重写进行中` 等占位符措辞，防止回潮。

#### 1C. 新建 `tests/fixtures_ng/hir/` 示例 goldens

- 为 dump-hir 选 3-5 个代表性程序（fun/val、class+方法、when、handle）生成 `.hir` golden，验证渲染稳定。

### Batch 2+（后续会话，typecheck 真实缺陷）

85 个真实失败按簇推进（每会话一簇，逐个加 fixture 验证）：
- **穷尽性/when 模式**（9 个）：or-pattern、is-pattern、嵌套 Option/Bool 组合穷尽。
- **smart-cast 流敏感收窄**（~6 个误报）：`is`/`!is` 后窄化类型。
- **annotation 强制**（7 个）：目标、实参常量性、retention、runtime 策略。
- **continuation/effect 纯度**（4 个）：answer 类型、escape binder、resume step effect、混合 arm。
- **call/overload/vararg**（6+3）：arity、类型不匹配、构造器重载、trailing lambda、funptr named args。
- **extern/funptr/sysroot**（8）：ABI、uintptr↔funptr、gc pin/unpin、atomic、sysroot 一致性。
- **字面量/类型/装箱**（8）：literal/tuple/option arity、boxing 到 interface、star projection、anyref/anyvalue。
- 其余杂项 + 诊断码/位置对齐（11）。

## 验证

- `cargo fmt && cargo clippy --all-targets -p scoop2_base -p scoop2_syntax -p scoop2_hir -p scoop2c -- -D warnings`
- `cargo test -p scoop2c -p scoop2_syntax -p scoop2_hir -p scoop2_base`（含扩展后的 no_placeholder 守卫）
- `SCOOPC_BIN=target/debug/scoop2c python3 tools/run_fixtures.py --fixtures tests/fixtures/parse`（应全绿）
- `target/debug/scoop2c dump-hir <sample>` 输出新缩进树、无「尚未接通」
- `grep -rn "not_wired\|尚未接通\|todo!\|unimplemented!" crates/scoop2_*/src crates/scoop2c/src`（应为空）

## 本次会话完成标准

Batch 1 全部落地：占位符清零、parse 阶段全绿、dump-hir 产出稳定新格式、no_placeholder 守卫扩展并绿。Batch 2 作为后续会话的登记清单。