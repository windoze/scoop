# M57 设计：普通类型与 companion 的条件 interface 实现

状态：实施中。更新：2026-10-23。本文件确定 M57 的语言方案、实现边界与验收范围，实际进度见 [PROGRESS.md](PROGRESS.md)，未完成项不计为已有能力。

现行合同以[语言规范](../specs/SCOOP-SPEC.md)、[实现规范](../specs/SCOOP-IMPL-SPEC.md)、[运行时规范](../specs/SCOOP-RUNTIME-SPEC.md)为准。M57 在 [M55](../milestone55/DESIGN.md) 的参数约束、固定 companion 投影与定义处绑定基础上增加条件接口实现；按第 12 节先同步权威规范，再逐功能实施与验证。

## 0. 目标与核心决定

一个名义类型只定义一次。允许在普通类型或 companion 的 body 内，以 `override interface I<...> where ... { ... }` 声明带条件的接口实现；条件成立时建立接口关系并提供相应成员，条件不成立时保留原类型及其普通能力。不同接口的实现可以同时成立，没有类型分支选择，也没有“更具体的实现覆盖较宽泛实现”。

首要用例是 HashMap 默认序列化：K／V 的 public companion 提供编码能力时，HashMap 的 companion 实现 `ValueEncoder<HashMap<K, V>>`；解码方向独立判断。HashMap 的字段、构造器、集合方法、原有 Hash／Equals 约束及显式 codec 工厂保持原定义，不为此引入 HashMapImpl 或复制 companion。

| 问题 | 决定 |
| --- | --- |
| 语法 | 宿主体内的 `override interface InterfaceType where ... { ... }`，where 必填，复用已有关键字 |
| 实现者 | 实现块直接所在的普通 class／struct／enum／object，或 companion 自身 |
| 条件作用 | 控制一条接口关系及块内实现成员的可用性，不改变宿主类型的合法实参域 |
| 条件范围 | 现有正向名义／kind bound，以及固定 `T.Companion` 的正向／ref bound |
| 多个实现 | 不同接口可叠加；同一完整接口的独立实现可能重叠时，在定义处报错 |
| 定义归属 | 实现块与被实现的宿主必须在同一 Cone；首版只允许直接写在该宿主的 body 中 |
| 类型与存储 | 一个原始声明、一个完整 application；字段、variant、基类和初始化序列不随实现条件改变 |
| 泛型检查 | 定义处证明所需的条件，检查完整实现；未知不能当成不成立 |
| 运行期 | 使用普通接口表与分派；条件在具体 HIR 形成前落实，不增加运行期条件分派 |

本期覆盖普通 class（含 open／abstract／sealed）、struct、enum、object，以及这些类型和其他既有合法宿主的普通 companion。既有 nested、inner、局部类型和匿名 object 的合法位置与参数作用域保持。interface 本身不使用实现块改变自己的继承契约；interface 的 companion 是普通对象，按相同规则处理。intrinsic 类型本体的条件接口登记不在本期，已有普通 companion 不因宿主具有 intrinsic 表示而被排除。

本期不做整份类型的条件偏特化、全特化或参数模式偏特化，不接受 `override struct`／`override class`／`override enum`。也不增加外置 impl、任意外部类型补充实现、接口实现特化、负向实现、associated type、任意 nested 投影、类型相等条件或任意编译期表达式。新实现块首版不接受负向 guard 和 `requires`；已有 M55 泛型函数的这些能力保持，查询接口时须计入本期新增关系。

## 1. 源码形式与首个闭环

### 1.1 HashMap companion

用户早期示意中的 `Encoder<T>`／`Decoder<T>` 对应现有 `ValueEncoder<T>`／`ValueDecoder<T>`；现有非泛型 `Encoder`／`Decoder` 是格式读写上下文。以下示意省略 HashMap 的存储及 Map 方法，保留[现有显式工厂](../../sysroot/src/scoop.std/src/collections/hash_map.scoop)，并调用[现有 map helper](../../sysroot/src/scoop.std/src/collections/map_codec.scoop)：

```scoop
public class HashMap<K, V> public constructor() : MutableMap<K, V>
    where K : Equals<K>, K : Hash {
    // Existing fields and Map implementation.

    public companion object {
        public fun encoder(key: ValueEncoder<K>, value: ValueEncoder<V>): ValueEncoder<HashMap<K, V>> =
            ValueEncoderFunction<HashMap<K, V>> { values, encoder -> encodeMap(values, key, value, encoder) }

        public fun decoder(key: ValueDecoder<K>, value: ValueDecoder<V>): ValueDecoder<HashMap<K, V>> =
            ValueDecoderFunction<HashMap<K, V>> { decoder ->
                val result = HashMap<K, V>()
                decodeMapInto(decoder, key, value, result)
                result
            }

        override interface ValueEncoder<HashMap<K, V>>
            where K.Companion : ValueEncoder<K>,
                  V.Companion : ValueEncoder<V> {
            public override fun encode(value: HashMap<K, V>, encoder: Encoder): Unit =
                encodeMap(value, K.Companion, V.Companion, encoder)
        }

        override interface ValueDecoder<HashMap<K, V>>
            where K.Companion : ValueDecoder<K>,
                  V.Companion : ValueDecoder<V> {
            public override fun decode(decoder: Decoder): HashMap<K, V> {
                val result = HashMap<K, V>()
                decodeMapInto(decoder, K.Companion, V.Companion, result)
                return result
            }
        }
    }
}
```

