# M2 设计：值类型基础

版本：0.1（草案）

对应 `docs/ROADMAP.md` 的 M2。目标：在 M1 主线上加入值类型基础——struct / tuple、字段访问、`val` / `var`、if / while、结构相等，以及支撑它们的最小表达式与类型能力（Int、Boolean、比较与算术）。

## 1. 语言子集

```
struct Point(val x: Int, val y: Int)

fun main() {
    val p = Point(1, 2)            // struct 构造（位置参数，全字段必填）
    var q = (1, "hello")           // tuple 字面量，类型 (Int, String)
    val x = p.x                    // 字段访问；tuple 用 q._1 位置访问
    var n = 0
    while (n < 3) {
        n = n + 1                  // var 重绑定
    }
    if (p == Point(1, 2) && x > 0) {
        println("ok")
    } else {
        println("ng")
    }
    val u = ()                     // Unit 字面量
    val s = (42,)                  // 1 元 tuple，类型 (Int,)
}
```

新增（相对 M1）：

- **声明**：`struct` 声明（仅主构造函数、全 `val`、无默认值、无成员函数）；局部 `val` / `var` 声明（必须有初始化器）。
- **类型**：`Int`（本目标平台 = i64，见 5.3）、`Boolean`、struct 类型、tuple 类型 `(T1, T2, ...)`、`Unit`（`Unit` 与 `()` 两种写法）。
- **表达式**：Int / Boolean 字面量；算术 `+ - * /`（Int）；比较 `< <= > >=`（Int）与 `== !=`（见 2.4）；布尔 `&& || !`；括号表达式；一元负号 `-`。
- **语句**：`val`/`var` 声明、`var` 赋值、表达式语句、`if` / `if-else`、`while`、嵌套 block。
- **内建输出扩展**：`print` / `println` 接受 `String` / `Int` / `Boolean`（见 5.2）。

子集之外的构造继续报正式的"not supported yet (milestone M2)"诊断。

明确不在 M2（引用 spec 章节，供后续里程碑认领）：

- struct 字段默认值、命名参数、次构造函数、成员函数（spec 4.1）——随函数默认参数里程碑；
- 副本更新 `s.{ f: 1 }`（spec 4.5）、解构（spec 4.6）——M4；
- `enum`、`when` 模式——M4；循环的 `break`/`continue`/`for`——后续；
- `String` 插值——M6。

## 2. 各 stage 设计

### 2.1 AST / parser（`scoop-ast` / `scoop-parser`）

- AST 新增：`StructDecl`、`FieldDecl`、`ValDecl`（含 `mutable: bool`）、`Assign`、`If`（`else` 分支可选——用 `Option<Box<Block>>` 是合法的：else 本来就可无）、`While`、`BinaryOp` / `UnaryOp`（op 为 enum）、`IntLiteral` / `BoolLiteral`、`FieldAccess`（接收者 + 字段名或 tuple 下标）、`TupleLiteral`、`StructInit`（类型名 + 位置实参）、`ParenExpr` 在 parser 阶段直接消解（不产生节点）。
- parser：表达式改用 precedence climbing（优先级：`||` < `&&` < `== !=` < 比较 < 加减 < 乘除 < 一元 < 后缀 `.`）；`* /` 与块注释 `/*` 的歧义在 lexer 无（`/` 后非 `*`/`/` 即运算符）。
- tuple 与括号的消歧（spec 4.3）：`()` 是 Unit 字面量、`(e)` 是括号表达式、`(e,)` 是 1 元 tuple、`(e1, e2, ...)` 是多元 tuple——`()` 与 `(e,)` 直接判定，其余解析到第一个 `,` 或 `)` 时判定。

### 2.2 HIR（`scoop-hir` / `scoop-hir-lower`）

- `Type` 扩展：`Int`、`Boolean`、`Struct(StructId)`、`Tuple(Vec<TypeId>)`。新增 `StructId`（newtype arena 索引）与 `StructDecl { name, fields: Vec<Field> }`，`Field { name, ty: TypeId }`。
- **作用域**：block 级词法作用域栈；`val`/`var` 声明即初始化（M2 不做延迟初始化分析）；重复声明、未定义引用、对 `val` 赋值、赋值类型不匹配均为诊断。
- **类型检查**要点：
  - 二元算术/比较的操作数类型必须匹配（`Int` 算术得 `Int`，比较得 `Boolean`）；
  - `==` / `!=`：两侧类型相同，且类型可比较——M2 全部类型（值类型 + String）都可结构相等；
  - `if` / `while` 条件必须 `Boolean`；
  - struct 构造实参个数/类型逐一匹配字段；字段访问的类型取自字段声明；
  - tuple 下标 `._n` 从 1 开始，越界是诊断。
- 表达式类型依旧结构上必填（`Expr::ty`）。

### 2.3 MIR（`scoop-mir` / `scoop-mir-lower`）

