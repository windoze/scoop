# M38：独立 CLI Cone 与 Decoder

本文件从属于 [总体设计](DESIGN.md)。类型描述由 [TypeInfo](TYPE-INFO.md) 提供，help 使用 [TextWriter](DEBUG.md)；参数对象继续由现有 `Decodable<T>` 构造。

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
    val output: Option<String> = None,
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
| defaultText 非空 | 仅用于 help，不求值、不反向解析为字段默认值 |

长名称必须以 `--` 起始并含非空名称，不能含 `=`、空白或控制字符；短名称为 `-` 加一个 ASCII 字母。反引号标识符若不能直接形成合法默认长名，使用显式 names。

`-h` 与 `--help` 保留；正负别名及不同字段的任意 alias 冲突都在 schema 构造报错。不自动生成 `--no-name`，需要通过 negativeNames 明确声明。

## 5. CliSchema 与类型映射

### 5.1 默认 record schema

自动 schema 仅适用于现有默认 Decodable 模型可处理的 struct 或无显式 class 基类的普通 final class，沿字段/property 与 primary constructor 参数的真实映射取数据。

它排除 Transient，读取 SerialName 作为 codec 的 wire key；computed property 不参与。存储 property 带自定义 accessor/委托、class 存在额外不可映射构造状态等情况，不猜测其编码含义，要求显式 schema/codec。

每项 CliEntry 保存以下逻辑信息：

- 实际成员引用及源码名称（显式 schema 可无物理成员关联）。
- `wireKey`：交给 keyed Decoder 的名称。
- role：带值 option、Boolean flag 或带 index 的 positional。
- names/negativeNames、帮助文本、metavar、defaultText。
- `required`：自动模式由关联 constructor 参数是否有 default 决定，用于 help 和位置参数布局。
- `valueShape: CliValueShape`：一次或多次出现如何构造现有 codec 需要的逻辑输入。

区分三类名称：源码字段 `outputPath`、CLI 的 `--output`/`-o`、SerialName 决定的 wire key。它们不能共用一个字符串充当实体 identity。help 与 parse 使用同一份 schema，避免两套名称推导。

schema 构造验证名称唯一性、role/type 匹配、位置顺序与受支持形状；构造后只读。CliParser 接收已检查的 schema，不再次完整重放验证。TypeInfo 在编译期生成，不代表普通 CLI 库的 schema 构造就是 CTFE；上述错误是运行期配置错误。

### 5.2 字段类型的默认含义

| 字段类型 | CLI 表面 | 提供给 Decoder 的内容 |
| --- | --- | --- |
| String / Char | 一个文本 token | Text；Char 由已有 codec 检查恰好一个 Unicode scalar |
| signed/unsigned integer | 一个十进制 token | 原 Text，读 Long/ULong 时精确解析，窄范围由 core codec 检查 |
| Float / Double | 一个十进制数 token | 原 Text，直接按目标精度解析 |
| Boolean flag | 不取后续 token | BooleanValue(true/false) |
| Boolean option | 一个 `true`/`false` token | Text，readBoolean 严格解析 |
| 普通纯 unit enum | 一个 variant 名称 | 单 key Record，其 value 是空 Record |
| Option<S> | S 的同一表面 | 已提供值包装成 `{"Some": [value]}`；未提供时整个字段缺失 |
| Array<S>/MutableArray<S>/ArrayList<S> | 允许重复 option，或末尾 variadic positional | Sequence，顺序与 argv 一致 |

自动识别通过 TypeRef 的实际 nominal identity/完整 application，不能按 `Option`、`Array` 等短名称判断。纯 unit enum 的文本拼写默认使用源码 variant 名，区分大小写；wire key 则采用该 variant 的 SerialName。payload enum 不纳入自动 scalar 映射。

自动支持标量/纯 unit enum 及其一层 Option 或一层上述序列包装。更复杂组合由显式 CliValueShape 表达，不凭字段名猜嵌套 record、路径 flatten 或 discriminator。

M37 整型 enum 不等于其底层整数，也没有从任意整数隐式构造 enum 的能力。M38 CLI 首版不假定它已有某种 Decodable 数字协议；使用显式 choices/schema 和普通手写 codec。TypeInfo 与 debug 仍支持描述/展示整型 enum。

