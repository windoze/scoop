# Scoop 0.1 编译器与运行时实现计划（LLVM/inkwell + 早期 C GC → Scoop GC）

> 目标：把 `SCOOP_FULL_SPEC.md` 落地为可用的 `scoopc` 编译器与最小运行时（含 GC、effect runtime、sysroot），并建立一套“可持续扩展”的 fixture/测试体系，保证规范与实现长期一致。

---

## 0. 总体原则（强约束）

1. **永远可回归**：每个阶段都要产出可执行的最小子集（能编译/能跑），并有 fixtures 覆盖新增语义。
2. **规范驱动**：以 `SCOOP_FULL_SPEC.md` 为唯一语言规范来源；代码块示例要能自动变成 fixtures（类似 doctest）。
3. **LLVM 为后端**：所有代码生成走 LLVM IR（Rust `inkwell`），最终产物为 `.o` + 链接运行时。
4. **GC 先 C 后 Scoop**：
   - 早期：GC/运行时用 C 实现，编译依赖 `clang`（可通过 Rust `cc` crate 或显式调用 clang）。
   - 后期：当语言具备 `@NoGC`、`@Unsafe`、指针/原子/线程等能力后，将 GC 逐步迁移到 Scoop 实现。
5. **多线程友好**：effect dispatch/unwinding 的运行时状态必须是 TLS；`Continuation` 允许跨线程 `resume`（语义为恢复其捕获的 handler stack）。

---

## 1. 仓库结构与工具链（阶段 0：工程化）

### 1.1 代码结构（Rust workspace 拆分）

- [x] `crates/scoopc/`：编译器前端 + 中端 + LLVM 后端（inkwell）（初始骨架已建立）
- [x] `crates/scoop/`：CLI（`scoop build/run/test`），负责调用 `scoopc`、链接、跑测试（已建立骨架）
- [x] `crates/scoop_runtime/`：早期运行时构建 glue（clang + C runtime）（已建立骨架）
- [x] `runtime/c/`：早期 C 运行时（GC + 基础内建 + 线程注册 + effect TLS）（已建立占位实现）
- [x] `sysroot/`：`.scoop` 形式的内建 API 声明（当前已包含 `core.scoop` + `delegates.scoop` + `collections.scoop` 的最小集合；后续补齐 integers/aliases、intrinsics、unsafe/ptr、gc、io 等）
- [x] `tests/fixtures/`：所有编译期/运行期 fixtures（见 §10）（已建立最小 smoke）
- [x] `tools/`：辅助脚本（已加入 `tools/scoop_tools`：spec doctest fixtures 抽取/一致性检查；后续扩展 golden 工具）

> 现阶段仓库还很小，可以先在单 crate 内落地；当模块多起来再迁移到 workspace。

### 1.2 基础构建与开发体验

- [x] 引入依赖：`clap`、`thiserror`、`miette`（诊断）、`tracing`（后续再引入 `inkwell`）
- [x] 统一日志：`tracing` + `tracing-subscriber`
- [ ] 提供命令行（拆分为可迭代子任务）：
  - [x] `scoop test`（fixtures harness，当前为最小 smoke）
  - [x] `scoop dump-ast`（当前为占位信息输出）
  - [ ] `scoop dump-hir` / `scoop dump-ir`（待 HIR/MIR/LLVM 落地）
  - [ ] `scoop build <main.scoop> -o <bin>`（待 codegen + 链接落地）
  - [ ] `scoop run <main.scoop>`（待 build 可用后落地）
- [x] `build.rs`：编译 `runtime/c`（强制 clang；当前通过 `crates/scoop_runtime` 实现）
- [x] CI：最小矩阵（ubuntu）跑 `cargo test --all` + `scoop test`

**本阶段 DoD**
- 能构建出 `scoop` 可执行文件（哪怕只是空壳），`scoop test` 能跑一个最小 fixture。

---

## 2. 词法/语法/AST（阶段 1：前端可解析）

### 2.1 词法分析（Lexer）

- [x] Token 集：关键字、标识符、数字、字符串、基础运算符、注解（`@`）、泛型尖括号、常用 modifier（`public/internal/private/open/abstract/sealed/inline/override`）等（见 `scoopc::syntax::lexer`）
- [x] 补齐位运算与移位运算符 token：`&` `|` `^` `~` `<<` `>>`（spec §2.3.4 / Appendix B.8）
- [x] 注释：行注释 `//`、块注释 `/* */`（当前实现为**非嵌套**；若后续需要可扩展为嵌套）
- [x] 字符串：
  - 普通字符串（`"..."`）
  - `f` 插值字符串（`f"..."`）（lexer 识别字面量边界；parser 将 f-string token 拆为文本段 + 插值 expr 列表，AST `FStringExpr`/`FStringPart` 已实现）
  - raw 三引号字符串（`""" ... """`）与 `f""" ... """`
  - 大括号转义（`{{` / `}}`）属于字符串内容层语义，lexer 无需特殊处理
- [x] Span（源代码位置）基础设施：`Span` + `SourceFile` 行列映射

### 2.2 语法分析（Parser）

- [x] Parser v0（最小可用）：支持 `package` / `import` / 顶层 `fun` + 基础类型声明（`class/interface/struct/enum/effect`），函数/类型体仅保证 `{ ... }` 括号平衡并记录 span
- [x] fun 签名最小解析：参数列表 + 返回类型（支持 Path/泛型参数列表/tuple/nullable 的 `TypeRef` 子集）
- [x] 工程化：拆分 `scoopc::parser` 为多文件模块（cursor/decls/types/file），避免单文件过长，便于后续语句/表达式迭代
- [ ] Kotlin-like 声明（逐步补齐）：`class/interface/struct/enum/effect/val/var/...`
  - [x] 顶层 `val`/`var`：解析声明头；initializer 暂仅保留 span（不解析表达式）
  - [x] 类型体内部成员声明：`val`/`var`/`fun`/nested type（T0201：TypeBody + Member 建模，parse_type_body 实现）
  - [x] 类型体 `val`/`var` 成员声明头：解析 `val x: T`/`var x: T`，带 pass/fail fixtures 覆盖（T0202）
  - [x] 类型体 `fun` 成员声明头：解析 `fun name(params): Ret { ... }`（body 仍是 span），含 pass/fail fixtures 覆盖（T0203）
  - [x] 类型体嵌套类型声明：class/interface/struct/enum/effect 均可作为成员，支持多层嵌套与修饰符（T0204）
  - [x] 声明修饰符列表：顶层与类型成员支持 `public/internal/private/open/abstract/sealed/inline/override`；AST 保存 `modifiers` 并排序去重（顺序无关）；新增 parse fixture 覆盖（T0245）
  - [x] class/interface 继承列表与主构造头（简化版）：解析 `class Dog(name: String) : Animal(name), IFoo` 的最小语法；AST `TypeDecl` 新增 `primary_ctor`/`supertypes`；新增 pass/fail fixtures 覆盖（T0248）
  - [x] 属性声明与 accessors：`ValDecl` 新增 `accessors: Vec<Accessor>` 字段；`Accessor` 节点支持 `get()`/`set(value)` + 表达式体（`= expr`）或块体（`{ stmts }`）；类型体中 `parse_property_decl` 在 `parse_val_decl` 后探测 `get(`/`set(` 模式并解析 accessor；`get`/`set` 作为上下文关键字（soft keyword），不加入 lexer 关键字表；6 个 pass/fail fixtures + 5 个 unit tests 覆盖（T0234）
  - [x] 委托属性 `by expr`：`ValDecl` 新增 `delegate: Option<Expr>` 字段；`parse_property_decl` 在 `parse_val_decl` 后探测 `by` 上下文关键字并解析委托表达式；`by` 与 accessors 在语法层互斥；支持 `val x: T by lazy { ... }` 等 trailing lambda 形式；2 个 pass/fail fixtures + 3 个 unit tests 覆盖（T0235）
  - [x] Rich enum variant 声明：`Member::Variant(EnumVariant)` 新增 AST 节点；`EnumVariant` 含 `name: Ident` + `params: Vec<Param>`；`parse_type_body` 接收 `TypeKind` 参数，对 `Enum` 类型识别裸标识符作为 variant 开始；`parse_enum_variant` 解析 `Name` / `Name(val field: T, ...)` 形式；variant 参数要求 `val` 关键字 + 类型标注；1 个 pass + 2 个 fail fixtures + 3 个 unit tests 覆盖（T0236）
