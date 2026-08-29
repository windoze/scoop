# M13 设计：字符串插值

版本：0.1（草案）

对应 `docs/ROADMAP.md` 的 M13。目标：f-string 插值（`f"..."` 单行与 `f"""..."""` 多行）与到 `StringBuilder` 链式调用的脱糖（spec 第 6 章）。

## 0. 范围说明

- spec 11.6 的 `StringBuilder` 是 `class`，方法体系属 M7。M6 的脱糖**语义目标**是 StringBuilder 的 new → add × n → build 链（spec 6.2），实现上由 **runtime 的 C 版 StringBuilder** 承载（`scoop_rt_sb_*` 函数族）；core 库的 `class StringBuilder` 声明随 M7 落地，runtime 后备就此就位（见 5.1）。
- spec 的 `add<T>` 依赖 `toString()` 分发（M7 的 `Any`/vtable）。M6 的 `${expr}` 只接受 **String / Int / Boolean**（与 print/println 的现行约束一致，见 5.2）。

## 1. 语言子集

```
fun main() {
    val n = 41
    println(f"n is ${n + 1}")          // n is 42
    val p = Point(1, 2)
    println(f"p.x = ${p.x}, sum = ${p.x + p.y}")
    val s = f"""line one
line two and n is ${n}"""
    println(s)
    println(f"${f"nested ${n}"}")      // 嵌套 f-string 也合法（表达式任意）
}
```

新增（相对 M5）：

- **单行 f-string** `f"..."`：与 M1 普通字符串同一套转义规则；`${expr}` 是插值孔（任意表达式，含调用、运算、嵌套 f-string）。
- **多行 f-string** `f"""..."""`：raw 规则（保留换行、**不处理转义**，`\n` 是两个字面字符），插值孔照常。
- **`$` 规则**（spec 6.1）：`${` 开启插值孔；未跟 `{` 的 `$` 是诊断（"use `${...}` for interpolation"）；字面 `$` 用转义 `\$`（单行）——多行 raw 模式下 `$` 后只要不是 `{` 就是诊断，想要字面 `$` 写 `\${`? 不行（raw 无转义）——**多行模式下 `$` 后非 `{` 一律诊断**的决定记录在 5.3（spec 未覆盖，待 spec 补充）。
- 插值孔内允许 `{`/`}` 嵌套（括号匹配，见 2.1）。

子集外诊断：`${expr}` 类型不是 String/Int/Boolean → "interpolation of type X is not supported yet (milestone M13)"；未终结的 f-string → 诊断。

## 2. 各 stage 设计

### 2.1 AST / lexer / parser

- **AST**：`Expr::FString { parts: Vec<FStringPart>, multiline: bool, span }`；`FStringPart = Literal(String) | Hole(Expr)`。Literal 段已完成转义（单行）或原文（多行）；空 Literal 段不保留。
- **lexer**：`f"` / `f"""` 进入 f-string 模式，产出结构化 token 流（字面段 token 与孔 token 交替）：
  - 单行：沿用普通字符串的扫描，遇 `${` 切出孔；
  - 孔内回到普通 token 扫描，**记录花括号深度**：`${` 后深度 1，`{` +1、`}` -1，深度归 0 的 `}` 结束孔；
  - 多行：扫描到 `"""` 结束，不处理转义；
  - `$` 后非 `{` → 诊断（span 在 `$`）。
- **parser**：把 token 流组装成 `Expr::FString`（原子表达式，可参与任意表达式上下文）。

### 2.2 HIR

- **类型检查**：每个 Hole 的类型必须 String/Int/Boolean（否则诊断）；`Expr::FString` 的类型是 String。
- **脱糖**（HIR 内建节点，形状对应 spec 6.2 的 StringBuilder 链）：
  - `ExprKind::SbNew`、`SbAdd { builder, value }`（按值类型分派 add_string/add_int/add_boolean）、`SbBuild(builder)`；
  - f-string 无孔 → 直接 StringLiteral（不产生 StringBuilder 调用，与 spec 6.2"普通字符串不脱糖"一致的精神——f 前缀但无孔也按普通字符串处理，见 5.4）；
  - 求值顺序：字面段与孔从左到右，孔只求值一次（孔是纯表达式直接嵌入链中，无隐藏局部需求——链是直线求值）。

### 2.3 MIR / LIR

- MIR：`SbNew`/`SbAdd`/`SbBuild` 映射为 runtime 调用（`RuntimeFn` 新增 `SbNew/SbAddString/SbAddInt/SbAddBoolean/SbBuild` 变体及符号）；
- LIR 无新指令（普通 Call）；`SbAdd*` 的参数类型决定 runtime 函数选择（MIR 按 HIR 类型分派，LIR 透传）。

### 2.4 codegen

无新内容（runtime 函数直调；签名 `ptr()`、`void(ptr, ptr)`、`void(ptr, i64)`、`void(ptr, i1)`、`ptr(ptr)`）。

## 3. runtime 新增

C 版 StringBuilder（always-leak 风格）：

```
struct ScoopSb { size_t len, cap; char *buf; };
```

- `ScoopSb *scoop_rt_sb_new(void)`：小初始容量（如 64B）；
- `void scoop_rt_sb_add_string(ScoopSb *, const ScoopString *)` / `_add_int(ScoopSb *, int64_t)` / `_add_boolean(ScoopSb *, bool)`：grow-and-append（int/bool 先格式化进临时缓冲）；
- `const ScoopString *scoop_rt_sb_build(ScoopSb *)`：按 `len` 分配 ScoopString、memcpy、返回（builder 本身泄漏，符合 always-leak）。

## 4. 测试计划

- **独立 fixture**（`tests/fixtures/m6-fstring/`）：单行插值（孔含运算/函数调用/字段访问）；多行插值（换行保留、raw 不转义）；相邻孔与孔在首尾；`\$` 转义；无孔 f-string（退化为普通字符串）；嵌套 f-string；
- **组合 fixture**：孔内是 `?:`、`!!`、数组下标、结构相等比较（Boolean 插值）；多行字符串再做 println；
- **negative fixture**：裸 `$name`；插值类型不支持（struct/Option/tuple）；未终结单行 f-string；未终结 `f"""`；多行中 `$` 后非 `{`；
- golden dump 锁定脱糖形状（SbNew/SbAdd/SbBuild 链与按类型分派的 add 选择）。

## 5. 临时决策（及退役里程碑）

1. **StringBuilder 由 runtime C 实现承载**：M7 在 core 库落地 `class StringBuilder` 声明与方法（spec 11.6 形态），runtime 后备不变；脱糖形状（new/add/build）即为最终形态。
2. **插值只支持 String/Int/Boolean**：`add<T>` 的 `toString()` 分发随 M7（Any/vtable）落地后放宽到任意类型。
3. **多行模式下 `$` 后非 `{` 一律诊断**：raw 字符串无转义，无法写字面 `$`——spec 未覆盖此角落，待 spec 补充后对齐（届时可能引入 `\$` 于 raw 或别的规则）。
4. **无孔 f-string 按普通字符串处理**（不产生 StringBuilder 调用）：行为与 spec 6.2 的字面脱糖等价（`StringBuilder().build()` 也是空串拼接），但零开销；如 spec 要求严格字面脱糖再改。

## 6. 明确不做

- `add<T>` 对任意类型（M7）；core 库的 `class StringBuilder` 声明（M7）；格式化选项（spec 没有，不做）；`f` 前缀之外的其他字符串前缀。
