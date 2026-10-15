# M38：独立 CLI Cone 与 Decoder

本文件从属于 [总体设计](DESIGN.md)。2026-10-14 修订以 [M46 Decoder 合同](../milestone46/DESIGN.md)和[统一 Serialize／Option 规则](../milestone46/SERIALIZE.md)为基线。类型描述由 [TypeInfo](TYPE-INFO.md) 提供，help 使用 [TextWriter](DEBUG.md)；参数对象继续由普通 `Decodable<T>` 构造。这些是待实施设计。

## 1. Cone 和依赖

CLI 库位于计划新增的 `sysroot/lib/scoop.cli/`，与 scoop.json 同级，源码 package 为 `scoop.cli`。manifest 为：

```toml
schema = 1

[cone]
group = "scoop"
name = "scoop.cli"
version = "0.1.0"
kind = "library"

[dependencies]
"scoop:scoop.core" = "0.1.0"
```

Cone identity 是 `scoop:scoop.cli:0.1.0`。应用显式增加 `"scoop:scoop.cli" = "0.1.0"` 依赖，再 import 相应源码名称。sysroot 只是发布位置，不使 CLI 成为 core 的隐式依赖。

CLI 注解、schema、argv 规则、Decoder、help 都属于该 Cone。core 只提供通用 TypeInfo、annotation 数据、Decoder 协议和 TextWriter。CLI 不依赖 scoop.json，编译器也不识别 `CliOption` 等名称的业务含义。

CLI 对 M46 的依赖是稳定解码视图、isNull、Serialize 及普通 Option codec；不依赖 JsonValue、JSON 树值转换或编码容器共享的实现。CLI 保留自己的逻辑输入与文本规则，不使用 JSON 文本或 JSON 私有节点作为中间表示。

## 2. 对外接口

以下是拟定签名目录，不是省略 body 的普通函数源码：

| API | 行为 |
| --- | --- |
| `Cli.schema<T>(info: TypeInfo<T>, config: CliConfig): CliSchema<T>` | 从适合默认 Decodable 模型的 record 生成 schema |
| `CliSchema<T>(info: TypeInfo<T>, config: CliConfig, entries: Array<CliEntry>)` | 显式 schema 的普通构造入口，使用与自动生成相同的数据和检查 |
| `CliSchema<T>.writeHelp(writer: TextWriter): Unit` | 只使用 schema 输出 help |
| `CliSchema<T>.help(): String` | 使用 StringWriter 返回相同 help 文本 |
| `CliParser<T>(schema: CliSchema<T>, codec: Decodable<T>)` | 保存不可变 schema 和显式 codec |
| `CliParser<T>.parse(arguments: Array<String>): CliResult<T>` | 参数列表，不包含 argv[0] |
| `CliParser<T>.parseCommandLine(argv: Array<String>): CliResult<T>` | 完整 Scoop main args，跳过 argv[0]，其余走同一解析器 |

`CliResult<T>` 是 `Parsed(value: T)` 或 `HelpRequested`。配置错误抛 IllegalArgumentException，输入错误抛 DecodingException；不增加隐式 stdout 输出、进程退出或全局解析状态。

`CliConfig` 至少包含明确的 `programName: String` 与 `summary: String = ""`。help 在解析前也可生成，所以 programName 不依赖某次 argv[0]，也不从启动路径猜应用名称。

Scoop 的 `main(args)` 保留原始 argv[0]。`parseCommandLine` 以其为程序项，不把它当作位置参数；空的完整 argv 是输入错误。`parse([])` 则是合法的空参数列表，是否缺少必填字段由所选 schema/codec 决定。两个入口复用同一内部游标，不重复 tokenize。

## 3. 完整使用示例

