# M57 设计：泛型类型的条件偏特化

状态：设计稿，未实现。更新：2026-10-19。本文件确定 M57 的语言方案、实现边界与验收范围；不把设计目标记为已有能力。

现行合同以[语言规范](../specs/SCOOP-SPEC.md)、[实现规范](../specs/SCOOP-IMPL-SPEC.md)、[运行时规范](../specs/SCOOP-RUNTIME-SPEC.md)为准。M57 在 [M55](../milestone55/DESIGN.md) 的条件关系与定义处绑定基础上扩展名义类型；实施前按第 12 节同步权威规范。本次设计不修改编译器或 runtime。

## 0. 目标与核心决定

允许同一个泛型名义类型根据类型参数的条件，选择不同的完整定义。不同分支可以有不同字段、布局、构造器、成员、父类和接口；每个具体 application 仍只有一个静态类型和一份确定的定义。

首要用例是 HashMap 的条件序列化：普通 `HashMap<K, V>` 保持原有集合能力，当 K／V 的 public companion 提供所需 codec 时，所选分支的 companion 再实现 HashMap 的编码／解码接口。共享存储和算法可以放在普通 `HashMapImpl<K, V>` 中，各分支通过普通继承复用；这不要求分支之间继承，也不要求 K／V 的数据实例实现 codec。`T.Companion` 约束是本期基础选择条件，HashMap 默认 codec 是第一个交付闭环。

| 问题 | 决定 |
| --- | --- |
| 同名条件声明是什么 | 一个名义类型族的多个定义分支，不是多个可相互转换的类型 |
| 如何选择 | 满足条件的分支中，唯一最具体者胜出；无匹配或存在多个不可比较的最优分支时报错 |
| 是否支持交叉条件 | 支持；`Debug`、`Hash` 的交叉可以由 `Debug + Hash` 分支覆盖，不要求所有分支形成一条链 |
| 是否能约束参数的 companion | 支持 `T.Companion` 的 M55 名义／负向／ref 条件；存在性与接口关系都按静态声明判断 |
| 分支之间是否继承实现 | 不继承、不拼接；选中的分支独立给出完整定义 |
| 类型身份由什么决定 | 原类型族、完整类型实参及既有必要 owner application；不含调用点、import、已知 bound 或运行期对象类型 |
| 泛型正文何时检查 | 定义处检查；分支未确定时只使用所有可能胜出分支共同保证的契约 |
| 谁能增加分支 | 类型所属 Cone，在同一个声明作用域中；消费 Cone 不能为外部类型追加分支 |
| runtime 是否参与选择 | 不参与；进入 LocalConcreteHir 前确定分支，后续仍是普通类型、调用与布局 |

本期覆盖普通 generic class、struct、enum，包括既有合法的静态嵌套、inner class 和局部声明位置。所有分支必须保持同一 nominal kind；不允许 `struct`／`class` 混合。局部分支的连续声明规则见 2.2。

选择条件包括已有正向 class/interface bound、`value`／`ref` kind bound，以及 M55 的负向 interface 条件和固定 `T.Companion` 投影条件。subject 可以是该类型自己的形参或该形参的 companion；投影仍通过 where 声明，条件中的类型可以引用原作用域合法可见的外层形参。类型头不接收 `requires` 成员要求；普通泛型函数和合法泛型成员继续完整使用 M55，且可以把特化类型作为实参。

interface 声明本身维持单一定义，不接收本期新增的负向／companion 条件或重复条件定义，保证被实现的接口契约仍按现有规则声明；它可以接收特化类型作为实参，也可以被不同分支实现。typealias、object、annotation、intrinsic 类型登记不提供条件偏特化入口。这里不增加外置 `impl`、associated type、任意 nested type 投影、类型参数模式拆解、具体类型相等条件、非类型参数、编译期任意布尔表达式或 SFINAE。

## 1. 源码形式与动机用例

### 1.1 HashMap 的条件序列化

用户示意中的 `Encoder<T>`／`Decoder<T>` 对应当前核心库的 `ValueEncoder<T>`／`ValueDecoder<T>`；现有非泛型 `Encoder`／`Decoder` 是格式读写上下文。下例假定普通 open 基类 `HashMapImpl<K, V>` 已实现 MutableMap 的存储和算法，并具有合法的 public 无参构造；沿用实际 HashMap 的 `K : Equals<K>, K : Hash` 条件。

以下是拟采用语言能力的源码示意，不是本次对集合库的重构。两个分支都保留[现有 HashMap](../../sysroot/src/scoop.std/src/collections/hash_map.scoop)的显式 codec 工厂；编码／解码逻辑调用[现有普通 map helper](../../sysroot/src/scoop.std/src/collections/map_codec.scoop)。

```scoop
class HashMap<K, V> public constructor() : HashMapImpl<K, V>()
    where K : Equals<K>, K : Hash {
    public companion object {
        public fun encoder(key: ValueEncoder<K>, value: ValueEncoder<V>): ValueEncoder<HashMap<K, V>> =
            ValueEncoderFunction<HashMap<K, V>> { values, encoder -> encodeMap(values, key, value, encoder) }

        public fun decoder(key: ValueDecoder<K>, value: ValueDecoder<V>): ValueDecoder<HashMap<K, V>> =
            ValueDecoderFunction<HashMap<K, V>> { decoder ->
                val result = HashMap<K, V>()
                decodeMapInto(decoder, key, value, result)
                result
            }
    }
}

class HashMap<K, V> public constructor() : HashMapImpl<K, V>()
    where K : Equals<K>, K : Hash,
          K.Companion : ValueEncoder<K>, K.Companion : ValueDecoder<K>,
          V.Companion : ValueEncoder<V>, V.Companion : ValueDecoder<V> {
    public companion object : ValueEncoder<HashMap<K, V>>, ValueDecoder<HashMap<K, V>> {
        public fun encoder(key: ValueEncoder<K>, value: ValueEncoder<V>): ValueEncoder<HashMap<K, V>> =
            ValueEncoderFunction<HashMap<K, V>> { values, encoder -> encodeMap(values, key, value, encoder) }

        public fun decoder(key: ValueDecoder<K>, value: ValueDecoder<V>): ValueDecoder<HashMap<K, V>> =
            ValueDecoderFunction<HashMap<K, V>> { decoder ->
                val result = HashMap<K, V>()
                decodeMapInto(decoder, key, value, result)
                result
            }

        public override fun encode(value: HashMap<K, V>, encoder: Encoder): Unit =
            encodeMap(value, K.Companion, V.Companion, encoder)

        public override fun decode(decoder: Decoder): HashMap<K, V> {
            val result = HashMap<K, V>()
            decodeMapInto(decoder, K.Companion, V.Companion, result)
            return result
        }
    }
}
```

这里的兜底是“只满足集合基础条件”的第一分支，不是取消 HashMap 必需的 Hash／Equals bound。两份定义都显式写这些条件；四项 companion codec 条件全部成立时第二分支更具体。任何一项确定不成立，包含参数没有 public companion 时，选择第一分支；参数关系未知时按第 4.3 节保留可能分支。

序列化接口由 **HashMap 的 companion** 实现，不由 HashMap 实例或 HashMapImpl 自动实现。K／V 的 companion 条件作为宿主分支的事实在 companion 内可用，所以 encode／decode 可以将 `K.Companion`、`V.Companion` 作为普通 codec 值传入 helper。基础 HashMap 操作仍来自共同基类；没有默认 codec 时仍可显式传入 codec 工厂，不缩小原有容器的可用类型范围。

