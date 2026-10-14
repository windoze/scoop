# M46 设计：统一 Serialize 与直接可用的 DTO

状态：设计完成，待实施。日期：2026-10-14。本文补充[主设计](DESIGN.md)，按后续讨论把统一序列化注解、可缺省字段合成和默认 Option codec 纳入 M46，替代初稿中“本期不改 annotation／Option”的限制。字段 codec 的声明式绑定仍留待后续。

## 1. 一个注解，四项配置

core 提供以下普通注解声明；采用现有 `String`、`Boolean` 参数与 `@Target`，不扩展 annotation 值域：

```scoop
@Target([
    AnnotationTarget.FIELD,
    AnnotationTarget.PROPERTY,
    AnnotationTarget.ENUM_VARIANT
])
public annotation class Serialize(
    val rename: String = "",
    val required: Boolean = false,
    val emitNull: Boolean = false,
    val ignore: Boolean = false
)
```

| 参数 | 含义 |
| --- | --- |
| `rename` | 非空时指定 wire 名；空串保持源码名。字段与 enum variant 共用此项 |
| `required` | 为 true 时强制输入含有该字段，并且编码时总是写出；false 表示按字段类型和 default 推导是否可缺省 |
| `emitNull` | 对 Option 字段，为 true 时不省略 None；使用默认 Option codec 时将其写为 null。只影响编码，不改变解码的必填性 |
| `ignore` | 两个方向都排除该存储字段，不要求其 codec；解码仍必须能正常初始化它 |

`required = false` 不把任意字段变成可选项。没有 default 的普通 `T` 仍然必填；`T?` 或有 default 的字段自然可缺省。相较于 `optional = true`，这个命名明确表示“是否额外要求出现”，避免让 `@Serialize(rename = "user_id") val id: Long` 看起来允许凭空补出一个 id。

没有注解与 `@Serialize()` 的行为相同。参数只依据补齐默认值后的普通静态值解释，不增加“调用处是否显式写了参数”的 metadata，也不通过源码位置猜测参数来源。主构造 `val`／`var` 沿现有规则把注解标在字段或 logical property 上。

此注解不自动创建 companion 或添加 conformance。DTO 仍通过普通 companion／codec 声明请求 `Encodable<Dto>`、`Decodable<Dto>` 的缺省方法；手写实现按现有规则优先。

## 2. 解码：存在性、默认值和值类型

合成器只对 keyed record 的参与字段应用下表，包含 struct、现有合成范围内的 class，以及 enum 的命名 payload。位置 payload、tuple 和序列元素没有“省略字段”。

| 声明 | 缺少 key | key 存在且为 null | 默认编码行为 |
| --- | --- | --- | --- |
| `val id: Long` | 报缺失错误 | 由 Long codec 报类型错误 | 总是写出 |
| `val limit: Long = 20L` | 使用 20L | 由 Long codec 报类型错误，不触发 default | 总是写出，包括值为 20L 时 |
| `val name: String?` | 得到 None | 默认 Option codec 得到 None | None 省略，Some(v) 写出 v |
| `val name: String? = None` | 使用 default，得到 None | 默认 Option codec 得到 None | None 省略，Some(v) 写出 v |
| `val name: String? = Some("guest")` | 使用 default，得到 Some("guest") | 默认 Option codec 得到 None | None 写为 null；Some(v) 写出 v，原因见第 3 节 |
| `@Serialize(required = true) val name: String? = None` | 报缺失错误 | 默认 Option codec 得到 None | 总是写出，None 为 null |
| `@Serialize(emitNull = true) val name: String? = None` | 使用 default，得到 None | 默认 Option codec 得到 None | 总是写出，None 为 null |

解码顺序为：先处理 ignore；然后由 required、字段的完整静态类型和已声明 default 决定取得 `required(name)` 还是 `optional(name)`。字段存在时，始终将 child 交给按现有优先级选定的完整字段 `Decodable<F>`。字段缺失时，显式 default 优先；没有 default 的核心 Option 才直接补 None；其他字段必须报错。