编码块的正文取得两项 encoder 条件，解码块的正文取得两项 decoder 条件；两者均取得宿主原有的 K bound。实现者是 HashMap 的 companion，方法中的 `this` 是这个对象；待编码 HashMap 由 `value` 传入。实现块不生成新的 receiver、对象、字段或隐式 codec 参数。

令 E、D 分别表示两个块的完整条件，下表只是结果组合，不是四份源码定义：

| E | D | 同一个 HashMap companion 的接口 |
| --- | --- | --- |
| 成立 | 成立 | ValueEncoder 与 ValueDecoder |
| 成立 | 不成立 | ValueEncoder |
| 不成立 | 成立 | ValueDecoder |
| 不成立 | 不成立 | 无上述接口；显式工厂仍可用 |

没有 public companion 的元素不能满足相应条件，但不影响 HashMap 的普通构造与存储。E／D 未知时也不妨碍 HashMap 类型本身合法；只有要求相应接口的操作需要额外事实。

[现有 Json 默认入口](../../sysroot/src/scoop.json/src/json.scoop)已分别要求 `T.Companion : ValueEncoder<T>`／`ValueDecoder<T>`。完成上述库适配后可以写：

```scoop
val values = HashMap<String, Int>()
values.put("answer", 42)
val text = Json.encodeToString(values)
val restored = Json.decodeFromString<HashMap<String, Int>>(text)
```

目标接口必须是 `ValueEncoder<HashMap<K, V>>` 等实际数据类型；实现 `ValueEncoder<MapNode<K, V>>` 不自动获得前者。MapNode 可作为普通中间表示，转换由普通方法负责。

### 1.2 普通类型的独立接口

```scoop
public struct Box<T>(val value: T) {
    override interface Hash where T : Hash {
        public override fun hash(): Long = value.hash()
    }

    override interface Equals<Box<T>> where T : Equals<T> {
        public override operator fun equals(other: Box<T>): Boolean =
            value.equals(other.value)
    }
}
```

两个条件分别控制 Box 的接口能力，T 无需满足其中任何一个才能构造 Box。两者同时成立时两份实现同时存在，不需要 `Hash + Equals` 交叉块。普通 struct 的派生比较仍按现行规则处理，不因本期自动产生未声明的 Equals conformance；显式同签名 operator 及其适用性须按第 5 节检查，不能再合成一个同签名兜底成员。

### 1.3 泛型转发

```scoop
public fun <K, V> encodeMapDefault(value: HashMap<K, V>): String
    where K : Equals<K>, K : Hash,
          K.Companion : ValueEncoder<K>,
          V.Companion : ValueEncoder<V> =
    Json.encodeToString(value)
```

编译器在定义处由 K／V 的 encoder 条件推出 `HashMap<K, V>.Companion` 的 encoder 关系，不要求 decoder。删除两项 encoder 条件后，此调用是定义处错误，不能等具体化为 String／Int 时重新使它合法；同一泛型正文仍可使用无条件的 Map 操作或显式 codec 工厂。

## 2. 语法、位置与 Cone 归属

### 2.1 实现块

```ebnf
InterfaceImplementation = "override" "interface" InterfaceType
                          ImplementationWhere
                          "{" { ImplementationMember } "}" ;
ImplementationWhere = "where" PositiveCondition { "," PositiveCondition } ;
PositiveCondition = Subject ":" PositiveBound ;
Subject = VisibleOwnerTypeParameter [ "." "Companion" ] ;
PositiveBound = Type | "value" | "ref" ;
```

每块实现一个完整 interface application；透明 alias 展开后必须指向 interface，不能是 class、类型参数或否定类型。where 必填且至少包含一项正向条件；省略或留空都是语法错误。无条件实现直接写在类型或 companion 的接口列表中，成员写在其普通 body 内，不另设等价的实现块写法。实现块没有新的类型参数列表，接口名称后的参数都是类型引用。条件沿用既有重复约束规则，不额外要求证明它严格缩小宿主的合法实参域。

`override interface` 在这里是宿主 body 内的实现声明，不声明新的 interface 类型。原有 `interface I<T> { ... }` 继续声明嵌套 interface；两者由 token 区分，不靠名称查找猜测意图。现有类型头中的 `: I<T>` 无条件接口语法继续可用。