[现有 Json 默认重载](../../sysroot/src/scoop.json/src/json.scoop)查询 `T.Companion : ValueEncoder<T>`／`ValueDecoder<T>`。因此若目标是直接支持 `Json.encodeToString(map)` 和 `Json.decodeFromString<HashMap<K, V>>(text)`，companion 实现的目标应是 `HashMap<K, V>`。示意中的 `MapNode<K, V>` 可以作为普通中间表示或由 helper 编码，但 `ValueEncoder<MapNode<K, V>>` 本身不等于 `ValueEncoder<HashMap<K, V>>`，需要相应的显式转换／委托。

共享基类仍遵守 open、构造和签名可见性规则，class 继承也不继承 companion；两个分支需要的 companion 成员须分别声明，或调用普通共享 helper。此例不要求库为复用而额外暴露一个公共实现基类；真实库改动采用满足普通可见性和既有 API 的组织方式。

该两分支方案有意把编码与解码一起提供。若希望按方向独立提供，令 E 表示 K／V 的 companion 都满足各自的 ValueEncoder，D 表示都满足各自的 ValueDecoder，可写成同一规则下的四分支：

| 相对于共同 Hash／Equals 条件增加的 guard | HashMap companion 新增接口 |
| --- | --- |
| E 且 D | ValueEncoder<HashMap<K, V>>、ValueDecoder<HashMap<K, V>> |
| E | ValueEncoder<HashMap<K, V>> |
| D | ValueDecoder<HashMap<K, V>> |
| 无 | 无默认编码／解码接口，保留显式 codec 工厂 |

E／D 是本段说明中的缩写，源码仍逐项写普通 where 条件，不新增条件别名语法。交叉分支保证 E、D 同时成立时有唯一赢家；这是库的能力组合选择，不另立“序列化特化”语言规则。两分支版本必须同时证明 E、D 才能使用默认编码；四分支版本只证明 E 即可保证默认编码，解码对称。

### 1.2 实例接口的四分支例子

沿用普通声明头、inline bound 和 `where`，不增加 `specialize`／`default` 关键字。下列声明头中的 `Debug`、`Hash` 表示普通名义接口；花括号内省略各分支必须独立提供的合法成员实现：

```scoop
struct Box<T>(val value: T) : Debug, Hash
    where T : Debug, T : Hash { /* complete branch body */ }

struct Box<T>(val value: T) : Debug
    where T : Debug { /* complete branch body */ }

struct Box<T>(val value: T) : Hash
    where T : Hash { /* complete branch body */ }

struct Box<T>(val value: T) { /* complete branch body */ }
```

多个 `where` 条目分别写 subject：`where T : Debug, T : Hash`。类型头冒号后的 `Debug, Hash` 表示 **Box 实现的接口**，`where` 中的条件表示 **对 T 的筛选**，二者没有自动推导关系。`where T : !Debug` 的分支也完全可以为 Box 自己实现 Debug。

| T 的静态接口关系 | 所选定义 | Box 的显式接口 |
| --- | --- | --- |
| 同时满足 Debug 和 Hash | 第一分支 | Debug、Hash |
| 仅满足 Debug | 第二分支 | Debug |
| 仅满足 Hash | 第三分支 | Hash |
| 两者都不满足 | 无条件分支 | 此例中无上述接口 |

无条件分支本身适用于所有参数，只是会被更具体的适用分支压过。它的声明条件不等于 `T : !Debug, T : !Hash`；表中最后一行是这组完整候选的选择结果。增加其他分支后，这个结果域也可能变化。

也可只写互补的两份定义，不要求存在无条件分支：

```scoop
struct Box<T>(val value: T) : Debug
    where T : Debug { /* complete branch body */ }

struct Box<T>(val value: T)
    where T : !Debug { /* complete branch body */ }
```

同名声明都至少有一个自身类型参数；普通非泛型类型仍不能重复定义。只有一份声明时也适用同一模型，其原有正向 bound 仍限制合法 application，不因存在“类型族”概念自动得到兜底定义。

## 2. 类型族、声明归组与完整定义

### 2.1 归组与身份

名称先按普通作用域规则解析到一个类型族，再在族内选择分支。不同包或不同 Cone 中恰好同名的类型不会因此合并，import alias／re-export 也不建立新族或新分支。

一个族的所有分支具有相同的原 Cone、typed lexical owner、名称、nominal kind、自身类型参数个数、声明可见性及 inner／非 inner 属性。类型参数名称可不同，以位置 alpha-normalize；外层 binder 保持各自身份。与同作用域中的其他 kind、不同 arity 或非类型绑定冲突时，仍按重复声明诊断，不建立按 arity 重载的类型名称。

普通非局部 public／internal 分支可以分布在同 Cone 同 package 的不同源文件；file-private 类型只能在同一文件归组。嵌套族必须属于同一个实际 owner。外层类型的不同定义分支分别拥有自己的嵌套声明，不能仅因内部名字相同就把它们合成同一嵌套类型。

所有分支的集合在该 Cone 正常收集声明后封闭。源码排列、文件枚举、import 顺序、第一次使用哪个分支都不影响选择。不要求无条件分支先声明；也不存在依赖某条使用点“之前可见的部分分支”的规则。

### 2.2 局部族与捕获

局部类型仍不进行跨语句的前向查找。同一局部族的分支必须连续声明在同一个块中；注释、空白和各声明的注解不打断该组，其他声明或语句会结束该组。族名在各分支自己的 body 和整组结束之后可见；组结束后再次以同名声明追加分支是重复声明错误。

各分支的外部词法环境均取组开始处；不能通过分支排列把组内某个局部 binding 引入其他分支。局部 class 仍只捕获允许的不可变 binding，所选分支使用自己的捕获字段与初始化；局部 struct／enum 仍不能捕获运行期值。捕获值不参与类型选择或类型身份。跨泛型实例化保留原词法族身份与外层类型实参。

inner 分支共享同一 outer 类型契约和构造所需的 outer 参数。外层对象 identity 不参与选择；每个分支仍按普通 inner class 规则保存 outer 引用。

### 2.3 分支完整性

每个分支独立定义字段、primary／secondary constructor、enum variant、基类／接口、成员、companion、初始化项、release block、注解和 class 修饰符；按对应 kind 的普通规则检查。不从较宽泛分支补入任何遗漏项目，也不提供“调用上一个特化”的特殊 `super`。普通 class 继承中的 `super` 保持原义。

分支具体性只决定选择，不建立分支间的继承关系，也不改变泛型不变性。即使 `Derived <: Base`，且两个 application 选中同一分支，`Box<Derived>` 也不会因此成为 `Box<Base>` 的子类型。

字段名字、类型和数量可以变化；enum variant 集合可以变化；class 的父类、abstract／final／open／sealed 状态可以变化。变化影响第 5～8 节的可用契约，不能被泛型调用者忽略。各分支仍分别遵守 struct 的不可变字段、interface obligation、override、sealed 归属、访问域、初始化与效果规则。

所有分支都在定义处检查，包括当前程序尚未实例化的分支。正文类型错误不是“条件不满足”，不能使编译器跳过该分支、回退到别的分支或等待某个消费者才诊断。确实依赖完整表示的既有检查，如泛型 `@NoGC`／FFI 类型条件，继续按原规则保存并在具体化时落实。

## 3. 条件含义、规范化与三态关系

### 3.1 可写条件

inline 和 where 条件合并为合取。正向 bound 仍只允许完整 class／interface application；每个 subject 至多一个 class 上界，正向 nominal bound 与 kind bound 仍互斥。负向条件独立保存，可以和 `value`／`ref` 组合，只能否定完整 interface application。`!Class`、`!value`、`!Any`、`!T` 均非法。

