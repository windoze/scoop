# M6 设计：引用类型层级

版本：0.2（实现校准）

对应 `docs/ROADMAP.md` 的 M6（2026-08-28 顺序调整后）。目标：class / 继承 / interface / 方法、vtable / itable 分派、装箱（spec 3、4.4、9.1；impl spec 2.9；runtime spec 2.2）。

## 0. 范围说明

这是引用类型的奠基里程碑，包含五块相互咬合的内容：(a) class 与方法；(b) 单继承与抽象类；(c) interface 及其实现（含值类型实现）；(d) 分派（vtable/itable）与 TypeDescriptor 全量落地；(e) 装箱、`Any`、`is`/`as`/`as?`、`===`。每块都取最小可用形态，边界见第 1 章末尾与第 6 章。

## 1. 语言子集

```
interface Describable {
    fun describe(): String
}

open class Shape(val name: String) : Describable {
    final override fun describe(): String = name
    open fun kind(): Int = 0
}

class Point(val x: Int, var y: Int) : Shape("point") {
    fun moveTo(nx: Int, ny: Int) {
        x = nx          // 错误（val 属性）；y = ny 合法（var 属性）
    }
}

abstract class Base {
    abstract fun kind(): Int
}

struct S(val v: Int) : Describable {       // 值类型实现 interface（spec 4.4.3）
    override fun describe(): String = "S"
}

fun main() {
    val p = Point(1, 2)
    p.y = 3                                // class var 属性可写
    println(p.y)
    println(p.describe())
    val d: Describable = p                 // 向上转型到 interface
    println(d.describe())                  // itable 分派
    val a: Any = S(1)                      // 值类型装箱（spec 4.4.4）
    println(a is S)                        // true
    val s = a as? S                        // as? → Option<S>
    println(s != None)
    val b: Any = p
    println(b === p)                       // true（引用相等）
}
```

新增（相对 M5）：

- **class 声明**：主构造函数（`val`/`var` 属性）、成员函数（含 `this`）、body 中的方法与构造函数属性访问（裸名或 `this.x`）；`var` 属性可赋值（`p.y = 3`——M2 起的"字段不可赋值"限制只对值类型）。
- **继承**：`open class`、`abstract class`（含 `abstract fun` 无体声明）、单继承与基类构造委托 `class B(...) : A(args)`、方法级 `open` / `final` / `abstract` 与 `override`（override 是必需的修饰符，缺失/多余均诊断）。类与普通方法默认 final；`override fun` 默认继续 open，`final override fun` 关闭后续覆写。
- **interface**：仅方法签名（无属性、无默认实现）；class 与 **struct/enum** 实现 interface；实现 interface 的方法均必须写 `override`（spec 4.4.3，见 5.2）。
- **分派**：通过 class 引用调用可覆写方法走 vtable；通过 interface 引用调用走 itable；静态接收者上已知的 final 方法直接调用。类的 `open` 与方法的 `open` 相互独立：开放类中的普通方法仍默认 final。
- **Any 与装箱**：`Any` 类型；值类型/类向上转型到 `Any` 或 interface（值类型在 O(1) 场景自动装箱，spec 4.4.4）；`is` / `!is` / `as` / `as?`（`as` 失败 trap，M8 改 `ClassCastException`）；`===` / `!==`（仅引用类型，对值类型是诊断，spec 4.4.2）。
- **smart cast**：`if (x is T)` / `if (!x is T)` 与 `&&` 连接的 `is` 条件，true 分支内局部变量按 `T` 使用（M6 简化：只对不可变局部变量、不支持 `||`、不支持 when 分支内，见 5.4）。
- **struct/enum 成员函数**：值类型的成员函数（`fun` in struct/enum body）；与 class/interface 方法一样，`this` 由 receiver value 按值初始化，是隐含不可变参数而不是调用方 binding place 的别名（spec 3.3）。struct 的方法可以读字段。

明确不在 M6（认领见第 6 章）：次构造函数、`init` 块、body 属性（非构造函数属性）、`super` 调用、interface 的属性与默认实现、`equals`/`hashCode`/`toString` 的用户覆写、companion object、`object` 声明、`sealed`、委托（`by`）、方法/函数重载（M7）、可见性修饰符。

## 2. 各 stage 设计

### 2.1 AST / parser

