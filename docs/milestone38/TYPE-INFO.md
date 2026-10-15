# M38：TypeInfo 与相关描述类型

本文件从属于 [总体设计](DESIGN.md)。2026-10-14 修订补充与 [M46 默认字段规则](../milestone46/SERIALIZE.md)对齐的声明处类型事实。下列签名表是拟定 API 目录；实现时使用普通 Scoop 声明和完整 body，只有明确登记的 intrinsic 省略 body。

## 1. 查询入口

```scoop
public fun <T> typeInfo(): TypeInfo<T>
```

| API | 含义 |
| --- | --- |
| `TypeInfo<T>.type: TypeRef` | `T` 的完整类型引用 |
| `TypeInfo<T>.describe(): TypeDescription` | 等价于 `type.describe()`，取得当前层描述 |
| `TypeRef.displayName: String` | 用于展示的名称或完整类型表达式 |
| `TypeRef.nominal: Option<NominalRef>` | 名义类型的原声明；tuple/function 等没有名义声明 |
| `TypeRef.arguments: Array<TypeRef>` | 当前名义 application 的实参，按声明顺序；非名义类型为空 |
| `TypeRef.describe(): TypeDescription` | 展开当前层，不递归展开所有子类型 |
| `TypeRef` 的 `Equals<TypeRef>` | 比较完整类型身份；不是名称、布局或实例地址比较 |

`TypeInfo<T>` 是普通不可变库值，`T` 保留调用端类型关系；异构字段列表使用非泛型 `TypeRef`。`field.type` 是描述值，不能写成运行期选定的泛型实参，也不提供任意 dynamic cast。

公开入口是普通 core 泛型正文。内部 `type_reference` intrinsic 生成引用的 identity、名称、名义声明和实参数据；普通 `__scoopTypeReference<T>()` 用它构造 TypeRef，并提供调用 `type_description<T>` 的普通闭包。两个 intrinsic 都在 concrete HIR 中消解；编译器登记实际引用构造函数的 typed callable identity，生成子引用时复用该函数的正常具体化请求，不新增 runtime 入口。

`typeInfo<Box<Int>>()` 与 `typeInfo<Box<String>>()` 的字段类型分别替换成 Int 与 String。`fun <T> describe(): TypeInfo<T>` 中可查询 T；在单态化时生成完整具体描述，不需要 `reified`。未绑定的类型参数描述可存在于编译器内部模板中，程序取得的公开 TypeInfo 总是对应完整类型。

透明 typealias 复用展开后的 TypeRef；alias 的源码拼写不影响 exact equality。函数、tuple、Option、指针等完整结构均参与类型身份。函数类型保留普通/suspend、receiver、参数和结果关系。

`Nothing` 可以被描述，但没有可供检查的合法值。annotation 声明不属于普通类型，使用第 5 节的 annotation 查询，不允许 `typeInfo<MyAnnotation>()`。

## 2. 类型形状

`TypeDescription` 具有 `type: TypeRef`、声明上的 `annotations: Array<AnnotationInfo>` 和封闭的 `shape: TypeShape`。没有源码声明的结构类型具有空的声明注解列表；使用位置上的注解在 TypeUseInfo 中。

| TypeShape 分支 | 数据与顺序 |
| --- | --- |
| `Struct(StructInfo)` | 本类型字段、构造关系、实现的接口；字段保持声明序 |
| `Enum(EnumInfo)` | 声明序 variant，各自独立 payload 与构造关系 |
| `IntegerEnum(IntegerEnumInfo)` | M37 整型枚举，底层整数类型、各 variant 的已检查静态值 |
| `Class(ClassInfo)` | isFinal、直接 base、接口、本 owner 的 logical property 和 storage、构造器 |
| `Object(ObjectInfo)` | singleton 自身的类型、base、property/storage；companion 的自身引用保留宿主实参，companionHost 指向宿主声明 |
| `Interface(InterfaceInfo)` | 父接口和本 owner 的 logical property，不伪造存储 |
| `Tuple(TupleInfo)` | 有序 TupleElementInfo，位置从 0 计，展示名从 `_1` 起 |
| `Unit` | 空积；与任意零字段 nominal 保持不同身份 |
| `Intrinsic(IntrinsicInfo)` | Boolean、定宽整数、Float/Double、Char、String、Array/MutableArray、Any/Nothing 等实际类别 |
| `Function(FunctionTypeInfo)` | 普通或 suspend、可选 receiver、参数与返回类型 |
| `Pointer(PointerTypeInfo)` | Ptr 的 pointee 或 FunPtr 的函数签名；不表示可解引用权限 |