- [x] `typealias` 声明：解析顶层 `typealias Name = Type` 并纳入 AST（T0251，为 sysroot 标准别名与 Kotlin 兼容铺路）
- [x] Expr/Stmt 最小骨架（T0205）：Ident/IntLit/StringLit/BlockExpr/Missing + Stmt::Expr/Stmt::ValDecl
- [x] val/var initializer 解析为原子表达式（T0206）：`ValDecl.init` 从 `Option<Span>` 升级为 `Option<Expr>`，支持 ident/int/string 原子
- [x] 块表达式解析（T0207）：`parse_block_expr` 解析 `{ stmt* }` 为 `BlockExpr { stmts }`；`FunBody::Block` 改用 `BlockExpr`（含 stmts）替代旧 `Block`（仅 span）；块内支持表达式语句与 val/var 声明
- [x] 块内 val/var 局部绑定（T0208）：`parse_stmt` 已支持 `val x: T = expr`/`val x = expr`/`var x = expr`；新增 pass/fail fixtures 覆盖（含 `val = 1` 缺名报错）
- [x] 函数调用表达式（T0209）：`parse_expr` 引入后缀调用循环，解析 `f(a, b)` 为 `CallExpr { callee, args }`；支持嵌套调用 `f(g(x))`、尾随逗号；`parse_stmt` 和 `ValDecl.init` 改用 `parse_expr`
- [x] 成员访问表达式（T0210）：后缀循环新增 `.` 分支，解析 `a.b` 为 `FieldAccessExpr { receiver, field }`；支持链式 `a.b.c(1)` 与调用组合
- [ ] 语句/表达式（逐步补齐）：lambda
  - [x] Lambda AST 节点：`Expr::Lambda(LambdaExpr)` + `LambdaParam`（T0221）
  - [x] Lambda 表达式解析：`{ params -> body }` / `{ -> body }` 的 lookahead 歧义消解 + 参数列表 + block body 解析；6 个 pass/fail fixtures 覆盖（T0222）
  - [x] Trailing lambda：`f(a, b) { ... }` 与 `expr { ... }` 形式，尾随 lambda 作为最后一个 `CallArg::Positional(Lambda)`；bare `{ body }` 无 `->` 时解析为零参数 lambda；5 个 pass fixtures 覆盖（T0232）
- [x] `when` 表达式解析：`when (subject) { pattern -> body, ... }`（T0215：AST `WhenExpr`/`WhenArm`/`WhenPattern` + parser + pass/fail fixtures）
  - [x] Pattern v0（T0238）：`WhenArm.pattern` 迁移为 `Pattern`，删除 `WhenPattern`；支持 wildcard `_`、int/string/bool 字面量、`is`/`!is` Type、`else`、裸标识符 bind；2 个 pass fixtures + 6 个 unit tests 覆盖
  - [x] Pattern v1 — tuple pattern（T0239）：`parse_when_pattern` 新增 `(` 检测调用 `parse_tuple_pattern()`，解析 `(p1, p2, ...)` 为 `Pattern::Tuple`；支持嵌套 pattern、尾随逗号、空 tuple `()`；`no_call` 标志 + `looks_like_tuple_pattern_ahead()` lookahead 消解 arm body call 与下一 arm tuple pattern 的歧义；1 个 pass + 1 个 fail fixture + 6 个 unit tests 覆盖
  - [x] Pattern v2 — enum variant pattern（T0240）：`parse_when_pattern` 在裸标识符后 peek `(` 调用 `parse_variant_pattern()`，解析 `Name(p1, p2, ...)` 为 `Pattern::Variant`；支持嵌套 variant（`Some(Some(x))`）、空参数（`Point()`）、尾随逗号、wildcard 字段；裸标识符（无括号）保持为 `Bind`（消歧留给 resolve 阶段）；1 个 pass + 1 个 fail fixture + 6 个 unit tests 覆盖
  - [x] Pattern v3 — struct pattern（T0241）：`parse_when_pattern` 在裸标识符后 peek `{` 调用 `parse_struct_pattern()`，解析 `Name { field, field: pattern, ... }` 为 `Pattern::Struct`；支持 shorthand（`x`）、rename（`x: pattern`）、空 struct（`Unit {}`）、尾随逗号、嵌套 pattern（`first: Some(x)`）；1 个 pass + 1 个 fail fixture + 6 个 unit tests 覆盖
  - [x] Pattern v4 — or-pattern（T0242）：`parse_when_pattern` 拆分为 `parse_when_pattern`（含 `|` 循环）+ `parse_when_pattern_atom`（单个 pattern）；`A | B` 解析为左结合 `Pattern::Or`；支持多级 `A | B | C`、嵌套在 tuple/variant/struct 内的 or-pattern、混合 literal/bind/variant/wildcard；1 个 pass + 1 个 fail fixture + 6 个 unit tests 覆盖
  - [x] Pattern v5 — guard `if <expr>`（T0243）：`parse_when_arm` 在 pattern 与 `->` 之间检测 `if` 关键字，解析 guard 表达式并包装为 `Pattern::Guard`；`looks_like_tuple_pattern_ahead` 更新为同时接受 `->` 和 `if` 作为 tuple pattern 判定条件；1 个 pass + 1 个 fail fixture + 6 个 unit tests 覆盖