- `class` 声明（构造函数属性 + body 方法）、`interface` 声明、类级 `open` / `abstract` 与方法级 `open` / `final` / `abstract` / `override` 修饰符；基类/接口列表 `: A(args), I1, I2`；`this` 表达式；`is`/`!is`/`as`/`as?` 中缀运算符（优先级同比较）；`===`/`!==`（优先级同 `==`）。
- struct/enum body 中的 `fun` 声明；`abstract fun` 无体。

### 2.2 HIR

- **类型**：`Type::Class(ClassId)`、`Type::Interface(InterfaceId)`、`Type::Any`。`ClassId`/`InterfaceId` 为类型化 id（AGENTS.md 准则）。
- **成员解析**：方法调用 `expr.name(args)`（新增 `Expr::MethodCall` 语法位）按接收者类型解析：class → 成员函数（含继承链）；interface → 接口方法；值类型 → 其成员函数；`this` 的类型为当前类/接口/值类型。所有 `this` 都是类型完整的隐含按值参数：ref type 复制 ref value，value type 复制完整值；HIR 不保留调用方 receiver binding place。类字段访问/赋值：构造函数属性按声明下标解析；`var` 才可写。
- **继承检查**：基类必须 `open`/`abstract`；`override` 必须对应基类/接口同签名且非 final 的方法；未标 `override` 的覆写、标记但未覆写、覆写 final 方法均为诊断。普通方法默认 final，`open fun` 才能首次被覆写；`abstract fun` 隐含 open；`override fun` 默认 open，`final override fun` 关闭后续覆写。open / abstract 方法必须由 open / abstract 类或 interface 承载；abstract 类不能实例化；derived 构造委托实参按基类构造函数检查。
- **interface 检查**：interface 方法必须被实现（含继承来的实现）；值类型实现 interface 时方法不得修改字段（值类型字段本来就不可写，自然成立，spec 4.4.3）。
- **装箱**：赋值/实参/返回位置从具体类型到 `Any`/interface 时插入 `ExprKind::Box`（值类型才需要；class 到 Any/interface 是零成本引用）；`Unbox` 由 `as`/`as?`/smart cast 产生。
- **`is`/`as`/`as?`**：静态类型必须是 `Any`/interface/可继承 class 之一才合法（对 final 类型的无意义检查给诊断）；`as?` 结果 `Option<T>`。
- **smart cast**：if 条件中的 `is`/`!is` 在相应分支内把目标局部变量的类型收窄（条件求值纯、目标不可变）。

### 2.3 MIR

- **call kind 标注落地**（impl spec 2.9）：direct（final 方法、值类型方法、私有场景）/ virtual（可覆写 class 方法）/ interface（接口方法）。
- **vtable 生成**：每 class 一份——槽 0/1/2 = `equals`/`hashCode`/`toString`（Any 固定三槽），其后按声明顺序加入 open / abstract 用户方法；继承时基类槽位布局前缀保持，override（包括 final override）原位替换，新 final 方法不占 vtable 槽；
- **itable 生成**：每 class / 每装箱值类型一份——`(接口 TypeDescriptor 符号 → 方法表)` 键值对数组（impl spec 2.9 的指针键查找，无全局槽位协调）；
- **装箱值类型的 adjust thunk**：值类型实现 interface 时，itable 表项指向 MIR 生成的 thunk；thunk 语义上复制 payload 以初始化真正值方法的 `this`。M6 可以在当时全部可观察值均 immutable 的前提下用 payload 指针消除该复制；M12 引入 `@InteriorMutable` / `addressOf(this)` 后对可观察场景必须物化私有副本（impl spec 2.9）；
- **方法体降级**：`this` = 参数 0，与显式参数使用同一按值规则；class/interface 传入 ref value，值类型传入完整 value。LIR/codegen 可以用间接 storage 作为不可观察的 ABI 优化，但不得改变 spec 3.3 语义；
- **TypeDescriptor 数据**：MIR meta 输出每种类型的 TD 记录（含 vtable/itable/parent/ref_offsets 引用），codegen 发射；
- **Box/Unbox/is/cast 降级**：Box → `scoop_rt_box(td, payload_size)` + payload 拷贝；`is` → `scoop_rt_is_instance(obj, td)`；`as` → 检查 + trap（M8 改异常）；`as?` → 检查 + Option 包装。

### 2.4 LIR