### 5.3 显式 schema 与手写 codec

`CliValueShape` 是库内的封闭语法数据，包含：

| 分支 | 归一化 |
| --- | --- |
| `Text` | 一个原始值 token |
| `Flag` | 由所选正/负 flag 名产生 BooleanValue |
| `Choice(Array<CliChoice>)` | 从明确的文本选择映射到 unit variant 的 wire key |
| `Some(inner)` | 包装为现有 Option.Some 的 record/sequence 形状 |
| `Repeated(inner)` | 收集多次出现，逐项使用 inner，然后组成 Sequence |

CliChoice 保存 `spelling` 与 `wireName`。同一个字段最多有一个 Repeated 层；其余 Some 包装可以嵌套，表示输入存在时的 Some 路径。没有额外 None 字面量，也不虚构缺失的内部值；重复嵌套序列等无法由当前 argv 表面表达的形状在 schema 构造时拒绝。

显式 schema 和自动 schema 都生成同一种 CliEntry，走同一归一化及 Decoder。比如 `Port(val raw: Int)` 若手写 codec 从单个数值解码，显式把该字段标成 Text 即可；不能因 TypeInfo 看到 Port 是 struct 就自动要求嵌套命令行对象。

自动 schema 的合同是匹配默认 record codec。任意手写 Decodable 可以具有完全不同的 wire 模型，库不分析其 body。调用者用显式 schema 和所选 codec 对齐；顶层仍是 keyed record，顶层 scalar codec 可由普通 Decodable adapter 从指定 key 读取。

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

位置参数 position 必须从 0 连续。必填单值在有 default 的位置参数之前；最多最后一项为 variadic 序列。提供的 token 从左向右分配，不按后续值类型回溯。没有提供任何 token 的 variadic 字段保持缺失；必填则由 codec 报缺失，有 default 则由 default 构造空或其他默认序列。

多余 positional、缺少 option 的值、未知 option 和非法 attached flag 值都属于输入错误。是否缺少 required 字段仍由现有 keyed.required 与目标 codec 的读取决定，不在解析器里提前执行 defaults 或构造目标。

### 6.3 help 请求

`--help`/`-h` 在 option 边界识别为 HelpRequested，立即停止后续 token 解析，且不调用 `codec.decode`、constructor 或 default。到达 help 之前已经发现的 token 语法错误照常报告。

`--path=--help` 的 `--help` 是附加值，`--` 后的 `--help` 是 positional。`--port --help` 先命中 port 缺值规则，报缺值；不会用 help 吞掉该错误。短 flag 组中的 h 按相同规则处理，到达它之前的未知 flag 仍报错。

help 无须创建 CliParser：只用 schema 即可输出。若应用已构造 parser 并把 companion 当 codec 实参传入，这次普通实参求值可能初始化 companion；库只保证 help 不执行 decode，不抹去已经发生的正常求值。

## 7. 归一化输入与 CliDecoder

### 7.1 使用已有协议需要的逻辑树

私有输入节点是 Text、BooleanValue、Record、Sequence 四种封闭分支。Record 保存有序且唯一的 wire key，Sequence 保存 argv 顺序；节点或成员附带必要的 token 下标和实际拼写，用于输入错误定位。

例如上面的输入中，部分归一化结果为：

```text
Record {
  "port": Text("9000"),
  "verbose": BooleanValue(true),
  "include": Sequence [Text("inc1"), Text("inc2")],
  "output": Record { "Some": Sequence [Text("")] }
}
```

这只是语义示意，不通过 JSON 编码/解析。数值仍是原文本，不先变成 Double；类型读取由 codec 的具体 single-value 方法决定。Option 的 Some/None 形状复用 [现有 Option decoder](../../sysroot/lib/scoop.core/src/option_decoding.scoop)。

没有输入的字段不放入 Record。字符串 `"null"` 是普通文本，不表示 None 或 null。这个 CLI 格式不产生单值 null，readNull 对其输入报告形状错误；它仍是完整实现的 Decoder 方法，不伪造一个 Unit/default 结果。

### 7.2 容器行为