- [x] `if` 表达式解析：`if (cond) thenExpr else elseExpr`（T0214：AST `IfExpr` + parser + pass/fail fixtures）
- [x] 值类型更新表达式：`expr with { path: value, ... }`（spec §2.6）（T0216：AST `WithExpr`/`WithField` + parser + pass/fail fixtures）
- [x] 运算符优先级（Pratt parser）：二元运算 `+ - * / %`、比较 `< > <= >= == !=`、逻辑 `&& ||`、位运算 `& | ^`、移位 `<< >>`；一元前缀 `- ! ~`（T0252）；括号分组 `(expr)`；`Percent` token 新增
- [x] Elvis `?:` 二元运算（最低优先级）与 not-null 断言 `!!` 后缀运算（T0212）
- [x] 类型判断/转换操作符：`is`/`!is`/`as`/`as?`（与比较运算符同优先级，RHS 为 TypeRef）（T0213）
- [x] 声明处泛型参数列表：`fun id<T>(...)` / `struct Box<T> { ... }` — AST `TypeParam` 节点 + `type_params` 字段 + `parse_type_param_list`（T0218）
- [x] 泛型语法补齐：type args 支持 `*`（star projection），type params 支持 `in/out` 声明处变型（T0249）
- [x] struct literal AST 节点：`Expr::StructLit(StructLitExpr)` + `StructLitField`（T0223）
- [x] struct literal 解析：`TypeName { field: expr, ... }`（T0224）— `looks_like_struct_lit()` lookahead 在 `parse_expr_primary` 中识别 `Ident(.Ident)*(<...>)? { (Ident: | })` 模式，调用 `parse_struct_lit_expr()` + `parse_path_type_inner()` 解析；6 个 pass/fail fixtures 覆盖
- [x] 关键歧义：struct literal vs lambda（对应 spec §12）（T0225）— `looks_like_struct_lit()` 增加 `has_arrow_inside_braces()` 扫描：在 `{ Ident :` 匹配后，前扫顶层 `->` 来排除 lambda with typed params；4 个 pass fixtures 覆盖
- [x] `return` 语句解析：`Stmt::Return(ReturnStmt)` + `parse_return_stmt`（T0226）— 支持 `return` 与 `return expr`；3 个 pass fixtures 覆盖
- [x] 赋值语句解析：`Stmt::Assign(AssignStmt)` + `parse_stmt` 中 `= rhs` 检测（T0227）— 支持 `x = expr` 与 `a.b.c = expr`；2 个 pass + 1 个 fail fixtures 覆盖
- [x] `while` 循环表达式解析：`Expr::While(WhileExpr)` + `parse_while_expr`（T0228）— 支持 `while (cond) body`；`break`/`continue` 作为 `Stmt::Break`/`Stmt::Continue`；lexer 新增 `While`/`Break`/`Continue` 关键字；2 个 pass + 1 个 fail fixtures 覆盖
- [x] 错误恢复：`parse_file_recovering()` 新 API，顶层/块内/类型体三级同步点恢复，收集多个诊断
- [x] safe-call `?.`：`FieldAccessExpr` 与 `CallExpr` 新增 `safe: bool` 标志；postfix 循环处理 `QuestionDot` token，支持 `x?.member` 与 `x?.foo(args)`；2 个 pass + 1 个 fail fixtures 覆盖（T0229）
- [x] 函数参数默认值：`Param` 新增 `default: Option<Expr>` 字段；`parse_param_list` 解析 `= expr`；1 个 pass + 1 个 fail fixtures 覆盖（T0230）
- [x] 命名参数调用：新增 `CallArg` 枚举（`Positional(Expr)` / `Named { name, value }`）；`CallExpr.args` 改为 `Vec<CallArg>`；`parse_call_arg` 通过 lookahead `Ident + =` 区分命名参数与位置参数；2 个 pass fixtures 覆盖（T0231）
- [x] 扩展函数 receiver：`FunDecl` 新增 `receiver: Option<TypeRef>` 字段；`parse_fun_receiver_and_name` 通过 lookahead 识别 `Type.name(...)` / `pkg.Type.name(...)` / `List<T>.name(...)` 模式并拆分 receiver 与函数名；type params 支持 spec 风格 `fun <T> Type.name(...)` 和 Kotlin 风格 `fun name<T>(...)`；resolve 侧同步处理 receiver TypeRef；3 个 pass + 1 个 fail fixtures + 5 个 unit tests 覆盖（T0233）

### 2.3 语法树表示（AST/Parse Tree）

- [ ] 建议区分：
  - `ParseTree`（保留所有 token/节点，利于错误恢复与格式化）
  - `AST`（更语义化的节点，利于后续分析）
- [x] AST（最小骨架）：File/Package/Import/Fun/TypeDecl/Block/Ident/Param/TypeRef，节点带 span 并可回切源文本
- [x] Pattern AST 节点（T0244）：新增 `Pattern`（Wildcard/Bind/Tuple/Struct）与 `ValBinding`，用于 block 内 `val` 解构绑定；`when` 分支模式仍使用 `WhenPat`（后续再统一迁移）
- [ ] Parser 收尾补齐：
  - [x] `import foo.bar.Baz as Qux`（Appendix B.7）
  - [x] use-site effect row 实参：`Type<eff Row>`（spec §3.4）
  - [x] pattern rest：`..`（spec §4.2）
  - [x] receiver function type：`T.(A, B) -> R / E`（spec §7.5）
  - [x] 泛型 `where` 子句（spec §3 / Appendix B）
- [ ] Kotlin-like 声明补齐：
  - [x] `init { ... }` blocks（Appendix B.2.2）
  - [x] secondary constructors（Appendix B.2.2）
  - [x] `object` / `companion object` 声明（Appendix B.9）

**本阶段 DoD**
- `scoopc` 能解析大部分 spec 示例，不做类型检查也能 `dump-ast`。

---

## 3. 包与名字解析（阶段 2：可绑定符号）

### 3.1 包系统（Cone 的源级部分）

- [x] `package` 声明、`import`、通配 `*`（已支持解析 + 最小名字绑定：TypeRef 按 import/star import 解析）
- [x] 可见性：`public/internal/private`
- [ ] 作用域：文件级、类/接口/结构体内部、泛型参数作用域（块级局部 `val/var` 已完成，见 T0304）

### 3.2 符号表与解析

- [x] 顶层符号索引（最小子集）：基于 `package + 顶层声明名` 构建 FQN 索引并检测重复定义；索引区分 type/fun/value 命名空间（见 `scoopc::resolve`）
- [x] 类型体成员索引：把 type body 的 fields/methods/nested types 纳入索引并检测同一类型体内重复定义（T0302）
- [x] 两阶段/多阶段解析（T0308）：
  - 先收集声明头（type/function/field signatures）
  - 再解析函数体与初始化表达式
- [x] import 解析与名字绑定（最小子集）：对 fun/val 顶层签名里的 `TypeRef::Path` 做存在性解析（含 star import）
- [x] import 表（T0303）：显式 import 按 type/value 命名空间拆分，并保留 `*` import 前缀（为 expr 解析准备）
- [x] `typealias` 名字解析：alias 作为 type-level symbol 纳入索引；冲突与可见性诊断
- [x] 作用域：块级（函数体/表达式块内局部 `val/var`，含遮蔽）（T0304）
- [x] 表达式裸标识符绑定写回：为 `ExprKind::Ident` 记录其解析到的局部/顶层引用（T0305）
- [x] 调用点候选收集：`Call(Ident)`/成员调用/构造调用写回候选集合 + 调用形状；多候选留给后续 typecheck 决议（T0319）
- [x] 成员访问解析（`.`）：把 `receiver.member` 绑定到类型体字段/方法并写回 `MemberIdent.resolved`（T0310）
- [x] 扩展成员 fallback：member 优先于 extension（同包）且 receiver 类型可匹配（T0312）
- [x] 作用域：泛型参数（声明处 type params 在签名内可解析）（T0309）
- [x] `where` 子句约束解析：约束左侧必须命中 type param scope，右侧 `TypeRef` 按包前缀/import 规则解析（T0320）
- [x] 作用域：`this`（类型体成员/扩展函数体）与主构造参数在成员里可见（T0313）
- [ ] 同名优先级：成员/顶层/扩展（逐步补齐）
- [x] import alias 绑定与冲突规则：`import foo.bar.Baz as Qux`（Appendix B.7）
- [x] class 初始化阶段作用域：property initializer / `init` / secondary constructor（T0316）
- [x] `object` / `companion object` 的名字解析与成员可见性（T0317：支持 `Obj.member` 与 `ClassName.member`）
- [ ] overload set 建模：
  - [x] 索引侧：顶层/成员/扩展函数与构造函数收集为候选集合（T0318）
  - [x] 调用点/构造点：从“唯一 callee”升级为“候选集合 + 调用形状”（T0319）
  - [x] typecheck：普通函数调用最小重载决议（T0453：过滤后唯一/歧义）
  - [x] typecheck：class 构造调用最小重载决议（T0454：primary/secondary + 默认参数）
  - [x] typecheck：扩展函数调用重载决议（T0455：member 优先 + receiver/参数 specificity）
  - [ ] inference：most-specific 与重载冲突诊断（后续任务逐步补齐）
