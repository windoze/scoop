# M41 设计：运算符接口与浮点默认比较

状态：实施中，基线 `9e715a3c`（M39 已完成验收与清理）。按功能先更新 spec，再实现；进展见 [PROGRESS.md](PROGRESS.md)。日期：2026-10-13。

依据：[语言规范](../specs/SCOOP-SPEC.md)、[实现规范](../specs/SCOOP-IMPL-SPEC.md)、[运行时规范](../specs/SCOOP-RUNTIME-SPEC.md)与[路线图](../ROADMAP.md)。本文记录已确定的设计方向、实施前决策和任务，不表示相关 core API 或语言行为已经交付。每批先更新对应 spec，再修改实现。

## 0. 目标与范围

为可重载运算符提供统一的 core 接口，使泛型库能够直接表达所需操作，同时保留普通成员和作用域内扩展运算符的使用方式。需要 key 支持同类型加法的容器，可以约束 `K : Add<K, K>`；内建类型及显式实现该接口的用户类型无需由容器使用者再次包装。

已确定的规则：

- 运算符表达式先按相应约定展开为调用，再检查候选是否存在、参数是否适用。不能在展开和调用决议之前要求 receiver 实现某个 core 运算符接口。
- “当前作用域可以调用这个运算符”与“类型满足某个 generic bound”是两项不同的能力。扩展运算符可以提供前者，不自动产生接口 conformance。
- core 运算符接口采用普通名义接口与显式实现关系。类型作者提供通用操作契约，各个泛型库共享该契约；特定场景的扩展操作不必承诺满足泛型库的要求。
- 删除原有 `Equality<T>`，以提供 `operator equals` 的 `Equals<T>` 代替；移除旧 `equalTo` 入口，统一默认相等的接口调用与运算符语义。
- 有界泛型正文仍在定义处绑定成员契约，具体化时映射到具体实现；不从调用者的 import 或扩展集合重新选择操作。
- Float/Double 的默认相等与关系运算按比较等价类构成全序：正负零相等，所有 NaN 相等且排在正无穷之后。IEEE 数值比较通过显式方法提供，不作为默认 Equals 的实现；四态枚举 API 草案见第 4 节。删除旧 `isTotallyOrdered` API，直接采用新比较方案。

运算符接口属于最小 core 的基础契约。Map 容器的存储与算法、结构化并发、平台库均不在本次交付范围；`FancyMap` 仅作为真实需求的最小示例。任意命名函数的可用性约束、追溯接口实现和按上下文选择接口实现也不属于本里程碑。

## 1. 当前基础与缺口

语言规范 3.2 已支持 `K : I<K>` 这样的名义接口上界；9.3 已要求运算符展开后使用普通显式 receiver 调用决议，并保留对应 `operator` 标记。普通扩展不会改变目标类型的接口集合。

当前缺少统一的算术等运算符接口。库作者可以自行声明 `Addable<K>`，但 [Int](../../sysroot/lib/scoop.core/src/integer_int.scoop) 即使已有 `operator fun plus`，也不会实现这个外部库定义的接口。泛化的 newtype 包装同样无法在无约束的 `K` 上调用加法。

M41 通过共享 core 接口解决通用运算符的约束问题，不把同名方法匹配改成结构型 conformance。现有 [bound 调用具体化](../../compiler/hir-lower/src/concretize/methods.rs) 已能按具体 receiver 的接口实现找到实际方法，可继续复用。

## 2. Core 接口与泛型使用

### 2.1 加法的最小闭环

加法接口定名为 `Add<Rhs, Result>`，保留现有方法名 `plus`；其余运算符方法同样保持现有拼写。

```scoop
public interface Add<Rhs, Result> {
    public operator fun plus(other: Rhs): Result
}

public class FancyMap<K : Add<K, K>, V> {
    public fun combineKeys(left: K, right: K): K = left + right
}
```

示例只展示容器依赖加法的部分。右操作数与结果分别作为类型参数，既能表达 `K + K -> K`，也能表达 `Point + Vector -> Point`，不需要隐式 `Self` 或 associated type。

core 的 Int、Long 等类型分别显式实现 `Add<Int, Int>`、`Add<Long, Long>`，对应的已有 intrinsic 方法实现接口 slot。用户类型同样可以提供通用契约：