显式 null 不触发 default。是否接受 null 由值 codec 决定：默认 Option 接受 null，Long／String 等拒绝 null，Unit 和 `JsonValue` 则可以把 null 当作自己的合法值。`required` 只规定 key 必须存在，不等于值不得为 null。

保留原有求值顺序：先按构造参数顺序读取所有已提供字段，完成容器检查，再按参数顺序为缺失项执行已绑定的 default，最后正常构造对象。default 可引用前面的参数，每次构造至多执行一次。缺失时直接补 None 不执行用户代码；缺失字段不求值其 codec。已提供字段的错误不会被 default 掩盖。

可缺省的序列化字段与源码构造器的可省略实参是两件事。`val name: String?` 的解码可以补 None，但普通源码调用要省略这个构造实参，仍需声明 `= None`；M46 不改变函数或构造器调用规则。

## 3. 编码：父 record 决定是否出现

每个参与字段只读取一次。ignore 字段不读取、不创建 child，也不求值 codec。其他字段先决定是否省略，再调用 `field(wireName)`、求值已选 codec，并把完整字段值交给 `Encodable<F>`；不能先创建 child 再决定不写。

None 仅在同时满足以下条件时自动省略：

1. 字段在合成定义处的实际类型为核心 `Option<T>`，包括 `T?` 和透明 alias。
2. `required = false` 且 `emitNull = false`。
3. 输入缺失后，合成器可以确定该字段恢复为 None：没有 default，或 default 是静态绑定到核心 None variant 的直接表达式。

括号和指向同一实体的限定名／透明 alias 不改变第 3 条；普通变量读取、函数调用或其他 default 表达式均不做执行、常量求值或等价性推断。无法直接确定时，编码 None 必须保留字段并调用其 codec，默认结果为 null。

这个规则避免以下错误往返：

```scoop
public struct Preferences(val label: String? = Some("guest")) {
    public companion object : Encodable<Preferences>, Decodable<Preferences> {}
}
```

`Preferences(None)` 必须编码为 `{"label":null}`。若编码成 `{}`，再次解码会得到 `Some("guest")`，改变 DTO 的值。即使 default 是一个恰好返回 None 的函数，也保守写出 None；encoder 不能为决定省略而执行该函数。

Some(v) 总是交给完整字段 codec 编码；其他类型的字段也照常写出。M46 不增加“等于 default 就省略”，不比较任意值，不调用 equals，也不在编码时求值 default。`emitNull = false` 表示允许满足上述条件的省略，不表示禁止输出 null。

显式注入的完整 `Encodable<Option<T>>`／`Decodable<Option<T>>` 继续优先；注解不绕过它们去强行调用元素 codec。对于这种自定义 codec，`emitNull = true` 的精确含义是“把 None 也作为字段值交给 codec”，wire 内容由该 codec 决定。本文中 None ↔ null 的例子均使用默认 Option codec。

## 4. Option 默认 codec 改为 nullable 值

仅改变字段出现规则还不够：若 Some(v) 仍编码成 `{"Some":[v]}`，DTO 无法直接表达常见 JSON。因此同步修改 core 的普通 `Option.encoder(element)`／`Option.decoder(element)` 实现，公共签名保持：

| 值 | 默认 wire 值 |
| --- | --- |
| `None` | null |
| `Some(v)` | 元素 codec 对 v 的输出 |

decoder 先调用 `singleValue().isNull()`；为 true 时返回 None，否则把同一 decoder 交给元素 codec 并返回 Some。它不通过捕获任意异常判断 null，且非 null 的错误按元素 codec 报告。这直接使用 M46 的 null 查询和稳定视图合同，全部是普通 core 源码。