- [ ] 跨包可见性：`public/internal/private` 在 source package / `.cone` 依赖边界上的规则与诊断（拆分为子任务；T0321b 依赖 T1105 `.cone` 读取，已在 TODO 中延后）
  - [x] T0321a：resolver 引入 cone 边界 + source-only 多 cone fixtures
  - [ ] T0321b：接入真实 `.cone` 依赖后的可见性过滤（等待 T1105）
- [ ] 跨包扩展导入：extension 在显式 import / star import / 成员候选之间的可见性、shadowing 与候选收集（依赖 T0321b）

### 3.3 sysroot 注入

- [x] sysroot 文件与 loader 骨架：可发现并解析 `sysroot/*.scoop`（当前实现见 `scoopc::sysroot`）
- [x] 编译流程注入：通过 `scoopc::session::Session` 默认加载 sysroot，并在 `build_top_level_index` 中纳入名字解析环境
- [x] sysroot：补齐内建标量类型的“可见声明”（spec §2.3.4 / runtime §3）
  - `Int/UInt`：word-sized（随 target 指针宽度变化，Swift 约定）
  - 固定位宽整数：`Int8/16/32/64`、`UInt8/16/32/64`
  - 标准别名：`Byte/Short/UShort/Long/ULong`，以及 `UIntPtr = UInt`
  - 说明：这些类型是语言 builtin（布局/语义由编译器固定），但它们的可见声明由 sysroot 提供
  - fixtures：`tests/fixtures/resolve/sysroot_scalar_types_ok.scoop`
- [x] sysroot：运行时错误枚举 `RuntimeError`（`NullAssertionFailed`/`ClassCastFailed`），用于 `Raise<RuntimeError>`（T0419）

**本阶段 DoD**
- 能在无类型检查情况下做 name resolution，并对未定义符号给出准确 span 的错误。

---

## 4. 类型系统（阶段 3：先类型检查再优化）

### 4.1 类型表示（核心）

- [x] 区分引用类型 vs 值类型（spec §2）：内部 `TypeKind::{Ref, Value}` 已落地（T0401）
- [x] 从 sysroot 收集内建类型/效果的声明头（`TypeEnv`：kind + arity），为后续 lowering/typecheck 提供环境起点（T0402）
- [x] TypeEnv：收集 enum variants（tag + payload fields），并检测重复 variant/字段（T0425）
- [x] enum variant ctor：支持 `Some(x)` 风格构造并做参数/类型检查（T0426）
- [x] `TypeRef` → `Type` lowering：支持 `Path`/`Tuple`/`Nullable` + 泛型 arity 检查（T0403）
- [x] Nullability 语法糖：`T?` → `Option<T>`（lowering 阶段 desugar）（T0411）
- [x] 顶层声明头检查：`fun/val/type` 的签名最小约束（类型注解等）（T0404）
- [x] 表达式类型检查 v0：字面量（Int/String/Bool/Unit）（T0405）
- [x] 表达式类型检查 v0：变量引用（局部/参数/顶层）（T0406）
- [x] 表达式类型检查 v0：函数调用（参数数量/类型匹配；无重载/无默认参数）（T0407）
- [x] 表达式类型检查：成员访问（struct 字段 + class 字段/属性最小子集，`p.x` / `this.x`）（T0408/T0438）
- [x] struct 声明最小语义检查：字段重复/`var`/默认值约束（T0409）
- [x] struct literal 类型检查：字段存在性/重复/类型匹配 + 必填字段覆盖（当前：必须显式提供所有字段）（T0423）
- [x] tuple/Unit（0 元 tuple）：tuple 类型与 tuple 字面量 typecheck（T0410）
- [x] 最小子类型规则：`Nothing <: T`（用于 `return`/不可达分支/后续 `Raise.raise`）（T0420）
- [x] `!!` 非空断言：`Option<T>` → `T` 的静态类型规则（T0421a）
- [x] `?.` safe-call 与 Elvis `?:`：`Option<T>` 语法糖的类型规则（`x?.m()` 返回 `R?`；`x ?: y` 返回 `T`）（T0422）
- [ ] 内建整数模型（spec §2.3.4 / runtime §3）
  - （已在 `scoopc::ty` 中建模 `Int/UInt/IntN/UIntN`；运算/布局语义后续补齐）
  - [x] 整数/布尔运算符类型规则：一元 `! - ~`；二元算术/比较/位运算/移位（shift count 固定为 `Int`）与 `&&/||`（T0447）
  - `Int/UInt` 的 bit width = target pointer size
  - 固定位宽整数类型与类型大小/对齐（为 FFI/序列化提供稳定布局）
  - 整数运算语义：wrap-around、算术/逻辑右移、shift count mask（避免 target 相关 UB）
- [x] `typealias` 语义：类型层展开（用于 `Byte/UIntPtr` 等 sysroot 标准别名；循环 alias 报错）（T0446）
- [x] `Unit`、tuple、`Option<T>`（`T?` sugar）：类型表示与格式化输出已完成（语义/typecheck 后续）（T0401）
- [x] 函数类型（含 effect row）：`(A, B) -> T / E`（spec §7.5）— AST `TypeFun`/`RowExpr` + `parse_paren_type`/`parse_row_expr` + pass/fail fixtures（T0219）
- [x] 函数类型（Type 表示 + lowering + 最小子类型规则）：参数逆变/返回协变 + effect row containment（T0435）
- [x] receiver function type：`T.(A, B) -> C / R`（Type 表示 + lowering；receiver 按第一个参数参与逆变比较）（T0435）
- [x] 类型参数（`TypeKind::Param`）与声明处变型（`in/out` + 最小位置规则 + variance 子类型，仅 ref args 生效）（T0437）
- [ ] 泛型约束：上界/下界、where 子句（spec §3 / Appendix B）

### 4.2 声明类型：class/interface/struct/enum/effect

- [x] class：主构造 `val/var` 参数作为字段/属性 + 成员方法体最小 typecheck（T0438）
- [x] class：继承/override 的最小静态规则（final/open/abstract/sealed + override 检查）（T0439）
- [ ] class：虚表/方法分发与 codegen（先单继承）
- [x] interface：多实现、默认方法（可先限制默认方法 codegen）（T0440）
- [ ] struct：布局（字段顺序/对齐），不可变，值语义
- [x] enum（rich enum）：tag + union 布局 + niche/boxing/lint 元数据（T0449；codegen 另见 §8.2）
- [ ] effect：像 interface 一样声明操作签名

### 4.3 Boxing 与 Any

- [x] 值类型装箱到 interface/`Any`（spec §2.5）
- [ ] 先实现“语义正确”，性能优化（如 O(n) 显式转换）后置

### 4.4 模式匹配与 smart cast（spec §4）

- [ ] `when` 表达式（穷尽性检查可分阶段做）
  - [x] 分支结果类型（最小 LUB）：一致 → 该类型；不一致 → `Any`（T0414）
  - [x] 分支 pattern 最小类型检查：tuple/variant 限定 + binder 注入分支作用域（T0427）
  - [x] 穷尽性检查 v0：enum/Bool/Option + `else`/`_` 规则（T0428）
  - [x] guard 分支视为不可覆盖（需 `else`/`_`）（T0429）
- [x] `is` / `!is` + smart cast（T0413：最小子集，仅 `if (x is T)`/`if (x !is T)`；仅参数 + `val`）
- [x] `as` / `as?`：基础类型规则已实现（T0412）；按 spec 的运行时失败路径（`Raise.raise(RuntimeError.ClassCastFailed)`）待 effect 系统（required effect row/try-catch）补齐后接入

### 4.5 值类型更新（`with` 表达式）（spec §2.6）