块本身不声明 public／private 等独立可见性，也不形成注解目标或新的成员命名空间；接口关系按宿主的原始声明导出，成员显式写自己的 modifier 与注解。公共接口义务仍须 `public override`，省略可见性不会自动升级。块内只允许为目标接口或其 superinterface 提供的普通 override 方法、computed property／合法 abstract member；方法、accessor 的效果及 operator 标记遵守现行规则。已有 default、继承实现或合法 codec 合成足以兑现义务时，允许空 body。

不在块中声明字段、stored／delegated property、constructor、init、release、enum variant、基类、companion、nested type 或任意 helper；需要 helper 时使用宿主普通成员或方法内局部函数。块不支持 `I by expression`，以免条件接口引入条件存储和初始化。它也不开放接口方法自身的泛型。

### 2.2 声明位置与封闭集合

首版只允许在宿主的原始 body 中声明实现块，禁止顶层、扩展、单独函数块和跨文件重新打开类型。局部类型的 body 仍可包含实现块。原声明和实现块因此必须同 Cone、同 typed owner；同 Cone 是必要条件，不意味着同 Cone 可以在任意文件为已声明的类型追加一个外置块。

限制的是**实现者的归属**。目标 interface 可以来自依赖 Cone，条件引用的参数类型也可以来自其他 Cone。消费方拥有某个类型实参、接口、别名或 re-export 路径，都不获得为外来宿主追加实现的能力。消费方新建自己的类型或子类时，可以在该新类型的 body 中实现接口。

检查宿主及其成员前收集其普通接口项和全部实现块；源码先后顺序不决定能力。同一个完整 application 在 provider、consumer、再次发布和链接时采用同一份声明集合。归属用普通符号和 typed owner 检查，不引入其他授权机制。

## 3. 条件与作用域

### 3.1 宿主参数环境

实现块可以约束在该宿主定义中合法可见的类型参数：普通类型自己的参数、companion 对应的宿主参数，以及 inner／局部声明按既有规则取得的外层参数。普通 static nested 类型仍不隐式取得外层参数；实现块不绕过这条边界，也不新增运行期捕获。

这是对 M55 的明确扩展：实现块可为宿主参数增加局部事实，块内实现方法不必声明自身类型参数。事实只在该块的接口目标、成员签名、默认值与正文中有效，不流入宿主字段、构造器、初始化项或其他块。嵌套 callable 沿已有词法规则取得这些事实。

宿主原有 bound 始终限制类型的合法 application。实现块的附加条件不移动到宿主 where，也不能用来证明宿主自身的基础 bound。即使一个附加条件不成立，原类型仍然良构；只有违反原有宿主约束才使该类型 application 非法。

### 3.2 首版的正向条件

条件是普通合取，顺序不改变含义；复用正向 class／interface bound 与 `value`／`ref` 的既有规则，包括每主体的 class 上界数量及 kind 与正向名义 bound 的互斥。条件按真实类型身份、原 binder、透明 alias 展开及规范化顺序比较。重复、直接矛盾和由普通继承可确定的矛盾在声明处报错。

subject 限于上述可见参数或固定 `T.Companion`，不允许对 `this`、任意表达式、constructed type pattern 或任意 nested type 查询追加 guard。本期不接收 `T : !I`、负向 companion 条件、`requires`、析取或类型相等条件；这些限制只作用于新实现块，不删改 M55 函数能力。

正向接口条件不表示目标集合互斥。`T : Debug` 和 `T : Hash` 可能同时成立，即使当前 Cone 没有见过一个同时实现它们的实际类型。

### 3.3 Companion 存在性与投影

`T.Companion : I<T>` 同时要求 T 直接拥有 public companion，并要求该对象的完整类型实现 I<T>。不能借用基类 companion；private／internal companion 不满足这个泛型投影条件。`T.Companion : ref` 只要求此存在性，`: value` 是矛盾条件。

先登记整个实现块的投影存在事实，再解析目标接口、条件中其他类型和成员签名，书写顺序不影响合法性。投影可用作类型和值；仅在某个类型位置写 `T.Companion` 不会隐式提供存在条件。`T.method` 的既有转发、命名 companion 的 Companion 别名及透明 alias 规则保持。

参数本身的接口与其 companion 的接口分别判断，不自动互相推出。查询只读取静态声明；值访问才执行原有 singleton ensure。未知宿主的投影保留原 binder，不能按当前恰好见到的一个实例提前落实对象，也不能从投影反向猜测宿主参数。

## 4. 接口关系与泛型检查

设 B 是宿主的合法性约束，C 是一个块的附加条件，H 是宿主 application，I 是目标接口。此块贡献规则 `B ∧ C ⇒ H : I`。接口闭包同时考虑普通无条件接口、基类和 superinterface；不能把这条充分条件误当成排除其他合法路径的唯一必要条件。

| 条件判定 | 接口和块内成员的使用 |
| --- | --- |
| 成立 | 可使用该实现，仍须通过普通可见性、签名和效果检查 |
| 不成立 | 该块不贡献接口或成员，宿主普通能力保持 |
| 未知 | 保留声明关系；当前环境不能凭此使用接口或成员，也不能推出其否定 |