该变化作用于所有默认 Option codec 的组合位置：根、字段、数组、tuple、map 的 value 等。根值 None 输出 null，数组中的 None 保留为 null 并占据一个元素；只有父 keyed record 能省略字段。普通 enum 的合成仍使用外层单 key，Map 的默认 pair-sequence 外形也不改变。

`T?` 与 `Option<T>` 是同一种类型，不能根据源码拼写选择不同的 wire 格式。泛型合成仍在定义处决定规则：`Box<T>(val value: T)` 的 value 没有 default，始终必须出现；即使后来 T 取 Option，也不重新决定字段存在性。要声明可缺省的 Option 字段，应在定义处写 `T?`。

nullable 表示无法无损区分所有嵌套 Option 值：例如 None、Some(None)，以及元素 codec 本身输出 null 的 Some(v)，可能映射到同一个 null。M46 明确接受这个取舍，以通常 DTO 的可空字段为默认使用场景；需要保存这种领域区分的调用方显式提供合适的 codec，不自动尝试旧带 tag 格式。普通 Option 的内存布局、语言类型身份及模式匹配语义不变。

## 5. DTO 示例与信息保留边界

```scoop
public struct UserDto(
    val id: Long,
    @Serialize(rename = "display_name")
    val displayName: String? = None,
    @Serialize(required = true)
    val email: String? = None,
    @Serialize(emitNull = true)
    val note: String? = None,
    @Serialize(ignore = true)
    val cacheVersion: Long = 0L
) {
    public companion object : Encodable<UserDto>, Decodable<UserDto> {}
}
```

`Json.encode(UserDto(7L), UserDto.Companion)` 的结果为：

```json
{"id":7,"email":null,"note":null}
```

displayName 为 None 所以省略；email 必须出现；note 主动写 null；cacheVersion 是本地字段。解码 `{"id":7,"email":null}` 成功，其余缺失字段按上述规则补值；解码 `{"id":7}` 则在 email 路径报缺失错误。业务代码直接读写同一个 DTO，不需要先转换为专门的 presence enum。

普通 `T?` 只有 None／Some 两种状态。默认解码允许把缺失和显式 null 归并为 None，随后编码按字段声明输出；这不承诺保存输入是否曾出现该 key。协议本身仍通过 `optional`／`isNull` 完整区分缺失、null 和有值。

对 PATCH 一类确实区分“没有修改”与“设置为 null”的业务，应用可以增加一个普通三态 enum，并直接作为 DTO 字段类型。M46 必须保证这种扩展能由普通 codec 完成，不需要修改 core 协议或 JSON 实现；具体约定见下节。不能用注解在普通 `T?` 中凭空存下第三种状态。M46 不为每个 DTO 添加隐藏 presence 位、伴随对象或运行期字段表，也不要求一般 DTO 采用三态 enum。

### 5.1 未来三态字段的普通扩展

应用可以自行定义以下类型；它不是新增的 core 类型，也没有特殊的 enum 名称或 variant 身份规则：

```scoop
public enum FieldState<T> {
    Missing,
    Null,
    Value(T)
}
```

以 PATCH DTO 的 `nickname: FieldState<String>` 为例，普通父 record codec 或接收 keyed 容器、字段名及元素 codec 的普通泛型 helper 实现以下映射：

| wire 输入／输出 | DTO 字段值 | 解码操作 | 编码操作 |
| --- | --- | --- | --- |
| `{}` | Missing | `optional("nickname")` 返回 None | 不调用 `field("nickname")` |
| `{"nickname":null}` | Null | 字段存在，child 的 `isNull()` 为 true | 创建 field，调用 `singleValue().writeNull()` |
| `{"nickname":"Ada"}` | Value("Ada") | 字段存在且非 null，把同一个 child 交给 String codec | 创建 field，把值交给 String codec |

这些操作保留全部三种状态，DTO 无需先转换成其他 wire DTO 或保存整份 JsonValue。Missing／Null 分支不调用元素 codec；非 null 的错误按元素 codec 报告，不能捕获后改成 Missing 或 Null。普通类型为元素时，可继续复用其合成 codec，null 查询不妨碍后续取得 keyed／unkeyed 视图。