where subject 支持 T 与固定的 `T.Companion`，其中 T 是本族自己的类型参数。二者的 bound 分开检查，例如 `T : value, T.Companion : ValueEncoder<T>` 合法；对 T 的 nominal／kind 互斥不跨到 T.Companion。companion subject 接受完整 class／interface 上界、负向 interface 和 `ref`；`T.Companion : value` 与 singleton 的引用性质矛盾，在声明处报错。`T.Companion : ref` 表示直接 public companion 存在，不要求额外接口。

任一 companion 条件都包含“实际静态类型直接拥有 public companion”的存在性要求，不能借基类 companion 或使用 private／internal companion。先登记当前分支所有投影存在条件，再解析完整条件与签名；投影可用于该分支的类型、字段、参数、结果、默认值和正文，也可以作为 companion 值访问。只有使用 `T.Companion` 不会隐式增加选择条件；无存在条件的未知 T 不能使用投影。类型引用保持固定的一层投影，不允许 `T.Factory`、任意 associated type 或 `T.Companion.Companion`。

条件只描述参数的类型关系，不查询运行期值，不初始化 companion，不调用普通函数或成员，不按同名方法推导 interface conformance。`T : Equals<T>` 等已有 F-bound 保持合法；重复条件、直接正负矛盾和普通继承闭包已能确定的矛盾在声明处报错。

族内各分支的条件不叠加。例如无条件分支的 T 没有从其他分支继承 Debug bound；也不存在所有分支必须先满足某个“主模板 bound”的隐含门槛。

### 3.2 成立、不成立、未知

关系查询复用 M55 三态规则：

- 完整静态类型按显式继承／接口闭包判断；确定不实现接口才满足相应负向条件。
- 对未知类型参数，没有 `T : Debug` 的证据不等于有 `T : !Debug` 的证据。仅有 Base class 上界也不排除实际子类型实现 Debug。
- 若 `RichDebug : Debug`，正向 `RichDebug` 推出 `Debug`，负向 `!Debug` 推出 `!RichDebug`；反方向不成立。
- `I<U>` 与 `I<String>` 的关系在 U 未确定时可以未知；不能把 binder 不同当成实际类型不相等。
- `Nothing` 按现行底类型规则满足正向上界，不满足负向接口条件；kind 规则不因本功能改变。

companion 条件先判断 public companion 是否存在，再判断其名义关系。以下表格中的 I 是同一个完整 interface application：

| 实际静态类型的直接 companion | `T.Companion : I` | `T.Companion : !I` | `T.Companion : ref` |
| --- | --- | --- | --- |
| 未直接拥有 public companion，包含仅基类有 companion | 不成立 | 不成立 | 不成立 |
| 有 public companion，且实现 I | 成立 | 不成立 | 成立 |
| 有 public companion，且确定未实现 I | 不成立 | 成立 | 成立 |
| 是否存在／是否实现仍未知 | 按已有事实作三态判断，不能猜成立 | 按已有事实作三态判断，不能把未知当否定 | 仅存在性已保证时成立 |

因此 companion 的正负条件仅在存在性已保证的域内互补。只有 `T.Companion : I`／`T.Companion : !I` 两个分支时，无 companion 的参数会无匹配；需要普通兜底覆盖它，而不是把它塞入负向分支。完整实参缺少 companion 是候选不适用，不是整个带兜底类型族的非法投影错误；分支内部存在投影的源码合法性仍须独立检查。Nothing 没有 companion，也不会借底类型关系凭空获得一个。

对 companion 接口的继承蕴含与普通 subject 相同，但 `T : I` 和 `T.Companion : I` 相互不能推出。class 上界不提供未知子类型的直接 companion；即使 Base 自己有 codec，Derived 也须按 Derived 自己的 companion 判断。K／V 本身若是另一个条件类型族，先对该完整静态 application 选择其定义，再查询它的 companion；未决 application 使用已保证的共同关系。

`Box<Base>` 始终按 Base 的静态关系选择，即使传入值的运行期类型是实现 Debug 的 Derived。smart cast 只影响普通表达式的静态类型；不能改变一个已经存在的 `Box<Base>` 的布局或使它成为 `Box<Derived>`。

### 3.3 规范化

条件身份使用真实类型身份、族参数位置及外层 binder，展开透明 alias，合并 inline／where，忽略书写顺序和诊断位置。保留 subject 是参数本身还是其 Companion 投影，以及投影存在性、完整 bound 和正负方向；重复使用同一投影不产生多份存在性身份。实现接口列表、字段、正文和声明顺序不参与条件比较。

规范化条件集合相同的两份定义是重复分支。条件集合不同，但按下节的现有关系规则能相互转发时，也视为等价分支并在声明处报错，例如 `T : RichDebug` 与 `T : RichDebug, T : Debug`。不能证明等价的条件不靠文本或猜测合并；可能在使用处产生歧义。

## 4. 选择规则

### 4.1 “更具体”的唯一含义

把分支 A、B 的自身形参按位置对齐，并保持为 rigid 参数。若以 A 的显式条件为已知事实，按普通类型关系能够满足 B 的全部条件，而反向不能，则记 `A > B`，表示 A 更具体。这里复用 M55 的条件转发关系，不按条件数量、正／负方向、源码先后或正文能力赋权。

此偏序针对族声明确定，不随调用点额外事实改变。一个分支为 Box 多实现了 Hash，不会因此比另一个分支更具体；只有对参数的选择条件参与排序。现有名义关系不能推出的任意逻辑等价，不引入通用定理证明来补齐。

companion 条件参与同一转发比较：`T.Companion : ValueEncoder<T>` 蕴含投影存在和 `ref`，但不蕴含 `T : ValueEncoder<T>`。HashMap 的四项 codec 条件连同基础 Hash／Equals 条件可转发到基础分支，反向不行，所以 codec 分支更具体；E 与 D 彼此不可比较，E 且 D 则同时比二者具体。

### 4.2 完整实参的选择

对合法完整参数向量 A：

1. 判断每个分支条件，得到适用集合；只有成立的条件才匹配。
2. 去掉被另一个适用且更具体的分支压过的候选。
3. 恰有一个剩余分支则选中；没有适用分支时报无匹配；剩余多个时报歧义，并列出这些不可比较的分支。

完整静态类型的关系查询若遭遇非法循环，应报告循环，而不是把它当作 false／unknown 后选兜底。abstract 类型、interface 类型和无实例的 Nothing 也可成为静态实参，选择不要求存在运行期对象。

第 1.2 节中 `Debug + Hash > Debug > 无条件`，也有 `Debug + Hash > Hash > 无条件`；Debug 与 Hash 彼此不可比较。删除第一分支后，两个接口都满足的 application 报歧义，兜底不会消除歧义。交叉分支本身允许声明，错误发生在不能唯一选择的使用处；不要求编译器预先枚举未来所有用户类型。

若更具体分支已胜出，但没有本次需要的构造器／成员或不实现期望接口，按所选定义报错，不能退回较宽泛定义。

### 4.3 含未知参数时：合法性与可能胜出集合

含外层参数的 `Box<U>` 仍是参数位置齐全的名义 application，但不一定已经确定分支。需要分别回答：在当前定义环境 Γ 下，该 application 是否保证存在唯一结果，以及结果可能来自哪些分支。

令分支条件为 `G_b`，则一个分支未被更具体候选压过的条件是：

```text
Maximal_b = G_b AND NOT(OR G_c for every c > b)
Unique = exactly one Maximal_b holds
```