泛型调用、向上转换、接口参数传递和 callable reference 都必须在定义环境 Γ 中证明所需条件。普通类型参数推断先确定所需实参，条件不用于重猜宿主、改变已有实参或反向推断 companion。已经绑定的普通重载不在具体化时重选；接口调用保存接口 application 与 slot，具体化或普通动态分派取得实际实现。

检查块内正文时可使用 `Γ + C` 及该块承诺的接口方法签名，因此普通方法互递归、通过 this 的接口调用可以形成正常 typed body。验证 C 是否成立则不能先假定该块贡献的接口关系，不能以正文调用成功来反证 guard。所有实现正文均在定义处检查，非法实现不降为“不匹配”，也不等到首次实例化才报告缺少成员。

定义处已证明的接口及操作必须在所有满足 Γ 的合法替换后继续成立。增加参数事实可以使更多独立接口可证明，不能改变已经选定成员的签名、效果或类型身份。同一个完整 H 的实际接口集合由其完整实参和原始声明决定，不由调用方暂时知道多少约束决定。

M55 的负向接口查询须遍历相同的接口闭包：具体类型只有在相关条件均已确定且不存在该接口关系时，才能证明 `!I`；部分 application 的条件未知时保持未知。未知派生类型不能因为 class 上界未实现 I 就被认为不实现 I。`is`／`as` 仍按实际对象的普通 TD 判断，不把编译期未知误编译为运行期 false。

## 5. 成员、默认实现与接口义务

块内成员属于宿主的正常成员表，带有该块的可用条件，没有另一套 receiver 或接口私有命名空间。直接成员调用、companion 转发、运算符和成员引用都须证明该成员条件；证明后使用普通调用协议。块外普通成员与初始化项不取得该条件，不能无条件调用只在块内可用的方法。

本期不新增依靠宿主 where 区分同签名普通成员的重载：同一 owner 中重复同签名方法、同名 property 按既有声明规则拒绝，即使它们写在不同块中。不同接口需要同一签名时，应由一个显式成员共同兑现，而不是写两份重叠正文。方法自身的合法 M55 泛型重载仍只用于普通 non-virtual 成员，不成为接口实现方法。

宿主体中的显式普通 override 成员可以兑现一个或多个条件接口的 slot：该普通成员的正文按自身原环境检查，逐个 slot 的签名、可见性及效果覆盖在相应接口条件下验证。即使所有这些接口条件不成立，该普通成员仍存在。这样可以共享一份无条件实现，同时避免把某个接口的 guard 泄漏进它的正文。块内带条件的成员要兑现另一条义务，则必须证明该义务成立时成员条件也成立。

每条条件接口在其条件成立的整个域内，必须拥有现行规则要求的实现。优先级沿用普通 class 实现／override、已有合法委托和 interface default；subinterface 的更具体 default 规则保持。这里的 default 选择是普通继承规则，不是按 guard 给实现块排序。具体 class／object／value type 不能留下 abstract obligation，abstract class 可以保留；getter／setter 分别检查，value type 不能实现需要 setter 的 var contract。

多个接口 default 可能同时适用且无法按普通继承规则消歧时，要求显式共同 override。不能因为某个当前调用只证明了其中一个条件，就忽略另一个在合法实参下可能出现的冲突。`super<I>` 只能在已证明 I 是相应 direct superinterface 的环境中调用其 concrete default，沿用现有显式 direct 调用语义。

成员的访问域、参数名、default、vararg、operator、suspend、Unsafe／NoGC 契约继续遵守普通规则。相同参数列表而仅返回类型不同的两个 decode 不能作为可选实现；不可用的方法也不能留下一个运行期报错 stub。普通派生 equality／codec 不越过显式同签名声明制造另一个兜底实现；已有合成机制的适用性及条件传递见第 8 节。

## 6. 重叠、继承与唯一实现

### 6.1 独立实现的重叠

将类型头的普通接口项和各实现块一起检查。对同一个完整宿主、同一个完整接口，不允许两份可能同时适用的独立实现。需要同时考虑宿主约束、两个 guard、接口实参可能相等及 alias 展开；无法证明不相交就报告潜在重叠，不留给实际调用点选择。即使两份正文相同、其中一个条件更强或一个接口没有方法，也不豁免。

例如，同一 Box<T> 对 Mark 分别声明 `where T : Debug` 与 `where T : Hash` 两个块会报错；增加 `Debug + Hash` 块不能修复。无条件 Mark 加任一可能成立的条件 Mark 同样冲突。`I<T>` 与 `I<Int>` 在合法 T 可以为 Int 时也可能重合，不能仅比较源码拼写。既有 kind／名义关系已能证明不相交的情形可以接受，但仍须独立满足第 5 节的成员唯一规则；不为此增加一般定理证明或枚举所有使用者。

