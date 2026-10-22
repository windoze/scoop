# M47 JSON 数据编码

本文件细化 [M47 协议](PROTOCOL.md)第 3～4 节的公共数据。对象字段使用 camelCase，封闭分支使用字符串 `kind`。此处规定已实现的基础编码；完整查询请求、成员／正文以及进程接入的交付状态仍以[公共基础实施记录](../language-service/PROGRESS.md)为准。

## 声明引用

identity 使用原 32 字节对应的 64 位十六进制字符串；写出小写，读取也接受大写。长度或字符不合法时报格式错误。Rust 的 `Reference<I>` 保留原 identity 类别，只表示一个待读取的引用；它不创建或重新派生语言实体。

- 名义声明：`{kind: "concrete" | "generic", id}`。
- 待填方法 MethodId：`{kind: "function" | "genericFunction", id}`。
- callable 声明：`{kind: "function" | "genericFunction" | "constructor" | "propertyAccessor" | "variantConstructor" | "generated", id}`。是否有普通正文仍由正文可用性回答。
- 字段：`{kind: "field" | "variantField", id}`。
- 参数引用使用原 callable 与声明位置，不能把参数位置解释为全局实体 identity。

## 完整类型

签名类型的封闭形状如下；`Type` 表示递归引用同一结构。

| kind | 其他字段 |
| --- | --- |
| `nominal` | `declaration`：普通名义 type identity |
| `nominalApplication` | `declaration`：泛型名义 identity；`arguments: [Type]` |
| `tuple` | `elements: [Type]` |
| `function` | `effect: "ordinary" | "suspend"`；`receiver: Type | null`；`parameters: [Type]`；`result: Type` |
| `rawPointer` | `pointee: Type` |
| `nativeFunctionPointer` | `callingConvention: "c"`；`parameters: [Type]`；`result: Type` |
| `binder` | `depth`、`index`：所属真实签名上下文的 binder 层与位置 |
| `companion` | `host: Type` |

类型的 nominal 字段不展开其成员。binder 类型只能随实际声明／正文的上下文传递；声明与应用类型各用其对应上下文，不能把开放参数当作某个全局 concrete type。作用域选择不改变访问权限。

类型输入沿同一形状解码。nominalApplication 的 arguments 和 tuple 的 elements 不得为空；单元素 tuple 合法，空 tuple 使用实际 Unit 名义引用。未知 kind、effect、callingConvention、非法引用文本或不能表示为 u32 的 binder 位置为格式错误。格式解码得到既有 DecodedSignatureTypeKey，不创建已解析的声明身份。

语义解析以当前源码和已选依赖的实际名义声明建立索引，分别查找普通与泛型身份；未找到为查询错误，不要求引用先在另一条响应中出现。泛型元数和 bound、函数指针的普通具体签名与 C ABI 合法性、companion 条件及 NoGC／Ptr／具体 CLayout 表示沿前端原有规则检查。结构表示的 Unit／整数／Any／Ptr／FunPtr 保留真实 core 声明关系。binder 必须存在于本次适配器选定的真实上下文，不能以 arena 序号或同名参数补足。

## 源码来源

SourceIdentity 为 `{cone, path}`，`cone` 使用 typed cone identity，`path` 为原规范化逻辑路径，不是宿主绝对路径或 LSP URI。来源为 `{source: SourceIdentity, span: {startByte, endByte}}`；byte range 为 UTF-8 的半开区间。带词法来源的正文记录额外保留原 `context` identity。投影不读取源码，也不据路径寻找同名文件。

## 常量与注解

| kind | 其他字段 |
| --- | --- |
| `integer` | `signed: boolean`、`width: 8 | 16 | 32 | 64`、`value: string` |
| `boolean` | `value: boolean` |
| `char` | `value`：恰含一个 Unicode 标量的字符串 |
| `string` | `value: string` |
| `float` | `width: 32 | 64`、`bits: string` |

整数 `value` 是按原宽度解释后的十进制字符串，例如 signed 8 位的 `0xff` 为 `"-1"`，unsigned 64 位全 1 为 `"18446744073709551615"`。浮点 `bits` 为 8 或 16 位小写十六进制字符串，不带 `0x`；保留负零、无穷及 NaN payload。不得经过 JSON 浮点数转换。

注解值为以下封闭分支：

- `scalar`：`value` 为上表的常量对象。
- `integralEnum`：`valueType` 为完整类型，`variant` 为实际 variant identity，`value` 为整数常量。
- `annotation`：`annotation` 为实际 annotation identity，`arguments` 按声明参数顺序保存完整注解值。
- `array`：`elementType` 保留 scalar／integralEnum／annotation 类别；`elements` 是 `{value, origin}` 数组，每个元素保留自己的来源。