| 协议入口 | CliDecoder 的行为 |
| --- | --- |
| `path` | 保存当前 wire key/序列 index 组成的数据路径，错误使用该路径 |
| `keyed()` | 要求 Record，并选定该容器；keys 只列实际输入存在的 key |
| `required(name)` | 缺失时抛 DecodingException，存在则返回该节点的 child decoder |
| `optional(name)` | 仅在 key 不存在时返回 None，存在时返回 Some(child)，不解析或吞掉错误 |
| `unkeyed()` | 要求 Sequence，维护当前游标与尚未结束的 child |
| `element()` | 返回下一项的 child；越界或前一 child 未完成时报错 |
| `singleValue()` | 选择单值读取，之后只能完成一次读取 |
| `end()` | 检查已打开 child 已完成；unkeyed 还要求已消费全部元素 |

一个 decoder 只能选择一种容器并完成一次。最外层 `codec.decode` 返回后检查根 decoder 完成，参照 scoop.json 的 `requireComplete`。keyed.end 沿用现有协议的未知 key/未打开成员规则，不新增与 core 不同的“每个 key 必须读一遍”合同；argv 中的未知 option 已由 schema/parser 拒绝。

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

这些规则与语言规范 11.13 保持一致：

| 字段声明 | 参数缺失时 |
| --- | --- |
| `val verbose: Boolean = false` | 缺失，Decodable 在正常 default 阶段得到 false |
| `val verbose: Boolean` | required 缺失错误，不自动补 false |
| `val output: Option<String> = None` | 缺失，default 产生 None |
| `val output: Option<String>` | required 缺失错误，不因为类型是 Option 就可省略 |
| `val include: Array<String> = []` | 缺失，default 产生空数组 |
| `val include: Array<String>` | required 缺失错误，不自动补空数组 |

有输入时先用所选字段 codec 完成类型化；有 default 的缺失字段只记录缺失。container.end 完成之后，按 constructor 参数声明顺序执行缺失项的 default，每项至多一次，允许引用此前参数，最后调用正常 constructor。

`--port=`、整数溢出、非法 Char 或未知 enum 值都是错误，不能触发 default。`--output=` 是 Some(空字符串)，与未提供 output 不同。lexer/schema 不求值 constructor default，也不从 defaultText 生成真实输入。

手写 codec 自己定义 missing 与构造行为，CLI 不代替它反射赋值，也不分析或重跑它的 body。constructor/default/codec 抛出的普通异常原样传播，不统一吞成 DecodingException 或空参数对象。

## 9. Help 和错误

help 仅从不可变 schema 生成，包含 programName/summary、usage、位置参数、option/flag 的所有 aliases、metavar、choices、required 状态、帮助文字及显式 defaultText。

options 保持模型声明顺序，positionals 按 position 排列。带 default 但没有 defaultText 时显示“有默认值”，不执行表达式来猜显示内容。自动 help 不实例化 T、不读取字段/getter、不调用 codec；查询 annotation 使用真实 key。

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
- struct/class constructor 映射、SerialName、Transient、同名不同 annotation、alias/re-export、M37 type-use target 均使用真实描述，不靠名称猜身份。
- 覆盖完整 argv、空参数列表、长短选项、短名组、负数、`--`、单 `-`、Unicode/空字符串、重复 alias、未知参数和所有缺值规则。
- Option 与 default 分层、Some(空值)、重复数组顺序、unit enum 的源码名/wire 名差别、显式 Port codec 均有运行 fixture。
- 默认值计数、参数依赖与 constructor 计数验证：成功时顺序正确，非法输入不回退 default；help 不 decode、不构造参数对象。
- 单值/容器双重读取、未完成 child、unkeyed 剩余元素、整型边界、浮点目标精度、非法 Char 与错误 path 覆盖协议边界。
- help 用 StringWriter 做文本断言，并由另一个普通 writer 验证输出独立于 print/println。schema 配置错误是运行期库错误，不伪装为编译期 negative fixture。
- 应用模型及其 annotation 在 provider Cone，消费方只见 provider/core/CLI 产物；必要泛型 body 和已绑定引用再次发布后仍一致。

子命令、环境变量、配置文件、shell completion、自动嵌套 record/payload enum 展平不属于首版；这些扩展不能成为 M38 当前单命令闭环的交付前提。