```scoop
public struct Offset(val value: Long) : Add<Offset, Offset> {
    public override operator fun plus(other: Offset): Offset =
        Offset(value + other.value)
}
```

在相关 core 实现完成后，`FancyMap<Int, String>()` 和 `FancyMap<Offset, String>()` 都能通过 bound 检查。类型作者无需依赖容器所在 Cone，容器使用者无需编写适配包装或传入加法对象。

### 2.2 接口覆盖与签名

以下是接口覆盖的设计清单，不把尚未确定的名字或签名当成已发布 API。各接口使用实际 operand/result 类型，不预设所有运算都同型，也不把不同算术能力合成一个必选的大接口。

| 运算符类别 | 接口表达方向 | 实施前需确定的细节 |
| --- | --- | --- |
| `+ - * / %` | `Add/Sub/Mul/Div/Rem<Rhs, Result>` | 已在语言规范 9.3.6 定稿，分批实现 |
| 一元 `+ - !` | `UnaryPlus/UnaryMinus/Not<Result>` | 已定稿；数值实现前两项，Boolean 实现 Not<Boolean> |
| `== !=` | 已确定的 `Equals<Rhs>`，成员为 `operator fun equals(other: Rhs): Boolean` | 删除旧 Equality/equalTo 并迁移声明、调用与产物；核对成员限定和显式派生实现规则 |
| `< <= > >=` | `Compare<Rhs>`，`compareTo(other: Rhs): Long` | 整数、Char、String 先行；浮点随默认全序迁移 |
| `in !in`、区间操作 | `Contains<Element>`、`RangeTo/RangeUntil<Rhs, Result>` | 已定稿，四种整数 range 与八种整数沿用既有方法 |
| `++ --`、复合赋值 | `Increment<Result : Increment<Result>>`、对应 Decrement；`AddAssign/SubAssign/MulAssign/DivAssign/RemAssign<Rhs>` | 已定稿，保留既有返回子类型、歧义与写回规则 |
| `get/set/invoke` | `Get/Set<Index, Value>`、`Get2/Set2<I, J, Value>`、`Invoke0<Result>`、`Invoke<Arg, Result>`、`Invoke2<A, B, Result>` | 已在 9.3.6 定稿；固定参数签名，函数值不自动实现接口 |
| `iterator/componentN` | 复用 Iterable/Iterator；`Component1/Component2<Result>` | 普通 componentN 调用支持 class、interface 与上界；struct/tuple 仍内建解构 |

普通 interface 方法当前不能声明自己的类型参数。现有 generic operator、`suspend` 或 unsafe operator 的适配必须逐项明确，不能通过接口隐去调用效果，也不能仅为接口统一而引入泛型虚方法或参数包机制。接口尚未覆盖某种签名，不应使原本合法的直接运算符调用被自动拒绝；签名覆盖与兼容性取舍须先写入 spec。

### 2.3 相等、排序与 Hash

删除现有 `Equality<T>`，以新的普通 core 接口 `Equals<T>` 代替。接口提供运算符成员，泛型库和普通表达式共用 `equals`，不再保留独立的 `equalTo` 入口：

```scoop
public interface Equals<T> {
    public operator fun equals(other: T): Boolean
}

public fun <T : Equals<T>> sameKey(left: T, right: T): Boolean =
    left == right
```

将 `Equality<T>` 的声明、bound、接口视图与引用改为 `Equals<T>`，把 `equalTo` 的实现和调用改为 `equals` 或 `==`。原类型同时有 operator equals 和 equalTo 时，合并为一份符合新默认契约的实现，保留相应 `operator` / `override` 标记。Float/Double 直接采用第 4 节的新默认比较语义。

不保留旧接口的兼容 alias、转发接口或第二套默认相等方法。普通同名用户声明仍按其实际身份处理；这里删除的是 core 的旧 API。core 及受影响依赖的源码、产物和缓存按实际 API 变化迁移、重建。

对于默认相等和全序比较，core 同时提供两项能力的内建类型应保证：

```text
a == b  <=>  a.compareTo(b) == 0
```

同时提供 Hash 时，还须保证按 `Equals<K>` 判断相等的 key 具有相同 hash。这是库实现的契约，不要求编译器证明任意用户方法的数学性质。