通过同一个显式实现的 superinterface 闭包到达相同祖先，或者普通继承菱形重复到达同一个 slot，不算两份独立实现；须合并真实关系并按普通 override/default 规则消歧。不同接口中的同签名义务可以共用一个合法方法，不能借“接口名称不同”容忍互不兼容的 slot。

### 6.2 类继承与 override

派生类沿普通基类 application 替换参数，继承相应条件接口及其成员。继承本身不追加一份独立实现；基类条件不成立时，也不会凭继承获得该接口。实现块不改变 class 的 direct base、open/final 性质或构造链。

派生类的普通 override 可以覆盖条件成员，但其可用域必须覆盖被覆写 slot 的整个适用域，不能靠更强条件只覆盖一部分。设派生类合法约束为 D、继承 slot 条件为 C、override 自身条件为 Q，必须证明 `D ∧ C ⇒ Q`，并按相同环境验证签名和效果；正文只使用其自身合法事实。无条件 override 的 Q 为 true，正文不能借此假定 C。final、访问域及 abstract 规则照常检查。

若派生类在实现块中显式重列已继承的同一接口以提供普通 override，首版要求该块与继承关系在 D 下具有等价的适用条件；它沿用原接口关系，不生成第二份 conformance。若 D 已保证继承条件，直接使用普通 override 即可。不得通过附加更强／不同 guard 将此写成接口实现特化；不满足上述普通继承情形的重列按独立实现检查重叠。

concrete 子类必须在每条继承义务成立的域内完成 abstract slot，不能等某个实际实例恰好避开条件后才放行整个非法泛型定义。继承默认方法及同签名冲突也按完整的可适用条件检查，不依赖当前 Cone 已出现的具体子类列表。

## 7. Companion、存储与其他语言机制

companion 仍由宿主显式声明至多一份；同一个完整宿主 application 只有原来的 companion 类型、singleton、状态和初始化结果。条件接口既不合成缺失的 companion，也不生成编码专用／解码专用 singleton。声明在不同参数 application 下的 companion 按既有身份规则区分，同一 application 跨 Cone 合并原有 ODR 状态。

companion 的字段、基类及初始化表达式按其原环境检查，不取得实现块的附加条件。需要 K.Companion 的实现方法在实际调用时访问该值；单纯判定条件、生成接口表和查询类型描述不执行 singleton 初始化。数据 class 的继承不继承 companion；companion 自己继承普通 codec class 时，才沿正常类继承取得接口关系。

条件不改变宿主实例字段、enum payload、构造器、release、初始化顺序或已有捕获。普通泛型字段的布局仍随实际类型参数决定；本期只是不会再因某个实现块适用而选另一份表示。GC scan、copy、boxing 与 ABI 使用同一份普通 nominal 布局，接口视图及虚方法表按实际适用成员正常生成。NoGC 实现对 Managed slot 的适配沿用现有合同。

sealed interface 的直接实现仍满足现有同 Cone 规则；导出的直接实现关系须保留条件，穷尽性检查不能把未知条件当成不可能，也不能允许消费方追加 sealed 子类型。这条规则与“实现者和块同 Cone”分别检查，不因目标 interface 来自依赖就绕过 sealed 限制。

透明 alias、类型描述和 re-export 保留同一完整类型身份。具体类型的接口／成员描述只报告实际适用的关系与成员，字段及构造描述保持原定义；generic 文档视图展示声明条件，不把条件成员展示为无条件可调用。公开接口目标与 guard 中的类型须满足普通签名可见性，private helper 作为模板支持存在不等于公开 API。

TypeInfo 的 interfaces 沿用直接关系：类型头接口在前，适用的实现块接口按声明序追加，基类及接口父关系不扁平化。条件属性在替换其类型前过滤，保留原声明身份、序号及注解；条件目标的 type-use 注解属于对应实现块中的类型使用。具体化过程中复用已求值的条件，不为类型描述重新检查声明或成员正文。

文档显示条件关系及成员的完整适用条件；继承与合成成员按完整 owner application 展示，未知条件不省略，已证明不可能的继承成员不出现。普通 override 仍展示自身无条件的可用性。实现块不生成页面、额外成员身份或方法类型参数，条件展示不影响普通编译的候选选择。

## 8. 序列化、派生与库范围

### 8.1 HashMap 的交付边界

本期必须交付第 1.1 节的两个独立条件接口，保留现有显式工厂及原始 Hash／Equals bound。复用原 map helper，不抽出新的公共实现基类，不复制集合存储与算法。默认 wire 仍为现有 entry 序列，重复 key、迭代顺序、异常路径和显式 codec 优先选择保持。

必须覆盖只有编码、只有解码、两者都有、两者都没有的元素能力，以及嵌套 HashMap。嵌套查询逐层使用已声明关系：内部容器 companion 的接口成立后，可作为外层元素 codec。不会因完成 HashMap 就自动让所有容器或 interface 静态类型具有默认解码结果。

