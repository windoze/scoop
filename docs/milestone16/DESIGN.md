# M16 设计：字符串插值

版本：0.2（草案）

对应 `docs/ROADMAP.md` 的 M16。目标：实现 `f"..."` / `f"""..."""`、`${expr}` 插值，以及 spec 6.2 规定的 `StringBuilder` 脱糖。M16 位于 M14 泛型上界与 `ToString` 接口化、M15 moving compaction之后，不再保留早期“仅 String / Int / Boolean”或 always-leak builder 的过渡实现。

## 0. 范围与关键决策

- 每个插值孔接受任意实现 `ToString` 的类型；不按具体类型在 HIR 写死 int/bool/string 分支；
- HIR 脱糖为真实的 core `StringBuilder` 构造和 `add` / `build` 调用，普通重载、generic bound 与单态化负责定型；MIR/LIR 不保留 f-string 专用节点；
- `StringBuilder` 是 managed core class。其公开 API 与 spec 11.6 一致，内部 storage 由编译器验证的 core contract 和 runtime 后备实现提供；builder 及 backing buffer 都受 M9 GC 管理，不泄漏 native malloc buffer；
- 孔、字面段与嵌套 f-string 严格从左到右求值，每个孔只求值一次；
- M16 不增加格式化 mini-language、`$name` 简写或其他字符串前缀。

## 1. 语言子集

```
struct Point(val x: Int, val y: Int)

fun main() {
    val n = 41
    println(f"n is ${n + 1}")

    val p = Point(20, 22)
    println(f"point = ${p}, sum = ${p.x + p.y}")

    val lines = f"""first line
second line: ${n}
nested: ${f"value=${p.x}"}"""
    println(lines)
}
```

- `f"..."` 使用普通单行字符串的转义规则；
- `f"""..."""` 使用 raw multiline 规则并保留换行；
- `${expr}` 中可以出现任意表达式，包括嵌套 f-string、lambda、调用和带花括号的控制结构；
- `${expr}` 的静态类型必须满足 `ToString` bound。失败时给普通 bounded-method 诊断并把 span 指向该孔，不使用“milestone 暂不支持类型”的临时错误；
- 无孔 f-string 与相同内容的普通 `String` 字面量等价，HIR 直接常量化，不构造 builder；
- `${` 开启插值孔；`$identifier` 明确诊断并提示 `${identifier}`；其他未跟 `{` 或 identifier-start 的 `$` 是普通字面字符。单行中的 `\$` 也产生字面 `$`，multiline raw 字符串不处理反斜杠转义。

## 2. AST / lexer / parser

### 2.1 token 与 AST

lexer 使用可嵌套的 f-string mode，产出字面段与普通表达式 token：

- 单行模式复用普通字符串的 escape scanner，遇 `${` 暂停字面扫描；
- multiline 模式扫描到匹配的 `"""`，不处理 escape；
- 孔内切回普通 lexer，并维护 `{` / `}` 深度；深度从 1 回到 0 的 `}` 结束当前孔；字符串、注释及嵌套 f-string 内的花括号由各自 lexer mode 消费，不能错误改变外层深度；
- EOF、单行换行、未终结 escape、空孔和未闭合孔都产生覆盖最小相关范围的诊断，并恢复到当前字符串的结束边界继续收集错误。

AST 使用：

```
Expr::FString {
    parts: Vec<FStringPart>,
    multiline: bool,
    span: Span,
}

FStringPart = Literal { value, span } | Hole { expression, span }
```

literal 已保存解码后的 UTF-8 内容；空 literal 段不保留。parser 把整个 f-string 当作原子表达式，因此可以出现在调用实参、lambda body、另一插值孔或任意更大表达式中。

### 2.2 `$` 消歧

- `${`：开始 hole；
- `$` 后是 identifier-start：报“string interpolation requires `${...}`”，但继续把标识符作为字面文本恢复，避免级联；
- 其他 `$`：字面字符；
- 单行 `\$`：由普通 escape scanner解码为字面字符；multiline 中 `\` 没有 escape 含义。

该规则必须同步写入 spec 6.1；lexer 不允许用“所有 `$` 都报错”的临时规则让 raw multiline 无法表达货币符号。

## 3. HIR 脱糖与定型

HIR 启动时验证唯一的 core `StringBuilder` contract，并保存类型化 `StringBuilderCore`：class id、零参数 constructor、`add(String)`、`add<T : ToString>(T)` 与 `build()` 的 function ids。验证失败是 core 配置错误；后续 stage 不按 FQN 或方法名 fallback。

对含孔 f-string，HIR 生成等价的隐藏 block：

```
val builder = StringBuilder()
builder.add(literal0)
val hole0 = expression0
builder.add(hole0)
builder.add(literal1)
...
builder.build()
```

- hidden local 与每次调用都有真实的类型化 id 和 source origin；
- 空 literal 不发 `add`；非空 literal 走 `add(String)`；孔走 `add<T : ToString>`，由 M14 的 bound resolution和单态化选择实现；
- hole expression 先完整求值到 hidden local，再执行 `add`，保证异常、挂起调用与副作用都只发生一次且顺序可见；
- f-string 出现在 suspend body 时，hole 可以挂起，脱糖后的 builder local 与此前结果按 M10 规则进入 frame；
- 整个表达式类型固定为 `String`。无孔时直接生成 `StringLiteral`；
- HIR output 不含 `FString` / `SbAdd` 等专用节点，只含普通 block、local、构造与已决议 call。

重载决议不允许被用户同名 `StringBuilder` 或扩展 `add` 劫持：脱糖目标来自已验证的 core typed ids；用户显式写 `StringBuilder` 时仍走普通名称解析。

## 4. core 与 runtime 后备

### 4.1 public core 形态

```
class StringBuilder {
    fun add(part: String): StringBuilder
    fun <T : ToString> add(part: T): StringBuilder
    fun build(): String
}
```

generic overload 是普通 Scoop 源码：调用 `part.toString()` 后转发给 `add(String)`。constructor、`add(String)` 与 `build()` 通过 compiler-validated intrinsic contract 落到 runtime 后备；不把每个 `T` 的字符串化逻辑复制进 C runtime。

### 4.2 managed layout

runtime 后备使用两个 managed concrete 类型：

```
StringBuilderObject {
    header,
    length,
    capacity,
    buffer: ByteBufferRef,
}