仅存在手写或派生的 `operator equals` 仍不自动产生接口实现。结构相等的浮点字段需跟随第 4 节的新默认语义，接口关系继续按显式声明与继承判断。

## 3. 运算符展开与作用域

### 3.1 先展开，再决议

以二元加法为例，语义上先把 `x + y` 转换为带加法 operator 角色的 `x.plus(y)` 调用请求，再走普通调用检查：

1. 保留 receiver、参数、源码位置和 operator 角色，按既有规则推导类型并收集成员、上界成员与可见扩展候选。
2. 仅相应的 function-like operator 候选参与决议；普通同名方法不会因为文本展开而自动获得运算符语法。
3. 复用可见性、参数映射、默认参数、泛型推断、重载选择及调用效果检查。存在适用成员时按既有规则优先于扩展；同层歧义和无匹配给出正常诊断。
4. 保存唯一选定的 typed callable 或 bound member。后续 stage 消费该结果，不再搜索同名方法，也不补做“必须实现 Add”的前置判断。

这里规定语义顺序，不要求 parser 执行字符串替换。方法是否存在仍是编译期检查。

求值顺序和控制流继续使用各操作已有的规则：receiver/operand 只求值一次；`a in b` 按现有约定先求值 b；自增、自减、下标与复合赋值保留 place 保存和写回；`!=` 使用同一次相等比较的否定。`&&`、`||` 的短路行为、引用 identity 比较及其他不可重载语法不转为普通的急切求值方法。

### 3.2 扩展可调用不等于接口 conformance

假设依赖库已经公开 `ForeignKey(value: Int)` 及其 value 属性，但没有实现 Add，使用者可在自己的作用域定义：

```scoop
operator fun ForeignKey.plus(other: ForeignKey): ForeignKey =
    ForeignKey(this.value + other.value)

fun combineLocal(left: ForeignKey, right: ForeignKey): ForeignKey =
    left + right
```

`combineLocal` 可以调用选定的扩展；`FancyMap<ForeignKey, String>()` 仍因缺少 `Add<ForeignKey, ForeignKey>` 而失败。这是预期边界：局部的特殊加法不必成为所有泛型库可依赖的通用契约。

这种扩展不修改 ForeignKey 的名义成员/接口集合、TypeDescriptor、`is/as` 结果或装箱后的接口视图。无需增加全局扩展 conformance 登记，也不涉及 Rust orphan rule 或 Swift 式追溯接口实现。

现有 `operator equals` 有成员限定，扩展 equals 被禁止。是否放宽这项规则需要在接口清单中单独决定；不能仅由加法示例推导出所有历史限制已被删除。

### 3.3 泛型、跨 Cone 与分派

`FancyMap<K : Add<K, K>, V>` 正文在定义处将加法绑定到 Add 的成员契约。具体化后按 K 的实际显式实现取得目标。无相应 bound 的泛型正文，不能依靠调用处后来出现的某个具体类型扩展补救定义处未成立的调用；定义处已经适用的 generic extension 则继续按普通规则处理。

普通扩展调用也在其所在正文的定义上下文选择目标。导出的 generic/default body 保留原来的 typed 引用，消费方 import 不会替换其操作。

对具体值类型和静态选定的扩展，调用可直接落到实际方法或 intrinsic，不引入运算符接口对象或函数值字段。实际存在的 virtual/interface receiver 仍保留其动态分派语义。泛型类型身份继续由声明与类型实参决定，不添加调用方作用域或隐藏操作实参。

## 4. 浮点默认全序与显式 IEEE 比较

### 4.1 默认比较：已确定

Float/Double 分别通过 `operator equals` 显式实现 `Equals<Float>` / `Equals<Double>`，并通过 `operator compareTo` 实现第 2.2 节的比较接口。`== != < <= > >=` 与默认相等、比较接口使用一致的全序语义。需要 IEEE 数值相等和有序/无序关系时，程序显式调用专门的方法；NaN 的 unordered 语义不进入默认 Equals 实现。

默认比较以数值的比较等价类为单位，不按每种 IEEE 位型分别排序：

