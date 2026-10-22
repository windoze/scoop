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