- [x] 语义：并行更新（静态约束：禁止重复/包含路径）（T0415）
- [x] path 解析：`a.b.c: value`（字段路径必须存在且类型匹配）（T0415）
- 说明：`TODO.md` 中的 T0424 与以上两项重复，已由 T0415 覆盖（本节保持为实现状态来源）。
- [ ] lowering：生成“拷贝 + 覆盖字段”的构造逻辑（对嵌套 path 生成中间拷贝）

### 4.6 变量绑定与解构（spec §9 + Kotlin-like）

- [x] `val`/`var`：
  - 不可变/可变规则（`val` 不可再次赋值；`var` 可）（T0416）
  - `var` 的赋值类型检查：lhs 可写性（局部 `var` / class `var` 属性）+ rhs 可赋值（T0416/T0443）
- [ ] 解构绑定（destructuring）：
  - [x] tuple/struct 的 `val (a, b) = expr` / `val Point { x, y } = expr`（T0430）
  - [ ] enum 的 `val Some(x) = expr`（可复用 `when` pattern，后续补齐）
  - [ ] `when` 分支中的解构 pattern
- [ ] 控制流基础：`if/while/for/return/break/continue`（非局部 return 仅允许 inline lambda 实参）
  - [x] `return`：函数内 `return expr?` 返回类型检查与诊断（T0417）
  - [x] `while`：条件必须为 Bool；`break/continue` 仅允许在循环体内（T0442）

### 4.7 属性系统（spec §10）

- [x] 类属性（T0431：typecheck 侧最小规则）：
  - [x] 默认 getter/setter 视为存在（因此可能生成 backing field）
  - [x] `field` 仅在 accessor 内可见；computed 属性引用 `field` 报错
  - [x] backing field 判定 v0：initializer 或默认 accessor
- [x] 值类型属性：
  - [x] computed property 仅允许 getter-only（禁止 setter）
  - [x] computed property 不允许 initializer（避免 backing field）
  - [x] struct/enum 内属性不允许 `var`
- [x] 扩展属性（T0433：解析 + typecheck 侧门禁）：
  - [x] 顶层语法：`val/var ReceiverType.name: Type get()/set()`
  - [x] computed 约束：禁止 initializer / 禁止 `field` / getter 必需 / `var` 需 setter
  - [ ] lowering：编译为静态 getter/setter（receiver 作为第一个参数）
- [ ] 委托属性（delegated properties）：
  - [x] T0434a：`by` 语法 + 最小静态规则（仅 class；检查 `getValue/setValue` 名称存在性）
  - [x] T0434b：对接 `PropertyMeta` 并升级为签名检查（与 §13 comptime/反射联动）
  - [ ] lowering：生成 `$delegate` 字段 + getter/setter 转发到 `getValue/setValue`（T1210）

### 4.8 函数声明细节（spec §7）

- [x] `inline`：non-local return 门禁（lambda 中 `return` 仅允许出现在 inline 调用的 lambda 实参内；T0444）
- [ ] `inline`：实际 inlining/闭包消除等优化（IR/后端阶段）
- [ ] 扩展函数：
  - [x] 解析与分发规则（静态分发、member 优先；typecheck 降糖为 receiver 第一个参数）
  - [ ] codegen：receiver 作为第一个参数的普通函数
- [x] enum 完整语义：niche optimization、oversized variant boxing、variant size disparity lint（spec §2.3.2）（T0449：前端固定元数据；后端待落地）
- [x] pattern rest `..` 的类型检查与绑定规则（spec §4.2）
- [x] class 初始化模型：property initializer、`init` blocks、secondary constructors、初始化顺序（Appendix B.2.2）（T0448：最小 typecheck + delegation 门禁）
- [x] `object` / `companion object`：单例类型、成员访问、伴生对象解析（Appendix B.9）
- [x] 委托属性标准库面：`ReadOnlyProperty` / `ReadWriteProperty` 与 `scoop.delegates`（`lazy`/`observable`/`vetoable`/map-backed）（spec §10.4）
- [ ] 通用重载解析（函数 / 构造函数 / 扩展）：
  - 候选筛选：arity、receiver、可见性、命名参数、默认参数
  - 决议规则：最具体候选（most specific candidate）与稳定歧义诊断
  - [x] enum variant / pattern 在同名跨 enum 时按期望类型或 subject type 消歧

**本阶段 DoD**
- `scoopc` 能对一批无泛型/少量泛型的示例做类型检查（含 struct/enum/Option/when/is/as）。

---

## 5. 类型推断（阶段 4：约束求解）

对齐 spec §14：constraint generation + solving（非 HM W）。

- [ ] 约束表示：`τ1 <: τ2`、相等、行约束（effects）
- [ ] LUB（if/when 分支）
- [ ] lambda 推断：参数类型下推、返回类型与 effect row 推断（见 spec §14.7.2）
- [ ] 错误报告：把“推断失败”映射到具体源 span 与最小可读解释
- [ ] overload resolution 与推断联动：
  - 泛型实参、lambda expected type、默认参数、命名参数、trailing lambda 共同参与候选决议
  - effect rows / `eff` 参数也必须能参与重载筛选与歧义诊断
- [ ] 真正的分支合并类型：LUB / 受限 union 的构造、比较与化简（替代简单 `Any` fallback）
- [ ] effect row 高级推断：高阶 row 约束、泛型 row 变量、归一化参与候选决议

**本阶段 DoD**
- 能跑 `tests/fixtures/infer/**`：涵盖 if/when/lambda/泛型调用推断的 compile-pass/compile-fail。

---

## 6. 效果系统（阶段 5：先 `Raise`，再完整三种 arm）

### 6.1 静态层：effect row + 多态 + 推断

- [ ] 语法：
  - [x] 函数/函数类型的 `/ RowExpr`
  - [ ] `eff` 作为上下文关键字：`<eff E = Pure>`、`eff E1+E2`（parser 已支持声明处 `<eff E = Pure>`；use-site `Type<eff Row>` 待补）
  - [x] `+` 并集、`Pure` 空行
  - [ ] 闭合行语法：`/ R!`（`!` 后缀作用于整个 row，不与 `+` 右操作数绑定；spec §5.8.4）
- [ ] 规则：
  - required effects（未处理效果检测，spec §14.7.1）
  - public 默认 `/ Pure` 的强制约束
  - private/internal 可推断 effect row
  - overriding：`R_over ⊆ R_base`
  - entry point 必须 `Pure`（等价于 `Pure!`，闭合语义）
  - 闭合行额外约束：所有来源的 effect（含 callback 透传）都不能逃逸出函数边界（spec §5.8.4）
  - 高级 row 语义：高级归一化、泛型 row 变量、必要的高阶 row 运算
- [ ] 语法糖：
  - `try/catch/finally` → `handle { } with { Raise.raise -> } finally { }`
  - `!!` 失败 → `Raise.raise(RuntimeError.NullAssertionFailed)`（T0421b；依赖 required effects + try/catch lowering：T0604/T0607）
  - `as` 失败 → `Raise.raise(RuntimeError.ClassCastFailed)`（T0445；依赖 T0604/T0607）
  - 多个 `catch` arm 与匹配顺序（不只单个 `catch`）

### 6.2 动态层：handler stack dispatch（Appendix A）

- [ ] 运行时必须维护 **handler stack**（按“最近匹配 handler”分发）
- [ ] arm body 在 dispatch scope 之外执行（避免 self-capture）

### 6.3 Codegen/Lowering：分三步落地

1) **非恢复 `->`（flag-based unwinding）**
   - [ ] TLS：`__scoop_effect_active` + perform slot
   - [ ] `perform` 写 slot + set flag + return
   - [ ] 调用链传播：检查 flag，沿栈向外返回
   - [ ] handler 边界消费 slot 并清 flag，然后执行 arm；`finally` 正确执行；必要时 re-raise