```text
-Infinity < 有限数值 < +Infinity < NaN

-0.0 == +0.0
任意 NaN == 任意 NaN
```

所有 NaN 无论符号、quiet/signaling 或 payload 都属于同一比较等价类。Float 与 Double 各自使用同一套规则，仍不引入两种精度之间的隐式转换或混合比较。

| 输入 | 默认 `equals` / `==` | 默认 `compareTo` 的符号 |
| --- | --- | --- |
| 两个 NaN，包括不同位型 | true | 0 |
| 左侧 NaN，右侧非 NaN | false | 正 |
| 左侧非 NaN，右侧 NaN | false | 负 |
| `-0.0` 与 `+0.0`，任一方向 | true | 0 |
| 其他值，包括无穷 | 按 IEEE 数值相等判断 | 按数值次序比较 |

`!=` 是默认相等的否定，`< <= > >=` 使用同一 compareTo 结果。因此 NaN 默认等于自身、排在正无穷之后，正负零既不小于也不大于彼此。

算术、转换、舍入、位型存储、FFI 和浮点环境仍遵守原有 IEEE 规则。比较时归类不修改值本身，`1.0 / -0.0` 仍为负无穷；传参、返回、复制仍保留原来的零符号和 NaN 位型。

### 4.2 显式 IEEE 比较：四态枚举草案

以普通 core enum 返回一次 IEEE 数值比较的结果。API 定名为 `IeeeComparison` 和 `ieeeCompare`：

```scoop
public enum IeeeComparison {
    Less,
    Equal,
    Greater,
    Unordered
}
```

Float 与 Double 分别提供同类型参数的方法 `ieeeCompare(other: Float): IeeeComparison` 和 `ieeeCompare(other: Double): IeeeComparison`。该方法是显式 IEEE 入口，不实现默认 Equals 或全序比较接口。

- 任一操作数为 NaN 时返回 `Unordered`，包括两个操作数为同一 NaN 的情况。
- 两个数值相等时返回 `Equal`，包括正负零。
- 其他输入按数值次序返回 `Less` 或 `Greater`，包含正负无穷。

`Unordered` 沿用 IEEE 754 的术语，表示两个操作数在数值比较中“无序／不可比较”。这是明确的比较结果，不表示计算尚未完成、结果未知、类型不兼容或发生错误。不能给它赋予大于、小于或等于的含义，也不能把它压成 compareTo 的三态符号结果。

令 `r = a.ieeeCompare(b)`，IEEE 关系可完整表达为：

| IEEE 关系 | 对 r 的判断 |
| --- | --- |
| 相等 | `Equal` |
| 不等 | 除 `Equal` 以外的全部变体，包括 `Unordered` |
| 小于 | `Less` |
| 小于等于 | `Less` 或 `Equal` |
| 大于 | `Greater` |
| 大于等于 | `Greater` 或 `Equal` |

因此不能以“不是 Less”实现 IEEE 大于等于；`Unordered` 对四种有序关系均为 false。`ieeeEquals` 等便捷谓词不是四态表达所必需的 API，是否增加按实际调用需要决定。

### 4.3 替换旧浮点比较 API：已确定

现有 `Equality<T>` 仅声明 `equalTo`；`isTotallyOrdered(belowOrEqualTo:)` 是 Float/Double 独立提供的 IEEE `totalOrder` 方法，不属于 Equality。本里程碑删除这两个旧入口：默认相等统一到 `Equals.equals`，默认排序使用 `compareTo`，显式 IEEE 数值比较使用第 4.2 节的四态方法。

目前没有需要保持旧浮点比较行为的库功能，因此按新语义一次性更新规范、core、编译器和相关 fixture。不保留旧 API 的兼容入口或单独的逐位型 totalOrder 功能。

### 4.4 语义迁移范围

当前规范与实现采用 IEEE 默认比较，本次按新语义统一更新以下已有处理环节：

- core 浮点方法与接口、默认比较 intrinsic，以及显式 IEEE 比较入口。
- const 比较、常量折叠、literal pattern、when 分支匹配及结构相等派生。
- 普通调用、泛型、alias、跨 Cone 模板和运行期比较的一致性。
- NaN 检测示例：默认比较下 `x != x` 不再是 NaN 检测方式，应使用 `isNaN()` 或显式 IEEE API。
- 常量求值与运行期算术对 NaN 位型的既有承诺。两者即使产生不同 NaN 位型，默认比较仍把它们视为相等；存储继续保留表示。