- 指令/类型基本够用（Ptr 世界）：间接调用指令 `CallIndirect { out, table, slot, args }`（vtable/itable 分派）；`HeapLoad { out, object, offset }` / `HeapStore { object, offset, value }` 使用布局确定的**字节偏移**访问堆对象，区别于值语义、按字段序号工作的 `ExtractValue`。class 字段按自然对齐连续布局，不能把字段序号解释为 8 字节槽（连续 `Boolean` 等 sub-8 字段必须保持 1 字节布局）；
- **布局 meta**：class 布局（header + 字段，ref_offsets 供 GC）；装箱布局（header + payload）。

### 2.5 codegen

- TD 发射升级为完整形态（runtime spec 2.2 + M6 字段：vtable ptr、itable 数组、parent、type_id 递增分配）；
- vtable/itable 全局数据发射；间接调用；`scoop_rt_box` / `scoop_rt_is_instance` / `scoop_rt_cast_trap` 调用。

### 2.6 driver

无变化（class 是语言内建声明，不需要 core 库配合——`Any` 作为编译器内建类型识别，spec 11.1 的库形态随 M7/core 完善再落）。

## 3. runtime 新增

- `void *scoop_rt_box(const ScoopTypeDescriptor *td, const void *payload, uint64_t payload_size)`：分配 boxed 对象 + 拷贝 payload；
- `bool scoop_rt_is_instance(const void *obj, const ScoopTypeDescriptor *td)`：parent 链 + itable 键扫描；
- Any 默认方法实现：`scoop_rt_any_equals`（引用相等）、`scoop_rt_any_hashcode`（地址哈希）、`scoop_rt_any_tostring`（类型名 + 地址——M6 最小形态）。

## 4. 测试计划

- **独立 fixture**：class 构造/字段读写/var 属性赋值（含连续 `Boolean` 后接 `Int` 的自然布局）；方法与 this；值类型 receiver 为局部 `var` 时 `this` 仍是按值副本；单继承与 override；abstract class；interface 实现与 itable 分派；值类型实现 interface + 装箱后分派；`is`/`as`/`as?`/`===`；smart cast；
- **组合 fixture**：interface 数组（`Array<Describable>`——数组元素是引用，验证元素布局）；泛型函数接受 `T: 无约束`……（无 bound，用具体类型）；装箱值进数组再 `as?` 取回；enum 实现 interface；
- **negative fixture**：对 final class 继承；覆写 final 方法；缺/多 `override`；在 final class 或值类型中声明 open / abstract 方法；abstract 类实例化；未实现接口方法；对值类型 `===`；对值类型字段赋值；接口方法的类型不匹配实现；`as` 到无关系类型（诊断）；
- **trap fixture**：`as` 失败（EXPECT-TRAP）；
- golden dump 锁定 vtable/itable 布局与 call kind 标注。

## 5. 临时决策（及退役里程碑）

1. **`equals`/`hashCode`/`toString` 的用户覆写不做**：vtable 前三槽始终指向默认实现（引用相等/地址哈希/类型名）；值类型装箱的 equals 用编译器生成的结构相等函数。用户覆写随后续里程碑（需要时单独立项，不阻塞 M7 重载）。
2. **值类型实现 interface 也要求 `override` 关键字**：已在 spec 4.4.3 明确；值类型方法本身保持 final，装箱后的 interface 调用走 itable。
3. **`as` 失败 = trap**：M8 改 `ClassCastException`。
4. **smart cast 简化**：仅不可变局部变量、仅 `is`/`!is` 与 `&&` 组合、不支持 `||`/嵌套 when；完整 flow analysis 随需要扩展。
5. **`Any` 为编译器内建类型**：spec 11.1 的 core 库形态随 M7/core 完善落地。

## 6. 明确不做

- 次构造函数、`init` 块、body 属性、`super` 调用；
- interface 的属性与默认实现；`equals`/`hashCode`/`toString` 用户覆写（见 5.1）；
- companion object、`object` 声明、`sealed`、委托（`by`）；
- 可见性修饰符（M6阶段临时全部public；M21已决定正式语义默认internal，并在M23-5多Cone名称语义前完成）；
- 重载（M7）；`?.` 后随方法调用（spec 6.3 形态——`?.` 目前只支持字段，方法版随本里程碑的 `?.method` 自然表达式扩展时单独评估，可先只做字段）。