```scoop
import scoop.cli.*

public struct ServeArgs(
    @CliArgument(position = 0L, help = "Input file")
    val input: String,
    @CliOption(names = ["--port", "-p"], help = "Listen port", defaultText = "8080")
    val port: Int = 8080,
    @CliFlag(names = ["--verbose", "-v"], help = "Verbose output")
    val verbose: Boolean = false,
    @CliOption(names = ["--output", "-o"], valueName = "FILE")
    val output: String?,
    @CliOption(names = ["--include", "-I"], valueName = "PATH")
    val include: Array<String> = [],
) {
    public companion object : Decodable<ServeArgs>, Inspectable<ServeArgs>
}

fun run(args: Array<String>, writer: TextWriter): Int {
    val schema = Cli.schema(
        typeInfo<ServeArgs>(),
        CliConfig(programName = "serve", summary = "Serve an input file"),
    )
    val parser = CliParser(schema, ServeArgs.Companion)
    return when (val result = parser.parseCommandLine(args)) {
        case CliResult.HelpRequested -> {
            schema.writeHelp(writer)
            0
        }
        case CliResult.Parsed(options) -> {
            DebugPrinter.write(ServeArgs.Companion.inspect(options), writer)
            writer.write("\n")
            0
        }
    }
}
```

此示例演示拟定 M38 API；companion 显式请求两个缺省方法，并不是未完成的空实现。应用的实际 main 负责选择 writer 并处理异常/退出码。

output 没有 constructor default，仍因原声明的 String? 而允许 CLI 输入缺失，并由合成 decoder 补 None。这不改变普通源码调用 ServeArgs 构造器时的实参要求。

输入 `serve input.txt --port 9000 -v -I inc1 -I inc2 --output=` 的逻辑结果是 input 为 `"input.txt"`、port 为 9000、verbose 为 true、include 含两个元素、output 为 `Some("")`。

## 4. CLI 注解

```scoop
@Target([AnnotationTarget.FIELD, AnnotationTarget.PROPERTY])
public annotation class CliOption(
    val names: Array<String> = [],
    val help: String = "",
    val valueName: String = "",
    val defaultText: String = "",
)

@Target([AnnotationTarget.FIELD, AnnotationTarget.PROPERTY])
public annotation class CliFlag(
    val names: Array<String> = [],
    val negativeNames: Array<String> = [],
    val help: String = "",
    val defaultText: String = "",
)

@Target([AnnotationTarget.FIELD, AnnotationTarget.PROPERTY])
public annotation class CliArgument(
    val position: Long,
    val help: String = "",
    val valueName: String = "",
    val defaultText: String = "",
)
```

这些是普通用户 annotation，参数使用 M37 的已绑定静态数据。role 注解互斥，不可重复；普通 target/type 错误由已有 annotation 检查诊断，CLI 角色冲突与名称冲突由 schema 构造抛 IllegalArgumentException。

| 情况 | 自动规则 |
| --- | --- |
| 无 CLI role 注解 | 按源码字段名生成 `--name`；exact Boolean 作为 flag，其余支持类型作为带值 option |
| names 为空 | 使用 `--` 加完整源码字段名，不隐式转 kebab-case |
| names 非空 | 各项是完整拼写，如 `--output`、`-o`；第一项是主要展示名，其余为 alias |
| CliOption 标在 Boolean | 接收显式 `true`/`false` 文本，不采用 flag 语义 |
| CliFlag | 要求实际字段类型是 exact Boolean，出现时 true；negativeNames 出现时 false |
| CliArgument | 零起始 position，进入位置参数序列 |
| valueName 为空 | 根据实际类型/枚举 choices 生成展示标签 |
| defaultText 非空 | 仅在参数可缺省时用于 help，不求值、不反向解析为字段默认值 |

长名称必须以 `--` 起始并含非空名称，不能含 `=`、空白或控制字符；短名称为 `-` 加一个 ASCII 字母。反引号标识符若不能直接形成合法默认长名，使用显式 names。

`-h` 与 `--help` 保留；正负别名及不同字段的任意 alias 冲突都在 schema 构造报错。不自动生成 `--no-name`，需要通过 negativeNames 明确声明。