编译器在该族有限的条件关系上作必要的情况划分：已知 true／false 先化简；普通 subject 上同一完整接口关系的正负查询互为补集，Nothing 按 3.2 的特别规则处理；companion subject 必须单独保留存在性。概念上 `T.Companion : I` 是 `HasPublicCompanion(T) AND Implements(T.Companion, I)`，负向条件则是 `HasPublicCompanion(T) AND NOT Implements(T.Companion, I)`；缺失投影时不查询其接口。二者的并集只覆盖“存在 public companion”，不是 true。已知继承蕴含、参数相等和 kind 关系排除不可能组合。未知保留为未知，不假定未来没有新增的参数类型。

对剩余每种未排除情况都能确定唯一最优分支，才能在 Γ 中使用该 application；所有这些结果组成可能胜出集合 W。W 是保守集合：若现有关系规则不能证明某种情况不可能，就保留它。因此实现不需要求解任意类型逻辑，但不能遗漏潜在胜出分支。

- 有完整四分支时，无约束 `fun <T> keep(x: Box<T>): Box<T> = x` 合法；W 包含四个分支，无需现在确定布局。
- 只有 Debug 分支时，上述无约束签名不合法；`T : Debug` 的函数可以使用它。
- 有 Debug、Hash、兜底但无交叉分支时，未知 T 不能排除重叠歧义，该无约束签名也不合法；`T : Debug, T : !Hash` 的环境可以使用。
- 正向／负向 Debug 两分支可共同覆盖所有合法静态参数；无需从未知 T 任选一个分支。
- 第 1.1 节的基础／codec 两分支，在 K 满足 Hash／Equals 的环境下共同保证 HashMap 合法，即使 K／V 的 companion 能力未知；未知时只能使用共同集合能力和显式 codec 工厂。四项 codec 条件都已保证时，才确定该例的默认 codec 分支。

这是有限候选的普通类型检查，不增加运行期选择、产物证明或资源配额。实现可按需分裂条件、提前剪枝并复用本次 lowering 的查询结果，不要求构造“所有可能类型”的集合或引入通用求解框架。

## 5. 定义处检查与分支内部环境

### 5.1 外部泛型正文

定义处已经唯一确定分支时，使用该分支的普通类型契约；尚有多个可能分支时，使用第 6 节的共同契约。不能因为一个宽泛分支的条件已经成立，就忽略条件仍未知的更具体分支。

例如在以下独立类型族中：

```scoop
struct Data<T>(val value: Int)
struct Data<T>(val value: String) where T : Debug

fun <T> read(data: Data<T>): Int = data.value
```

`read` 在定义处报错：value 在可能胜出分支中的类型分别是 Int 和 String。不能先按兜底的 Int 检查，通过后在 `T : Debug` 的实例中生成 String 读取；也不自动把字段类型扩大到 Any。

同样，若 Debug 分支使 `Box<T> : Debug`，更具体的 RichDebug 分支却没有实现 Debug，则只知道 `T : Debug` 不足以把 Box 转成 Debug。允许这种完整替换，但调用者必须承受它缩小共同契约的结果。反之，第 1.2 节的两个可能分支都实现 Debug，故 `T : Debug` 足以保证 `Box<T> : Debug`。

实例化只把定义处已经选定的普通调用、接口槽或族成员契约落实到具体实现，不重新对泛型源正文做名称查找、重载选择或“试到能编译为止”。分支新增重载不会夺走正文已经绑定的另一个重载。

### 5.2 分支自己的正文

检查分支 b 时，环境包含它显式写出的参数条件，并额外知道：**当前族以这组宿主参数应用时，所选定义就是 b**。这是分支实现的词法事实，不是新源码类型、隐式 Self 或 runtime witness。

据此，`this`、同一完整宿主 application 的参数／局部值、该 application 的构造与 companion 可以使用 b 的完整契约。否则无条件分支连自己的私有字段都无法可靠检查。此事实随分支正文中的 lambda、局部 callable、默认值和初始化模板保留；不会作为额外类型实参传播给外部普通泛型函数。

分支头中显式建立的 companion 存在性与 bound 也随宿主 binder 进入其成员、companion、默认值和初始化环境，包括没有自身类型参数的普通方法。第 1.1 节因此可以直接把 `K.Companion`／`V.Companion` 传给 codec helper；按 M55 的转发语义，`K.method(...)` 在有该投影条件的环境中指向 `K.Companion.method(...)`。签名与正文先固定接口／class bound 契约，实例化替换真实 singleton。普通 static nested 声明仍是宿主参数的作用域边界，不因本规则隐式捕获 K／V。

该事实只适用于参数逐项相同的 application。分支正文中写 `Box<Other>` 必须重新按 Other 判断，不能强行构造当前分支。成员若返回 `Box<T>`，在当前分支的有效实例范围内就返回同一族 application，不产生“分支私有 Box 类型”。

无条件分支不自动获得 `T : !Debug` 等源语言负向 bound；需要按负向 bound 调用普通泛型函数时，应显式写条件。分支选择事实用于确定这个族的定义，不任意反向推导参数的新接口条件。对无条件分支而言，普通函数重载仍按其显式条件在定义处绑定。

### 5.3 M55 函数重载与类型选择的区别

M55 函数调用在定义处选中一个声明，实际参数变具体后不改选同名函数。M57 的 `Box<T>` 则先绑定类型族，完整类型参数决定该族的唯一定义；定义处如尚未确定，只能绑定共同契约。

二者都保持已检查契约有效，但绑定的对象不同。不得把函数的“现在选兜底声明”直接移植为类型的“现在固定兜底布局”，也不得借 M57 顺带改变普通函数的重载时机。

## 6. 未决分支的共同契约

共同契约只取当前 W 中每个分支都保证的能力，不合并各分支能力的并集，也不要求所有分支长成同一形状。只传递、保存、返回一个合法的 `Box<T>` 不需要读出其成员或提前计算尺寸；实际表示继续在单态化时确定。

| 操作 | 定义处必须保证的事实 |
| --- | --- |
| 转成 interface／class 上界 | 每个可能分支的普通父类型闭包都包含目标 exact application |
| 读取属性／字段 | 每个分支都存在普通访问规则允许的同名读取，替换后的结果类型相同 |
| 写属性／place 操作 | 每个分支都有合法 setter／可写 place，并具有同一输入类型和所需调用契约 |
| 调用方法 | 存在各分支共同提供的完整方法契约，按下述规则进行一次重载选择 |
| 调用构造器 | 每个分支都可构造，并提供所用完整构造契约；见第 7 节 |
| 解构／operator | 各分支都提供对应的普通字段位置或 typed operator 契约，类型与调用形态一致 |
| enum variant 构造／模式 | 对应 variant 在各分支都存在，payload 的形态、参数位置／名称与类型一致 |
| 静态描述、布局、GC 属性 | 具体结果由最终分支产生；需要静态保证的操作继续检查相应条件，不能读取兜底作为代表 |

方法契约比较保留名称、参数顺序和名字、完整参数／结果类型、receiver、方法自身泛型参数及其规范化条件、default 位置、vararg、suspend、operator／infix 等调用角色。类型引用以族 binder 对齐后的身份比较，不能只比较显示名称。结果类型不同不形成自动公共返回类型；需要较宽接口视图时可显式向共同接口转换，再按该接口契约调用。

共同候选通过普通重载规则在定义处选定。每个分支提供与该契约对应的唯一成员映射，具体化只查该映射；该分支额外具有的其他重载不重新竞争。例如所有分支都有 `f(Any)`，部分分支另有 `f(Int)`，泛型正文绑定的共同 `f(Any)` 在后者中仍调用 `f(Any)`。

效果采用各实现都能兑现的承诺：suspend 形态必须相同；只有全部实现 safe 才提供 safe 契约，否则按 unsafe 调用要求；只有全部实际调用都满足 NoGC 才提供 NoGC 保证，否则为 Managed。默认值、vararg 组装、companion ensure 仍参与既有效果检查。可见性取使用点在所有可能目标上都拥有的访问能力，不因某个分支是 public 就暴露另一个分支的 private 成员。