StringBuilderByteBuffer {
    header,
    capacity,
    bytes[],
}
```

- builder TypeDescriptor 扫描 `buffer`；byte buffer没有出站引用，使用空扫描描述；
- constructor 分配 builder 与小的初始 byte buffer；grow 时分配更大 managed buffer、复制现有字节并以普通 heap store + card barrier更新引用；
- `add(String)` 读取 String 的 UTF-8 bytes并追加；容量按几何倍增，单次 append 摊销 O(n)；
- `build()` 分配精确长度的 immutable `String` 并复制 bytes。调用后 builder 仍可继续 add/build，已有 String 快照不受后续修改；
- 所有可能分配的后备调用都是 managed call/statepoint，可以触发 GC并返回 relocated 引用；不得标为 NoGC、leaf或 nounwind；
- 不使用永不释放的 `malloc` buffer，不依赖 finalizer，也不把 raw native pointer伪装成 GC ref。

runtime API 名称与对象内偏移只存在于 compiler/runtime 的 typed contract中，不进入 Scoop public API。MIR 可用 `RuntimeFn::{StringBuilderNew, StringBuilderAddString, StringBuilderBuild}` 表示三个已登记的后备目标；LIR 仍只看到普通 managed call。

## 5. MIR / LIR / codegen

- MIR 只接收普通构造、generic实例和 call；`add<T>` 单态化 body按具体 `T` 调用其 bounded `toString` target，再调 `add(String)`；
- hidden builder/hole locals参与现有 CFG、异常 unwind与 coroutine liveness，不设置特殊 cleanup；不可达 builder由 GC自然回收；
- LIR/codegen复用普通 managed对象、TypeDescriptor、HeapLoad/HeapStore、statepoint和runtime call路径；
- builder与byte buffer布局/扫描描述进入 LIR golden；grow 后的 buffer store必须保留写屏障；
- 后续可以把短常量拼接常量折叠或预估容量，但 M16基线不依赖这些优化。

## 6. 测试计划

### 6.1 lexer / parser / HIR

- 单行/multiline、相邻孔、首尾孔、空 literal、嵌套 f-string与孔内嵌套花括号；
- 普通 escape、字面 `$`、`$name`诊断、raw multiline中的反斜杠与 `$`；
- 未终结字符串/孔、空孔、孔内独立语法错误及多错误恢复；
- HIR golden锁定 typed StringBuilderCore目标、hidden local、从左到右顺序、无孔常量化及 source origin；
- `ToString` bound成功/失败、generic孔、值类型派生实现、class opt-in实现。

### 6.2 IR / runtime

- MIR不含 f-string专用节点；generic `add<T>` 按 concrete type单态化；
- LIR扫描 builder.buffer但不扫描 byte payload，grow更新带 barrier，三个后备调用都是 managed statepoint；
- runtime unit覆盖0/1/多次grow、UTF-8多字节、空串、重复build与build后继续append；
- 强制GC发生在builder创建、hole求值、grow、toString和build中，结果保持正确且无悬垂地址。

### 6.3 端到端与 negative

- fixture目录使用 `tests/fixtures/m15-fstring/`；覆盖 primitive、String、struct派生ToString、class实现ToString、nested/generic/suspend hole；
- 组合覆盖孔内 `?:` / `!!` / array访问 / lambda调用 / throwable，以及孔求值副作用计数恰好一次；
- negative覆盖未实现ToString的类型、`$name`、空/未闭合孔、未终结单行/multiline和独立孔语法错误；
- M1–M14全部 fixture原样通过。

## 7. 实现顺序与验收门

1. lexer mode、结构化 AST与错误恢复；
2. core StringBuilder contract及managed runtime backing；
3. HIR typed脱糖、ToString bound与求值顺序；
4. MIR/LIR/runtime扫描、barrier、statepoint golden；
5. suspend/异常/强制GC组合及全量回归。

只把 `${Int}` 拼成 runtime字符串、不走 `ToString` bound不算完成；只在无GC路径工作、grow或hole挂起后丢失builder不算完成；向MIR长期加入按具体孔类型分派的f-string专用节点不算完成。

## 8. 明确不做

1. 格式宽度、精度、alignment等format mini-language；
2. `$name`简写；
3. locale-aware格式化；
4. 除`f`外的字符串前缀；
5. rope、编译期大规模拼接优化或escape analysis；
6. 把未实现`ToString`的值静默退化为type name/address。