产物中用于常量保存、身份或去重的 raw bits 仍按表示处理，不能套用源码比较的归类策略而丢失负零或 NaN 位型。后端也不能因默认全序就对算术或显式 IEEE 比较启用改变其语义的 fast-math 假设。

## 5. 实现边界

- core 接口通过普通声明、继承与导出接口表提供，不按接口拼写触发编译器特权。运算符决议只需要相应的 operator 角色，不需要先识别 Add 的身份。
- HIR 完成调用决议和 bound 绑定；具体化消费真实的接口实现。MIR/LIR 与 codegen 保留已确定的 dispatch 和 intrinsic 语义，不在下游重放成员查找。
- 默认浮点比较与显式 IEEE 四态比较使用明确的 typed 操作表示，常量求值与后端各自实现同一语义。删除旧 totalOrder 的 intrinsic 登记与专用操作。四态结果使用普通 enum 表示，不增加异常通道或接口对象。
- core 新增接口及 conformance 通过既有 `.slib` 声明与方法发布路径消费。兼容性、缓存和格式版本按实际 API、语义或 wire 变化处理，不预先增加新格式或 runtime ABI。
- 装箱和接口调用使用原有 TD/itable/ABI；扩展函数不增加接口表项。NoGC 实现与 Managed slot 的适配继续使用既有规则。

## 6. 文档与实施任务

### 6.1 设计与 spec 任务，先于对应实现

- [x] 完成第 2.2 节的接口覆盖清单：最终名称、签名、各内建类型的实现，以及 generic/suspend/unsafe、多参数和 place 操作的处理。
- [x] 确定删除 `Equality<T>` / `equalTo`，由 `Equals<T>` / `operator equals` 代替，见第 2.3 节；spec 与 core 接口迁移已完成，本机验证见 PROGRESS.md。
- [x] 确定默认浮点比较按等价类排序：正负零相等、全部 NaN 相等且排在正无穷之后，见第 4.1 节；实际实现仍待完成。
- [x] 确定删除旧 `isTotallyOrdered` API 及其 IEEE totalOrder 功能，直接采用第 4 节的新比较方案；实际 spec 和代码更新仍待完成。
- [x] 核对现有成员限定 equals、结构派生与显式 conformance 的规则；不把签名匹配自动变成接口实现。
- [x] 定稿第 4.2 节 IEEE 四态 enum API 的名称：IeeeComparison / ieeeCompare。
- [ ] 更新 [SCOOP-SPEC.md](../specs/SCOOP-SPEC.md) 3.2、8.5～8.6、9.3、11.8、11.11、12.4 及相应 core API 小节：标准运算符接口、先展开再决议、扩展与 bound 的区别、默认比较契约和兼容性规则。
- [ ] 更新语言规范 5、9.1.2、11.2.2 及结构相等相关条款：默认等价类全序、显式 IEEE 四态比较、const、模式匹配与派生比较的一致性；删除旧 `isTotallyOrdered` API 与“普通关系直接采用 IEEE 比较”等被新方案替代的条款。
- [x] 全文迁移旧 Equality/equalTo 的规范与示例，包括 core 内建类型、generic bound、Hash 契约、NoGC 接口适配和 Ptr 声明；不能只替换接口定义而遗漏旧调用语义。
- [ ] 更新 [SCOOP-IMPL-SPEC.md](../specs/SCOOP-IMPL-SPEC.md) 2.2～2.3、2.9～2.10、2.12～2.13、2.18～2.19：调用绑定与具体化、普通 core 接口发布、默认/IEEE 浮点操作、常量求值、优化和产物消费职责，移除旧 totalOrder 操作约定。
- [ ] 更新 [SCOOP-RUNTIME-SPEC.md](../specs/SCOOP-RUNTIME-SPEC.md) 2.2～2.3、6.1：接口关系、装箱与现有 Equality 描述的迁移，浮点默认比较与 IEEE 算术/FFI 边界；按实际需要修订引用，不把库方法变更扩展为新 runtime ABI。
- [ ] 同步三份 spec 的章节交叉引用、core API 示例与相关语言示例；核对 M30 浮点、M33 相等设计中的历史描述，标明后续替代关系，保留原验收事实。
- [ ] 更新 API 与示例说明：统一后的 Equals/equals、compareTo 默认全序、显式 IEEE 四态比较与 NaN 检测写法；删除旧 Equality/equalTo 和 isTotallyOrdered 的用法。
- [ ] 随实际实施更新本设计的状态；验收后补充实际命令、目标、profile、结果与剩余限制，不以设计文档完成代替功能交付。