2) **立即恢复 `-> resume`（栈 state machine）**
   - [ ] 把 handle body 分段（perform/call 边界）
   - [ ] lifted locals（跨段变量提升）
   - [ ] while-loop 调度 state
   - [ ] `resume(value)` 必须恰好一次（编译期/运行期双保险）

3) **逃逸 continuation `, k ->`（堆 state machine + continuation 对象）**
   - [ ] continuation 捕获 handler stack（fiber-local 语义）
   - [ ] 支持跨线程 `resume`：恢复 captured handler stack 到当前线程 TLS（见 spec §5.5）
   - [ ] one-shot：原子状态位保证并发下只能成功一次

- [ ] use-site effect row 实参：`Type<eff Row>` 的解析/类型检查/推断（spec §3.4）
- [ ] `Task<T>` 与 `async fun` 语义：
  - `async fun foo(): T` desugar 为 `fun foo(): Task<T>`
  - 调用者签名不携带 `/ Async`
  - `Task<T>` 懒执行，直到 `await` 或显式启动
- [ ] Appendix A 一致性：嵌套 handler 必须支持“最近匹配 handler”分发，不能停留在单层 handler 模型
- [ ] program boundary 不只 `main`：库导出入口、多 entry point 与 host/embedded 边界规则
- [ ] perform slot ABI：从单 slot 扩展到可承载复杂 payload / 多 effect op 的稳定表示

**本阶段 DoD**
- compile-pass + run-pass 覆盖 `Raise`、`try/catch/finally`、自定义 effect + handle，以及一个最小 async/await demo。

---

## 7. 中间表示与单态化（阶段 6：为 LLVM 做准备）

### 7.1 HIR/MIR 设计

- [ ] HIR：保留大部分结构但已解析/已类型化
- [ ] MIR：显式控制流（基本块）、显式临时变量、显式 drop/cleanup（用于 `finally`/effect unwinding）

### 7.2 泛型单态化（monomorphization）

- [ ] 为每个具体实例生成专用 IR（含 `eff` 参数实例化）
- [ ] 缓存键：符号 + type args + effect row args
- [ ] 支持“预编译常见实例”（对齐 Cone 的 pre-specialize）

### 7.3 闭包与函数值

- [ ] lambda → `{ env_struct, fn_ptr }` 形式
- [ ] 捕获变量布局与 GC trace 信息生成
- [ ] effectful function type 的调用约定统一化
- [ ] 可变捕获：捕获 `var` 时的 box/lift 策略、别名与写回语义

**本阶段 DoD**
- 纯子集（无 class 虚分发也可）能 lowering 到 MIR，并能生成可链接 `.o`（下一阶段）。

---

## 8. LLVM 后端（阶段 7：inkwell codegen）

### 8.1 LLVM Module/Pass 管线

- [ ] 目标三元组与数据布局（target machine）
- [ ] 基本优化 pass（O0/O1/O2 可选）
- [ ] 调试信息（DWARF）可后置

### 8.2 数据布局与 ABI

- [ ] 值类型（struct/tuple/enum）按 LLVM struct layout 映射
- [ ] 引用类型：对象头（type descriptor 指针 + flags + size 等）
- [ ] interface/虚表：最小可行实现（先只支持接口方法调用与装箱）

### 8.3 与 GC 的接口（推荐：shadow stack 精确根集）

为了避免早期实现 LLVM `gc.statepoint` 的复杂度，建议先实现 **shadow stack**：

- [ ] TLS：`scoop_gc_tls.current_frame`
- [ ] 每个函数 prologue 建立 `GcFrame`（包含 prev 指针 + roots 数组）
- [ ] 在需要的地方把 GC 引用写入 roots slot（局部变量活跃区）
- [ ] 分配触发 GC 时，runtime 扫描所有线程的 frame 链得到根集

> 优点：实现难度低、语义清晰、可逐步演进到移动 GC；缺点：需要编译器插桩，性能一般，但足够 bootstrap。

- [ ] `when` lowering：补齐 or-pattern / guard（spec §4.2）
- [ ] tuple 字段访问统一为 `._0` / `._1`，并同步修正文档、fixtures、lowering、codegen（spec §2.3.3）
- [ ] enum layout/codegen：补齐 niche optimization、oversized variant boxing、variant size disparity lint（spec §2.3.2）
- [ ] `object` / `companion object` codegen：单例存储、一次初始化、静态成员访问（Appendix B.9）
- [ ] `trimIndent()`：运行期 fallback 与字符串 API 对接（spec §8.4）

**本阶段 DoD**
- 生成的二进制可运行（至少支持整数运算、函数调用、打印、Option/enum 基本构造）。

---

## 9. 早期运行时（C + clang）（阶段 8：可执行与可观测）

### 9.1 最小运行时组件

- [ ] 启动入口：`main`/平台 glue，初始化 TLS（GC + effect）
- [ ] 分配器：`scoop_alloc(size, type_desc)`
- [ ] GC（先易后难）：
  - v0：非移动 mark-sweep（实现简单，pin/unpin 可先是 no-op 或计数）
  - v1：可选移动/压缩（实现 pin/unpin 语义）
- [ ] 类型描述（type descriptor）：
  - pointer bitmap 或 trace 回调
  - 用于扫描对象内的引用字段（struct/enum/closure env）
- [ ] 线程注册：新线程必须注册到 runtime 以便 GC stop-the-world 扫描其 shadow stack
- [ ] `object` / `companion object`：跨 DLL / 动态链接的一次初始化与全局可见性策略

### 9.2 effect runtime（C 或编译器插桩）

- [ ] TLS：handler stack 指针、perform slot、flag
- [ ] 最小原语：push/pop handler frame、读写 perform slot、原子 one-shot continuation

### 9.3 与 clang 的构建集成

- [ ] `runtime/c` 用 clang 编译成静态库/对象
- [ ] `scoopc` 链接时自动把 runtime 拉进来
- [ ] fixtures 中提供 `--emit-llvm`/`--emit-obj`/`--emit-asm` 选项方便排查
- [ ] effect runtime 必须支持多层 handler stack（最近匹配分发 + arm body 在 dispatch scope 外；Appendix A）
- [ ] `Task<T>` / executor 最小 runtime 原语：任务状态、入队/恢复、可选 start（spec §5.7）
- [ ] `object` / `companion object` 的 once/init 支持（Appendix B.9）

**本阶段 DoD**
- 有一个“运行期回归套件”（见 §10）能持续压测 GC 与 effect。

---

## 10. Fixtures 与测试体系（贯穿所有阶段，必须先行）

这里的目标是：**任何规范点都有对应的 fixture**，并且 fixtures 能区分：
- 解析是否正确
- 语义/类型/效果是否正确
- 代码生成/运行期行为是否正确

### 10.1 Fixture 目录规划（建议）

```
tests/
  fixtures/
    parse/               # 仅解析：AST snapshot / 语法错误恢复
    resolve/             # 名字解析：import/visibility
    resolve_multi/        # 名字解析：多文件编译单元（目录为 case）
    typecheck/           # 类型检查：compile-pass / compile-fail
    typecheck_multi/      # 类型检查：多文件编译单元（目录为 case）
    infer/               # 推断专项
    effects/             # effect rows / handle / required effects / entrypoint Pure
    codegen/             # 运行输出对比
    runtime_gc/          # GC/alloc/pin/unpin/压力测试
    unsafe_nogc/         # @Unsafe/@NoGC 规则
    language/            # 字符串/with/属性/委托/操作符等语法语义专项（按章节分组也可）
    comptime/            # const fun / comptime / 反射 intrinsics
    cone/                # .cone 打包/消费/单态化缓存
```