Option、ArrayList、其他 std 容器及普通 DTO 可采用相同机制；只有确有库适配和测试的类型才记为支持。Array／MutableArray 的普通 companion 也可按普通 object 规则适配，无需特化 intrinsic 表示，但它们的库迁移不是 HashMap 闭环的前置。Comparator、字段格式和外来类型策略仍使用显式参数；不增加动态 registry 或隐式全局 codec 状态。

### 8.2 既有 codec 合成的衔接

块可以请求现有 ValueEncoder／ValueDecoder 的缺省方法合成。例如：

```scoop
public struct Envelope<T>(val value: T) {
    public companion object {
        override interface ValueEncoder<Envelope<T>>
            where T.Companion : ValueEncoder<T> {}

        override interface ValueDecoder<Envelope<T>>
            where T.Companion : ValueDecoder<T> {}
    }
}
```

这不是任意空接口实现自动获得正文。只有现行核心 codec 合成规则覆盖的 slot 和数据形状才适用，其他义务照常检查。合成正文在所属实现块环境中生成并检查，字段类型为参数时，可以使用环境已经证明的对应 companion 投影。

同步语言规范 11.13.4 的字段 codec 来源：保持显式 primary val 依赖与递归 this 的原优先级，在普通 companion 来源步骤允许已证明的参数 companion 投影，以及可证明具有相应接口的完整泛型 application。随后仍可使用原 core 容器／tuple 组合路径；未知参数不能凭实例化猜测 codec。固定 nominal companion 的既有合法访问不统一收紧为 public-only，只有泛型投影沿 M55 要求 public。

字段 codec、default、访问权限及效果在定义处绑定，具体化只替换参数和已选投影，不重新搜索或改变来源优先级。现行序列化形状、Serialize 配置和构造顺序保持，不因条件接口支持扩大自动派生的目标范围。

## 9. 查询循环与终止性

条件查询只读取判定关系所需的声明，不展开任意成员正文、字段布局或 singleton 初始化。实现不能用自己尚待证明的接口关系证明 guard；同 Cone 并不消除这类依赖。以下正向条件也会形成没有独立依据的循环：

```scoop
interface Mark {}

open class Gate<T> {
    override interface Mark where T : Mark {}
}

class X : Gate<X>()
```

查询 `X : Mark` 依赖 `Gate<X> : Mark`，后者又要求 `X : Mark`。没有其他已成立的声明事实时，报告这条实际依赖环，不能把活动查询标成 true，也不能作为 M55 的负向事实返回 false。存在独立成立的路径时应使用该路径，不因无关循环否定已经确定的事实。

活动查询保存原声明、完整 application 与所查询的接口关系。相同未决查询重新出现时定位闭环；通过同一声明依赖环不断扩张参数的查询，沿现有泛型结构终止性原则拒绝并报告扩张边。严格进入输入类型子项的嵌套容器查询合法，不用深度、实例数或超时配额决定程序是否成立。

正常 F-bound、已显式声明的递归 codec 及同实例方法递归不等于关系自我证明：可以先取得已有接口和方法签名，再检查普通正文。实现体中 `this` 的局部接口事实只服务正文检查，不能写回无条件关系缓存。依赖 Γ 的未知／成立结果按相同环境复用；具体 application 的确定结果保持全局一致。

## 10. 编译阶段与产物

### 10.1 AST 与 typed 语义数据

Parser 增加明确的宿主体内 interface implementation AST 节点，保存目标类型、条件、成员和源码位置，复用 `override`、`interface` 和现有 bound AST。不生成第二个同名 nominal，也不把实现块伪装成嵌套 interface 声明。

HIR 需要区分原 nominal、条件接口声明、普通成员／slot、完整 application 与 concrete 实现映射。条件声明身份由原 Cone、typed owner、目标完整接口及规范化条件确定；成员仍有自己的 typed 声明身份和所属条件引用。源码顺序、消费者、显示 FQN 和进程内 arena 序号不替代实体身份。

宿主 binder 只有原来一组，companion 引用原 host application；实现块没有自己的泛型 binder 或运行期参数。IR／meta crate 保存条件、引用与结果，不复制一套语言查询器。无需旧方案的类型族、定义分支、赢家集合或跨分支成员契约实体。

### 10.2 定义检查与 Export HIR

HIR 按顺序完成宿主／成员收集、作用域与归属检查、投影存在事实登记、目标与条件绑定、重叠／继承／slot 覆盖检查、全部正文及合法合成正文检查。已完成的关系与成员检查在未变化时复用，不在后续 stage 重放。

Export HIR 保存宿主的完整条件接口集合、目标及继承关系、原 binder 条件、块内成员的适用性、真实 slot 映射和正常消费需要的 typed body／支持声明。普通成员及 constructor 表面保持原身份。private 支持声明不会因模板导出变成公开名称；可用关系不能按某个 consumer 的 import 或已用实参裁剪。