字段是否存在仍由父 record 决定：只有字段值的 `Encodable<FieldState<T>>` 无法撤销父容器已经创建的 field，只有 child 的 `Decodable<FieldState<T>>` 也无法观察不存在的 key。因此本期的支持方式是普通父 codec／字段 helper；新增 enum 不会自动获得 Option 专属的省略规则。未来若要让自动合成直接采用这套映射，应在合成配置层扩展，基础协议无需改变。

这里的 Missing 表示 keyed 字段不存在，不自动适用于根值或数组元素；不能把它静默变成 null 或删除数组位置。普通带 tag 的 enum codec 仍可在这些位置表达全部 variant。该字段映射以元素 codec 的非 null 值域为前提；若 T 自身也能编码成 null，应用须为 Null 与 Value(null) 选择明确的可区分表示。

M46 验收必须包含普通用户 enum 与手写父 codec 的三态往返、非 null 结构元素的合成 codec 组合，以及已知错误值的路径检查。这验证扩展能力，不提前发布三态标准类型或通用字段 codec 协议。

## 6. 使用位置、冲突与 ignore

- rename 适用于参与 record 的存储字段／property，以及 enum variant；其余三个参数仅适用于存储字段／property。enum 的命名 payload 按 record 字段处理。type 本体、computed property、tuple 和位置 payload 不接受本期 Serialize。
- `ignore = true` 时，rename 必须为空，required 和 emitNull 必须为 false。这样保留原 SerialName／Transient 不同时作用于同一字段的明确规则，不留下无效的非默认配置。
- ignore 字段的合成 decode 必须能通过声明的 default、普通 class 初始化，或 Option 的 None 补值完成构造。没有补值方式的非 Option 字段仅在请求 decode 合成时报错；只请求 encode 时，不额外要求 default。
- `emitNull = true` 只接受定义处已知的核心 Option 类型；对其他类型报错。`required = true` 可以标在本来已必填的字段上，也可以和 emitNull 一起用于 Option；二者不冲突。
- 同一实际 target 不允许重复 Serialize；导入别名、re-export 与跨 Cone 使用沿现有 annotation identity 和 use-site 规则处理。手写 codec 不被注解自动包装，其具体行为由正文负责。
- 同一 record 的参与字段 wire 名，以及同一 enum 的 variant wire 名仍须唯一。即使某个 Option 在某次运行会省略，也不能让两个参与字段静态使用同一个 key。ignore 字段不参加输出名冲突检查。

由于 `rename = ""` 表示不改名，声明式重命名不能指定空 key。当前 SerialName 可以携带空字符串，这是合并时明确的兼容性边界；需要空 key 的数据可以使用 keyed 容器的动态名称或手写 codec。JSON、`JsonValue` 和动态 map 继续支持空 key，不为这个边界扩展 annotation 的 Option 参数值域。

## 7. 两组基线如何使用

API 的普通 record 优先直接使用上述 DTO 合成：名称差异用 rename，可缺／可空字段用 `T? = None`，必须出现的可空字段用 required，明确需要输出 null 的请求字段用 emitNull。不同字段集合仍由对应的消息／内容／事件类型表达，不用一个含大量可选字段的 record 吞掉所有分支。tag、flatten、未知字段收集等结构组合继续允许普通手写 codec。

`Cone.toml` 没有 null 值。等价 JSON 的可缺但非可空字段可以使用非 Option 类型与正常 default，例如 `val cxx: Boolean = false`；null 会由 Boolean codec 拒绝。确实需要以 Option 保存“未提供”的配置字段，由普通 manifest codec 在已有逐层字段检查中拒绝显式 null，再读取其值。该规则与严格未知字段检查、短长式、locator 冲突等 manifest 约束同属 schema 的普通代码，不能把 null 当成省略来通过基线。DTO 仍可使用普通 Option，不要求额外三态类型。