上述分支覆盖所有合法完整类型的表示类别。普通核心库类型如 ArrayList、StringBuilder 仍是 Class；其 debug 展示可以由普通 inspector 选择逻辑内容。MaybeUninit 等 compiler-represented 类型保留独立 intrinsic 类别与完整参数，不被当成已初始化的 T。

泛型/inner/局部/匿名声明使用 M37 确定的 typed owner、捕获与 application。`ObjectInfo.companionHost: Option<NominalRef>` 指向宿主声明，泛型宿主不凭空选择具体实参。描述不把隐藏 outer/capture/delegate 存储伪装成用户声明的公开字段。接口委托等编译器内部字段使用 Generated 来源。需要保留的内部 storage 带明确来源种类，logical property 列表只包含实际 property。

AccessorInfo.kind 保留 Storage／Constant／Body／Abstract 的源码事实；普通存储 accessor 的函数体物化不产生新的公开 kind。

class 的继承关系不扁平化：`ClassInfo.base: Option<TypeUseInfo>` 指向直接基类，基类字段通过其自身描述取得。同名成员仍有不同的实际成员身份，override 指向实际被覆写声明。companion 的描述不包含宿主实例的字段。

## 3. 声明与成员

### 3.1 身份和可见性

`NominalRef`、`FieldRef`、`PropertyRef`、`AccessorRef`、`VariantRef`、`ConstructorRef`、`ParameterRef`、`AnnotationKey` 分别包装对应的 typed declaration identity，不能互相代用。泛型原声明与具体 application 由声明引用和完整 TypeRef 分开表达。

引用可以比较相等，名字用于展示。跨 Cone、alias import 或 re-export 后，原声明仍是同一引用；两个同名声明不相等。内部继续区分 generic declaration、resolved application、concrete function/type 等身份，不把公开反射对象地址当作持久化身份。

描述记录声明的 visibility、owner 与来源种类，以便解释其结构。M37 局部声明使用单独的 Local 描述状态，不能将省略 visibility 误记成 public。实际可访问性仍由普通编译器访问规则判断；此数据模型不提供运行期权限求值 API。

### 3.2 字段与 property

| 类型 | 主要成员 |
| --- | --- |
| `FieldInfo` | `id: FieldRef`、owner、源码名称或位置、`typeUse: TypeUseInfo`、`isOptionAtDeclaration: Boolean`、声明序、注解、visibility、来源、构造参数关联 |
| `PropertyInfo` | `id: PropertyRef`、owner、name、typeUse、`isOptionAtDeclaration: Boolean`、val/var、visibility、注解、getter/setter 描述、存储类别、关联的 backing/delegate field |
| `AccessorInfo` | 实际 accessor 引用、visibility、注解及普通/abstract/存储访问类别；不提供 invoke |
| `TupleElementInfo` | index、展示名、typeUse；不伪造 FieldRef 或构造参数声明 |
| `VariantInfo` | `id: VariantRef`、所属 enum、name、注解、声明序、payload 形状 |

`FieldInfo.type` 可作为 `typeUse.type` 的只读便捷属性。字段位置用 `MemberName.Named(String)` 或 `MemberName.Positional(Long)` 表达；命名 payload、位置 payload 不能只凭一个可空字符串混在一起。

enum payload 用 `Unit`、`Named(Array<FieldInfo>)`、`Positional(Array<FieldInfo>)` 分支表示。类型描述列出全部 variant；这不允许从实例读取非活动 variant 的 payload。

