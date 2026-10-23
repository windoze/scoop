# Scoop Regex 设计与实施路线

初次调研：2026-10-18。更新：2026-10-22。状态：PCRE2-8 方案已实现，三目标相关验证通过。公开合同见 [regex 规范](../specs/SCOOP-REGEX-SPEC.md)，实际完成与验证范围见[实施记录](PROGRESS.md)。本文记录选型、API 设计和实施顺序，不修改现有语言、runtime 或 pipeline 职责。

## 1. 已确定的方案

采用 **PCRE2 的 8-bit 库，初始版本固定为 10.49**，以独立的普通 `scoop.regex` Cone 提供公开 API。底层通过薄 C 适配使用 PCRE2 原生模式语法与匹配语义；Scoop 负责字符串借用、索引转换、结果对象、错误和资源生命周期。首版只使用这一个引擎，不自研正则引擎，不建立可切换后端框架。

这个选择同时满足当前三个目标的 C 集成、UTF-8 原生输入、Unicode 属性和丰富的高级正则功能。PCRE2 10.49 使用 Unicode 17.0.0 数据，与 std 当前使用的 Unicode 17.0 基线一致；10.49 也包含 10.48 之后的上游修复。[发布记录][pcre-release]、[变更记录][pcre-changes]

PCRE2 以独立的 C 静态库交付，不包装进现有 Rust 文本后端，也不新增 Rust `staticlib`。M61 已解决 Rust 文本后端的构建与分发，但其中的 ICU4X 分段、属性和大小写组件不是 regex 引擎；regex 的语义与数据由 PCRE2 自己提供，不借用这些组件补齐或改写匹配结果。

**上游 bug 随上游处理。** `\X` 等功能中的上游问题属于所选版本的限制，不在 Scoop 中打补丁、改写模式、匹配后修正或切换另一引擎绕过，也不把修复它们作为 Scoop 的交付前置条件。测试仅覆盖 Scoop 自有适配和 API 行为，边界见第 4.3、8.2 节。

## 2. Scoop 已有约束

| 已有合同 | 对 Regex 的影响 |
| --- | --- |
| String 是合法 UTF-8、允许 U+0000、无隐含 NUL；`length/get/slice` 按 Unicode 标量计数，计数使用 Long | 使用 PCRE2-8 的指针加长度接口；公开位置不能直接返回 PCRE2 的字节偏移，也不能改用 UTF-16 code unit 或字素簇编号 |
| String 不执行 normalization；组合字符分别计数 | Regex 默认不做 normalization；匹配位置必须指回实际输入 |
| core 已有 `String.withUtf8Bytes` | 单次 C 调用可以同步借用、保活并固定输入，不必先复制整串或增加 runtime 专属入口 |
| `scoop.std` 只包含普通 Scoop 源码，不声明 extern 或携带 native source | 新增独立的普通 `scoop.regex` Cone，依赖 core 和 std；std 不反向依赖 regex |
| 普通 C extern 默认 NativeSafe；GCLeaf 要求预期短时完成 | 编译、匹配、替换使用 NativeSafe，不因正则“通常很快”标为 GCLeaf |
| 三个现有目标是 Darwin/AArch64、Linux/amd64 GNU、Linux/amd64 musl | 同一版本的 PCRE2 源码分别构建目标资源；不混用 GNU 和 musl archive |
| M52 的预编译 sysroot、目标 native 资源与发行流程已完成 | 复用现有准备和交付流程，普通应用消费目标 PCRE2 archive，不重新构建上游库 |
| M61 的共享 Rust 文本后端已完成 | 保持其现有职责；PCRE2 是独立 C 依赖，不引入第二个 Rust staticlib 或跨引擎 Unicode 适配 |
| 有 Closeable/use 和受约束的 GC-free release block | native owner 采用现有生命周期机制，不另建 finalizer 服务 |

依据：[语言规范 11.4、11.18、13.4、13.11](../specs/SCOOP-SPEC.md)、[运行时规范 2.4、3.4、3.5、3.8](../specs/SCOOP-RUNTIME-SPEC.md)、[实现规范 1、2.15](../specs/SCOOP-IMPL-SPEC.md)、[标准基础库规范 1、4](../specs/SCOOP-STDLIB-SPEC.md)、[库 API 风格](../specs/SCOOP-API-STYLE.md)。

## 3. 选型依据

以下比较说明已确定方案的取舍，不构成多后端实施计划。