普通 API DTO 的缺失／null 规范化与需要保留原始 wire 的测试分别验收：前者检查字段声明决定的输出，后者使用协议查询和适当的 JsonValue 子值保存字段存在性。保留任意值的入口不能代替已知字段的 typed 解码和错误检查，详见[基线与验收](BASELINES.md)。

## 8. 实施边界与迁移

| 层次 | 变更 |
| --- | --- |
| core／语言合同 | 在现有 annotation 值域内定义 Serialize；修改普通 Option codec；新增 null 查询仍按主设计实施 |
| 编译器合成 | 用一份已绑定的字段配置供 encode／decode 使用；合成存在性分支、None 省略、default／ignore 补值；保持定义处的 codec 选择和普通 typed body |
| `scoop.json` | 实现主设计的容器与 JsonValue 合同；不识别 Serialize，不查询 DTO 字段类型或 Option identity |

实施时先修订语言规范 9.4、11.13.1／11.13.3／11.13.4 和实现规范 2.17，再改 core 声明与普通方法、HIR 合成和相关测试。已有注解正规化与 typed metadata 足够承载四个参数，不改 parser 的注解语法，不加 JSON intrinsic、专用 IR、runtime ABI 或跨 stage 的重复注解解释。

迁移一次完成：`@SerialName("x")` 改为 `@Serialize(rename = "x")`，`@Transient` 改为 `@Serialize(ignore = true)`；删除旧 core 声明和合成解释入口，同步源码、fixtures、示例与规范。Option 缺省 wire 格式与 Option 字段缺失规则是有意的行为变更；相关测试应更新到新合同，依赖旧协议的数据需显式选择相应 codec。现有依赖／产物按普通接口变化规则重建。

以后新增静态的命名、tag、flatten 或 unknown-field 配置，可继续扩展 Serialize 及其合成规则。字段 codec 本身涉及类型／函数等如何成为注解值，不能仅靠增加一个 String 参数或以 FQN 查找对象解决；本期不设计该绑定能力。保持“一个注解作为配置入口”，不承诺任何未来扩展都无需修改语言或其他层。

## 9. 必要验收

每项合成行为有独立 fixture，并与泛型、手写 codec、普通 `.slib` 消费组合验证；格式输出可由 `scoop.test` 覆盖。

| 组别 | 必须覆盖 |
| --- | --- |
| 自然默认 | 无注解的普通必填字段、带 default 的非可空字段、`T?`／`Option<T>`／alias 的缺失和 null；单独 rename 不改变必填性 |
| 输出 | None 省略、Some 值直写、required 可空字段、emitNull、根／序列／tuple 中 None 保留；错误值不因可选而被吞掉 |
| 默认值 | 直接 None、Some(v)、带副作用或引用前参的 default；编码不执行 default，缺失才执行一次；显式 None 不会往返成非 None default |
| ignore | 无 codec 的本地字段、Option 补 None、非 Option default、只编码时无 default；解码无补值方式、冲突参数的准确编译错误 |
| 位置与名称 | enum variant rename、命名 payload 的可选字段、错误 target、重复注解、rename 冲突和忽略字段不参与冲突 |
| codec 与泛型 | 完整 Option codec 注入优先；省略时不创建 child 或求值 codec；定义处 T 与 T? 的字段规则不因实例化而重选 |
| 三态扩展 | 普通 FieldState enum 经父 codec／字段 helper 保留 Missing／Null／Value；复用元素合成 codec，不修改 core／JSON；错误值不折叠为缺失或 null |
| 组合与兼容 | 统一注解跨 Cone 导出／消费；API DTO 与手写 tag／flatten 组合；Cone 显式 null 拒绝；移除旧注解和旧 Option 默认输出断言 |

不以每个布尔组合复制整套协议测试；测试围绕有不同可观察结果的分支、真实 DTO 和必要错误展开。本文件仍是设计，以上验收尚未执行。