default 表达式可以不同，但可省略的参数位置和调用契约必须相同；实例化引用对应分支在定义处绑定的 default 模板，不到调用者作用域重新查找。求值次数和次序遵守 8.5，探测候选不执行任何表达式。

字段读取可以映射到不同 offset，属性读取可以映射到不同 getter；不提前固化布局。读、写、addressOf、复合赋值等操作分别保留自己的 place 条件；有共同 getter 不等于存在共同可取地址的 storage。

族成员契约是编译期的 typed 操作引用，不是新的结构型 interface conformance，也不是用户可写的交叉类型。成员实现相同或不同都不改变类型身份。通过共同契约形成普通 callable reference 时也要固定同一契约及完整函数类型；不能把尚未决定重载的函数值留给具体化选择。

## 7. 构造、推断与显式实参

已有完整类型实参时，先选唯一分支，再对它的普通构造器／variant 执行参数映射和重载选择。例如 `Box<X>(value)` 不能因所选分支的构造参数不匹配就改用兜底。

省略实参或写 `_` 时，保留既有整组参数推断、postponed argument、命名／spread／default 规则，并加上以下一致性要求：

1. 在各可供探测的分支构造签名中进行 candidate-local 普通推断，正向 bound 按现行规则参与，负向条件在类型确定后检查。探测不执行 default、初始化或方法正文。
2. 对推得的参数向量按第 4 节选择类型定义。只有原构造器确属该胜出分支，候选才成立；不得保留被更具体分支覆盖的构造器。
3. 若不同分支路径得到多个不同的合法完整参数向量，报类型实参推断歧义，要求显式实参；不按分支条件的强弱擅自替调用者选择参数类型。
4. 参数向量相同的候选只可能来自同一胜出分支，其内部按普通构造器重载规则选择。不同分支的构造器不组成运行期可调用的联合重载表。

若推断结果仍含 caller 的 rigid 参数，如从 `value: U` 得到 `Box<U>`，使用第 6 节的共同构造契约，而不是把某个未知条件视为满足。所有可能分支具有同形 `Box(value: U)` 时可正常构造，具体化替换为所选构造器。

只有返回／期望类型、其他实参等既有推断来源能够约束类型变量；接口否定不负责“找一个未实现接口的类型”，也不能把已确定的实际类型改成 Any 以通过候选。companion 投影不反向推断宿主：不能因需要某个 ValueEncoder 就猜 K 是哪种类型；先从普通位置确定 K，再检查其 companion。只写类型名、查看字段或尝试失败的构造均不能改变类型族选择结果。

类型位置仍须写满所有类型实参，不允许 `_`、裸 generic 类型或部分 application 语法。透明 alias 展开后使用同一族与实参，不建立 alias 专属的选择缓存或身份。

## 8. 与既有语言能力的组合

### 8.1 接口、继承与动态分派

每个分支独立满足自己声明的全部接口，生成该分支需要的 itable。不同 application 的接口集合可以不同；同一个 application 经不同调用路径得到的集合必须相同。继承闭包仍按完整 application 区分 `I<Int>` 与 `I<String>`。

从未决族继承时，必须对每个可能的 base 分支通过普通继承检查：允许被继承、基类构造可调用、abstract obligation 得到实现、override 契约成立且访问合法。子类正文使用共同 base 契约，`super` 调用固定相应映射；任一可能分支为 final 或存在不满足的 obligation，就在子类定义处报错。不能在某次实例化中忽然增加未经检查的虚槽。

虚方法依旧按实际对象类型动态分派；类型分支只按静态完整类型参数选择。这两层机制不混合。generic virtual/interface method 的现有排除项保持，不借共同契约引入泛型虚槽。

sealed 直接子类型关系保留各分支的条件和实际父 application；定义 Cone 提供完整关系。穷尽性只统计当前 subject 下可能存在的子类型，不因消费者没用到某个分支而遗漏它。无法在定义处证明所有可能分支都被覆盖时，正常要求 else。

### 8.2 enum、模式与派生

具体 enum application 只具有所选分支的 variants 和 payload 布局。未决 application 只能命名共同存在且 payload 契约一致的 variant；其余情况使用 else 或先把环境约束到可确定的分支。

穷尽性对 W 中每个分支检查：只覆盖共同 variants，且某个分支还有其他 variant，则需要 else；所有分支都被覆盖才可省略。不能把所有 variants 的并集当成一种固定 enum 布局。普通 `is`／`as` 使用真实族 application identity，不按分支显示名称产生新运行期类型。

结构相等、解构、静态描述、codec／inspector 方法合成读取所选定义的形状。泛型定义处申请合成时，其正文所需的字段类型、codec／inspector 来源和访问必须已经成立：在某个分支内按该分支检查；在外部未决环境按共同保证检查。不能到具体化时重新搜索另一个默认 codec、同名 helper 或消费者的扩展。

### 8.3 companion、嵌套类型与 M55 要求

各分支可以提供自己的 companion 定义。`Box<A>.Companion` 的逻辑身份取决于族和完整宿主 application；只物化所选分支的 singleton、storage 和初始化模板，同一个 application 跨 Cone 仍只初始化一次。分支判断、类型查询和静态描述本身不触发初始化。

未决宿主只有在所有可能分支都直接提供可见 companion 时才能使用这个投影；成员调用采用相同的共同契约规则。M55 对 public companion 的要求仍逐项适用，不能把 internal／private companion 当成 public，也不能借基类的 companion。命名 companion 的别名按各分支的普通名称规则处理；通用 `Companion` 入口不制造多个 singleton。

区分作为选择输入的 `K.Companion`／`V.Companion`，与选择结果所提供的 `HashMap<K, V>.Companion`。前者读取参数类型的声明能力；后者才决定 HashMap 能否通过 Json 的默认 codec 重载。不能假设当前 HashMap 分支将要提供某个接口，再以此证明自己的选择条件。codec 参数值的内容、配置及 singleton 的初始化成功／失败状态均不参与分支选择；用户仍可显式传入不同 codec 策略。

M55 函数以完整 `Box<A>` 为实参时，负向接口、companion 和公开成员要求查询其已选定义；以含 rigid 参数的 application 为实参时，只能使用定义环境已经保证的关系和契约。M55 要求绑定到族契约后，具体化只落实该契约，不重新选择普通正文的重载。要求本身不向类型族追加接口。

HashMap 用例中，带相同 K／V codec 条件的泛型函数可以把 `HashMap<K, V>` 交给 Json 默认重载：检查先由这些条件确定宿主分支或其共同 codec 接口，再满足 M55 对整个 HashMap companion 的条件。无这些事实的函数不能因为未来某次调用的 K／V 恰有 codec 就通过检查；它仍可使用现有显式 codec 参数。像 `HashMap<String, HashMap<Int, String>>` 的条件查询可沿实际嵌套参数向内选择，并复用内层 companion 的声明关系。

普通嵌套类型保持自己的名义身份，不能按各分支中同名的 `Nested` 自动形成共同类型。选择依赖宿主分支时，必须提供足以确定分支的完整宿主 application；分支未定且目标不是同一已绑定声明时，诊断不能确定嵌套类型。static nested 的非捕获规则不变，不因限定路径写了宿主实参就隐式捕获它们；inner 的完整 outer application 则照常参与身份。

## 9. 循环、有限布局与表示合同

### 9.1 选择不能依靠自己的结论