继承义务触发既有 codec／inspector 合成时，生成成员保留原条件声明及代入后的完整基类 application。例如 `Base<Wrapper<T>>` 的条件仍作用于 `Wrapper<T>`，不能改写为 T 的上界。生成正文在该条件下检查，直接调用、成员引用、导入与具体化均沿用这份成员条件；不为继承合成创造第二份实现块或无条件兜底方法。

泛型 body 保存已绑定的成员或接口 slot，以及使用时已证明的条件关系；未决条件用明确的类型化数据表示，不以缺失类型、空方法表或伪造无接口结论代替。查询缓存保留参数环境，块正文中的局部事实不能污染仅以完整 application 为键的具体结果。

### 10.3 LocalConcreteHir 与后续阶段

完整实参确定后，落实每条声明条件及继承关系，形成唯一完整接口集合和实际成员／slot 目标。没有“赢家”或失败后的回退。已有普通重载、字段 codec 和 default 表达式绑定保持；继承中的虚调用继续映射普通 virtual／interface family。

在进入 LocalConcreteHir 前消去条件待定状态和参数 companion 投影，产生确定的普通类型、调用、singleton 访问、完整 itable／vtable 输入及必需正文。生成机器定义仍由实际使用和表项引用决定，不为所有声明提前生成所有泛型实例。实际调用边继续遵守既有单态化终止性规则。

MIR、LIR、codegen、linker 和 runtime 消费确定结果，不重做条件证明或根据调用点选择实现。不添加 dictionary、类型分支状态、新的 TD 字段或 singleton 初始化协议；接口表内容必须正确反映适用关系，并供静态转换、动态 is/as、装箱和分派共同使用。

## 11. 跨 Cone、兼容性与方案取舍

导入方只能消费宿主 Cone 导出的完整关系集合，不能为原类型或 companion 追加块。再次发布保留原实体与条件；同一完整类型在多个 consumer 中使用时，其接口表、成员和 companion 状态必须一致。外部仍可定义新参数类型并按其真实关系实例化库类型，封闭的是实现声明位置，不是类型实参的全集。

新增条件接口可能改变 bound 满足、成员决议、M55 负向查询以及运行期接口检查；同 Cone 限制不等于所有新增实现都兼容。按普通公共 API 演进和真实输入变化更新依赖，产物条件表示有实际变化时升级相应格式，不另外建立能力版本、授权或证明体系。