### 6.2 实施顺序

1. **加法闭环。** 完成 Add 与内建类型实现，用用户定义类型、`FancyMap` 最小操作及普通扩展验证通用契约和局部可调用性的区别。
2. **其余运算符接口。** 按已完成的签名清单补齐 core 声明与内建实现，删除 Equality/equalTo 并迁移到 Equals/equals；保持普通调用决议、求值顺序及调用效果，逐类补充诊断。
3. **浮点比较替换。** 将已确定的等价类全序与 IEEE 四态比较写入 spec 后，同步 core、常量求值、模式匹配、结构派生和后端；删除 `isTotallyOrdered` 及仅用于旧 totalOrder 的编译支持，已有浮点比较 fixture 按新语义更新。
4. **跨 Cone 与性能验证。** 验证 core 重建、接口发布、仅消费产物、模板再次发布与普通具体化；检查已知值类型和扩展调用的实际目标。
5. **迁移与验收。** 更新受影响的源码、示例和期望，完成相关回归与正式 CLI 覆盖，再记录交付状态。

每批代码变更先按仓库要求格式化、lint，再验证。仅设计文档阶段检查文档内容、引用和修改范围，不运行编译器构建或批量刷新 fixture。

## 7. 验收重点

| 类别 | 必须覆盖的行为 |
| --- | --- |
| 标准能力 | Int 等内建类型与用户类型通过 Add bound；异构 operand/result、多个能力约束及 alias |
| Equals 迁移 | 原 Equality 声明、bound、接口视图及 equalTo 调用迁移到 Equals/equals；`==` 与泛型接口调用共用默认语义，旧 core API 不再发布 |
| 扩展边界 | 无 Add conformance 的具体类型可通过可见扩展执行加法；同一类型用于 Add bound 时给出准确错误；`is/as` 与接口视图不因扩展改变 |
| 调用决议 | 成员优先、不同 import 层、歧义、无匹配、无 operator 标记、错误 arity/结果或效果；诊断保留运算符源码位置 |
| 泛型绑定 | 泛型定义处的合法扩展保持选定目标；调用方扩展不能补救未满足的 bound 或改变已发布正文 |
| 求值与 place | receiver/operand 单次求值、`in` 顺序、前后缀更新、下标和复合赋值的写回与副作用；不可重载语法保持原行为 |
| 浮点默认比较 | 有限值、无穷、subnormal；正负零相等、不同 NaN 表示全部相等且排在正无穷之后；compareTo 的零结果与 Equals 一致；Float/Double 不再提供旧 isTotallyOrdered API |
| IEEE 四态比较 | NaN 在任一侧、两侧同时为 NaN 均返回 Unordered；正负零返回 Equal；Less/Equal/Greater 数值关系与六种关系谓词的映射正确，Unordered 不误作有序比较成功 |
| 组合语义 | const/runtime、模式匹配、struct/enum/tuple 派生相等、generic/default body、alias 与 Float/Double 精度保持一致 |
| 产物与分派 | provider/consumer 独立编译、core 重建、仅 `.slib` 消费及下游再次发布；具体值与扩展调用保持 direct/intrinsic，真实 interface/virtual 调用保持动态语义 |

每项新语言规则有独立 fixture，并覆盖实际功能组合；每项编译错误有位置和信息准确的 negative fixture。普通功能用例验证诊断与运行结果，关键调用绑定和优化只用少量 typed IR 或专门结构断言，不恢复全阶段快照。

正式验收按实际改动执行相关 Rust 测试、三个 CLI binary 构建、fixture runner unittest 和完整文件 fixture 的适用目标覆盖，并记录实际验证范围。没有运行的平台或模式不能记为通过。