选择查询只使用已声明的类型关系和正常参数替换，不为了匹配而展开任意成员正文。若确认某条件需要先选中当前分支，而选中当前分支又依赖该条件成立，必须报告选择／接口关系循环，不能假设 conformance 或把未完成查询当作负向事实。

companion 存在性／接口查询进入同一依赖链。参数类型已有明确 companion codec 声明时，不必先检查或执行 encode／decode 正文才能使用这条名义关系；普通递归数据的手写 codec 不因此构成选择循环。只有 codec 关系本身依赖尚未确定的分支、并最终回到待证明条件时才诊断；正常嵌套容器对更小参数 application 的查询继续合法。

活动查询栈保存真实族、完整查询实参与所查询关系；同一规范化查询再次出现即可定位闭环。通过声明依赖环持续扩张实参的查询按既有泛型递归检查原则拒绝，诊断导致扩张的边。对输入类型树的严格子项递归，例如判定 `Box<Box<Int>>` 时先判定 `Box<Int>`，不是这个错误。普通不需要循环证明选择的 F-bound 和相同已确定实例的正文递归继续合法，不用深度／数量配额替代这些规则。

查询只展开判定当前关系实际需要的声明边，不先物化整个字段图或所有接口实参的布局。普通递归类型中已经足够确定的接口事实不能因为这种无关展开而被误判成选择循环。

### 9.2 有限值布局

只将实际内联字段／payload 加入布局依赖，引用／指针仍切断内联路径，phantom 参数仍不产生存储边。建立依赖边时带入可同时成立的分支条件，不把互斥分支的字段直接并成一个结构。

多分支类型以具体定义分支作为布局递归节点；一条可成立的内联路径再次进入同一值定义分支时，即使实参改变也按现行有限布局原则拒绝。进入同族的另一个已确定、非递归分支不自动构成环。普通单分支类型是此规则的原有特例。不增加靠不断改换参数、期待将来某次条件终止的任意类型级递归。

需要依赖选择结果才能确定的边在相应类型具体化时落实；已在定义处发现的非法布局立即报错，不留到 LLVM 或 runtime。选择循环与内联布局循环使用不同诊断，避免把正常引用递归误报为选择递归。

### 9.3 GC、FFI 与注解

分支注解只作用于该完整定义；无条件分支上的 `@NoGC`／`@CLayout` 不继承给其他分支。选中分支仍必须满足自己的 GC-free、layout 和 FFI 合同，失败不能回退。

未决族作为字段、数组元素、协程局部或返回值时保留完整类型表达式，具体化后按所选布局生成 scan、boxing、copy 和 ABI 分类。需要 NoGC／GC-free／CLayout 的表示条件沿既有泛型机制传播，完整 application 检查实际选中分支；不能因未决类型现在没有具体布局就拒绝所有泛型 NoGC 用法。若正文检查需要直接使用某项保证，则必须对所有可能分支成立，可取地址也须具有共同 storage 契约。不能把“nominal kind 同为 struct”当作表示相同或 GC-free。

不改变 runtime 的对象模型、GC 扫描协议或调用 ABI。每个 materialized exact type 仍只有一份 TD 和确定的 layout；不同类型参数即使选中同一分支、产生相同布局，也不共享名义类型身份。

## 10. 编译阶段、typed identity 与产物

### 10.1 实体区分

下表是必须表达的语义区分；具体 Rust 命名可服从现有 kind-specific ID，不另造一套平行身份注册机制。

| 实体 | 身份内容及职责 |
| --- | --- |
| Generic nominal family | 原 Cone、typed owner、作用域、kind、名称与 arity；不含某个分支的 guard 或正文 |
| Nominal definition branch | family 与规范化条件；拥有自己的成员、字段、构造器、默认值和源码来源 |
| Nominal application | family、完整自身参数及既有 outer／enclosing application；含 rigid 参数时仍是明确的 application |
| Family operation contract | family 与 binder-normalized 的完整成员／构造／variant 操作契约，保存能够兑现它的分支到真实目标的映射；使用时检查 W 被覆盖 |
| Concrete nominal | 完整实际 application 及已确定的 selected branch、布局输入与成员映射；不能用未决 application ID 代替 |

族身份不因只增加一个条件分支就变成另一个类型；族的导出内容变化仍使正常依赖失效。分支成员 key 保留 branch owner，防止不同分支中同形方法／字段被误合并。分支顺序、文件遍历序号、显示名称和消费者都不进入选择身份。

普通单分支类型使用同一表示，避免维护“旧泛型类型”和“新特化类型”两套语义。为局部族保留原词法组身份，为 inner 保留 outer application；不要用 FQN 或 arena 整数回填缺失引用。

### 10.2 AST → Export HIR

Parser 复用现有 nominal 头、bound 与 where AST，包括 M55 已有的参数／Companion subject；保留每个分支及条件的诊断位置。声明收集在源码类型检查阶段归组并检查组的一致性、重复条件、普通可见性及局部连续规则；在解析分支签名前先登记其投影存在条件。

HIR lowering 负责条件关系、偏序、application 合法性和共同契约查询。各分支正文在自己的环境中检查为完整 typed body；外部泛型正文的操作保存固定普通目标或族契约引用，不保存“以后重查这个名称”的字符串。

Export HIR 保存完整族目录、分支条件与有类型的关系、每个分支的定义／签名、所需支持声明、已绑定模板及用到的族操作映射。未决选择用明确的数据分支表达，例如 `SelectedBranch` 与 `ConditionalFamilyApplication`；后者包含完整 application 引用，不能以 `Option<TypeInfo>`、空字段表或兜底布局伪装成已完成类型。

companion 条件复用 M55 的固定投影、原 host binder、存在性、名义／kind bound 和独立负向条件数据，不降成显示名称字符串。分支的 companion 方法、默认值和初始化模板须保留这些宿主条件及已绑定投影引用，跨 Cone 消费不能丢掉宿主条件后再按方法自身形参重建。参数本身的 bound 与 companion bound 不能共用一份混合列表。

偏序和成员对应关系在定义方建立后作为普通语义数据复用。IR／meta crate 只承载数据、格式和引用，不复制一套条件判定器，不把成功检查包装为凭证链。查询结果不变时在 lowering 内复用。

### 10.3 Export HIR → LocalConcreteHir

实际类型参数完整后，选择唯一分支，落实其字段、基类／接口、variant、初始化、companion 与正文；替换预先绑定的族操作映射。该步执行当前实际 application 所需的选择及表示条件，不重新解析 provider 源码或重选已绑定的普通重载。

输入的 `K.Companion`／`V.Companion` 替换为原静态参数类型真实的完整 companion application，其类型实参、singleton 和 ensure 沿用 M55。选择查询不生成初始化；只有正文的值访问产生普通 ensure。不会合成额外 codec 参数、运行期字典、分支选择状态或新的 singleton 协议。

只有被选分支中实际需要的机器定义进入物化闭包；其他分支保留可消费的模板／签名即可，不因提供完整候选目录就生成所有代码。实际产生的调用边继续参加泛型递归检查。

LocalConcreteHir 必须只有确定的普通类型、构造和调用，所有必要类型、目标和布局输入完整。MIR、LIR、codegen、linker 和 runtime 不识别源码条件，不承担“发现 unknown 后补选一个分支”的职责。

## 11. 跨 Cone、一致性与演进

导出可使用的族时必须提供其完整分支集合和正常消费所需的 support closure，不能按消费者的 import、已用实参或成员可见性裁剪候选。私有实现细节作为模板支持声明保存，不因此变成 public 名称。消费者先解析到原族，再按同一完整目录选择。

同一 family 和同一完整参数向量，在 provider、本地消费者、再次发布的模板、不同优化级别与不同调用路径中必须得到同一分支。分支集合的封闭只针对类型定义权；后续 Cone 仍可定义新参数类型并用它实例化这个族。