class 的 `PropertyInfo` 是源码 property；其 backing field 是另一个实体。computed property 没有 backing field，delegated property 的 delegate storage 也不等于 property 的值类型。property/getter/setter/backing/delegate 上的注解保持原 target，不互相复制。

struct/enum 字段按现有规则具有固定 public representation visibility，与 owner 的访问域取交集。class property 省略修饰符仍为 internal。描述保留非 public 成员的信息，不因此允许直接读取；描述完整性不随查询位置变化。

### 3.3 构造关系

| 类型 | 主要成员 |
| --- | --- |
| `ConstructorInfo` | 实际 ConstructorRef、owner、primary/secondary/variant 类别、visibility、注解、有序参数 |
| `ParameterInfo` | ParameterRef、name、typeUse、声明序、注解、`hasDefault: Boolean`、`isVararg: Boolean`、对应的实际成员 |
| `ParameterMember` | `None`、`Field(FieldRef)` 或 `Property(PropertyRef)`；普通参数不是字段 |

描述公开 default 是否存在，不公开其可执行函数、可调用地址、自动求值入口或未经执行就伪造的默认值。默认表达式可以引用此前参数并有副作用，不能把它解释成 TypeInfo 内必有的常量。

primary 参数与字段/property 的映射用于 CLI 的必填规则及 schema 对齐。编译器内部已有的 default template 引用继续供正常 Decodable 合成使用；公开元数据无须复制可执行 body。

字段/property 的完整声明顺序与 constructor 参数顺序分别保存。CLI 必须沿构造映射确定 default，不能假设两个数组的第 n 项必然相同。

### 3.4 声明处 Option 与具体化后的类型

`FieldInfo.isOptionAtDeclaration` 和 `PropertyInfo.isOptionAtDeclaration` 表示：该成员的类型在原声明处完成绑定、透明 alias 展开后，外层是否已经是实际核心 `Option` application。这个布尔值在宿主泛型具体化前确定，随原成员保留；不按短名、源码是否写了 `?` 或具体化后的 TypeRef 重新判断。

| 原字段类型 | 具体化后的类型示例 | isOptionAtDeclaration | 无 default、无 required 覆盖时 |
| --- | --- | --- | --- |
| `String?` 或 `Option<String>` | `Option<String>` | true | 默认解码允许缺失，补 None |
| `T?`，T 取 String | `Option<String>` | true | 默认解码允许缺失，补 None |
| `T`，T 取 `Option<String>` | `Option<String>` | false | 默认解码仍要求字段存在 |
| 展开为 `Option<T>` 的 alias | `Option<String>` | true | 与直接声明 Option 相同 |
| 展开为 T 的 alias，T 取 `Option<String>` | `Option<String>` | false | 与直接声明类型参数相同 |