| 方案 | Unicode 能力 | 高级语法与执行方式 | Scoop 集成与判断 |
| --- | --- | --- | --- |
| **PCRE2 10.49** | UTF-8/16/32；类别、Script/Script_Extensions、若干二元属性；`\X`、Unicode 大小写等价；不是完整 Unicode 文本算法集合 | 回溯；JIT；lookaround、反向引用、递归/子模式调用、条件、原子组、占有量词、分支重置、字符集合运算 | C API；只需 8-bit 库；不依赖 ICU。最符合此次功能与现有目标的组合 |
| **ICU4C 78.3** | 广泛的 Unicode 属性；`\X`；字符串字面量的 full case folding；可选 UAX #29 单词边界 | 回溯；lookaround、反向引用、原子组、占有量词、UnicodeSet 集合运算；不提供 PCRE2 那套递归子模式能力 | 有 C API，实现仍是 C++，需要交付 ICU 数据与组件；与 M61 的 ICU4X 不同，也不是 PCRE2 的透明替换。本次不采用 |
| **Rust regex 1.13.1 / regex-automata** | Unicode 属性丰富、默认 Unicode 类和简单大小写折叠；不提供 `\X` 或 full case folding | 单次搜索最坏 O(m×n)，m 为模式规模、n 为输入规模；不支持 lookaround、反向引用 | M61 已有 Rust 构建与 C ABI 通道，但无法满足本次高级语法范围。本次不采用 |
| **fancy-regex 0.19.2** | 基于 Rust regex 的 Unicode 能力；不支持 `\X` | 支持 lookaround、反向引用、可变长 lookbehind、递归子程序等；高级功能仍可能指数级回溯 | 能力比初次调研描述更完整，但语法不是 PCRE2 的直接替代；采用它需要改变功能合同。本次不采用 |

PCRE2 本体为 `BSD-3-Clause WITH PCRE2-exception`，所带 JIT 为 BSD-2-Clause；可按普通第三方依赖交付并保留许可证。[许可证][pcre-license]

Rust `fancy-regex` 将部分模式交给 Rust regex，复杂部分使用回溯 VM；使用 Rust 不等于具有线性时间保证。其可变长 lookbehind 和递归子程序有自身限制，不能据此承诺 PCRE2 语法兼容。[Rust regex][rust-regex]、[fancy-regex API][fancy]、[fancy-regex 解析器][fancy-parser]

不自行实现引擎、Unicode 算法或 replacement 模板解释器。薄适配层只承接现有 Scoop String、FFI、结果与资源合同。

这里没有做跨引擎性能或最终可执行文件体积基准。JIT 是 PCRE2 的优化手段，不能据此断言它在所有工作负载中都更快。最终体积也应比较实际链接后的程序，而非整个安装包或静态库文件大小。

## 4. Unicode 应承诺到哪里

### 4.1 默认语义

| 项目 | 设计 |
| --- | --- |
| 编码 | 固定合法 UTF-8，使用 PCRE2-8；公开 Regex 不接受任意二进制输入 |
| 默认编译选项 | 同时设置 `PCRE2_UTF`、`PCRE2_UCP`、`PCRE2_NEVER_BACKSLASH_C` |
| `.` 与量词 | 以 Unicode 标量为基本单位；`.` 默认排除所选换行序列，DotMatchesAll 改变换行行为 |
| `\X` | 使用所选 PCRE2 版本原生的扩展字素簇实现，包括该版本已知限制 |
| `\d` | 默认使用 Unicode Nd；匹配 ASCII 数字时显式使用 `[0-9]` 或局部 ASCII 选项 |
| `\w` / `\b` | 使用 PCRE2 UCP 的原生定义，不额外换成 ICU 或自行定义的单词规则 |
| 大小写 | IgnoreCase 使用 PCRE2 的 Unicode 大小写等价规则，不依赖系统 locale |
| normalization | 不自动处理 NFC/NFD/NFKC/NFKD，也不提供误导性的 CanonicalEquivalence 开关 |
| 换行 | 默认显式配置 `PCRE2_NEWLINE_ANY` 和 `PCRE2_BSR_UNICODE`，让锚点、`.`、`\R` 有明确的 Unicode 换行基线；模式内合法指令仍遵守 PCRE2 规则 |

UTF 和 UCP 是不同设置：仅开启 UTF 并不会让 `\d`、`\s`、`\w` 自动采用 Unicode 属性。`\C` 则可以匹配 UTF-8 编码中的单个字节，产生无法表示为 Scoop String/Char 的分组边界；使用 PCRE2 已有的禁止选项是维持 String 合同所需的正常边界检查。[PCRE2 Unicode][pcre-unicode]

### 4.2 属性支持、大小写折叠与分词要分开说明

PCRE2 支持 `\p{L}`、`\p{Nd}`、`\p{sc=Han}`、`\p{scx=Hira}`、`\p{Extended_Pictographic}` 等。`sc` 和 `scx` 不同，例如长音符 U+30FC 的 Script 不是 Hiragana，但 Script_Extensions 包含 Hiragana。PCRE2 不支持所有 Unicode 属性查询，包括 `\p{Age=1.1}` 和 `\p{Block=Basic_Latin}`；不能把 `\p` 宣传为完整 UCD 查询接口。[模式语法][pcre-pattern]