名义 specialization 的语义 key 继续为 `{origin generic family, complete exact arguments}`，嵌套／局部 owner 关系按现有 application 编码保留。selected branch 是该 key 的确定结果，不把它、调用点、import、额外 witness 或“从哪个约束进入”加成新的类型参数。分支成员仍有自己的声明身份，具体 callable key 引用该声明与完整 owner application。

同一 application 的 TD、layout、dispatch、static storage 和 companion 初始化状态继续走既有 ODR 规则；不能由不同 consumer 各保留一种分支。linker 不重新求条件，也不通过比较源码／正文决定赢家。

增加、删除或修改分支，以及修改参数类型的显式接口关系、直接 companion 的存在／public 可见性／接口关系，都可能改变选择、构造方式、接口集合和布局。这些是普通源码／导出接口变化，可能构成源代码或 ABI 不兼容；不能承诺“增加一个接口总是兼容”。依赖变化按现有构建更新判断重新编译，不增加缓存内容审计、语义 replay 或新的生产收据。

本方案中的 `T : !I` 描述当前完整静态类型的关系，不是对未来版本永不实现 I 的承诺。若库需要稳定契约，应让相关分支持续保证该契约，或使用显式接口／包装类型表达；不引入 Rust 式负向 impl 声明来实现此功能。

实际导出编码变化必须升级相关 HIR/meta 格式及必要的持久化编码版本，旧 reader 正常报告不兼容；不在设计稿中猜测实施时的版本号。只有实际 runtime 数据编码变化才考虑 runtime ABI 升级；本方案本身不要求它。

## 12. 权威规范同步清单

此表用于实施前修订现行合同，不将本设计稿与当前“仅泛型函数可声明负向／companion 条件”的规范混作同一版本。三份规范更新后再实现对应行为，并同步引用章节。

| 文档／章节 | 必须同步的内容 |
| --- | --- |
| 语言 3.2、3.2.2～3.2.3，新增 3.2.5 | nominal family／branch、普通与 companion 条件、投影存在性与三态关系、偏序、抽象合法性、完整替换、定义处共同契约；扩展 nominal 的负向／companion 条件范围，保留 requires 为函数级 |
| 语言 3.2、4.1～4.2 | application 身份、选择后的字段／payload、值布局循环以实际定义分支判断 |
| 语言 7 的模式／穷尽性、8.5～8.6 | 共同 variant、逐分支覆盖，构造推断与唯一选择；保持函数重载绑定时机 |
| 语言 9.1.3～9.1.7、9.3、9.5 | 局部连续组、inner／nested／companion、访问域、继承与 override、sealed、operator、alias 组合 |
| 语言 9.6、11.11、11.13、11.17 | 所选定义的描述／相等／codec／inspector 与定义处派生要求；HashMap 默认 codec 的条件接口与保留显式 codec 工厂 |
| 语言 12.4～12.5、13.2～13.5、13.9、15 | 导出完整族、格式兼容、分支效果与表示条件、选择条件边界和明确排除项 |
| 实现 2.1～2.2、2.6、2.9、2.11、2.13、2.17 | AST 收集、族／分支／契约的 typed identity，Export HIR 与 LocalConcreteHir 边界、meta 编码、dispatch 与 ODR／描述生成 |
| runtime 2.2、2.3、2.7 | 明确单个 exact application 只采用一个分支的 TD、值布局和 companion 状态；既有 ABI 与初始化协议保持 |

## 13. 实现入口与交付顺序

以下路径是设计时确认的现有入口，调整时应沿职责扩展，不在 IR/meta 中另实现语言语义：

| 职责 | 现有入口 |
| --- | --- |
| nominal 身份、声明与嵌套 | [persistent_nominals.rs](../../compiler/hir-lower/src/persistent_nominals.rs)、[declarations](../../compiler/hir-lower/src/declarations)、[local_types/declarations.rs](../../compiler/hir-lower/src/local_types/declarations.rs) |
| 条件及具体性 | [types/constraints](../../compiler/hir-lower/src/types/constraints)、[constraints/companions.rs](../../compiler/hir-lower/src/types/constraints/companions.rs)、[overload/specificity.rs](../../compiler/hir-lower/src/overload/specificity.rs)、[generic_entities.rs](../../compiler/hir-lower/src/generic_entities.rs) |
| 导出名义定义和模板 | [nominal_interfaces](../../compiler/hir/src/cross_cone_interface/nominal_interfaces)、[generic_templates](../../compiler/hir/src/cross_cone_interface/generic_templates) |
| 实际条件、模板具体化与递归 | [concretize](../../compiler/hir-lower/src/concretize)、[effects/generic_recursion.rs](../../compiler/hir-lower/src/effects/generic_recursion.rs) |
| HashMap 与默认 codec 闭环 | [hash_map.scoop](../../sysroot/src/scoop.std/src/collections/hash_map.scoop)、[map_codec.scoop](../../sysroot/src/scoop.std/src/collections/map_codec.scoop)、[json.scoop](../../sysroot/src/scoop.json/src/json.scoop) |

按功能纵向交付，每一步完成对应正常源码编译、产物消费、链接与运行，不先搭建一个无真实消费者的通用特化框架。

| 阶段 | 交付与出口 |
| --- | --- |
| M57-1：类型族与 HashMap 闭环 | 同步基础规范；族／分支身份、普通与 companion 正负条件、存在性、重复／等价诊断、偏序和唯一赢家；基础 class 继承、共同基类／构造及分支 companion 必须在本步可用。完成第 1.1 节 HashMap 默认 codec 的本地与跨 Cone JSON 往返，缺 codec 的普通 HashMap 和显式 codec 工厂仍可用，并覆盖 struct 四分支／歧义 |
| M57-2：泛型正文与构造 | 在首个闭环上扩展一般有限条件划分、可能胜出集合、不同形状下的共同接口／成员／构造契约、完整实参和 `_` 推断；覆盖带 K／V companion 条件的 generic Json 转发与独立编码／解码四分支，防止缺失投影被当作负向成立 |
| M57-3：完整名义行为 | class 继承／override／sealed，enum variant／模式，效果、有限布局、GC／FFI、派生与静态描述；不同分支走普通后端布局与 dispatch |
| M57-4：作用域与组合 | 嵌套／inner／局部连续组、捕获、companion、default／lambda、泛型成员和 M55 条件／要求；跨 Cone 再次发布和多个 consumer 共享 ODR 身份／singleton |
| M57-5：完整验收 | 完成格式迁移及依赖重建，补齐错误诊断和组合 fixture，执行正式 CLI 全量与支持平台验证，记录结果及实际限制 |

M57 不要求 M56 的性能调查继续扩展。依赖 M55 已建立的语言关系和数据合同；实施时对照其实际完成状态接入，不以旧设计中的占位状态替代真实可用能力。

基础 companion 选择与 HashMap 往返不是 M57-4 的可选后续；M57-4 处理更广的作用域和功能组合。HashMap codec 继续复用普通 map 编码协议与手写 helper，本里程碑不借此设计新的序列化格式、对象图协议或统一容器框架。

## 14. 诊断与测试矩阵

每项语言错误都需要独立 negative fixture，断言实际出错位置和核心原因；正向和组合 fixture 通过正式 CLI 编译、链接、运行观察行为。普通功能不保存整份 AST/HIR/符号表快照；仅对代表性的族／分支引用、契约映射和 ODR 数据作小而精确的 IR／产物测试。