当前 runner 约定：fixture 的一级目录名就是 phase（例如 `parse/`、`resolve/`、`typecheck/`）。未实现的 phase 也必须给出清晰诊断，便于先写 fixture 再补实现。

- [x] phase 路由：按 `tests/fixtures/<phase>/**` 目录名决定执行阶段（未实现 phase 返回“未实现”诊断）

默认每个 fixture 采用“单文件 + 注释指令”的形式（类似 LLVM lit 或 Rust compiletest）。
对于需要跨文件验证的规则（例如 `private` 可见性、跨文件引用、sealed 继承等），额外提供 `<phase>_multi/<case>/`：
- `<case>/` 目录内包含 2+ 个 `.scoop` 文件
- runner 先把同一 case 的所有文件作为一个编译单元构建索引，再逐文件执行 `<phase>` 并按各自文件头注释断言 pass/fail

- [x] `// EXPECT: pass|fail`
- [x] `// EXPECT-ERROR: <substring>`（当前为子串匹配；后续可升级为 regex）
- [x] `// EXPECT-AST: <file>`（parse fixtures：AST snapshot / golden）
- [x] `// RUN-STDOUT: <file>`
- [x] `// RUN-STDERR: <file>`
- [x] `// EXPECT-EXIT: <code>`
- [x] `// TIMEOUT: <ms>`
- [x] `// ARGS: ...`

### 10.2 诊断（compile-fail）的 golden 规范

- [x] 诊断必须包含：错误码（稳定 ID）、主消息、关联 span（行列）、可选 note/help（当前 lexer/parser 已提供 code + label span）
- [x] fixtures 断言策略：支持匹配“错误码 + 错误位置（行列）+ 关键片段”（先用文件头注释指令实现；未来可再升级为独立 `.golden`）

推荐模板（compile-fail fixture 文件头）：

```
// EXPECT: fail
// EXPECT-ERROR: <关键片段>
// EXPECT-ERROR-CODE: <稳定错误码>
// EXPECT-ERROR-AT: <line>:<col>
```

### 10.3 spec doctest（强烈建议）

- [x] 工具：从 `SCOOP_FULL_SPEC.md` 抽取包含 `// FIXTURE:` 的 fenced code block，生成 `tests/fixtures/spec_doctest/*`
- [x] 约定：代码块通过注释标记其期望（`// EXPECT:` / `// EXPECT-ERROR:`），`// FIXTURE:` 指定输出路径
- [x] 在 CI 中强制：`cargo run -p scoop_tools -- spec-fixtures check` + `cargo run -p scoop -- test`
- [x] 本地修复：`cargo run -p scoop_tools -- spec-fixtures check --fix`（只写回受影响文件）

### 10.4 运行期 fixtures（run-pass）

- [x] T0106a：fixtures runner 识别 `codegen/`（或 `run-pass/`）phase，并实现 stdout golden 比对（对比逻辑可单测独立验证）
- [ ] T0106b：接入 `scoop run`（T0807）真正“编译 + 运行” fixture，并断言 stdout（stderr 后续补齐）
- [ ] 支持超时、退出码断言（fixtures 指令：`TIMEOUT`/`EXPECT-EXIT`）
- [x] T0111a：支持 stderr golden 断言（对比逻辑 + 稳定诊断，可单测）
- [ ] T0111b：新增 run-pass fixtures 覆盖 stderr（需要 T0106b2 真正执行）
- [ ] 对 GC 压测类测试，支持 `SCOOP_GC_STRESS=1` 之类的环境变量切换（让 CI 可控）

### 10.5 Fuzz/性质测试（可选但很有价值）

- [x] lexer/parser fuzz（避免崩溃，保证错误恢复）— 实现为 `crates/scoopc/tests/fuzz.rs`：adversarial + deterministic random + structured fragment 三类测试（5000+ iterations）
- [ ] IR lowering fuzz（随机小 AST → 不崩溃）
- [ ] GC 压测（随机分配/释放/跨线程）

### 10.6 覆盖矩阵（建议维护）

- [x] `cargo run -p scoop_tools -- fixtures-matrix check`：按 phase 目录扫描 fixtures，报告缺少 pass 或 fail 的缺口（见 `tools/scoop_tools/src/fixtures_matrix/`）
- [ ] 后续可细化为按 spec 章节粒度检查（当前为 phase 粒度）

为每个 spec 章节至少准备：
- 1 个 compile-pass
- 1 个 compile-fail（覆盖常见误用）
- 若涉及运行期语义（GC/effect/async），再加 1 个 run-pass

---

## 11. `@NoGC` / `@Unsafe` / `@Extern`（阶段 9：实现“系统编程通道”）

- [ ] 通用注解系统（spec §15）：
  - [x] 解析注解声明（`annotation class`）
  - [ ] 解析注解使用（`@Name(...)`）
  - 注解 target（函数/类型/字段/参数/表达式块等）与合法性检查
  - 注解仅编译期存在（不进运行时布局）
  - 内建注解：`@Intrinsic/@Extern/@Inline/@Deprecated`（具体名字按 sysroot 定义）
- [ ] `@Unsafe`：
  - 函数级与块级 `@Unsafe { ... }`
  - 非 unsafe context 禁止：指针运算/unsafe 原语/调用 `@Unsafe` 函数/调用 `@Extern`
- [ ] `Ptr<T>` / `UIntPtr` 与指针整数转换（spec §15.9.4 / runtime §4~§5）
  - `UIntPtr` 仅为 `UInt` 的别名（类型本身不 unsafe）
  - 指针 ↔ 整数转换必须在 unsafe context，且通过 sysroot intrinsics（不通过 `as/as?`）
  - `Ptr<T>` 的 `T` 必须是 GC-free value type（不允许直接/间接包含 GC ref）
- [ ] `@NoGC`：
  - 禁止 GC 堆分配；只能调用 `@NoGC` 与 `@Extern`
  - 编译器证明不了“无分配”就必须报错（保守）
- [ ] `@Extern`：
  - 默认视为 `@NoGC`
  - 是否默认 `@Unsafe`：建议 **调用点要求 unsafe context**（更符合“外部世界不可信”）
- [ ] 注解系统补齐：
  - 内建注解：`@TailRec/@AllowIntrinsic/@Suppress/@CLayout/@Target/@Retention`
  - `AnnotationTarget` enum 与 target 合法性检查
  - meta-annotations 与 `.cone` 导出策略
- [ ] 注解参数补齐：常量表达式、数组/enum/class-literal 等非纯字面量参数的解析与合法性检查
- [ ] 注解 use-site targets：`field:/property:/param:/get:/set:/file:`（spec §15.3）
- [ ] namespaced annotations：`@Namespace.Annotation(...)`（spec §15.4）
- [ ] 后期 runtime / std 阶段的 intrinsic 预算规则：
  - 默认不再新增 intrinsic，优先用纯 Scoop 库补 runtime/stdlib 缺口
  - 若审计证明缺少底层 primitive，则单独立项增加最小 intrinsic，并与上层库任务拆开推进

fixtures：
- `tests/fixtures/unsafe_nogc/*` 覆盖所有违规路径（必须 compile-fail）

---

## 12. Cone（包/稳定 IR/分发）（阶段 10：工程化分发）

### 12.1 Scoop IR（scoopir）

- [ ] 定义一个稳定的 IR schema（建议独立文档 + 版本号）
- [ ] `api.scoopir`：仅含 public API（用于类型检查与 IDE）
- [ ] `generics.scoopir`：含泛型/const fun 的可执行 IR（供下游单态化）

### 12.2 `.cone` 归档格式

- [ ] archive（可先用 zip/tar，后续换自定义格式）
- [ ] 读写 `Cone.toml`、依赖解析、目标平台信息
- [ ] 预编译实例（pre-specialize）：cache key 与选择规则
- [ ] pre-specialize：补齐类型实例（不只函数实例）的打包与消费规则