在 10.49 的 UCP 模式中，`\w` 是 `L ∪ N ∪ Mn ∪ Pc`。它包含 non-spacing mark，但不因此包含所有 Mark 或 Join_Control。ICU/Rust regex 的 `\w` 集合与之不同；这属于引擎语义选择。

PCRE2 IgnoreCase 能处理非 ASCII 大小写和 `Σ/σ/ς` 这样的单码点等价类，但不提供 `ß → ss` 这类多码点 full case folding。`straße` 忽略大小写可以匹配 `STRAẞE`，不能匹配 `STRASSE`。ICU 的字符串字面量可以匹配后一种情况；其字符集合仍一次匹配一个码点，`fu(s)(s)ball` 也不能简单等同于 `fussball` 的折叠匹配。[PCRE2 Unicode][pcre-unicode]、[ICU 大小写规则][icu-regex]

PCRE2 `\b` 表示 `\w` 与非 `\w` 的边界。ICU 默认也使用传统单词边界，但可用 `UREGEX_UWORD` / `(?w)` 启用 UAX #29。例如 `can't` 的边界位置，传统规则为 0、3、4、5，ICU UWORD 为 0、5。UAX #29 的默认单词边界本身也不等于面向所有语言的词典分词。

**不通过“先规范化/折叠整个模式与输入再匹配”补齐这些语义。** 这种做法会影响模式结构、捕获与原始字符串的位置；规范化字符串也不能一般性地实现 canonical-equivalence regex。PCRE2 没有该能力，ICU 的 `UREGEX_CANON_EQ` 同样尚未实现。已有文本 API 的 normalization 和大小写转换保持显式调用，结果索引属于调用者实际传给 Regex 的字符串。[UTS #18][uts18]

### 4.3 上游职责与测试边界

第 4.1、4.2 节用于说明所选引擎的语义，不承诺完整 Unicode 符合性。上游 Unicode 算法、数据和已知问题由上游维护；不新增 PCRE2、ICU 或 Rust 库 Unicode 兼容性／完备性的正例或负例测试，不运行官方 Unicode 语料、跨引擎差分或上游符合性测试作为本库验证。

原调研中的 Unicode 对照实验不纳入本设计的实施与验收计划，不继续扩展或重跑。依赖升级依据上游资料确定变化，只验证受影响的 Scoop 适配，不维护一套上游算法测试。

非 ASCII、补充平面字符或 NUL 可以作为 Scoop 索引转换、显式长度和结果复制的输入；这些用例验证适配层，不扩展为引擎 Unicode 算法的正确性矩阵。

## 5. 高级功能范围

| 功能 | 首版范围 |
| --- | --- |
| 正/负 lookahead、lookbehind | 原生支持；lookbehind 不是任意无限长。10.43 起支持有界变长分支，默认上界 255 个字符；固定长分支另有规则 |
| 命名捕获、数字/命名反向引用 | 支持；用 `Option` 区分未参与和匹配空串 |
| 原子组 `(?>...)`、占有量词 `*+`/`++`/`?+` | 支持 |
| 递归和子模式调用 `(?R)`、`(?&name)` | 支持；可用于递归文本结构，不据此承诺它适合替代正式语言 parser |
| 条件、分支重置 `(?\|...)` | 支持原生语法 |
| `\K`、`\G`、回溯控制动词 | 保留原生语义；必须在锚定匹配和迭代文档中说明相关边界 |
| 字符集合交/差/对称差 | 10.45 起可用 Perl 扩展字符类 `(?[...])`；首版直接采用这一语法 |
| 重复命名组 | 默认仍拒绝；显式 DuplicateNames / `(?J)` 开启，并定义公开查询规则 |
| partial matching、重叠搜索、callout、自定义字符表 | 首版不增加相应 Scoop 接口；它们需要额外的结果、推进或 callback 合同，等实际需求再设计 |

例如 `(?<=colou?r)ful` 能处理 `colorful` 与 `colourful`；`(?<=a*)b` 会在模式编译时失败。`\X` 和 `\R` 也不能放进 PCRE2 lookbehind，不能把“有界变长 lookbehind”解释为任意子模式都可以使用。[模式语法][pcre-pattern]

## 6. 上层 API 设计

以下是公开声明的形状，不是可以直接编译的完整实现。命名沿用 Scoop/Kotlin 的常用词汇，`Option<T>` 以 `T?` 表示，集合来自 std。

### 6.1 编译对象与匹配入口

```scoop
public enum RegexOption {
    IgnoreCase,
    Multiline,
    DotMatchesAll,
    Comments,
    AsciiClasses,
    DuplicateNames
}

public class Regex public constructor(
    pattern: String,
    options: Set<RegexOption> = emptySet()
) : Closeable {
    public val pattern: String
    public val options: Set<RegexOption>
    public val isClosed: Boolean

    public fun matches(input: String): Boolean
    public fun containsMatchIn(input: String): Boolean
    public fun matchEntire(input: String): MatchResult?
    public fun matchAt(input: String, index: Long): MatchResult?
    public fun find(input: String, startIndex: Long = 0L): MatchResult?
    public fun findAll(input: String, startIndex: Long = 0L): Sequence<MatchResult>

    public override fun close()

    public companion object {
        public fun escape(literal: String): String
    }
}
```

- `Regex(pattern)` 编译一次，可多次使用；options 在构造时取得独立快照。`String.toRegex()` 是等价便利入口，`toRegexOrNone()` 只转换模式语法错误。
- `matches`/`matchEntire` 从输入起点执行模式并要求在输入末尾成功；前者适合只要 Boolean 的场景，不必生成全部匹配对象。
- `matchAt` 从指定标量位置开始执行，不向后寻找其他起点；`find` 才是搜索。必须把完整 input 和起点交给引擎，不能先 slice 输入，否则会改变锚点、lookbehind 和边界语义。
- startIndex/index 的合法范围为 `0..input.length`，包含结尾。非法位置抛 IndexOutOfBoundsException。
- AsciiClasses 仅切换 `\d`/`\s`/`\w`、关联 `\b` 和 POSIX 类的约定；不关闭 UTF-8，不让 `.` 按字节工作，不把 IgnoreCase 自动改成 ASCII。可映射 PCRE2 已有 ASCII 类选项。
- 模式中的 `(?i)`、局部选项等继续有效；options 属性描述构造选项，不伪装成模式每个位置的有效状态。

`\K` 会重设返回的 0 组起点。例如 `foo\Kbar` 在 `foobar` 上可以满足上述完整执行语义，但报告的 0 组是 `bar`。`matchEntire`/`matchAt` 应文档化这个原生行为，不在结果阶段改写，也不简单通过检查 0 组起点来否定引擎的锚定成功。

### 6.2 结果与位置

```scoop
public struct MatchRange(
    val start: Long,
    val endExclusive: Long
)

public struct MatchGroup(
    val value: String,
    val range: MatchRange
)

public class MatchResult {
    public val value: String
    public val range: MatchRange
    public val groups: MatchGroups
}

public class MatchGroups {
    public val size: Long
    public operator fun get(index: Long): MatchGroup?
    public operator fun get(name: String): MatchGroup?
    public fun getAll(name: String): List<MatchGroup?>
}
```

`MatchRange` 属于 regex 库，使用半开标量区间，不使用闭区间 LongRange。这样空匹配自然表示成 `[i, i)`，且可直接用于 `input.slice(start, endExclusive)`。例如在 `A雪😀B` 中匹配 `😀`，公开范围是 `[2, 3)`，PCRE2 的内部字节范围则是 `[4, 8)`。

groups 包含 0 组，size 包含 0 组在内；0 组与 MatchResult 的 value/range 一致。合法但未参与的捕获返回 None；参与且长度为零的捕获返回 `Some(MatchGroup("", ...))`。索引越界抛 IndexOutOfBoundsException，未知名字抛 IllegalArgumentException，不把调用错误伪装成未参与。

开启重复命名组时，`groups[name]` 返回按组号排序的第一个已参与组；全部未参与则返回 None。`getAll(name)` 按组号返回该名字对应的所有捕获槽，保留未参与的 None。分支重置共享同一组号时仍只有一个槽。量词中重复参与的同一组沿用 PCRE2 最终保留的捕获，不另做 capture history。

结果是独立、不可变的 Scoop 数据；C 的下一次匹配不会覆盖既有结果，关闭 Regex 后已有结果仍可使用。首版直接复制实际捕获字符串并保存转换后的范围，避免让公开 MatchResult 持有 native match-data。组名到组号的元数据在编译成功后复制成不可变 Scoop 数据，供结果共享；关闭 Regex 后的组名查询不访问已释放的 PCRE2 code。嵌套或重复的大捕获确实可能产生较多复制，后续依据实际负载再优化，不先引入 StringView 类型体系。

lookaround 中的捕获可以位于 0 组范围之外，所以实现不能要求所有组都包含在 MatchResult.range 内，或假设各组端点按组号单调排列。

### 6.3 惰性迭代与空匹配

`findAll` 返回可重复取得独立 iterator 的 Sequence。参数在入口校验，匹配从首次 next 开始；首次 None 后始终返回 None。每次结果均为快照。

使用 **`pcre2_next_match`** 推进全局匹配。这个 API 自 10.47 提供，负责返回下一次的字节起点与 match options，包含空匹配的处理，不必自行维护一套容易出错的推进算法。[迭代 API][pcre-next-match]

它在一次空匹配后用 `PCRE2_NOTEMPTY_ATSTART` 允许从同一位置找到非空结果，然后继续前进。例如 `.*?` 在 `ab` 上依次产生 `""`、`"a"`、`""`、`"b"`、`""`。不能无条件“空匹配后前进一个字符”，否则会丢掉这些合法非空匹配。CRLF 的推进也遵守选定 PCRE2 newline 模式；采用 ANY 时，空模式遍历 `\r\n` 不会在两者中间额外报告一个空匹配。

保持默认禁止 lookaround 内的 `\K`，不启用 `PCRE2_EXTRA_ALLOW_LOOKAROUND_BSK`，避免引入可能倒置的原生匹配范围；这是选择上游已有的默认语义。

iterator 在 next 之间只保存 Regex、输入 String、普通位置和选项等 Scoop 状态。单次 next 内创建并释放 native match-data，调用 `pcre2_next_match` 后只带出下一次的位置/flags；不把 String 指针或 match-data 留到下一次 next。这样提前停止迭代不需要新增可关闭 Sequence 协议。消费期间须保持 Regex 打开；close 后新的匹配操作抛 IllegalStateException。

### 6.4 替换、拆分与转义

```scoop
public fun Regex.replace(input: String, replacement: String): String
public fun Regex.replace(input: String, transform: (MatchResult) -> String): String
public fun Regex.replaceFirst(input: String, replacement: String): String
public fun Regex.replaceFirst(input: String, transform: (MatchResult) -> String): String

public fun Regex.replaceTemplate(input: String, replacement: String): String
public fun Regex.replaceFirstTemplate(input: String, replacement: String): String

public fun Regex.split(input: String, limit: Long = 0L): List<String>
public fun Regex.splitToSequence(input: String, limit: Long = 0L): Sequence<String>
```

**replace 的 String 参数默认是字面文本，捕获引用显式使用 replaceTemplate**。这是相对 Kotlin 的刻意差异；与 Scoop 已有 String.replace 的文本参数含义一致，也不要求调用者为了插入普通 `$` 和反斜杠去记第二套转义。

replaceTemplate 使用 PCRE2 默认 replacement 语法，例如 `$1`、`${name}`、`$$`，不启用 extended replacement 模式。直接复用 `pcre2_substitute`，不自研模板解释器。未参与的已知组按空串展开，未知组引用报告模板错误；对应选择 `PCRE2_SUBSTITUTE_UNSET_EMPTY`，不启用 UNKNOWN_UNSET。字面替换使用 `PCRE2_SUBSTITUTE_LITERAL`。全局替换与 split 的匹配次序沿用相同的原生全局规则。

模板解释的时点也沿用上游：没有匹配时返回原文本，不额外预校验未被使用的捕获引用。例如模式 `a` 对输入 `b` 使用 `${unknown}` 返回 `b`；实际替换 `a` 时才报告未知组。需要预先校验模板的独立入口以后按实际需求设计，不为了让两种情况都报错再写一套模板 parser。

回调替换在 C 匹配返回、结果复制完成并结束输入指针借用之后执行普通 Scoop lambda，不通过 PCRE2 callout 反向进入 Scoop。回调可以分配或抛异常，但不挂起；异常传播且不返回部分输出，已经发生的回调副作用不回滚。

split 只返回分段，不将分隔表达式的捕获组插入结果。limit 为 0 表示不限，正数限制段数、最后一段保留余下内容，负数抛 IllegalArgumentException；保留首尾和连续分隔产生的空段。空输入统一返回单个空串，与现有 text API 一致。Regex 可以匹配空分隔，例如空模式 split `ab` 得到 `["", "a", "b", ""]`，不继承普通 String.split 对空 String delimiter 的禁止规则。

Regex.escape 只处理 pattern 的字面量引用，必须正确处理输入中的 `\E` 等终止引用字符；不提供含义混杂的通用 escape。普通 Scoop 字符串不插值，模板 `${name}` 可以直接写在普通字符串中；模式推荐使用 `"""..."""` 避免重复反斜杠。首版不增加 regex literal、编译期 pattern 类型检查或捕获组到 tuple 的编译器合成。

示意调用：

```scoop
import scoop.regex.*

Regex("""(?<name>\p{L}+)=(?<value>\d+)""").use { regex ->
    val input = "数量=１２"
    when (val matched = regex.matchEntire(input)) {
        case Some(result) -> println(result.groups["name"]!!.value)
        case None -> println("no match")
    }
    println(regex.replace(input, "$1"))
    println(regex.replaceTemplate(input, "${name}: ${value}"))
}
```

### 6.5 错误

| 情况 | 结果 |
| --- | --- |
| 没有匹配 | Boolean false / None，不抛异常 |
| 模式语法错误 | RegexSyntaxException，包含 pattern、标量 offset、native code 和已复制的 detail |
| replacement 模板错误 | RegexReplacementException，保留 native code、detail；上游提供位置时转成标量 offset |
| 起点越界、组号越界 | IndexOutOfBoundsException |
| 未知组名、负 limit 等非法实参 | IllegalArgumentException |
| 使用已关闭 Regex | IllegalStateException |
| match/depth/heap/JIT stack 限制或其他执行失败 | RegexMatchException，保留可区分的原因与 native code，不能转成 None |

`toRegexOrNone()` 只将模式语法错误转为 None；native 分配失败等不能吞掉。错误映射在所属 Cone 的 `scoop.test` 中验证类别、位置与必要原始码，不锁定整段可能随 PCRE2 升级改变的英文诊断，也不按上游每种错误建立测试矩阵。

## 7. native 接入与性能边界

### 7.1 编译、JIT 与生命周期

使用 `pcre2_compile_8` 编译和 `pcre2_match_8` 匹配，先完成不依赖 JIT 的功能闭环，再接入 JIT。接入后在构造阶段、发布对象前尝试 JIT，避免多个使用线程同时修改已发布 code。`pcre2_match_8` 自动选择可用路径：JIT 编译不适用或不可用时保留解释器能力，不改变公开模式语义；不以 `pcre2_jit_match` 直接入口替代公共匹配路径。[PCRE2 JIT][pcre-jit]

`matchAt` 使用 match-time `PCRE2_ANCHORED`；`matches`/`matchEntire` 从零偏移开始，同时使用 `PCRE2_ANCHORED` 和 `PCRE2_ENDANCHORED`。**这些 match-time 选项会禁用 JIT。** 不应宣称每个入口都一定使用 JIT，也不要简单把模式包进 `\A(?:...)\z`，因为原生递归、开头指令等可能受影响。若真实性能数据证明值得优化，再考虑同一模式的专门编译形式。

匹配状态按 PCRE2 API 映射：返回值大于等于零表示匹配成功，零表示输出捕获向量容量不足；只有 `PCRE2_ERROR_NOMATCH` 表示未匹配，其他负值作为执行错误处理。Boolean 路径不构造全部捕获结果，使用较小的向量时仍须将零判定为成功；需要捕获的路径按模式分配完整 match-data。[匹配 API][pcre-api]

Regex 作为 native code owner 实现 Closeable，close 幂等；使用与关闭同一实例由调用者串行化。编译/JIT 完成后的模式可以供并发匹配共享，每次调用使用独立 match-data/context；不在 Regex 中共享一个可变 Matcher。无关闭竞争时，共享只读编译结果不需要将全部匹配串行化。

确定性释放使用 close/use；遗漏关闭时可利用现有 release block 兜底，但不得承诺及时回收或退出时回收。release 路径只读取 native handle，调用遵守 GC-free 合同的原生释放，不分配 Scoop 对象或调用用户 callback。对选定 PCRE2/SLJIT 构建的释放链确认现有 release 合同即可，不新增资源清理服务。大量短命动态模式仍宜显式使用 use。

不提供全局隐式 pattern cache。编译后对象由调用者复用，避免无意延长 native code 生命周期。PCRE2 序列化数据和 JIT code 也不进入 `.slib`；当前普通库能力不需要新的产物格式。

### 7.2 String、GC 与索引

C shim 仅使用普通 native source 和 extern；不解析 Scoop 对象头，也不在 runtime 写 regex 专属函数。整个 PCRE2 调用在 `String.withUtf8Bytes` 借用内，使用显式字节长度，保留 U+0000。

String 的合法 UTF-8 不变量已经在既有构造边界成立。对未变化的输入可以使用 PCRE2_NO_UTF_CHECK，避免每次 findAll 都重验整串；传入的 start offset 必须来自受检标量转换或引擎合法结果，不能是任意字节位置。这里复用 String 合同，不是接受非法 UTF-8 的宽松模式。

公开范围转为 Long 标量索引。不能对每个匹配或捕获反复从头扫描，也不能用逐个 String[index] 做定位。一次查询/iterator 内可以使用按需扩展的稀疏 byte↔scalar 定位表：每个索引块建立一次，端点查找在块内顺序解码；lookaround 产生的非单调端点也使用同一份定位信息。Boolean 路径不需要结果位置，不建立这份定位表。这只是 regex 内部的字符串定位辅助，不改变 String 布局或新增公共索引体系。

复制捕获文本时使用已有字节端点，在有效借用内读取相应区间，生成独立合法 UTF-8 String；不要先转标量范围再反复调用 O(n) 的 String.slice。计数转换和输出分配仅检查实际溢出与表示边界。

### 7.3 回溯和限制

PCRE2 不承诺最坏线性时间，JIT 也不改变这一点。保留并正确报告上游 match/depth/heap/JIT-stack 限制；有实际调用需求时再提供 regex 局部的薄配置入口，不为此新增跨阶段、跨库资源预算或计费。

这些上游限制不是可移植的毫秒级 timeout：match limit 是引擎工作计数，解释器和 JIT 的计数方式不同，非锚定搜索还可能按起点重新计数；解释器的 heap/depth limit 不能当成 JIT 的同名保证。JIT 有独立的 stack 约束。

NativeSafe 使匹配期间 GC 可以继续协调，但不会让同步 C 匹配自动响应 Scoop Job 取消。外层 timeout 无法抢占正在执行的引擎。首版不承诺硬截止时间；有严格最坏时间要求的场景需要另外评估受限模式或线性引擎，不自动切换执行后端。[执行限制][pcre-api]

### 7.4 构建与交付

固定 PCRE2 10.49 上游源码及其随带 Unicode 数据，分别构建 `aarch64-apple-darwin`、`x86_64-unknown-linux-gnu` 和 `x86_64-unknown-linux-musl` 的 8-bit 静态库；首版不增加 Windows 支持。不构建产品不使用的 16/32-bit、POSIX wrapper、pcre2grep 等工具、上游测试或 benchmark 目标。JIT 使用上游对所选目标的支持，并保留无 JIT 时的正常执行。

上游源码与许可按现有 vendor 方式固定；维护脚本准备每目标 `sysroot/lib/<triple>/libpcre2-8.a` 与对应头文件，发行沿现有流程收集 PCRE2/SLJIT 许可。`scoop.regex` 通过 `native.libraries` 声明静态库 `pcre2-8`，薄 C 适配随 Cone 的 native source 构建；不重复编入上游对象。

沿用已经完成的 [M52](../milestone52/DESIGN.md) 预编译 sysroot 与目标资源交付，以及 [M61](../milestone61/DESIGN.md) 使用的更新复用流程，具体入口见[准备脚本说明](../../scripts/README.md)。输入未变化且所需输出存在时直接复用；更新判断先于解包、源码复制、工具探测和构建。普通应用只消费目标 `.slib` 和 `.a`，不重新下载或构建 PCRE2，不执行 Cargo，也不新增来源审计、产物复核或工具隐藏测试。

PCRE2 保持独立 C archive，不添加 Rust wrapper，不修改 `scoop_text_backend` 的依赖图或要求 Scoop final-link 读取 Rust bitcode。现有 FFI、借用、Closeable/release 与 native library 通道是本设计的基础，不预设新的编译器或 runtime 能力为前置工作。

## 8. 实施顺序与必要验证

### 8.1 实施顺序

以下五个阶段已完成，具体产物与验证结果见[实施记录](PROGRESS.md)。

1. **公开合同。** 将本文 API、UTF/UCP 默认值、Long 标量位置、生命周期和错误写入独立 regex spec，按需同步索引和引用。优先使用现有语言、runtime 和 pipeline 合同，不以新增基础机制扩展范围。
2. **最小可用闭环。** 准备 PCRE2-8，建立普通 `scoop.regex` Cone 和 C 适配；先在 Darwin/AArch64 接通源码编译、预编译库消费、链接与运行。完成 Regex 编译与 close/use、Boolean 查询、find/matchAt/matchEntire、编号／命名捕获和错误映射；同时落实输入借用、结果快照、组名元数据与标量索引转换。
3. **迭代与文本操作。** 使用 `pcre2_next_match` 完成 findAll；使用 `pcre2_substitute` 完成字面／模板替换，回调替换在借用结束后执行。split/splitToSequence 复用匹配推进，补齐 String 扩展和 escape。
4. **必要的性能工作。** 确认 Boolean 路径不物化捕获、查询与迭代不重复从头转换索引；在功能闭环后接入构造阶段 JIT。优化依据实际使用成本，不另建模式缓存、Matcher 池或通用索引框架。
5. **三目标交付。** 完成 GNU/musl 目标资源与运行，接入已有 sysroot 准备和发行收集；在三个目标运行本库 release Cone 测试及实际消费者，只检查本次交付所需的库构建、链接和运行闭环。

遇到编译器、runtime、FFI、链接或其他底层缺口时，先按 spec 先行修复对应层，再继续库实现；不得在 regex 中绕过。遵循用户确定的测试边界，不为这些底层缺口新增任何正例、负例或 fixture。

### 8.2 测试范围

库行为测试放在 `scoop.regex` 所属 Cone 的 tests 中，统一使用 `scoop.test` 和 release 模式；消费预编译库的普通行为也使用 Cone 测试，不新增 fixture。独立与组合用例围绕 Scoop 的实际职责组织：

| Scoop 职责 | 必要覆盖 |
| --- | --- |
| 参数与引擎调用 | 公开选项传递；完整输入加非零起点；搜索、指定起点与整串匹配的区别；Boolean 成功状态映射 |
| 字符串与结果 | 用非 ASCII／补充平面字符验证 byte↔scalar 偏移；NUL 与显式长度；未参与／空捕获、组名查询和独立结果快照 |
| 迭代 | 空匹配推进的适配、耗尽后持续 None、不同 iterator 独立、提前停止及消费期间的关闭状态 |
| 替换与分割 | 字面／模板入口、捕获引用映射、回调异常与重入、limit 与空段合同；不重测上游模板解释器算法 |
| 生命周期与错误 | close 幂等、关闭后操作报错、已有结果及组名查询仍可用；错误类别／位置转换、非法实参与执行失败不被吞成未匹配 |

高级语法只作为实际 Scoop API 组合所需的输入，不逐项建立引擎语法或算法的正反例全集。不运行上游 Unicode 兼容性／完备性、官方语料或跨引擎差分测试；不以修复上游问题作为交付条件。

不增加 debug/release 全模式矩阵、普通功能 GC stress、底层缺口测试、HTTP client 组合测试或无关全量回归。既有编译器、构建缓存、链接及 GC 合同不在每个库中重复验收；原始日志保留在仓库外，文档只记录验证范围、结果与实际限制。出现具体性能问题时，定向观察已编译模式复用、Boolean 查询、findAll、捕获复制与索引转换成本，不建立跨引擎 benchmark 门槛。

## 9. 主要资料

- [PCRE2 10.49 发布][pcre-release]、[固定版本 ChangeLog][pcre-changes]、[许可证][pcre-license]。
- [PCRE2 Unicode 支持][pcre-unicode]、[模式语法][pcre-pattern]、[API 与全局迭代][pcre-api]、[JIT][pcre-jit]。PCRE2 链接固定到 10.49 源码中的官方手册。
- [ICU 78.3 发布][icu-release]、[ICU Regex 用户指南][icu-regex]，仅作为选型资料。
- [Unicode UTS #18][uts18]，用于说明语义术语，不作为本库的上游符合性验收计划。
- [Rust regex 1.13.1][rust-regex]、[上游 Unicode 支持说明][rust-unicode]、[fancy-regex 0.19.2][fancy]、[固定版本解析器][fancy-parser]，仅作为选型资料。
- [Kotlin Regex API][kotlin-regex]，用于名称与常见用法对照，不代表 Scoop 采用各 Kotlin 平台不同的引擎语义。

[pcre-release]: https://github.com/PCRE2Project/pcre2/releases/tag/pcre2-10.49
[pcre-changes]: https://github.com/PCRE2Project/pcre2/blob/pcre2-10.49/ChangeLog
[pcre-license]: https://github.com/PCRE2Project/pcre2/blob/pcre2-10.49/LICENCE.md
[pcre-unicode]: https://github.com/PCRE2Project/pcre2/blob/pcre2-10.49/doc/html/pcre2unicode.html
[pcre-pattern]: https://github.com/PCRE2Project/pcre2/blob/pcre2-10.49/doc/html/pcre2pattern.html
[pcre-api]: https://github.com/PCRE2Project/pcre2/blob/pcre2-10.49/doc/html/pcre2api.html
[pcre-next-match]: https://github.com/PCRE2Project/pcre2/blob/pcre2-10.49/doc/html/pcre2_next_match.html
[pcre-jit]: https://github.com/PCRE2Project/pcre2/blob/pcre2-10.49/doc/html/pcre2jit.html
[icu-release]: https://github.com/unicode-org/icu/releases/tag/release-78.3
[icu-regex]: https://unicode-org.github.io/icu/userguide/strings/regexp.html
[uts18]: https://www.unicode.org/reports/tr18/
[rust-regex]: https://docs.rs/regex/1.13.1/regex/
[rust-unicode]: https://github.com/rust-lang/regex/blob/1.13.1/UNICODE.md
[fancy]: https://docs.rs/fancy-regex/0.19.2/fancy_regex/
[fancy-parser]: https://github.com/fancy-regex/fancy-regex/blob/0.19.2/src/parse.rs
[kotlin-regex]: https://kotlinlang.org/api/core/kotlin-stdlib/kotlin.text/-regex/