这些注解只描述 argv 语法与帮助信息。Serialize 描述默认 codec 的字段名称、参与性和缺失规则；CLI names、position、help 等仍由本 Cone 的注解承担。`Serialize.rename` 不自动变成选项拼写，CLI 注解也不改变字段类型或正常 constructor default。

## 5. CliSchema 与类型映射

### 5.1 默认 record schema

自动 schema 仅适用于 M46 默认 Decodable 模型可处理的 struct 或无显式 class 基类的普通 final class，沿字段/property 与 primary constructor 参数的真实映射取数据。

它按实际核心 annotationKey 读取 Serialize：先排除 ignore 字段，再取非空 rename 或源码字段名作为 wire key，并读取 required。没有注解时使用 Serialize 的普通默认参数；emitNull 只影响编码，不参与 CLI 的解码、help 或必填推导。computed property 不参与。存储 property 带自定义 accessor/委托、class 存在额外不可映射构造状态等情况，不猜测其编码含义，要求显式 schema/codec。

每项 CliEntry 保存以下逻辑信息：

- 实际成员引用及源码名称（显式 schema 可无物理成员关联）。
- `wireKey`：交给 keyed Decoder 的名称。
- role：带值 option、Boolean flag 或带 index 的 positional。
- names/negativeNames、帮助文本、metavar、defaultText。
- `required`：自动模式由下述默认字段规则确定，用于 help 和位置参数布局。
- `hasDefault`：关联 constructor 参数是否声明 default，与 required 分开保存；显式 schema 可按所选 codec 提供相应展示信息。
- `valueShape: CliValueShape`：一次或多次出现如何构造现有 codec 需要的逻辑输入。

对已经参与的字段，自动 required 精确为：

```text
serialize.required || (!parameter.hasDefault && !member.isOptionAtDeclaration)
```