数组的 scalar elementType 使用 `{kind: "scalar", scalar: ...}`：integer scalar 带 `signed` 与 `width`，float scalar 带 `width`，其余 scalar 仅含 `kind`。integralEnum elementType 带 `valueType`，annotation elementType 带 `annotation`。注解应用使用 `{annotation, arguments, origin}`，源码与依赖投影遵循同一形状。

注解声明为 `{annotation, name, parameters, targets, repeatable, visibility, origin}`。parameters 按声明顺序，每项为 `{declaration: {annotation, index}, name, valueType, default}`；无默认值时 default 为 null，已存在的默认数组与其中的逐元素来源保持。valueType 区分 `{kind: "value", valueType: Type}`、`{kind: "annotation", annotation}` 与 `{kind: "annotationArray", array, element}`，后者保留实际 Array 名义声明和元素 annotation 声明。targets 使用语言中的 AnnotationTarget 名称，如 Class、Field、ValueParameter；不以整数 ordinal 或任意位图传输。visibility 沿成员编码，origin 统一为 SourceLocation。读取声明不展开其他注解声明，也不重新执行默认常量。

注解 target 使用以下封闭形状；其类别直接来自原语义模型。字段中的 callable 使用 CallableDeclaration（局部 callable 为 generated），PropertyDeclaration 为 `{kind: "property" | "extensionProperty", id}`。

| kind | 其他字段 |
| --- | --- |
| nominal | `owner: NominalId` |
| variant | `variant` |
| field | `field` |
| variantField | `field` |
| property | `property` |
| extensionProperty | `property` |
| annotation | `annotation` |
| callable | `callable: CallableDeclaration` |
| parameter | `callable: CallableDeclaration`、`index` |
| callableReceiver | `callable: CallableDeclaration` |
| propertyReceiver | `property: PropertyDeclaration` |
| typeAlias | `alias` |
| propertyStorage | `property: PropertyDeclaration`、`role: "backing" | "delegate"` |

参数位置仅在该 callable 内有意义；receiver 不伪装成值参数 0，存储 target 不伪装成属性本身。每个 typed identity 仍采用声明引用的十六进制格式，实际引用解析属于当前固定视图。

target 的解析覆盖当前源码和已选依赖中的实际声明，也覆盖没有注解的合法目标。字段／variant 字段、普通／扩展 property 和 callable 类别分别查找；不存在的参数位置、receiver 或存储组成部分返回未找到，不能当作一个空注解列表。已解析的目标保留原规范化引用、可供读取注解的本地关联和定义来源。字段、variant 字段与值参数使用自己的来源；receiver／存储位置关联到实际 callable／property 的定义位置。

依赖按已发布的实际组成部分查询：`RuntimeAccessor` 本身不能推出隐藏存储；已有 storage 访问器、native storage、委托模板或已发布的存储注解目标才提供对应的 backing／delegate 引用。局部 callable 的值参数沿原参数 selector 读取，receiver、闭包环境和 capture ABI 参数不计入值参数位置。

## 待填方法签名

CompleteMethodSignature 包含 `method`、`owner`、`name`、`receiver`、`binders`、`contextParameters`、`parameters`、`result`、`effects` 和 `origin`。method 使用 MethodId，owner 使用实际名义声明引用，所有类型按本签名的 binder 上下文解释。

每个 binder 保存 `depth`、`index`、`name`、`declaration`、`bounds`、`excludedInterfaces`、`companion`、`requirements` 和 `origin`。declaration 为 `{kind: "nominal", owner: NominalId, index}` 或 `{kind: "callable", owner: MethodId, index}`，宿主参数继续属于名义声明，方法参数属于实际方法。bounds 区分 unconstrained／value／ref／nominal；nominal bounds 按源码顺序保留 class／interface 类型和来源。排除的接口和 companion 约束均保留原绑定。requires 项保存真实 ordinal、完整 contract 与来源，显式类型实参与待推导实参分开。

值参数保存 `declaration: {owner: CallableDeclaration, index}`、`name`、`valueType`、`calling` 和 `origin`。calling 区分 required／default／vararg：default 携带 `{owner, parameter, arguments}`；vararg 另有 `elementType`，omission 区分 emptyArray 与 default。引用中的 owner、parameter 是实际参数声明，arguments 按原 default 提供方顺序给出完整类型，不携带 arena 下标。context 参数保存其独立参数位置、named／unnamed label、完整类型和来源。

effects 保留 execution、safety、gc、callingConvention、operator、propertyDelegateOperator、infix 与 release；release 区分 unavailable 与 noTransition，并使用签名中的真实 binder 引用表达其条件。签名读取不会执行默认值或填充正文。

正文的完整节点、局部绑定及默认值应用规则见[逻辑正文 JSON](BODY-JSON.md)。

完整类型、字段、logical property、构造器、方法及访问域的字段见[类型与成员 JSON](MEMBERS-JSON.md)。