这更接近 Rust 的普通条件 impl：约束决定接口实现是否适用，不能用重叠实现选择更具体版本。Rust 的泛型 impl 与 coherence 规则见 [Rust Reference](https://doc.rust-lang.org/reference/items/implementations.html)。Scoop 保留自己的声明体归属、class 继承和 companion 模型，不照搬外置 impl／orphan 体系。

| 旧的整份类型特化问题 | 本方案的处理 |
| --- | --- |
| 条件偏序、全特化优先与唯一赢家 | 不存在；独立接口叠加，重复实现检查冲突 |
| 编码／解码需要组合分支 | 两个独立实现块覆盖四种能力组合 |
| 字段、构造器、基类和 companion 随分支替换 | 一份定义和原始初始化序列 |
| 泛型使用需要所有可能定义的共同契约 | 普通成员直接使用，条件接口／成员使用前证明条件 |
| 未决 companion 要映射到所选类型分支 | 原始 companion 声明不变，只查询其条件接口 |
| 不同分支的布局、GC scan 与 ABI 一致性 | 普通泛型布局合同，接口表随实际条件正常生成 |

仍须完成条件推导、成员适用性、继承／默认方法冲突、查询终止性、跨产物保存和接口表生成；这不是只修改 parser。整份类型特化和同一接口实现的特化均暂不实施，未来确有改变表示或重叠实现的需求时再单独设计，不能以放宽本期冲突检查代替。

## 12. 实施前的权威规范同步

实施以语言规范新增的 9.1.8 为统一入口；实现各步前同步下列条款，不能先改代码再补语义。相关文档的章节引用同时维护，规范合同不等于实现验收已完成。

| 位置 | 必须同步的内容 |
| --- | --- |
| 语言 3.2、3.2.2～3.2.4 | 新实现块的宿主参数作用域、正向条件、投影存在性；M55 正负向及成员要求查询计入条件关系 |
| 语言 8.6 | 条件成员的可用性、定义处绑定、禁止以宿主 guard 新增同签名成员重载 |
| 语言 9.1.1、9.1.3～9.1.5、9.1.8 | 实现块语法、单一定义和固定初始化、companion、slot 覆盖、继承与可见性 |
| 语言 9.1.7、9.2.2 | sealed 条件关系；实现块不引入条件 delegation 存储 |
| 语言 9.5～9.7、11.11、11.13 | codec 接口与投影合成、显式相等与合成冲突、类型／成员描述及文档条件展示 |
| 语言 12.4、13.2～13.3 | 原 Cone 归属、完整关系导出、既有效果及接口适配 |
| 实现 2.1～2.2、2.11、2.13 | AST／typed 条件接口与成员数据、Export 完整集合、具体化关系与 body 绑定、实际编码迁移 |
| 运行时 1、2.1～2.2、2.7 | 编译期落实条件；普通 TD、itable、装箱与原 singleton 协议，无新增 ABI |
| 标准库及相关 codec 规范 | HashMap 的独立默认编码／解码能力、原 wire 与显式工厂保持，逐类型记录后续适配 |

## 13. 实施顺序与验收

### 13.1 实施顺序

1. **普通类型闭环**：同步语法与关系合同，实现 parser、条件作用域、成员／slot 检查、重叠诊断；用 Box 的条件 Hash／Equals 验证普通接口转换和调用。
2. **Companion 与 HashMap**：接通宿主 binder 和 M55 投影，落地两个独立 codec 实现块及 Json 默认调用，覆盖四种能力组合和嵌套 HashMap。
3. **组合与派生**：完成 open／abstract 继承、default／property／效果组合、Envelope 的条件 codec 合成及查询循环诊断。已有无条件能力不得回退。
4. **产物与全链路**：完整导出条件关系与支持正文，验证跨 Cone、再次发布、ODR、动态接口表和原初始化语义，完成必要的目标与优化组合验收。

各步先形成真实源码编译、产物消费和运行闭环，再扩大组合；完整 M57 验收必须覆盖以下职责，不将仅能 parse 的语法记为完成。

### 13.2 正向行为与必要组合

普通行为使用所属 Cone 的 `scoop.test` 测试；core 的用例放在独立测试 Cone。普通 consumer 功能也用 Cone 测试，不因消费预编译库就转成 fixture。

| 范围 | 代表性行为 |
| --- | --- |
| 普通类型 | class／struct／enum／object、条件真／假、与类型头无条件接口组合、实例方法与 computed property、两个接口叠加 |
| 泛型使用 | 有约束的转发、无条件字段／构造操作、同一完整类型从不同环境到达同一接口关系 |
| Companion | K／V 投影、命名 companion、不同宿主实参、显式工厂与 singleton 状态保持 |
| HashMap | encode-only、decode-only、双向、无默认 codec，嵌套容器和原 wire／重复 key 行为 |
| 既有合成 | Envelope 的两个独立 codec、显式实现优先、字段投影、递归 this 和默认值绑定 |
| 继承与接口 | 普通 override、条件域覆盖、abstract 子类义务、default／superinterface 菱形和共同实现 |
| 其他语言组合 | inner／nested／局部参数作用域、alias、类型描述、M55 正负向／requires 消费、新类型实参来自外部 Cone |

### 13.3 诊断、产物与 ABI

每项编译错误规则有必要的代表性 expected-error fixture，定位到实现块、条件、冲突声明或使用点；按编译器规则去重，不为每个库复制一套 negative 用例。至少覆盖：

- 错误声明位置、外置／跨 Cone 追加、重新打开类型、错误目标 kind、新 binder、缺少或空 where、负向 guard／requires、缺失投影存在事实及不可访问的参数 companion；
- 块内非法存储／初始化／helper／泛型接口方法、缺失实现、错误签名／可见性／效果、同签名成员和 property 冲突；
- 无条件与条件重复、等价 guard、Debug／Hash 交叉、替换后目标相同、alias 重叠；不能用更强块修复；
- 泛型环境缺少条件、块条件泄漏、继承 override 缩小适用域、default 冲突、具体子类未兑现条件 abstract obligation；
- 正向关系循环、companion 自我证明、参数扩张依赖、未知关系被当作 M55 否定的错误。

普通行为测试不能替代以下专门合同验证，也不需要为普通语法保存全量 IR 快照：

| 专测 | 必要断言 |
| --- | --- |
| Export／导入与再次发布 | 条件关系、原 binder、slot 和投影绑定完整；provider 与多个 consumer 结果一致 |
| 接口表与 ABI | 条件真／假、值装箱、open class 分派、接口继承／转换、is/as、NoGC 到 Managed slot 适配 |
| Companion ODR／初始化 | 同 application 跨 Cone 共用状态；不同 application 独立；关系查询不执行初始化 |
| 泛型及优化 | 定义处绑定在直接调用、接口调用、导入与内联后保持；无条件成员不因具体化重选 |
| 必要的 GC 正确性 | 条件接口视图及装箱在移动 GC 后仍可正确访问与分派，不改变原字段扫描 |

正常 fixtures 和 Cone 测试使用 release；只有 IR、优化、ABI、跨产物或默认 profile 合同需要时才组合模式。GC stress 只用于定向 GC 正确性验证。测试复用预编译系统库与共享 Cone cache，不以删除源码／工具、复制完整 sysroot 或重复冷／热构建作为功能门槛。

实现变更按仓库规定先格式化和 lint，再运行对应 Rust 测试、Cone 行为测试与必要正式 CLI 验收。原始日志留在仓库外，里程碑记录只保留验证范围、结果概要、失败闭合与实际限制；本设计改写不宣称上述未来用例已经执行。