isOptionAtDeclaration 的定义见 [TypeInfo 3.4](TYPE-INFO.md#34-声明处-option-与具体化后的类型)。例如原字段是 T，后来具体化为 Option<String>，没有 default 时仍必填；原字段是 T? 才获得隐式 None。CLI 使用当前完整 TypeRef 推导值形状，使用原声明事实决定字段存在性，不能用具体化后的 Option 类型替代这个判断。针对具体目标直接请求的默认 codec 遵守同一规则。

required 为 true 时，即使参数有 default 或为 Option，也必须在输入中出现；false 表示允许缺失，不代表 parser 要生成一个值。实际缺失处理由 codec 完成。hasDefault 为 false 的隐式 None 字段可缺省，但 help 不能因此声称 constructor 声明了默认值。

区分三类名称：源码字段 `outputPath`、CLI 的 `--output`/`-o`、Serialize.rename 决定的 wire key。它们不能共用一个字符串充当实体 identity。help 与 parse 使用同一份 schema，避免两套名称与必填性推导。

schema 构造验证选项名称与 wire key 唯一性、role/type 匹配、位置顺序与受支持形状；构造后只读。CliParser 接收已检查的 schema，不再次完整重放验证。TypeInfo 在编译期生成，不代表普通 CLI 库的 schema 构造就是 CTFE；上述错误是运行期配置错误。

本节只描述 M46 默认模型，不从某个 codec 对象反推出其正文。自定义字段 codec、手写 record codec 或其他 wire 约定使用显式 schema 对齐；库不分析任意 Decodable body。Serialize 新增影响默认字段的语义时，应同步本节并以同一 DTO 的 help／decode 组合用例验证；未知参数不自动获得 CLI 含义。

### 5.2 字段类型的默认含义

| 字段类型 | CLI 表面 | 提供给 Decoder 的内容 |
| --- | --- | --- |
| String / Char | 一个文本 token | Text；Char 由已有 codec 检查恰好一个 Unicode scalar |
| signed/unsigned integer | 一个十进制 token | 原 Text，读 Long/ULong 时精确解析，窄范围由 core codec 检查 |
| Float / Double | 一个十进制数 token | 原 Text，直接按目标精度解析 |
| Boolean flag | 不取后续 token | BooleanValue(true/false) |
| Boolean option | 一个 `true`/`false` token | Text，readBoolean 严格解析 |
| 普通纯 unit enum | 一个 variant 名称 | 单 key Record，其 value 是空 Record |
| Option<S> | S 的带值输入表面，重复性由 S 决定 | 直接提供 S 的逻辑节点，由 Option decoder 构造 Some；未提供时整个字段缺失 |
| Array<S>/MutableArray<S>/ArrayList<S> | 允许重复 option，或末尾 variadic positional | Sequence，顺序与 argv 一致 |

自动识别通过 TypeRef 的实际 nominal identity/完整 application，不能按 `Option`、`Array` 等短名称判断。纯 unit enum 的文本拼写默认使用源码 variant 名，区分大小写；wire key 则采用该 variant 的 Serialize.rename，空串仍使用源码名。payload enum 不纳入自动 scalar 映射。

自动值形状从标量／纯 unit enum 开始，允许 Option 透明包裹和最多一层 Array／MutableArray／ArrayList。Option<Array<S>> 归一化为 Repeated(shape(S))，Array<Option<S>> 的每个已提供元素直接按 S 归一化，Some 由各层普通 Option decoder 构造。多个 Option 层不增加逻辑节点，也不增加缺失的中间值；两层或更多序列无法由当前 argv 表面区分，schema 构造时拒绝。

flag 角色仍要求字段的实际完整类型为 exact Boolean；Option<Boolean>、Array<Boolean> 使用带值 option 的 true／false 文本，不能因展开元素类型而变成 flag。输入缺失始终保留为缺失，不为数组补空列表，也不为 Option 补逻辑 null。无法由上述规则表达的用户 wrapper 使用显式 schema／codec；不凭字段名猜嵌套 record、路径 flatten 或 discriminator。

M37 整型 enum 不等于其底层整数，也没有从任意整数隐式构造 enum 的能力。M38 CLI 首版不假定它已有某种 Decodable 数字协议；使用显式 choices/schema 和普通手写 codec。TypeInfo 与 debug 仍支持描述/展示整型 enum。

### 5.3 显式 schema 与手写 codec

`CliValueShape` 是库内的封闭语法数据，包含：

| 分支 | 归一化 |
| --- | --- |
| `Text` | 一个原始值 token |
| `Flag` | 由所选正/负 flag 名产生 BooleanValue |
| `Choice(Array<CliChoice>)` | 从明确的文本选择映射到 unit variant 的 wire key |
| `Repeated(inner)` | 收集多次出现，逐项使用 inner，然后组成 Sequence |

CliChoice 保存 `spelling` 与 `wireName`。Repeated 的 inner 为 Text 或 Choice；Flag 只用于单值 flag 角色。没有 Some 分支；Option 不属于 argv 归一化操作。重复嵌套序列、Repeated(Flag) 等不符合角色或当前输入表面的配置在 schema 构造时拒绝。没有额外 None／null 字面量，也不虚构缺失的内部值。

显式 schema 和自动 schema 都生成同一种 CliEntry，走同一归一化及 Decoder。比如 `Port(val raw: Int)` 若手写 codec 从单个数值解码，显式把该字段标成 Text 即可；不能因 TypeInfo 看到 Port 是 struct 就自动要求嵌套命令行对象。

自动 schema 的合同是匹配默认 record codec。任意手写 Decodable 可以具有完全不同的 wire 模型，库不分析其 body。调用者用显式 schema 和所选 codec 对齐；顶层仍是 keyed record，顶层 scalar codec 可由普通 Decodable adapter 从指定 key 读取。

例如 timeout 的手写 codec 可以先尝试从 Text 读取整数，在该候选失败后从同一个 decoder 读取字符串并检查是否为 auto。该字段的显式形状仍然只是 Text；重试依赖 M46 的稳定视图，不新增 CLI 的 untagged 类型或候选解析器。失败候选的用户副作用不会回滚，最终错误由这个普通 codec 决定。

## 6. argv 语法

输入已经由操作系统/shell 分成 String 数组。库不重做引号、反斜杠、空格切分、glob 或变量展开；空字符串是实际值。

### 6.1 长短选项

| 输入 | 解释 |
| --- | --- |
| `--port=9000` / `--port 9000` | 同一带值 option |
| `-p9000` / `-p=9000` / `-p 9000` | 同一短 option；attached 的一个起始 `=` 不属于值 |
| `-vq` | v 和 q 必须均为 flag，按顺序处理 |
| `-vp9000` | v 是 flag；p 是带值 option，余下 `9000` 全部作为其值 |
| `--verbose` / `--quiet` | 按 schema 分别生成 true 或显式 negative alias 的 false |
| `--verbose=false` | flag 不接受附加值；需要 CliOption<Boolean> 时显式选择其角色 |
| `--output=` / `-o=` | 显式空字符串 |
| `--` | 终止选项解释，其后的所有 token 都是位置参数 |
| `-` | 普通值/位置参数，不是 option |

短名组遇到带值 option 后停止拆分；没有剩余文本则取下一个 token。未知短名立即报错，不把整组改猜成位置参数。

需要后续值时，下一个 token 若是 `--`、长 option 或以 `-` 加 ASCII 字母起始的短 option，报告当前 option 缺值，不把另一个 option 名吃掉。`-3`、`-0.5` 等因此可作为数值；以 `--` 或 `-x` 开头的字符串值使用 `--name=value`/attached 形式消歧。

没有待取值 option 时，同样按上述规则识别 option；负数可作为 positional。options 可以出现在 positional 之间，直到 `--`。

### 6.2 重复与位置参数

同一 scalar/flag 不论使用哪个 alias，重复出现都报错，包括 true 与 negative alias 混用；不采用 last-wins。Repeated 字段保留出现顺序，合并成一个 wire key 的 Sequence；不按逗号切分单个 token。

位置参数 position 必须从 0 连续。按 schema.required 将必填单值排在可缺省的位置参数之前；可缺省既包含 constructor default，也包含声明处 Option 的隐式 None。最多最后一项为 variadic 序列。提供的 token 从左向右分配，不按后续值类型回溯。没有提供任何 token 的 variadic 字段保持缺失；必填则由 codec 报缺失，有 default 则由 default 构造默认序列，声明处 Option 且无 default 时补 None。

多余 positional、缺少 option 的值、未知 option 和非法 attached flag 值都属于输入错误。是否缺少 required 字段仍由现有 keyed.required 与目标 codec 的读取决定，不在解析器里提前执行 defaults 或构造目标。

### 6.3 help 请求

`--help`/`-h` 在 option 边界识别为 HelpRequested，立即停止后续 token 解析，且不调用 `codec.decode`、constructor 或 default。到达 help 之前已经发现的 token 语法错误照常报告。

`--path=--help` 的 `--help` 是附加值，`--` 后的 `--help` 是 positional。`--port --help` 先命中 port 缺值规则，报缺值；不会用 help 吞掉该错误。短 flag 组中的 h 按相同规则处理，到达它之前的未知 flag 仍报错。

help 无须创建 CliParser：只用 schema 即可输出。若应用已构造 parser 并把 companion 当 codec 实参传入，这次普通实参求值可能初始化 companion；库只保证 help 不执行 decode，不抹去已经发生的正常求值。

## 7. 归一化输入与 CliDecoder

### 7.1 使用已有协议需要的逻辑树

私有输入节点是 Text、BooleanValue、Record、Sequence 四种封闭分支。归一化完成后节点只读；Record 保存有序且唯一的 wire key，Sequence 保存 argv 顺序；节点或成员附带必要的 token 下标和实际拼写，用于输入错误定位。多个 decoder／视图共享这些节点，不因候选重试重新 tokenize 或深拷贝输入。

例如上面的输入中，部分归一化结果为：

```text
Record {
  "port": Text("9000"),
  "verbose": BooleanValue(true),
  "include": Sequence [Text("inc1"), Text("inc2")],
  "output": Text("")
}
```

这只是语义示意，不通过 JSON 编码/解析。数值仍是原文本，不先变成 Double；类型读取由 codec 的具体 single-value 方法决定。Option 直接复用 M46 修改后的普通 [Option decoder](../../sysroot/lib/scoop.core/src/option_decoding.scoop)，CLI 不构造带 Some tag 的 record／sequence。

没有输入的字段不放入 Record。字符串 `"null"` 是普通文本，不表示 None 或 null。这个 CLI 格式不产生单值 null，single-value 视图的 isNull 对所有合法节点返回 false，readNull 对所有节点抛 DecodingException；不伪造 Unit/default 结果。`--output=` 得到 Some("")，`--output null` 得到 Some("null")，不提供 output 才进入缺失规则。

### 7.2 容器行为

| 协议入口 | CliDecoder 的行为 |
| --- | --- |
| `path` | 保存当前 wire key/序列 index 组成的数据路径，错误使用该路径 |
| `keyed()` | 要求 Record，重复取得视图合法；keys 只列全部实际存在的 key，与读取顺序和次数无关 |
| `required(name)` | 缺失时抛 DecodingException；存在时每次返回同一逻辑节点的 child 入口，可重复读取 |
| `optional(name)` | 仅在 key 不存在时返回 None，存在时返回 Some(child)；查询不消耗节点，不解析或吞掉错误 |
| `unkeyed()` | 要求 Sequence，每次取得初始位置为 0 的独立游标；同一容器引用共享自己的游标 |
| `element()` | 成功取得 child 后推进当前游标一次；越界报错且不推进；子值解码的成功或失败不再推进父游标 |
| `singleValue()` | 对任何合法节点返回查询视图；具体 read 再检查类型，成功或失败后均可复读 |
| `isNull()`／`readNull()` | 前者对任何合法节点返回 false；后者报错；两者均不污染后续读取 |
| keyed `end()` | 本层检查，不关闭视图，不要求每个 key 都被访问；重复成功调用无额外效果 |
| unkeyed `end()` | 检查当前视图到达末尾；失败时不推进，仍可继续读取；不影响其他游标 |

decoder 是稳定输入值，不锁定容器类别，也不记录全局 selected／consumed／pending／completed 状态。先通过 singleValue().isNull() 查询 Choice 生成的 Record，再通过 keyed() 解码 enum 合法；错误的标量读取或结构候选失败也不影响后续视图。

重试一个序列元素时，保留 element() 返回的 child；重试整个序列时，重新取得 unkeyed 视图。第二次调用同一游标的 element() 表示读取下一项。固定长度 codec 的读取次数与 unkeyed.end 继续检查长度。

正常解析仍必须完成全部 argv token 的语法与归一化检查，未知 option、重复单值和缺少 option 的值由 schema/parser 拒绝；help 的提前返回沿第 6.3 节处理。进入 codec 后不再检查根 requireComplete 或“所有 child 已完成”。业务 codec 可以只读取需要的字段，键集合不是尚未消费项集合；同一稳定值可由多个 codec 组合读取。

选项名称冲突、重复出现及唯一 key 事实在前面的明确边界建立后直接复用，不在每次 required/optional 重新扫描验证整个 schema。

### 7.3 标量转换

- readString 只接受 Text，保持空值、原顺序和 Unicode；BooleanValue 不自动变成字符串。
- readBoolean 接受 BooleanValue，或 exact `true`/`false` 文本；不接受 0/1、大小写变体或空值。
- readLong/readULong 使用完整 token 的十进制数字，允许正号及前导零；Long 允许负号，ULong 拒绝负号。空值、空白、下划线、十六进制、小数和指数均拒绝。逐位运算在乘加前检查范围，Long.MIN_VALUE 正确处理，不经 Float/Double。
- readFloat/readDouble 接受完整十进制 token：可选正负号、至少一位整数、可选的小数点及至少一位小数、可选 exponent；不接收空白、hex、NaN/Infinity 拼写。直接使用目标精度转换，并检查结果有限；Float 不经 Double 中转。
- Char、窄整数范围和用户 wrapper 的进一步检查由对应 Decodable 完成，错误不会改成缺失值。

CLI 使用普通内部整数文本 helper。当前 `parseFloat`/`parseDouble` 是 [scoop.json 内部的普通 extern 声明](../../sysroot/lib/scoop.json/src/floating.scoop)，不是 core 的现成公开 API；实际转换已在 [runtime 浮点文本实现](../../runtime/src/floating_parse.c) 中完成。CLI 在自身 Cone 内以同一普通 extern ABI 绑定现有 `scoop_rt_json_parse_float`/`scoop_rt_json_parse_double`，复用其目标精度、C locale、全 token 和有限值检查，不依赖 JSON Cone，也不复制一套 C 数值解析器。

现有 runtime symbol 的 json 前缀不要求引入 JSON 语法或 JSON 对象；本里程碑无需为命名调整增加新 ABI。CLI 仍明确检查自己的数字语法，不能仅依赖底层转换接受某个前缀。

## 8. default、Option 与正常构造

这些规则与 M46 对语言规范 11.13 的修订保持一致；下表的 Option 均在字段原声明处可知：

| 字段声明 | 参数缺失时 |
| --- | --- |
| `val verbose: Boolean = false` | 缺失，Decodable 在正常 default 阶段得到 false |
| `val verbose: Boolean` | required 缺失错误，不自动补 false |
| `val output: Option<String> = None` | 缺失，default 产生 None |
| `val output: Option<String>` | 合成 decoder 直接补 None，不要求 constructor default |
| `@Serialize(required = true) val output: String? = None` | required 缺失错误，default 不作为该输入的回退 |
| `@Serialize(emitNull = true) val output: String?` | 补 None；emitNull 不改变解码规则 |
| `val include: Array<String>?` | 补 None；存在输入时得到 Some(数组) |
| 原声明 `val value: T`，T 后来取 Option<String> | 无 default 时仍为 required，不按具体化类型改判 |
| `val include: Array<String> = []` | 缺失，default 产生空数组 |
| `val include: Array<String>` | required 缺失错误，不自动补空数组 |

有输入时先用所选完整字段 codec 完成类型化；有 default 的可缺省字段只记录缺失，声明处 Option 且无 default 时记录 None。container.end 完成之后，按 constructor 参数声明顺序执行缺失项的 default，每项至多一次，允许引用此前参数，最后调用正常 constructor。ignore 字段不进入 CLI schema，按 M46 的正常补值规则构造；CLI 不补写它们。

`--port=`、整数溢出、非法 Char 或未知 enum 值都是错误，不能触发 default。`--output=` 是 Some(空字符串)，与未提供 output 不同。lexer/schema 不求值 constructor default，也不从 defaultText 生成真实输入。

手写 codec 自己定义 missing 与构造行为，CLI 不代替它反射赋值，也不分析或重跑它的 body。constructor/default/codec 抛出的普通异常原样传播，不统一吞成 DecodingException 或空参数对象。

## 9. Help 和错误

help 仅从不可变 schema 生成，包含 programName/summary、usage、位置参数、option/flag 的所有 aliases、metavar、choices、required 状态、帮助文字及显式 defaultText。

options 保持模型声明顺序，positionals 按 position 排列。usage 的可省略标记只依据 schema.required。可缺省项有 defaultText 时展示该文本；没有 defaultText 但 hasDefault 为 true 时显示“有默认值”；仅因 Option 可缺省而没有 default 的字段不虚构默认文本。required 为 true 时不展示会暗示可省略的默认提示，即使 constructor 本身有 default。所有展示均不执行表达式来猜内容。

自动 help 不实例化 T、不读取字段/getter、不调用 codec；查询 annotation 使用真实 key。help 与实际解码的 required/default/implicit None 结果作为组合合同验收，不能分别依据不同规则生成。

help 通过 TextWriter 输出纯文本，不检测终端、不加 ANSI、不给 stdout/stderr 绑定默认值。它作为完整文档以换行结束；DebugPrinter 是否换行是另一个 API 合同。help() 与 writeHelp() 产生相同文本。

错误区分：

| 来源 | 类型与信息 |
| --- | --- |
| 非法 CLI schema | IllegalArgumentException，包含冲突名称/成员及配置原因 |
| argv token 语法或重复错误 | DecodingException，包含相关 option/position、实际 token 下标与原因 |
| Decoder 容器/标量错误 | DecodingException.path 指向实际 wire 数据位置，能关联原 token 时附带来源 |
| core 窄类型或用户 codec 错误 | 保留已有 path/message 或原异常；不承诺恢复不存在的原始 alias 信息 |
| writer 异常 | 原样传播，输出可能已部分写入 |

显示名可用 schema 从 wire path 映射到主要 option 名称。无需修改 core DecodingException 布局、为所有异常建立来源链或扩展 runtime ABI。应用决定如何展示错误和返回什么退出码；库不调用 exit。

## 10. 验收

- 自动 schema 与显式 schema 都经真实独立 scoop.cli `.slib` 消费；core 的依赖图中没有 CLI。
- struct/class constructor 映射、Serialize 的 rename／ignore／required／emitNull、同名不同 annotation、alias/re-export、M37 type-use target 均使用真实描述，不靠名称猜身份；单独 rename 不改变必填性，emitNull 不影响 CLI 输入。
- 覆盖完整 argv、空参数列表、长短选项、短名组、负数、`--`、单 `-`、Unicode/空字符串、重复 alias、未知参数和所有缺值规则。
- Option 有／无 default、Some(空字符串)、Some("null")、Option<Array<S>>／Array<Option<S>>、重复数组顺序、可选 unit enum 的 null 查询后 keyed 解码、variant 源码名／wire 名差别、显式 Port codec 均有运行 fixture。没有输入时不伪造空数组；不支持的多层序列或 flag 包装准确报配置错误。
- 原声明 T 与 T?、透明 alias、针对具体目标直接请求缺省 codec、泛型 codec 实例化与跨 Cone 消费均核对同一 DTO 的 schema.required／help 和实际解码；TypeRef 具体化不改变原成员事实。
- 缺省位置参数、required 覆盖已有 default、required 可空字段和 ignore 补值同时覆盖 schema 布局、help 文本与 decode；隐式 None 不误报 constructor default。
- 默认值计数、参数依赖与 constructor 计数验证：成功时顺序正确，非法输入不回退 default；help 不 decode、不构造参数对象。
- 单值／字段复读、isNull 恒 false、readNull 失败后复读、错误容器后换视图、独立 unkeyed 游标、同一 child 上候选重试及 unkeyed 剩余元素覆盖新协议；整数／auto 的手写候选验证最终错误与 token 来源。保留整型边界、浮点目标精度、非法 Char 与错误 path，不再断言旧的一次性消费错误。
- help 用 StringWriter 做文本断言，并由另一个普通 writer 验证输出独立于 print/println。schema 配置错误是运行期库错误，不伪装为编译期 negative fixture。
- 应用模型及其 annotation 在 provider Cone，消费方只见 provider/core/CLI 产物；必要泛型 body 和已绑定引用再次发布后仍一致。

子命令、环境变量、配置文件、shell completion、自动嵌套 record/payload enum 展平不属于首版；这些扩展不能成为 M38 当前单命令闭环的交付前提。