- **结构相等展开**：`==` 作用于 struct / tuple 时，在 MIR 展开为逐字段/逐元素的比较表达式树（字段是 String 时调 `scoop_rt_string_eq`，见 3.2；嵌套 struct/tuple 递归展开）。String 的 `==` 直接映射为 runtime 调用。不在 codegen 生成 per-type equality 函数——M2 用内联展开，函数化留待需要时。
- **控制流**：MIR 保持结构化（`If` / `While` 语句节点），basic block 的生成留给 LIR。
- **String `+`**：映射为 runtime 调用 `scoop_rt_string_concat`（见 3.2）。
- 局部变量在 MIR 仍是命名变量（`LocalId`，newtype 索引），不在这里做 SSA。

### 2.4 LIR（`scoop-lir` / `scoop-lir-lower`）

- LIR 从直线型升级为 **basic block + terminator**：`Terminator = Br | CondBr | Return`；指令新增 `Alloca`（局部变量与 struct/tuple 值的栈槽）、`Load` / `Store`、`ExtractValue`（字段/元素访问）、`BinOp` / `UnaryOp`、`Call`。
- **局部变量一律 alloca**（load 使用、store 赋值），依赖 LLVM mem2reg 优化；不做手写 SSA/phi。
- **布局计算正式落地**：struct/tuple 的布局（字段按声明顺序、自然对齐）在 LIR 计算并写入 LIR meta（`ref_field_offsets` 必须正确——struct 含 String 字段时它是引用，M9 的 GC 依赖该信息）；`Int`/`Boolean`/引用（String、struct 装箱暂无）布局一并列出。
- struct/tuple 值在 LIR 用**聚合值**表示（LLVM literal struct），Alloca/Load/Store 整体搬运。

### 2.5 codegen（`scoop-codegen`）

- LIR → LLVM IR 机械翻译扩展：alloca/load/store/extractvalue、算术与比较指令（`add/sub/mul/sdiv`、`icmp`）、`br`/条件 `br`、phi 由 mem2reg 负责（codegen 不生成 phi）。
- struct/tuple 聚合 ↔ LLVM `{ ... }` literal struct 类型，按 LIR meta 的布局。
- 调用 runtime 新函数（`scoop_rt_string_concat` / `scoop_rt_string_eq`）按符号直调。

## 3. runtime 新增

C11，沿用 always-leak：

- `const ScoopString *scoop_rt_string_concat(const ScoopString *a, const ScoopString *b)`：`scoop_rt_alloc` 分配 `header + len + a.len + b.len`，两次 `memcpy`（即 runtime spec 14.4 示例的 always-leak 版）；
- `bool scoop_rt_string_eq(const ScoopString *a, const ScoopString *b)`：长度 + `memcmp`；
- `void scoop_rt_print_int(int64_t)` / `scoop_rt_print_boolean(bool)`（及 println 变体）：支撑内建输出扩展。

## 4. 测试计划

- **独立 fixture**（`tests/fixtures/m2-values/`）：struct 构造与字段访问、tuple 与 `._n`、`val`/`var` 与重绑定、if/else、while、结构相等（struct/tuple/String/嵌套组合）、String `+`。
- **组合 fixture**：struct 含 tuple 字段、tuple 含 struct 元素、控制流嵌套中的 var 更新（检验 alloca/mem2reg 路径）、含 String 字段的 struct 比较（检验 runtime eq 调用）。
- **negative fixture**：对 `val` 赋值、类型不匹配（赋值/算术/比较/if 条件）、未知字段、tuple 下标越界、struct 实参个数/类型错误、未定义变量、重复声明。
- golden dump 与运行快照沿用 M1 harness（合并快照：四级 dump + stdout / 诊断）。

## 5. 临时决策（及退役里程碑）

1. **Int = i64**：本开发目标平台为 64 位；spec 11.2 要求编译器明确 Int 与 Int32/Int64 的对应关系——M2 固定 i64，平台抽象在需要第二目标时引入。
2. **内建 print/println 扩展为 String/Int/Boolean 重载（已退役）**：M7 已迁移为 core 普通重载；底层输出/转换 intrinsic 到 M11 再由 FFI 取代。
3. **局部变量全 alloca + mem2reg**：性能足够且最简单；如未来证明是瓶颈再手写 SSA 构造。
4. **结构相等内联展开**：不生成 per-type equality 函数；递归深度失控或代码膨胀时再函数化。
5. **无错误恢复**、**诊断 fail-fast 于 parser、收集于 HIR**：沿用 M1。

## 6. 明确不做

见第 1 章末尾清单；另外不做：`else if` 链的语法糖（用嵌套 `else { if }`，parser 不特殊处理）、无符号整数与其余定宽类型、溢出检查（M2 算术为 LLVM 默认回绕语义，溢出策略待 spec 补充）。