fixtures：
- `tests/fixtures/cone/*`：
  - 打包后消费编译的 API 兼容性
  - IR 版本兼容（旧版本可读）

---

## 13. 编译期执行与反射（阶段 11：comptime）

- [x] Parser 语法：支持 `const` 修饰符、`comptime { ... }` / `comptime if` / `comptime for`、以及 splice `value.[field]`（见 TODO T0246）
- [ ] `const fun` 解释器（先支持 value types/纯计算；`String` 作为特例允许——具有值语义）
- [ ] `const fun` 静态检查：禁止闭包/lambda（捕获环境导致 const 语义难以验证）
- [ ] `comptime { ... }` 执行上下文（限制 effect：必须 `Pure`）
- [ ] 反射 intrinsics：`fieldsOf/nameOf/sizeOf` 等（先从 sysroot 声明开始）
- [ ] 反射 intrinsics 补齐：`variantsOf/alignOf/superTypesOf/annotationsOf/paramsOf`（spec §6.4 / §15.6）
- [ ] 编译期元数据补齐：`VariantMeta/ParamMeta/FunctionMeta/AnnotationMeta/AnnotationArgMeta`（spec §6.4 / §15.6）
- [ ] 编译期注解访问：复杂参数表达式 / 数组 / enum / class-literal 的归一化与读取（不只字面量）
- [ ] `trimIndent()`：编译期求值 + 运行期 fallback（spec §8.4）
- [ ] sysroot/stdlib：补齐 scope functions（§11）；delegated property API surface 已在 sysroot 落地（spec §10.4）

fixtures：
- `tests/fixtures/comptime/*`：覆盖常量折叠、生成代码（若支持）、错误诊断

---

## 14. Kotlin 语义兼容项（阶段 11+：按需逐步补齐）

spec §16 指出以下功能“遵循 Kotlin 语义”，实现上建议按需求拆分落地，每一项都要配 fixtures：

- [ ] 操作符重载（operator overloading）
  - 解析 `a + b` → 解析/绑定到 `plus`/`minus` 等约定方法（按 Kotlin 规则）
  - 补齐位运算与移位：`and/or/xor/inv/shl/shr`（Appendix B.8）
  - 运行期与值类型/引用类型的 codegen 覆盖
- [ ] `object` 与 companion object（如需要）
- [x] `typealias`（纯类型层语法糖；当前仅非泛型别名 + 展开 + 循环检测，T0446）
- [ ] Ranges/progressions 与 `for` 迭代协议
- [ ] 基础集合与常用操作（`map/filter/fold` 等更多是库工作，但需要类型推断与泛型单态化支撑）
- [ ] import alias：`import foo.bar.Baz as Qux`（Appendix B.7）
- [ ] `object` / `companion object`：从 parse/resolve 扩展到 typecheck/codegen/初始化语义（Appendix B.9）
- [x] 类初始化语义：property initializer、`init` blocks、secondary constructors、初始化顺序（Appendix B.2.2）（T0448：最小落地）
- [ ] 标准 delegated properties：`lazy`/`observable`/`vetoable`/map-backed（spec §10.4；运行期语义待补齐）
- [ ] Kotlin runtime gap closure（when applicable）：
  - 先审计 Scoop core runtime / stdlib 与 Kotlin runtime 语义缺口
  - 优先用纯 Scoop 补齐；只在审计证明无法表达时回流到 §11 的最小 intrinsic 通道
- [ ] 全量 `std` 库工程：
  - 目标能力与 Rust `std` 同量级、可比较，但不要求 API 一致
  - 建议分层：`core` / `alloc` / `std` / 平台适配层
  - 覆盖 collections、text/regex、iterators、io/fs/path/process/env、time、sync/thread/channels、net、async adapters、test/support utilities 等
- [ ] Kotlin 风格重载决议兼容：
  - most specific candidate 规则
  - 默认参数 / 命名参数 / trailing lambda 与重载集合的交互
  - 扩展函数、成员函数、构造函数之间的优先级与歧义处理
- [ ] 默认参数：中间参数省略与命名参数联动
- [ ] 多 trailing lambda：语法、expected type 与重载决议联动
- [ ] varargs spread：集合/序列到 vararg 的桥接规则
- [ ] delegated properties：`lazy`/`observable`/`vetoable` 的线程安全语义与平台 policy
- [ ] 类初始化兼容：复杂继承链与 effect 细节

fixtures：
- `tests/fixtures/language/*` 下为每个特性提供 compile-pass/compile-fail + 必要的 run-pass

---

## 15. GC 迁移到 Scoop（阶段 12：自举路线）

### 15.1 迁移前置条件

- [ ] `@NoGC` 可写且可验证（GC 核心不应触发 GC 分配）
- [ ] `@Unsafe` + 指针/原子/线程 API 完备
- [ ] FFI 能调用 OS/clang runtime 的最低集合（mmap/VirtualAlloc、thread local、mutex 等）

### 15.2 迁移策略（建议渐进）

1) **在 Scoop 中实现 GC 算法库（仍由 C runtime 驱动）**
   - C runtime 负责“触发 GC/暂停世界/枚举线程/提供原子与 OS API”
   - Scoop 代码负责“标记/扫描/整理”的纯算法部分

2) **把类型描述与扫描逻辑迁移到 Scoop**
   - type descriptor 结构体改由 Scoop 定义（C 只保留 ABI glue）

3) **最终替换 C GC**
   - C runtime 仅保留极薄的启动层，甚至可以被 Scoop runtime 取代

4) **Scoop GC 进入多线程阶段**
   - 线程注册、stop-the-world / 协调协议、跨线程 root 扫描、线程本地分配策略都由 Scoop GC 接管
   - 单线程 mark-sweep 只作为 baseline；多线程正确性与可回归性必须先固定

5) **引入更高性能 GC 变体（如 Immix）**
   - 在 baseline GC 可用后，引入 Immix 或类似 line/block allocator 作为改进路径
   - 保持与 baseline GC 共存，避免把算法升级和 runtime 自举耦死

6) **GC 后端可替换 / 可编译期选择**
   - 编译期可选择 mark-sweep、Immix、embedded/minimal、WASM GC adapter 等不同实现
   - 通过稳定的 GC runtime ABI / trait 边界隔离上层 runtime 与具体 GC 算法

7) **runtime 去 C 化**
   - 逐步把启动、effect runtime、GC runtime、线程/调度 glue 从 C 迁移到 Scoop
   - 允许继续直接调用 libc / OS ABI，但 runtime 核心逻辑不再依赖 C
   - 对 non-resuming effect / unwind 路径，可评估引入 `libunwind` 作为底层依赖，而不是继续依赖 C runtime 自带异常/展开机制

fixtures：
- 运行期 GC fixtures 必须在“C GC”和“Scoop GC”两套实现下都能跑（同一套测试，不同 runtime 实现）。
- 迁移后，运行期 fixtures 应至少在两类 GC backend 下可回归：baseline GC 与高性能 GC（如 Immix）；若提供 WASM/embedded 适配器，还应维护 capability matrix 与分层禁用测试。

---

## 16. 风险点与建议的优先级

- **高风险/高复杂度**：effect（尤其 `, k ->` + 跨线程）、GC（移动/压缩 + pin/unpin）、类型推断（subtyping + effect rows）
- **建议优先级**：
  1) 先把 fixtures 与诊断体系立住（否则后期难以迭代）
  2) 先做“语义正确”的实现（优化后置）
  3) effect 先 `Raise`/`->`，再扩展 `-> resume`、`, k ->`
  4) GC 先非移动，再移动（pin/unpin 在移动 GC 上才真正有意义）