它是原成员类型的事实，不表示成员是否参与序列化，也不直接表示字段必填性。computed property 同样可以有此事实，但不会因此成为合成 codec 的字段。`Serialize.ignore`／`required`、constructor 的 hasDefault 与这个事实共同决定 CLI 默认 schema；规则见 [CLI 5.1](CLI.md#51-默认-record-schema)。实际字段 TypeRef 始终保留完整具体类型，用于值形状、类型判等和展示。

生成该事实和 M46 合成中的对应判断复用同一已绑定成员类型查询。既有 HIR／产物已经保留原声明的带 binder 类型；从该事实生成一个普通 Boolean 即可，不公开未绑定的运行期 TypeRef，不复制默认表达式，也不新增通用 codec 描述协议。普通 TypeRef equality 不受此成员属性影响。

这个默认字段合同属于数据成员声明，针对具体化目标直接请求缺省 codec 时也保持一致；手写 codec 可以选择其他缺失行为，此时 CLI 使用显式 schema。TypeInfo 不读取、分析或保证任意 `Decodable<T>` 正文的行为。

## 4. 类型使用位置

同一个 `TypeRef` 的每次使用可以带不同的 type-use annotation。因此 `TypeUseInfo` 具有：

| 成员 | 含义 |
| --- | --- |
| `type: TypeRef` | 当前使用位置的完整语义类型 |
| `annotations: Array<AnnotationInfo>` | 只属于当前类型使用节点的注解 |
| `children: Array<TypeUseChild>` | 按类型语法角色组织的子使用位置 |

`TypeUseChild` 以封闭角色区分 nominal argument、tuple element、function receiver/parameter/result、pointer pointee。角色含需要的序号，不依赖字符串路径猜位置。

alias 展开及 generic 替换沿 M37 的 type-use 规则保留位置。例如两个 `List<String>` 字段的 String 实参注解可以不同，但其 TypeRef 仍相等。替换得到的类型身份与该字段源使用位置的注解分别保存。

constructor 参数、字段/property、base/interface 与函数签名均通过 TypeUseInfo 表达使用关系。元数据展开不修改共享 TypeRef 上的注解，也不产生带注解的新 nominal identity。

## 5. AnnotationInfo 与查询

### 5.1 注解声明不变成普通 runtime class

注解参数在编译期绑定并正规化，运行期只得到其数据描述。采用以下明确入口：

```text
annotationKey<Help>(): AnnotationKey
```

该查询由编译器识别，尖括号中的名字按正常 visibility/import 解析到 annotation declaration；它不是接收普通类型 T 的泛型函数。参数必须是一个实际 annotation 声明，错误名称、普通 class 或不可访问声明均在查询处诊断。通过 alias import/re-export 到达同一声明时保留同一 key。

调用表达式在 HIR 中直接保存已绑定的 AnnotationId，跨 Cone 模板不在消费方重新查找名字。TypeInfo 与此查询的编译器支持只用于生成数据，不参与成员访问权限。

不定义 `field.annotation<Help>(): Help`，因为 Help 仍不是可实例化类型；不引入 runtime annotation constructor、泛型 annotation kind 或按短名称匹配。

### 5.2 声明、应用和值

| 类型 | 主要成员 |
| --- | --- |
| `AnnotationKey` | 注解声明身份、displayName、`describe(): AnnotationDeclarationInfo` |
| `AnnotationDeclarationInfo` | 声明身份、visibility、有序参数及声明本身的元注解 |
| `AnnotationParameterInfo` | 参数名称、完整 AnnotationParameterType、`defaultValue: Option<AnnotationValue>`，有值时已正规化 |
| `AnnotationInfo` | key、按参数声明序排列的 arguments；嵌套注解值也使用此结构 |
| `AnnotationArgument` | 参数引用/名称、参数类型、`value: AnnotationValue` |

应用的实际 target 由包含它的成员、声明或 TypeUseInfo 描述节点确定，不把同一应用复制到其他 target。AnnotationInfo 本身表示值，不强制携带一个被标注声明：嵌套 annotation 参数/default 是值，不能为它虚构字段或声明 target。编译器内部的 application 仍保存 M37 已绑定的实际 typed target。

普通标量类型与 annotation 声明不是同一种类型。`AnnotationParameterType` 明确区分 `Scalar(TypeRef)`、`IntegerEnum(TypeRef)`、`Annotation(AnnotationKey)` 以及一维 `Array(AnnotationAtomType)`。AtomType 不含 Array，保留 M37 的一维限制。

`AnnotationValue` 是封闭的带标签值：Boolean、String、Char、Integer(IntegerValue)、FloatBits(UInt)、DoubleBits(ULong)、整型 enum 的合法 variant、嵌套 AnnotationInfo、有序 Array。IntegerValue 的八个带类型分支保留实际位宽和符号，浮点保留目标精度及 raw bits，整型 enum 不退化成裸整数。

数组携带完整元素种类及有序值，空数组仍有类型。嵌套 annotation 自己可以含数组参数；这不允许 Array<Array<T>>。M38 直接发布 M37 已检查的 typed 常量，不重新运行用户代码或常量求值。

应用中的缺省参数已补齐，arguments 按声明顺序排列。合法的 repeatable 应用保留源码顺序和重复值；不同 target 的应用分开保存。注解参数 default 是静态数据，与普通 constructor default 的可执行表达式不同。

### 5.3 库的使用方式

库以实际 AnnotationKey 从某个成员的 `annotations` 中筛选应用，再按声明中的参数取值。可以提供普通 `findAnnotations(items, key): Array<AnnotationInfo>` helper；它只遍历当前列表，不沿 owner/base/accessor 自动继承或合并注解。

debug 与 CLI 分别查询自己声明的注解 key；CLI 的默认 schema 还查询实际核心 Serialize 的 key。Serialize 的 String／Boolean 参数全部属于现有 AnnotationValue 值域，不增加特殊反射表示。编译器不因遇到 `CliOption`、`DebugIgnore` 等名称而改变一般 annotation 绑定、读取权限或 default 求值。

## 6. 生成、存储与生命周期

编译期生成意味着结构内容由源程序和类型实参决定，不承诺查询调用不分配、不进入 GC 或适用于 `@NoGC`。结果可由普通不可变 struct/class/Array/String 承载，必要缓存按现有 singleton/ODR 规则共享。

对递归结构，先建立独立类型引用，再惰性取得当前层描述；字段只保存对子 TypeRef 的引用。不得在 Node 描述初始化时递归调用 Option<Node>、再回到 Node 的完整初始化。引用初始化不执行其 describe 函数。

运行期惰性展开不等于编译期可以生成无限多份代码。描述函数及其引用闭包也纳入既有泛型实例化终止检查：同型自递归/互递归共享已有描述；若例如 `Grow<T>` 的成员引用 `Grow<Array<T>>`，为可继续 describe 的类型引用生成闭包会沿同一递归环不断改变实参，则该描述请求按现有 polymorphic recursion 规则报编译错误，并指出触发查询和成员路径。不能用运行期懒加载、深度阈值或缺失子描述来掩盖问题，也不因此把未请求描述的普通类型声明一概判为非法。

不要求应用观察到唯一 TypeInfo 对象 identity，类型判等必须用 TypeRef 的语义 equality。底层实现可以共享描述，不能把地址相同作为正确性的必要条件。跨 Cone 相同类型及声明使用已有 exact/persistent identity 关系。

描述只保留实际引用闭包。查询类型的 companion 描述不初始化 companion；查询 `typeInfo<T>()` 不隐式寻找 `T.Companion`，也不合成 Inspectable 或 Decodable conformance。

公开描述中的成员名称和注解属于程序可观察数据。改变它们必须影响实际产物和相应依赖缓存；调试构建与 release 构建不能静默删去被程序使用的数据。没有查询且没有依赖的描述允许沿普通 dead-code/data elimination 删除。

不扩展 C runtime TypeDescriptor 为源码字段/注解总表，不引入全局 `typeByName`。普通闭包和引用按现有 GC rooting/relocation 契约处理，不保存未经 rooting 的对象地址。

## 7. 错误与验证

- 非法类型实参、错误 annotation 查询、不可见声明、缺失必要的 `.slib` 结构信息都在对应编译边界报错，不返回内容缺失的 TypeInfo。
- HIR 的查询结果具有确定类型；各 shape 分支所需内容由结构保证，不用 `Option<TypeInfo>` 表示“以后再补”。有意义的可选关系，例如无基类或无默认值，才使用 Option/封闭分支。
- 字段顺序、完整替换、enum payload、constructor 关联、同名不同身份、泛型 companion、同型递归、增长型描述闭包诊断及 type-use 注解都需要本地与产物消费用例。
- 同一具体字段类型来自原声明 T 或 Option<T> 时，isOptionAtDeclaration 的结果必须保留区别；包含透明 alias、本地查询、仅产物消费和泛型再次发布。它与 M46 默认字段分支及 CLI help 的必填状态组合验证。
- 验证初始化计数：纯元数据查询不执行目标 constructor/default/getter/companion；metadata 自身的正常库分配允许发生。
- 私有成员可以出现在描述中，但取得描述不能用作读取原始值的 API。实际读取错误由 [DEBUG.md](DEBUG.md) 中的普通方法检查覆盖。