| 范围 | 正向／组合 | 必须拒绝或特别验证 |
| --- | --- | --- |
| 基本分支 | 单分支旧行为；正负互补；四分支；无兜底但当前参数有唯一匹配 | 无匹配；交叉分支无唯一赢家；胜出后缺成员不回退 |
| HashMap 主用例 | `HashMap<Int, String>` 默认 JSON 往返；缺默认 codec 的 K／V 仍可保存且可显式提供 codec；带条件的泛型转发；嵌套 HashMap；本地／跨 Cone／再次发布 | 缺一项 codec 时两分支例不能获得默认接口；仅编码／仅解码按所写分支选择；不得改变已有集合／显式 codec API，或把 MapNode codec 当成 HashMap codec |
| 具体性 | interface 继承、负向蕴含、多参数合取、声明／文件重排 | 不按条件数排序；等价分支；重复／矛盾条件；kind 混用 |
| 声明归组 | 同 Cone 跨文件、file-private、alias／re-export | 不同 kind／arity／可见性；跨 Cone 追加；重复非泛型；interface 偏特化 |
| 三态 | generic T、class 上界、`I<U>`、Nothing、静态 Base／动态 Derived | unknown 不当作 false；不能用运行期接口改变既有 Box 类型 |
| 抽象合法性 | 四分支的无约束传递；正负覆盖；条件排除重叠 | 无约束签名仍可能无匹配／歧义；未知更具体分支不能忽略 |
| 共同接口 | `T : Debug` 下 D 与 D+H 均提供 Debug | 更具体分支去掉 Debug，泛型转换定义处失败 |
| 字段／属性 | 不同 offset 的同型字段；共同 getter／setter；lambda 捕获 | 字段类型改变；成员缺失；无 setter／无地址；访问域不一致 |
| 调用 | 同形共同方法、generic final 方法、operator／infix、callable reference | 结果／调用形态不一致；新增分支重载不能抢走已绑定调用 |
| default 与效果 | 分支 default 不同、命名／vararg、求值计数、Managed／NoGC | unsafe／suspend／NoGC 不满足；查询不得执行 default 或 ensure |
| 构造推断 | 显式全部参数、`_`、整组推断、期望类型、共同构造器 | 候选属于被覆盖分支；多个不同参数向量；负向条件不反推 Any |
| 分支内环境 | this／同型参数／同型构造、私有成员、局部函数与默认值 | `Box<Other>` 偷用当前分支；兜底凭空获得负向函数 bound |
| class／interface | 不同 base 和 itable、虚分派、共同 super 调用、generic base | 某 base final、override 不兼容、缺 abstract 实现、泛型虚槽 |
| enum／sealed | 不同 variants、共同 payload、逐分支穷尽性、跨 Cone 子类型 | 不存在的 variant；payload 类型不同；某分支未覆盖却省略 else |
| companion | 每个完整 application 一次初始化、命名别名、M55 投影和要求 | 部分分支缺 companion；private 当 public；消费者重复 singleton |
| companion 选择条件 | subject 正／负／ref、存在性蕴含、K／V 独立 bound、宿主值类型与 companion 引用类型、branch body／companion／default 保留条件 | 无／private／internal／仅基类 companion 不匹配正负条件；无投影存在事实直接使用；`Companion : value`；正负两分支不能覆盖缺失投影 |
| companion 查询与初始化 | 嵌套条件类型、明确 codec 声明的递归数据、两参数指向同一真实 singleton、条件查询不触发初始化而真实访问触发一次 | 自我证明 companion 接口的选择环；读取 codec 值或初始化状态决定分支；从投影反猜宿主；格式／类型错误被吞为 fallback |
| nested／inner／local | 静态边界、outer 实参、连续组、不可变捕获和构造引用 | 非连续局部追加；捕获 var；同名 nested 被误合并；缺 outer |
| 循环与布局 | 有限嵌套 application、引用递归、phantom、互斥分支切断布局边 | 循环证明选择、实参扩张、同分支无限内联、非法继承环 |
| 表示与派生 | 不同 size／align／scan、数组元素、装箱、moving GC、协程局部、描述／codec | 用兜底 scan 或 ABI；NoGC／CLayout 失约；具体化重搜 codec |
| 产物与演进 | 本地／依赖／再次发布一致，多消费者身份与 ODR，真实接口或分支变更后重建 | 元数据缺合法引用／格式不兼容；import 改变赢家；混合旧新格式 |

选择诊断至少给出族名与实际类型实参、使用点、相关候选的定义位置和条件。companion 失败区分缺少直接 public companion、已存在但缺少所需接口、与否定冲突及关系仍未知，并指出 K／V 等实际 subject；有合法兜底时这些只是被淘汰候选的原因，不额外报错。抽象失败报告当前 bound 不能排除的无匹配／歧义情形；共同契约失败报告造成差异的两个分支及具体成员类型／效果，不只说“泛型不支持”。循环错误给出实际依赖链；内部格式或 I/O 错误不伪装成候选不适用。

验收遵循仓库的先格式化／lint 再测试流程；Rust 测试不能替代 fixture runner 与正式 CLI 覆盖。缓存测试只检查真实输入变化和正常命中复用，不通过隐藏源码／工具、破坏产物或增加重放检查制造门槛。原始日志保存在仓库外，里程碑记录只保留范围、结果概要和必要限制。

## 15. 与 Rust／C++ 的关系及取舍依据

这套功能更接近受条件约束的 class template 偏特化：选择一份完整类型定义，各分支不自动继承成员。C++ 的[类模板偏特化规则](https://eel.is/c++draft/temp.spec.partial)还包括参数模式推导、主模板与偏特化的特殊规则；M57 仅对同一组位置参数的声明条件排序，不照搬整套 template 机制。Scoop 保留定义处契约检查，不以实例化时重新检查任意依赖正文取得表达能力。

Rust 的限制有几类不同动机，不能一概归因于“实现起来复杂”：

| 动机 | 对 Scoop 的适用性与本方案处理 |
| --- | --- |
| 泛型检查所用契约不能在特化后失效 | 直接适用；字段类型、接口集合和构造器都能造成同类问题，因此保留第 5～6 节的共同契约 |
| 不同 crate 独立增加 impl 时要保持一致结果 | 一致性目标适用；本方案只允许定义 Cone 提供封闭分支集合，不引入开放外置 impl，因此不需要为此复制 orphan 规则 |
| 特化继承默认项目时需要明确父链 | 本方案整份替换、不继承默认项目，可接受 Debug／Hash 的交叉并用更具体分支消歧 |
| 生命周期条件在代码生成前被擦除，无法可靠据此选实现 | 当前 Scoop 没有对应的 lifetime 参数体系；不因这个 Rust 特有问题禁用静态名义条件选择 |
| 负向 impl 表示库对未来不实现 trait 的承诺 | 与本方案的静态负向条件不同；M57 明确接口／分支变化可能破坏兼容性，不作永久否定承诺 |

上述对 Rust 的依据分别见 [specialization RFC 的类型检查风险、lattice rule 和 lifetime 讨论](https://rust-lang.github.io/rfcs/1210-impl-specialization.html)、[rustc coherence 说明](https://rustc-dev-guide.rust-lang.org/coherence.html)与 [negative impls 说明](https://doc.rust-lang.org/unstable-book/language-features/negative-impls.html)。RFC 用来说明设计动机，不把它当成已稳定语言功能。表中适用性和 M57 的处理是本设计的判断。

不采用“所有分支必须拥有同一布局／接口”、仅允许一条特化链、unknown 时立即用兜底、实例化时重查整份正文或外部 Cone 任意追加分支。这些做法分别损失目标能力或破坏已经选定的身份与检查规则；以上方案在允许不同完整定义的同时，给出了确定的选择边界和可检查契约。
